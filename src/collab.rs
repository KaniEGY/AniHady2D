//! Local-first collaborative rooms for Stick Studio.
//! A participant hosts the TCP session; UDP broadcast discovers rooms on a LAN.
//! Internet peers can use host-IP:port with appropriate router/firewall rules.
//! This is not an encrypted Internet service or a global room directory.
use crate::project::{AniProject, Stroke};
use crate::cursor_sync::{CursorPosition, CursorSender, RemoteCursors};
use crate::project_sync::{self, ProjectEdit};
use crate::remote_access::{self, TailnetInfo};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const TCP_PORT: u16 = 47554;
const DISCOVERY_PORT: u16 = 47553;
const DISCOVER_REQUEST: &[u8] = b"STICK_STUDIO_LAN_DISCOVER_V1";
// v5 adds transient cursor packets and the new brush presets.
const PROTOCOL: u16 = 5;
const MAX_PACKET: usize = 8 * 1024 * 1024;
const MAX_MEMBERS: usize = 12;
const MAX_FRAME: usize = 20_000;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum OnlineMode { Drawing, Animation }

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RoomListing {
    pub title: String,
    pub address: String,
    pub mode: OnlineMode,
    pub members: usize,
    pub limit: usize,
    pub locked: bool,
    pub port: u16,
    pub protocol: u16,
    #[serde(default)]
    pub room_code: String,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Participant {
    pub id: u64,
    pub name: String,
    pub level: u32,
    pub color: [u8;3],
}

const MEMBER_COLORS: [[u8;3];12] = [
    [64,185,250], [247,115,87], [159,125,248], [47,207,147],
    [240,186,54], [229,98,184], [76,200,210], [244,141,68],
    [164,223,75], [128,166,251], [248,102,135], [191,150,247],
];
fn member_color(id:u64) -> [u8;3] { MEMBER_COLORS[id as usize % MEMBER_COLORS.len()] }
fn new_room_code() -> String {
    // System-seeded SipHash state + fresh time: an 8-digit locator, NOT an auth token.
    let state = std::collections::hash_map::RandomState::new();
    let mut h = state.build_hasher();
    h.write_u128(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos());
    format!("{:08}", 10_000_000 + h.finish() % 90_000_000)
}
pub fn find_room_by_code<'a>(rooms:&'a [RoomListing], code:&str) -> Option<&'a RoomListing> {
    let key = code.trim();
    if key.len()!=8 || !key.bytes().all(|b| b.is_ascii_digit()) { return None; }
    rooms.iter().find(|r| r.room_code == key)
}

#[derive(Clone, Serialize, Deserialize, Debug)]
enum Packet {
    Hello { name: String, level: u32, protocol: u16, mode: OnlineMode },
    Challenge { nonce: String },
    Auth { proof: String },
    Welcome { project: Box<AniProject> },
    Rejected { reason: String },
    Strokes { layer: usize, frame: usize, strokes: Vec<Stroke> },
    Edits { edits: Vec<ProjectEdit> },
    Snapshot { project: Box<AniProject> },
    Members { count: usize },
    Roster { participants: Vec<Participant> },
    Pointer { position: Option<CursorPosition> },
    ParticipantPointer { id: u64, position: Option<CursorPosition> },
    Kicked { reason: String },
    HostHandoff { address: String, promote: bool, title: String, room_code: String,
        limit: usize, project: Option<Box<AniProject>> },
    Goodbye,
}

#[derive(Debug)]
pub enum OnlineEvent {
    Connected(AniProject),
    Disconnected(String),
    Strokes { layer: usize, frame: usize, strokes: Vec<Stroke> },
    Edits(Vec<ProjectEdit>),
    Snapshot(AniProject),
    Members(usize),
    Roster(Vec<Participant>),
    Cursor { id: u64, position: Option<CursorPosition>, received: Instant },
    HostHandoff { address: String, promote: bool, title: String, room_code: String,
        limit: usize, project: Option<AniProject> },
    Notice(String),
}

fn proof(password: &str, nonce: &str) -> String {
    format!("{:x}", Sha256::digest(format!("{}:{}", nonce, password).as_bytes()))
}
fn nonce() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    format!("{:x}-{:x}", t, NEXT.fetch_add(1, Ordering::Relaxed))
}

fn packet_write(stream: &mut TcpStream, msg: &Packet) -> io::Result<()> {
    let bytes = serde_json::to_vec(msg).map_err(io::Error::other)?;
    if bytes.len() > MAX_PACKET { return Err(io::Error::new(io::ErrorKind::InvalidData, "Message too large")); }
    stream.write_all(&(bytes.len() as u32).to_be_bytes())?;
    stream.write_all(&bytes)?;
    Ok(())
}

// Read precisely a framed message. Timeouts never discard partial headers/bodies.
fn read_exact_running(stream: &mut TcpStream, buf: &mut [u8], running: &AtomicBool) -> io::Result<()> {
    let mut pos = 0;
    while pos < buf.len() {
        if !running.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "Session closed"));
        }
        match stream.read(&mut buf[pos..]) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Peer disconnected")),
            Ok(n) => pos += n,
            Err(e) if matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
