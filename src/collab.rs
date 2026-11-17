//! Local-first collaborative rooms for Stick Studio.
//! A participant hosts the TCP session; UDP broadcast discovers rooms on a LAN.
//! Internet peers can use host-IP:port with appropriate router/firewall rules.
//! This is not an encrypted Internet service or a global room directory.
use crate::project::{AniProject, Stroke};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const TCP_PORT: u16 = 47554;
const DISCOVERY_PORT: u16 = 47553;
const DISCOVER_REQUEST: &[u8] = b"STICK_STUDIO_LAN_DISCOVER_V1";
const PROTOCOL: u16 = 1;
const MAX_PACKET: usize = 8 * 1024 * 1024;
const MAX_MEMBERS: usize = 12;
const MAX_FRAME: usize = 10_000;

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
}

#[derive(Serialize, Deserialize, Debug)]
enum Packet {
    Hello { name: String, protocol: u16, mode: OnlineMode },
    Challenge { nonce: String },
    Auth { proof: String },
    Welcome { project: Box<AniProject> },
    Rejected { reason: String },
    Strokes { layer: usize, frame: usize, strokes: Vec<Stroke> },
    Snapshot { project: Box<AniProject> },
    Members { count: usize },
    Goodbye,
}

#[derive(Debug)]
pub enum OnlineEvent {
    Connected(AniProject),
    Disconnected(String),
    Strokes { layer: usize, frame: usize, strokes: Vec<Stroke> },
    Snapshot(AniProject),
    Members(usize),
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
    peers: Mutex<HashMap<u64, mpsc::Sender<Packet>>>,
    project: Mutex<AniProject>,
    password: String,
    title: String,
    mode: OnlineMode,
    limit: usize,
    events: mpsc::Sender<OnlineEvent>,
    peer_ids: AtomicU64,
}

impl HostShared {
    fn member_count(&self) -> usize { self.peers.lock().map(|p| p.len()+1).unwrap_or(1) }
    fn broadcast(&self, msg: &Packet, exclude: Option<u64>) {
        if let Ok(peers) = self.peers.lock() {
            for (id, tx) in peers.iter() {
                if Some(*id) != exclude { let _ = tx.send(clone_packet(msg)); }
            }
        }
    }
    fn announce_members(&self) {
        let count = self.member_count();
        self.broadcast(&Packet::Members { count }, None);
        let _ = self.events.send(OnlineEvent::Members(count));
    }
}

fn clone_packet(packet: &Packet) -> Packet {
    match packet {
        Packet::Strokes { layer, frame, strokes } => Packet::Strokes { layer: *layer, frame: *frame, strokes: strokes.clone() },
        Packet::Snapshot { project } => Packet::Snapshot { project: project.clone() },
        Packet::Members { count } => Packet::Members { count: *count },
        _ => Packet::Goodbye,
    }
}