fn packet_read(stream: &mut TcpStream, running: &AtomicBool) -> io::Result<Packet> {
    let mut header = [0u8; 4];
    read_exact_running(stream, &mut header, running)?;
    let len = u32::from_be_bytes(header) as usize;
    if len == 0 || len > MAX_PACKET { return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid packet length")); }
    let mut body = vec![0u8; len];
    read_exact_running(stream, &mut body, running)?;
    serde_json::from_slice(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
fn valid_strokes(layer: usize, frame: usize, strokes: &[Stroke], project: &AniProject) -> bool {
    layer < project.layers.len() && (1..=MAX_FRAME).contains(&frame)
        && !strokes.is_empty() && strokes.len() <= 32
        && strokes.iter().all(|s| s.points.len() <= 15_000 && s.points.iter().all(|p|
            p.x.is_finite() && p.y.is_finite() && p.pressure.is_finite()))
}
pub fn apply_strokes(project: &mut AniProject, layer: usize, frame: usize, strokes: Vec<Stroke>) -> bool {
    if !valid_strokes(layer, frame, &strokes, project) { return false; }
    project.total_frames = project.total_frames.max(frame);
    let keyframes = &mut project.layers[layer].keyframes;
    if !keyframes.contains_key(&frame) {
        // Match Stick's editable-keyframe inheritance (important for animation).
        let previous = keyframes.iter().filter(|(index, _)| **index <= frame)
            .max_by_key(|(index, _)| **index).map(|(_, data)| data.clone()).unwrap_or_default();
        keyframes.insert(frame, previous);
    }
    if let Some(data) = keyframes.get_mut(&frame) { data.strokes.extend(strokes); }
    true
}
// Never publish absolute host filesystem paths to other room participants.
// Media transfer is intentionally outside the first collaboration preview.
fn public_project(mut project: AniProject) -> AniProject {
    for track in &mut project.audio_tracks { track.file_path.clear(); }
    for layer in &mut project.layers {
        for frame in layer.keyframes.values_mut() {
            for item in &mut frame.media_items { item.path.clear(); }
        }
    }
    project
}

fn local_address() -> String {
    UdpSocket::bind("0.0.0.0:0").ok()
        .and_then(|s| { s.connect("8.8.8.8:80").ok()?; s.local_addr().ok() })
        .map(|addr| format!("{}:{}", addr.ip(), TCP_PORT))
        .unwrap_or_else(|| format!("127.0.0.1:{}", TCP_PORT))
}

struct HostShared {
    running: Arc<AtomicBool>,
    peers: Mutex<HashMap<u64, (mpsc::Sender<Packet>, Participant, IpAddr)>>,
    local: Participant,
    local_color: Mutex<[u8; 3]>,
    room_code: String,
    project: Mutex<AniProject>,
    /// Serialize application + broadcast so parallel peer threads cannot reorder edits.
    edit_order: Mutex<()>,
    password: String,
    title: String,
    mode: OnlineMode,
    limit: usize,
    events: mpsc::Sender<OnlineEvent>,
    cursor_wake: CursorWake,
    peer_ids: AtomicU64,
}

impl HostShared {
    fn member_count(&self) -> usize { self.peers.lock().map(|p| p.len()+1).unwrap_or(1) }
    fn broadcast(&self, msg: &Packet, exclude: Option<u64>) {
        if let Ok(peers) = self.peers.lock() {
            for (id, (tx, _, _)) in peers.iter() {
                if Some(*id) != exclude { let _ = tx.send(msg.clone()); }
            }
        }
    }
    fn announce_members(&self) {
        // Publish under the membership lock so concurrent joins/leaves cannot
        // deliver an older roster after the new one and resurrect a departed cursor.
        let Ok(peers) = self.peers.lock() else { return; };
        let mut local = self.local.clone();
        if let Ok(color) = self.local_color.lock() { local.color = *color; }
        let mut roster = vec![local];
        roster.extend(peers.values().map(|(_, member, _)| member.clone()));
        roster.sort_by_key(|member| member.id);
        let count = roster.len();
        for (tx, _, _) in peers.values() {
            let _ = tx.send(Packet::Members { count });
            let _ = tx.send(Packet::Roster { participants: roster.clone() });
        }
        let _ = self.events.send(OnlineEvent::Members(count));
        let _ = self.events.send(OnlineEvent::Roster(roster));
    }
}

fn writer_loop(mut stream: TcpStream, rx: mpsc::Receiver<Packet>, running: Arc<AtomicBool>) {
    // Drain queued handoffs and kick notices before closing guest sockets.
    loop {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(packet) => {
                let finish = matches!(packet,
                    Packet::Goodbye | Packet::Kicked { .. } | Packet::HostHandoff { .. });
                if packet_write(&mut stream, &packet).is_err() || finish { break; }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !running.load(Ordering::Relaxed) { break; }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn host_peer(mut stream: TcpStream, host: Arc<HostShared>) -> io::Result<()> {
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_millis(300)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let (name, level, protocol, mode) = match packet_read(&mut stream, &host.running)? {
        Packet::Hello { name, level, protocol, mode } => (name, level, protocol, mode),
        _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "Handshake required")),
    };
    if protocol != PROTOCOL || mode != host.mode || name.trim().is_empty() || name.chars().count() > 40 {
        packet_write(&mut stream, &Packet::Rejected { reason: "Invalid name or client protocol".into() })?;
        return Ok(());
    }
    // Each connection gets a fresh nonce; the raw password is never transmitted.
    let challenge = nonce();
    packet_write(&mut stream, &Packet::Challenge { nonce: challenge.clone() })?;
    let supplied = match packet_read(&mut stream, &host.running)? {
        Packet::Auth { proof } => proof,
        _ => String::new(),
    };
    if supplied != proof(&host.password, &challenge) {
        packet_write(&mut stream, &Packet::Rejected { reason: "Incorrect room password".into() })?;
        return Ok(());
    }
    // Serialize admission and snapshot under the peer lock to enforce capacity.
    let id = host.peer_ids.fetch_add(1, Ordering::Relaxed);
    let guest_ip = stream.peer_addr()?.ip();
    let (out_tx, out_rx) = mpsc::channel();
    {
        // Admission must share the edit ordering gate: Welcome cannot
        // contain an edit which is then redundantly replayed to the new peer.
        let _order = host.edit_order.lock().map_err(|_| io::Error::other("Edit gate poisoned"))?;
        let mut peers = host.peers.lock().map_err(|_| io::Error::other("Peer registry poisoned"))?;
        if peers.len() + 1 >= host.limit {
            packet_write(&mut stream, &Packet::Rejected { reason: "Room is full".into() })?;
            return Ok(());
        }
        // Snapshot and registration are ordered with subsequent host operations.
        let project = host.project.lock().map_err(|_| io::Error::other("Room project lock poisoned"))?.clone();
        packet_write(&mut stream, &Packet::Welcome { project: Box::new(public_project(project)) })?;
        let local_color = host.local_color.lock().map(|color| *color).unwrap_or(host.local.color);
        let color = (0..MEMBER_COLORS.len()).map(|offset| member_color(id.wrapping_add(offset as u64)))
            .find(|color| *color != local_color && peers.values().all(|(_, member, _)| member.color != *color))
            .unwrap_or_else(|| member_color(id));
        peers.insert(id, (out_tx, Participant { id, name, level: level.min(1000), color }, guest_ip));
    }
    host.announce_members();
    let writer = stream.try_clone()?;
    let running = host.running.clone();
    thread::spawn(move || writer_loop(writer, out_rx, running));
    loop {
        if !host.peers.lock().map(|peers| peers.contains_key(&id)).unwrap_or(false) { break; }
        match packet_read(&mut stream, &host.running) {
            Ok(Packet::Pointer { position }) if position.is_none_or(CursorPosition::valid) => {
                // The authenticated socket determines identity. Never echo a pointer to its owner.
                let _ = host.events.send(OnlineEvent::Cursor { id, position, received: Instant::now() });
                if let Some(wake) = host.cursor_wake.get() { wake(); }
                host.broadcast(&Packet::ParticipantPointer { id, position }, Some(id));
            }
            Ok(Packet::Strokes { layer, frame, strokes }) => {
                // Strokes and structural edits must use the same host gate;
                // otherwise a stroke and a layer delete may be broadcast in
                // the opposite order from their canonical application.
                let _order = host.edit_order.lock().map_err(|_| io::Error::other("Edit gate poisoned"))?;
                // A kicked guest must never sneak in one final queued edit.
                if !host.peers.lock().map(|p| p.contains_key(&id)).unwrap_or(false) { break; }
                let accepted = host.project.lock().map(|mut p|
                    apply_strokes(&mut p, layer, frame, strokes.clone())).unwrap_or(false);
                if accepted {
                    let _ = host.events.send(OnlineEvent::Strokes { layer, frame, strokes: strokes.clone() });
                    host.broadcast(&Packet::Strokes { layer, frame, strokes }, Some(id));
                }
            }
            Ok(Packet::Edits { edits }) => {
                if !host.peers.lock().map(|p| p.contains_key(&id)).unwrap_or(false) { break; }
                // Acquiring the gate until after relay is essential: otherwise
                // peer B could broadcast edit #2 ahead of peer A's edit #1.
                let _order = host.edit_order.lock().map_err(|_| io::Error::other("Edit gate poisoned"))?;
                let accepted = host.project.lock().map(|mut p|
                    project_sync::apply_edits(&mut p, &edits).is_ok()).unwrap_or(false);
                if accepted {
                    let _ = host.events.send(OnlineEvent::Edits(edits.clone()));
                    host.broadcast(&Packet::Edits { edits }, Some(id));
                } else {
                    // Invalid/stale edits cannot silently diverge: restore this peer
                    // to the host's authoritative document.
                    let state = host.project.lock().ok().map(|p| public_project(p.clone()));
                    if let Some(state) = state {
                        if let Ok(peers) = host.peers.lock() {
                            if let Some((tx, _, _)) = peers.get(&id) {
                                let _ = tx.send(Packet::Snapshot { project: Box::new(state) });
                            }
                        }
                    }
                }
            }
            Ok(Packet::Goodbye) => break,
            Err(_) => break,
            _ => {}
        }
    }
    if let Ok(mut peers) = host.peers.lock() { peers.remove(&id); }
    host.announce_members();
    Ok(())
}

type CursorWake = Arc<OnceLock<Box<dyn Fn() + Send + Sync>>>;

pub struct OnlineSession {
    pub title: String,
    pub mode: OnlineMode,
    pub address: String,
    pub is_host: bool,
    pub members: usize,
    pub room_code: String,
    /// Kept only in memory for a planned handoff/reconnection.
    pub password: String,
    event_rx: mpsc::Receiver<OnlineEvent>,
    outgoing: Option<mpsc::Sender<Packet>>,
    host: Option<Arc<HostShared>>,
    /// Local optimistic baseline; new edits are relative to this document.
    baseline: Mutex<Option<AniProject>>,
    cursor_sender: CursorSender,
    cursor_wake: CursorWake,
    pub cursors: RemoteCursors,
    running: Arc<AtomicBool>,
}

impl OnlineSession {
    pub fn host_room(title: String, password: String, limit: usize, mode: OnlineMode, project: AniProject, nickname: String, level: u32) -> Result<Self, String> {
        Self::host_room_with_code(title, password, limit, mode, project, nickname, level, None)
    }

    pub fn host_room_with_code(title: String, password: String, limit: usize,
        mode: OnlineMode, project: AniProject, nickname: String, level: u32,
        retained_code: Option<String>) -> Result<Self, String> {
        if title.trim().is_empty() || title.chars().count() > 48 { return Err("Room name must have 1–48 characters".into()); }
        if password.chars().count() > 128 { return Err("Password is too long".into()); }
        if !(2..=MAX_MEMBERS).contains(&limit) { return Err("Room capacity must be 2–12".into()); }
        let listener = TcpListener::bind(("0.0.0.0", TCP_PORT)).map_err(|e|
            format!("Cannot host on port {}: {} (allow Stick through the firewall)", TCP_PORT, e))?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let (event_tx, event_rx) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let room_code = retained_code.filter(|code|
            code.len() == 8 && code.bytes().all(|c| c.is_ascii_digit()))
            .unwrap_or_else(new_room_code);
        let cursor_wake = CursorWake::default();
        let host = Arc::new(HostShared {
            running: running.clone(), peers: Mutex::new(HashMap::new()),
            local: Participant { id: 0, name: nickname, level, color: member_color(0) },
            local_color: Mutex::new(member_color(0)),
            room_code: room_code.clone(),
            project: Mutex::new(project.clone()), edit_order: Mutex::new(()), password: password.clone(),
            title: title.clone(), mode, limit, events: event_tx, cursor_wake: cursor_wake.clone(),
            peer_ids: AtomicU64::new(1),
        });
        let host_accept = host.clone();
        thread::spawn(move || {
            while host_accept.running.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let shared = host_accept.clone();
                        thread::spawn(move || { let _ = host_peer(stream, shared); });
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(100)),
                    Err(_) => break,
                }
            }
        });
        let host_discover = host.clone();
        thread::spawn(move || {
            let socket = match UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT)) {
                Ok(s) => s,
                Err(_) => {
                    let _ = host_discover.events.send(OnlineEvent::Notice("LAN discovery unavailable. Direct-IP join still works.".into()));
                    return;
                }
            };
            let _ = socket.set_read_timeout(Some(Duration::from_millis(400)));
            let mut buf = [0u8; 512];
            while host_discover.running.load(Ordering::Relaxed) {
                if let Ok((len, address)) = socket.recv_from(&mut buf) {
                    if &buf[..len] == DISCOVER_REQUEST {
                        let listing = RoomListing {
                            title: host_discover.title.clone(), address: String::new(),
                            mode: host_discover.mode, members: host_discover.member_count(),
                            limit: host_discover.limit, locked: !host_discover.password.is_empty(),
                            port: TCP_PORT, protocol: PROTOCOL,
                            room_code: host_discover.room_code.clone(),
                        };
                        if let Ok(json) = serde_json::to_vec(&listing) { let _ = socket.send_to(&json, address); }
                    }
                }
            }
        });
        Ok(Self { title, mode, address: local_address(), is_host: true, members: 1,
            room_code, password, event_rx, outgoing: None, host: Some(host),
            baseline: Mutex::new(Some(project)), cursor_sender: Default::default(), cursor_wake, cursors: Default::default(), running })
    }

    pub fn join_room(address: String, password: String, nickname: String, level: u32, mode: OnlineMode) -> Result<Self, String> {
        Self::join_room_internal(address, password, nickname, level, mode, false)
    }

    /// A peer may receive the redirect before the newly elected host binds.
    pub fn rejoin_after_handoff(address: String, password: String, nickname: String,
        level: u32, mode: OnlineMode) -> Result<Self, String> {
        Self::join_room_internal(address, password, nickname, level, mode, true)
    }

    fn join_room_internal(address: String, password: String, nickname: String,
        level: u32, mode: OnlineMode, is_handoff: bool) -> Result<Self, String> {
        let address = crate::online_invite::validate_endpoint(&address)
            .map_err(|reason| reason.to_string())?;
        let address = if address.starts_with('[') {
            if address.ends_with(']') { format!("{}:{}", address, TCP_PORT) } else { address }
        } else if address.contains(':') { address } else { format!("{}:{}", address, TCP_PORT) };
        // Resolve DNS inside the connection worker, never block egui's event loop.
        let guest_password = password.clone();
        let (event_tx, event_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let running_worker = running.clone();
        let cursor_wake = CursorWake::default();
        let worker_wake = cursor_wake.clone();
        let address_for_worker=address.clone();
        thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                let addrs: Vec<SocketAddr> = address_for_worker.to_socket_addrs()
                    .map_err(|_| format!("Cannot resolve host address: {address_for_worker}"))?
                    .take(8).collect();
                if addrs.is_empty() { return Err("No IP addresses found for this host".into()); }
                let mut retry_count = 0;
                let mut stream = loop {
                    let mut connected = None;
                    let mut last_error = None;
                    for addr in &addrs {
                        match TcpStream::connect_timeout(addr, if is_handoff {
                            Duration::from_millis(500)
                        } else { Duration::from_secs(3) }) {
                            Ok(stream) => { connected = Some(stream); break; }
                            Err(error) => last_error = Some(error),
                        }
                    }
                    if let Some(stream) = connected { break stream; }
                    if is_handoff && retry_count < 25 {
                        retry_count += 1;
                        thread::sleep(Duration::from_millis(220));
                        continue;
                    }
                    return Err(format!(
                        "Connection failed: {}. Check host TCP 47554, firewall and network reachability. A room code or invite does not bypass NAT.",
                        last_error.map(|e| e.to_string()).unwrap_or_else(|| "no route".into())));
                };
                // All connection attempts use the intended address, never
                // a public proxy or an untrusted third-party room directory.
                stream.set_nodelay(true).map_err(|e| e.to_string())?;
                stream.set_read_timeout(Some(Duration::from_millis(300))).map_err(|e| e.to_string())?;
                stream.set_write_timeout(Some(Duration::from_secs(3))).map_err(|e| e.to_string())?;
                packet_write(&mut stream, &Packet::Hello { name: nickname, level: level.min(1000), protocol: PROTOCOL, mode }).map_err(|e| e.to_string())?;
                let nonce = match packet_read(&mut stream, &running_worker).map_err(|e| e.to_string())? {
                    Packet::Challenge { nonce } => nonce,
                    Packet::Rejected { reason } => return Err(reason),
                    _ => return Err("Unexpected room handshake".into()),
                };
                packet_write(&mut stream, &Packet::Auth { proof: proof(&password, &nonce) }).map_err(|e| e.to_string())?;
                match packet_read(&mut stream, &running_worker).map_err(|e| e.to_string())? {
                    Packet::Welcome { project } => { let _ = event_tx.send(OnlineEvent::Connected(*project)); }
                    Packet::Rejected { reason } => return Err(reason),
                    _ => return Err("Room rejected the connection".into()),
                }
                let writer = stream.try_clone().map_err(|e| e.to_string())?;
                let writer_running = running_worker.clone();
                thread::spawn(move || writer_loop(writer, out_rx, writer_running));
                while running_worker.load(Ordering::Relaxed) {
                    match packet_read(&mut stream, &running_worker) {
                        Ok(Packet::ParticipantPointer { id, position }) if position.is_none_or(CursorPosition::valid) => {
                            let _ = event_tx.send(OnlineEvent::Cursor { id, position, received: Instant::now() });
                            if let Some(wake) = worker_wake.get() { wake(); }
                        }
                        Ok(Packet::Strokes { layer, frame, strokes }) => {
                            let _ = event_tx.send(OnlineEvent::Strokes { layer, frame, strokes });
                        }
                        Ok(Packet::Snapshot { project }) => { let _ = event_tx.send(OnlineEvent::Snapshot(*project)); }
                        Ok(Packet::Edits { edits }) => { let _ = event_tx.send(OnlineEvent::Edits(edits)); }
                        Ok(Packet::Members { count }) => { let _ = event_tx.send(OnlineEvent::Members(count)); }
                        Ok(Packet::Roster { participants }) => { let _ = event_tx.send(OnlineEvent::Roster(participants)); }
                        Ok(Packet::Kicked { reason }) => return Err(reason),
                        Ok(Packet::HostHandoff { address, promote, title, room_code, limit, project }) => {
                            let _ = event_tx.send(OnlineEvent::HostHandoff {
                                address, promote, title, room_code, limit, project: project.map(|p| *p),
                            });
                            running_worker.store(false, Ordering::Relaxed);
                            return Ok(());
                        }
                        Ok(Packet::Goodbye) => break,
                        Err(e) => return Err(format!("Lost connection: {}", e)),
                        _ => {}
                    }
                }
                Ok(())
            })();
            if running_worker.load(Ordering::Relaxed) {
                let _ = event_tx.send(OnlineEvent::Disconnected(result.err().unwrap_or_else(|| "Room closed".into())));
            }
        });
        Ok(Self { title: String::new(), mode, address, is_host: false, members: 1,
            room_code: String::new(), password: guest_password, event_rx, outgoing: Some(out_tx),
            host: None, baseline: Mutex::new(None), cursor_sender: Default::default(), cursor_wake, cursors: Default::default(), running })
    }

    pub fn events(&self) -> Vec<OnlineEvent> { self.event_rx.try_iter().collect() }

    pub fn wake_on_cursor(&self, setup: impl FnOnce() -> Box<dyn Fn() + Send + Sync>) {
        self.cursor_wake.get_or_init(setup);
    }

    pub fn update_cursor(&mut self, position: Option<CursorPosition>, now: Instant) {
        if !self.cursor_sender.should_send(position, now) { return; }
        if let Some(host) = &self.host {
            host.broadcast(&Packet::ParticipantPointer { id: host.local.id, position }, None);
        } else if let Some(tx) = &self.outgoing {
            let _ = tx.send(Packet::Pointer { position });
        }
    }

    pub fn set_baseline(&self, project: &AniProject) {
        if let Ok(mut baseline) = self.baseline.lock() { *baseline = Some(project.clone()); }
    }

    /// Merge an incoming host-ordered edit with the visible document and the
    /// optimistic baseline; never echo remote edits as local changes.
    pub fn ingest_remote_edits(
        &self, project: &mut AniProject, edits: &[ProjectEdit]
    ) -> Result<(), String> {
        let mut baseline = self.baseline.lock().map_err(|_| "Project sync lock poisoned")?;
        let mut preview = project.clone();
        project_sync::apply_edits(&mut preview, edits)?;
        if let Some(ref mut known) = *baseline {
            project_sync::apply_edits(known, edits)?;
        }
        *project = preview;
        Ok(())
    }

    /// Send every document mutation, not merely newly drawn strokes.
    /// The host applies batches serially and relays them to all other peers.
    pub fn sync_local_changes(&self, project: &AniProject) -> Result<(), String> {
        let mut baseline = self.baseline.lock().map_err(|_| "Project sync lock poisoned")?;
        let Some(known) = baseline.as_ref() else { return Ok(()); };
        // Pointer-only repaints must not clone and serialize the entire document.
        if known == project { return Ok(()); }
        // Never transmit host/guest filesystem paths through incremental edits.
        let edits = project_sync::diff_projects(
            &public_project(known.clone()), &public_project(project.clone()))?;
        if edits.is_empty() {
            // A local media path can change without changing the public document.
            *baseline = Some(project.clone());
            return Ok(());
        }
        let packet = Packet::Edits { edits: edits.clone() };
        let encoded = serde_json::to_vec(&packet).map_err(|e| e.to_string())?;
        if encoded.len() > MAX_PACKET { return Err("Project edit is too large for online sync".into()); }
        if let Some(host) = &self.host {
            let _order = host.edit_order.lock().map_err(|_| "Edit gate poisoned")?;
            let mut canonical = host.project.lock().map_err(|_| "Room project lock poisoned")?;
            project_sync::apply_edits(&mut canonical, &edits)?;
            drop(canonical);
            host.broadcast(&packet, None);
        } else if let Some(tx) = &self.outgoing {
            tx.send(packet).map_err(|_| "Room connection is no longer active")?;
        } else { return Err("Online room is not connected".into()); }
        *baseline = Some(project.clone());
        Ok(())
    }

    pub fn send_strokes(&self, layer: usize, frame: usize, strokes: Vec<Stroke>) {
        if let Some(host) = &self.host {
            if let Ok(_order) = host.edit_order.lock() {
                if host.project.lock().map(|mut p| apply_strokes(&mut p, layer, frame, strokes.clone())).unwrap_or(false) {
                    host.broadcast(&Packet::Strokes { layer, frame, strokes }, None);
                }
            }
        } else if let Some(tx) = &self.outgoing {
            let _ = tx.send(Packet::Strokes { layer, frame, strokes });
        }
    }

    /// Explicit host-authoritative structure resync; clients never replace host state.
    pub fn resync_from_host(&self, project: AniProject) {
        if let Some(host) = &self.host {
            let Ok(_order) = host.edit_order.lock() else { return; };
            if let Ok(mut state) = host.project.lock() { *state = project.clone(); }
            self.set_baseline(&project);
            host.broadcast(&Packet::Snapshot { project: Box::new(public_project(project)) }, None);
        }
    }
    /// First admitted guest inherits this room on a graceful host exit.
    /// A crashed/offline old host cannot send this transfer without a relay.
    pub fn handoff_before_leave(&self) -> Result<bool, String> {
        let Some(host) = &self.host else { return Ok(false); };
        let _gate = host.edit_order.lock().map_err(|_| "Room edit gate poisoned")?;
        let peers = host.peers.lock().map_err(|_| "Room participants unavailable")?;
        let Some((&successor_id, (_, _, successor_ip))) =
            peers.iter().min_by_key(|(id, _)| *id) else { return Ok(false); };
        let address = format!("{}:{}", successor_ip, TCP_PORT);
        // A host must transfer its authoritative document, including remote
        // edits already accepted by the network worker.
        let snapshot = host.project.lock().map_err(|_| "Room project unavailable")?
            .clone();
        let snapshot = public_project(snapshot);
        let mut transfers = Vec::new();
        for (&id, (tx, _, _)) in peers.iter() {
            let promote = id == successor_id;
            let packet = Packet::HostHandoff {
                address: address.clone(), promote,
                title: self.title.clone(), room_code: self.room_code.clone(),
                limit: host.limit,
                project: if promote { Some(Box::new(snapshot.clone())) } else { None },
            };
            if serde_json::to_vec(&packet).map_err(|e| e.to_string())?.len() > MAX_PACKET {
                return Err("Room project is too large to transfer the host safely".into());
            }
            transfers.push((tx.clone(), packet));
        }
        for (tx, packet) in transfers {
            tx.send(packet).map_err(|_| "A participant disconnected during host transfer")?;
        }
        Ok(true)
    }

    pub fn kick_member(&self, id: u64) -> Result<(), String> {
        if id == 0 { return Err("Host cannot kick itself".into()); }
        let host = self.host.as_ref().ok_or("Only the host can kick members")?;
        let (sender, _, _) = host.peers.lock().map_err(|_| "Room participant lock failed")?
            .remove(&id).ok_or("Member already disconnected")?;
        let _ = sender.send(Packet::Kicked { reason: "The room host removed you".into() });
        host.announce_members();
        Ok(())
    }

    pub fn change_member_color(&self, id: u64, color: [u8; 3]) -> Result<(), String> {
        let host = self.host.as_ref().ok_or("Only the host can change member colors")?;
        if id == 0 {
            *host.local_color.lock().map_err(|_| "Host color lock failed")? = color;
        } else {
            let mut peers = host.peers.lock().map_err(|_| "Room participant lock failed")?;
            let (_, member, _) = peers.get_mut(&id).ok_or("Member already disconnected")?;
            member.color = color;
        }
        host.announce_members();
        Ok(())
    }

    pub fn close(&self) {
        if let Some(tx) = &self.outgoing { let _ = tx.send(Packet::Goodbye); }
        self.running.store(false, Ordering::Relaxed);
    }
}
impl Drop for OnlineSession { fn drop(&mut self) { self.close(); } }