fn writer_loop(mut stream: TcpStream, rx: mpsc::Receiver<Packet>, running: Arc<AtomicBool>) {
    while running.load(Ordering::Relaxed) {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(packet) => {
                if packet_write(&mut stream, &packet).is_err() || matches!(packet, Packet::Goodbye) { break; }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn host_peer(mut stream: TcpStream, host: Arc<HostShared>) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(300)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let (name, protocol, mode) = match packet_read(&mut stream, &host.running)? {
        Packet::Hello { name, protocol, mode } => (name, protocol, mode),
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
    let (out_tx, out_rx) = mpsc::channel();
    {
        let mut peers = host.peers.lock().map_err(|_| io::Error::other("Peer registry poisoned"))?;
        if peers.len() + 1 >= host.limit {
            packet_write(&mut stream, &Packet::Rejected { reason: "Room is full".into() })?;
            return Ok(());
        }
        // Snapshot and registration are ordered with subsequent host operations.
        let project = host.project.lock().map_err(|_| io::Error::other("Room project lock poisoned"))?.clone();
        packet_write(&mut stream, &Packet::Welcome { project: Box::new(public_project(project)) })?;
        peers.insert(id, out_tx);
    }
    host.announce_members();
    let writer = stream.try_clone()?;
    let running = host.running.clone();
    thread::spawn(move || writer_loop(writer, out_rx, running));
    loop {
        match packet_read(&mut stream, &host.running) {
            Ok(Packet::Strokes { layer, frame, strokes }) => {
                let accepted = host.project.lock().map(|mut p|
                    apply_strokes(&mut p, layer, frame, strokes.clone())).unwrap_or(false);
                if accepted {
                    let _ = host.events.send(OnlineEvent::Strokes { layer, frame, strokes: strokes.clone() });
                    host.broadcast(&Packet::Strokes { layer, frame, strokes }, Some(id));
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

pub struct OnlineSession {
    pub title: String,
    pub mode: OnlineMode,
    pub address: String,
    pub is_host: bool,
    pub members: usize,
    event_rx: mpsc::Receiver<OnlineEvent>,
    outgoing: Option<mpsc::Sender<Packet>>,
    host: Option<Arc<HostShared>>,
    running: Arc<AtomicBool>,
}

impl OnlineSession {
    pub fn host_room(title: String, password: String, limit: usize, mode: OnlineMode, project: AniProject) -> Result<Self, String> {
        if title.trim().is_empty() || title.chars().count() > 48 { return Err("Room name must have 1–48 characters".into()); }
        if password.chars().count() > 128 { return Err("Password is too long".into()); }
        if !(2..=MAX_MEMBERS).contains(&limit) { return Err("Room capacity must be 2–12".into()); }
        let listener = TcpListener::bind(("0.0.0.0", TCP_PORT)).map_err(|e|
            format!("Cannot host on port {}: {} (allow Stick through the firewall)", TCP_PORT, e))?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let (event_tx, event_rx) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let host = Arc::new(HostShared {
            running: running.clone(), peers: Mutex::new(HashMap::new()),
            project: Mutex::new(project), password: password.clone(),
            title: title.clone(), mode, limit, events: event_tx,
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
                        };
                        if let Ok(json) = serde_json::to_vec(&listing) { let _ = socket.send_to(&json, address); }
                    }
                }
            }
        });
        Ok(Self { title, mode, address: local_address(), is_host: true, members: 1,
            event_rx, outgoing: None, host: Some(host), running })
    }

    pub fn join_room(address: String, password: String, nickname: String, mode: OnlineMode) -> Result<Self, String> {
        let address = if address.contains(':') { address } else { format!("{}:{}", address, TCP_PORT) };
        let addr: SocketAddr = address.parse().map_err(|_| "Enter a valid IPv4 address and port, such as 192.168.1.5:47554".to_string())?;
        let (event_tx, event_rx) = mpsc::channel();
        let (out_tx, out_rx) = mpsc::channel();
        let running = Arc::new(AtomicBool::new(true));
        let running_worker = running.clone();
        thread::spawn(move || {
            let result = (|| -> Result<(), String> {
                let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5)).map_err(|e| format!("Connection failed: {}", e))?;
                stream.set_read_timeout(Some(Duration::from_millis(300))).map_err(|e| e.to_string())?;
                stream.set_write_timeout(Some(Duration::from_secs(3))).map_err(|e| e.to_string())?;
                packet_write(&mut stream, &Packet::Hello { name: nickname, protocol: PROTOCOL, mode }).map_err(|e| e.to_string())?;
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
                        Ok(Packet::Strokes { layer, frame, strokes }) => {
                            let _ = event_tx.send(OnlineEvent::Strokes { layer, frame, strokes });
                        }
                        Ok(Packet::Snapshot { project }) => { let _ = event_tx.send(OnlineEvent::Snapshot(*project)); }
                        Ok(Packet::Members { count }) => { let _ = event_tx.send(OnlineEvent::Members(count)); }
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
            event_rx, outgoing: Some(out_tx), host: None, running })
    }

    pub fn events(&self) -> Vec<OnlineEvent> { self.event_rx.try_iter().collect() }

    pub fn send_strokes(&self, layer: usize, frame: usize, strokes: Vec<Stroke>) {
        if let Some(host) = &self.host {
            if host.project.lock().map(|mut p| apply_strokes(&mut p, layer, frame, strokes.clone())).unwrap_or(false) {
                host.broadcast(&Packet::Strokes { layer, frame, strokes }, None);
            }
        } else if let Some(tx) = &self.outgoing {
            let _ = tx.send(Packet::Strokes { layer, frame, strokes });
        }
    }

    /// Explicit host-authoritative structure resync; clients never replace host state.
    pub fn resync_from_host(&self, project: AniProject) {
        if let Some(host) = &self.host {
            if let Ok(mut state) = host.project.lock() { *state = project.clone(); }
            host.broadcast(&Packet::Snapshot { project: Box::new(public_project(project)) }, None);
        }
    }
    pub fn close(&self) {
        if let Some(tx) = &self.outgoing { let _ = tx.send(Packet::Goodbye); }
        self.running.store(false, Ordering::Relaxed);
    }
}
impl Drop for OnlineSession { fn drop(&mut self) { self.close(); } }

pub fn discover_rooms() -> Vec<RoomListing> {
    let mut rooms = Vec::new();
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0") else { return rooms };
    let _ = socket.set_broadcast(true);
    let _ = socket.set_read_timeout(Some(Duration::from_millis(450)));
    let _ = socket.send_to(DISCOVER_REQUEST, ("255.255.255.255", DISCOVERY_PORT));
    let _ = socket.send_to(DISCOVER_REQUEST, ("127.0.0.1", DISCOVERY_PORT));
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
    fn packet_roundtrip() {
        let packet = Packet::Members { count: 7 };
        let text = serde_json::to_string(&packet).unwrap();
        assert!(matches!(serde_json::from_str::<Packet>(&text).unwrap(), Packet::Members { count: 7 }));
    }
}