#[derive(Debug, Clone)]
pub struct RoomSearchResult {
    pub rooms: Vec<RoomListing>,
    pub tailnet: TailnetInfo,
}

/// Discover both local LAN broadcasts and remote Tailscale peers.
/// Tailscale provides private routing; room discovery uses unicast UDP,
/// because normal LAN broadcasts are not forwarded across a tailnet.
pub fn discover_rooms_with_remote() -> RoomSearchResult {
    let tailnet = remote_access::current_tailnet();
    let rooms = discover_rooms_from_peers(&tailnet.peers);
    RoomSearchResult { rooms, tailnet }
}

pub fn discover_rooms() -> Vec<RoomListing> {
    discover_rooms_with_remote().rooms
}

fn discover_rooms_from_peers(peer_ips: &[Ipv4Addr]) -> Vec<RoomListing> {
    let mut rooms = Vec::new();
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0") else { return rooms };
    let _ = socket.set_broadcast(true);
    let _ = socket.set_read_timeout(Some(Duration::from_millis(1200)));
    let _ = socket.send_to(DISCOVER_REQUEST, ("255.255.255.255", DISCOVERY_PORT));
    // Windows routers may drop global broadcast but accept the subnet broadcast.
    if let Ok(addr) = local_address().parse::<SocketAddr>() {
        if let std::net::IpAddr::V4(ip) = addr.ip() {
            let mut octets = ip.octets();
            if !ip.is_loopback() {
                octets[3] = 255;
                let bcast = std::net::Ipv4Addr::from(octets);
                let _ = socket.send_to(DISCOVER_REQUEST, (bcast, DISCOVERY_PORT));
            }
        }
    }
    let _ = socket.send_to(DISCOVER_REQUEST, ("127.0.0.1", DISCOVERY_PORT));
    // Peer-to-peer VPN does not forward local Ethernet broadcasts. Directly
    // ask each visible friend device if Stick is hosting a room.
    for &ip in peer_ips.iter().take(64) {
        let _ = socket.send_to(DISCOVER_REQUEST, (ip, DISCOVERY_PORT));
    }
    let mut buf = [0u8; 4096];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, from)) => {
                if let Ok(mut listing) = serde_json::from_slice::<RoomListing>(&buf[..len]) {
                    if listing.protocol != PROTOCOL || listing.title.is_empty() || listing.port == 0 { continue; }
                    listing.address = format!("{}:{}", from.ip(), listing.port);
                    if !rooms.iter().any(|r: &RoomListing| r.address == listing.address) { rooms.push(listing); }
                }
            }
            Err(_) => break,
        }
    }
    rooms
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn room_code_is_eight_digits_and_resolves_only_discovered_rooms() {
        let code = new_room_code();
        assert_eq!(code.len(),8);
        assert!(code.bytes().all(|b| b.is_ascii_digit()));
        let listings = vec![RoomListing {title:"Art".into(),address:"127.0.0.1:47554".into(),
            mode:OnlineMode::Drawing,members:1,limit:4,locked:false,port:TCP_PORT,protocol:PROTOCOL,
            room_code:code.clone()}];
        assert_eq!(find_room_by_code(&listings,&code).unwrap().title,"Art");
        assert!(find_room_by_code(&listings,"123").is_none());
    }
    #[test]
    fn challenge_changes_proof_and_does_not_contain_password() {
        let a = proof("private-pass", "challenge-a");
        assert_ne!(a, proof("private-pass", "challenge-b"));
        assert!(!a.contains("private-pass"));
    }
    #[test]
    fn remote_strokes_reject_out_of_range_indices() {
        let mut project = AniProject::default();
        let stroke = Stroke { points: vec![], color: [0,0,0,255], fill_color: None,
            base_width: 3.0, is_vector: false, brush_preset: Default::default(),
            opacity: 1.0, brush_texture_id: None, tip_shape: Default::default() };
        assert!(!apply_strokes(&mut project, 99, 1, vec![stroke.clone()]));
        assert!(!apply_strokes(&mut project, 0, MAX_FRAME + 1, vec![stroke.clone()]));
        assert!(apply_strokes(&mut project, 0, 2, vec![stroke]));
        assert_eq!(project.total_frames, 2);
    }
    #[test]
    fn loopback_room_handshake_and_stroke_delivery() {
        let mut host = OnlineSession::host_room(
            "Loopback Test".into(), "unique-test-password".into(), 3,
            OnlineMode::Drawing, AniProject::default(), "Host".into(), 9,
        ).expect("open local TCP room");
        let mut guest = OnlineSession::join_room(
            format!("127.0.0.1:{}", TCP_PORT),
            "unique-test-password".into(), "Guest".into(), 5, OnlineMode::Drawing,
        ).expect("start local TCP join");
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        let mut connected = false;
        while std::time::Instant::now() < deadline {
            for event in guest.events() {
                if let OnlineEvent::Connected(project) = event {
                    assert_eq!(project.layers.len(), 1);
                    connected = true;
                }
            }
            if connected { break; }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(connected, "guest must receive an authenticated project snapshot");
        // Verify the password challenge rejects a stranger before admitting them.
        let wrong_password = OnlineSession::join_room(
            format!("127.0.0.1:{}", TCP_PORT),
            "incorrect-password".into(), "Stranger".into(), 1, OnlineMode::Drawing,
        ).expect("start an unauthorized local guest");
        let unauthorized_deadline = std::time::Instant::now() + Duration::from_secs(4);
        let mut rejected = false;
        while std::time::Instant::now() < unauthorized_deadline {
            for event in wrong_password.events() {
                match event {
                    OnlineEvent::Disconnected(reason) => {
                        assert!(reason.contains("Incorrect room password"));
                        rejected = true;
                    }
                    OnlineEvent::Connected(_) => panic!("invalid password joined the room"),
                    _ => {}
                }
            }
            if rejected { break; }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(rejected, "incorrect room password must be rejected");
        wrong_password.close();
        let guest_two = OnlineSession::join_room(
            format!("127.0.0.1:{}", TCP_PORT),
            "unique-test-password".into(), "Guest Two".into(), 6, OnlineMode::Drawing,
        ).expect("start second guest");
        let peer_deadline = std::time::Instant::now() + Duration::from_secs(8);
        let mut peer_connected = false;
        while std::time::Instant::now() < peer_deadline {
            for event in guest_two.events() {
                if matches!(event, OnlineEvent::Connected(_)) { peer_connected = true; }
            }
            if peer_connected { break; }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(peer_connected, "second guest must join the same host room");
        fn next_cursor(session: &OnlineSession) -> (u64, Option<CursorPosition>) {
            let deadline = Instant::now() + Duration::from_secs(4);
            while Instant::now() < deadline {
                if let Some(cursor) = session.events().into_iter().find_map(|event| match event {
                    OnlineEvent::Cursor { id, position, .. } => Some((id, position)),
                    _ => None,
                }) { return cursor; }
                thread::sleep(Duration::from_millis(10));
            }
            panic!("Timed out waiting for a room cursor");
        }
        let guest_point = Some(CursorPosition { x: 42.0, y: 17.5, frame: 1 });
        let host_point = Some(CursorPosition { x: 80.0, y: 1.0, frame: 1 });
        let sent = Instant::now();
        guest.update_cursor(guest_point, sent);
        assert_eq!(next_cursor(&host), (1, guest_point));
        assert_eq!(next_cursor(&guest_two), (1, guest_point));
        host.update_cursor(host_point, sent);
        assert_eq!(next_cursor(&guest), (0, host_point)); // no self-echo
        assert_eq!(next_cursor(&guest_two), (0, host_point));
        guest.update_cursor(None, sent + Duration::from_secs(1));
        assert_eq!(next_cursor(&host), (1, None));
        assert_eq!(next_cursor(&guest_two), (1, None));
        assert_eq!(*host.host.as_ref().unwrap().project.lock().unwrap(), AniProject::default());
        let stroke = Stroke {
            points: vec![], color: [1, 2, 3, 255], fill_color: None,
            base_width: 3.0, is_vector: false,
            brush_preset: Default::default(), opacity: 1.0,
            brush_texture_id: None, tip_shape: Default::default(),
        };
        guest.send_strokes(0, 1, vec![stroke]);
        let mut delivered = false;
        let deadline = Instant::now() + Duration::from_secs(4);
        while std::time::Instant::now() < deadline {
            for event in host.events() {
                if let OnlineEvent::Strokes { layer, frame, strokes } = event {
                    assert_eq!((layer, frame, strokes.len()), (0, 1, 1));
                    delivered = true;
                }
            }
            if delivered { break; }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(delivered, "host must receive guest stroke once");

        // Regression: deleting or creating a layer/frame is an edit just like
        // painting, and must reach the host and every room participant.
        let mut guest_project = AniProject::default();
        guest.set_baseline(&guest_project);
        let added_layer = guest_project.layers[0].clone();
        guest_project.layers.insert(0, added_layer);
        guest_project.layers[0].name = "Guest's new layer".into();
        guest_project.layers[0].keyframes.insert(4, Default::default());
        guest_project.total_frames = 4;
        guest.sync_local_changes(&guest_project).expect("send structural edits");
        let deadline = std::time::Instant::now() + Duration::from_secs(8);
        let mut received_edits = false;
        while std::time::Instant::now() < deadline {
            for event in host.events() {
                if let OnlineEvent::Edits(edits) = event {
                    assert!(!edits.is_empty());
                    received_edits = true;
                }
            }
            if received_edits { break; }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(received_edits, "the host must see layer/frame changes");
        let peer_deadline = std::time::Instant::now() + Duration::from_secs(8);
        let mut forwarded = false;
        while std::time::Instant::now() < peer_deadline {
            for event in guest_two.events() {
                if let OnlineEvent::Edits(edits) = event {
                    forwarded = !edits.is_empty();
                }
            }
            if forwarded { break; }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(forwarded, "a layer/frame edit must also reach the OTHER guest");
        let host_document = host.host.as_ref().unwrap().project.lock().unwrap().clone();
        assert_eq!(host_document.layers.len(), 2);
        assert_eq!(host_document.layers[0].name, "Guest's new layer");
        assert!(host_document.layers[0].keyframes.contains_key(&4));

        // Host-only moderation propagates through the live roster.
        host.change_member_color(1, [247, 115, 87]).expect("host recolors first guest");
        let deadline = std::time::Instant::now() + Duration::from_secs(4);
        let mut color_received = false;
        while std::time::Instant::now() < deadline {
            for event in guest.events() {
                if let OnlineEvent::Roster(members) = event {
                    if members.iter().any(|p| p.name == "Guest" && p.color == [247,115,87]) {
                        color_received = true;
                    }
                }
            }
            if color_received { break; }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(color_received, "host color edits must reach the guest");

        // The earliest guest is elected. Real takeover/binding requires
        // separate Windows PCs because both instances here use TCP 47554.
        assert!(host.handoff_before_leave().expect("handoff should queue"));
        host.close();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut first_promoted = false;
        let mut second_redirected = false;
        while std::time::Instant::now() < deadline {
            for event in guest.events() {
                if let OnlineEvent::HostHandoff { promote, project, room_code, .. } = event {
                    assert!(promote, "earliest guest should become the host");
                    assert!(project.is_some());
                    assert_eq!(room_code, host.room_code);
                    first_promoted = true;
                }
            }
            for event in guest_two.events() {
                if let OnlineEvent::HostHandoff { promote, project, .. } = event {
                    assert!(!promote);
                    assert!(project.is_none());
                    second_redirected = true;
                }
            }
            if first_promoted && second_redirected { break; }
            thread::sleep(Duration::from_millis(25));
        }
        assert!(first_promoted && second_redirected,
            "all connected peers must receive the controlled host transfer");
        guest.close();
        guest_two.close();
    }

    #[test]
    fn host_transfer_packets_serialize_with_room_identity() {
        let packet = Packet::HostHandoff { address: "100.77.8.9:47554".into(),
            promote: true, title: "Artist room".into(), room_code: "12345678".into(),
            limit: 8, project: Some(Box::new(AniProject::default())) };
        let wire = serde_json::to_vec(&packet).unwrap();
        let decoded: Packet = serde_json::from_slice(&wire).unwrap();
        match decoded {
            Packet::HostHandoff { address, promote, room_code, project, .. } => {
                assert_eq!(address, "100.77.8.9:47554");
                assert!(promote);
                assert_eq!(room_code, "12345678");
                assert!(project.is_some());
            }
            _ => panic!("Handoff should deserialize"),
        }
    }
    #[test]
    fn packet_roundtrip() {
        let packet = Packet::Members { count: 7 };
        let text = serde_json::to_string(&packet).unwrap();
        assert!(matches!(serde_json::from_str::<Packet>(&text).unwrap(), Packet::Members { count: 7 }));
    }
}
