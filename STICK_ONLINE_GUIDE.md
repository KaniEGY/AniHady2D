# Stick Online — LAN-first collaboration (preview)

Stick Online extends the existing Drawing and Animation editors. It does not require a developer-operated server: one participant hosts a room using their own PC. **This preview is not a complete multi-user editor.**

## How to start

1. Open Stick Studio and choose **Online / أونلاين** underneath Drawing and Animation.
2. Pick **Draw Together / نرسم سوا** or **Animate Together / أنيميشن سوا**.
3. Choose **Create Room** to host, or **Join Room** to connect.
4. Hosting: name your room, optionally provide a unique password, and select a maximum of 2–12 total participants (including the host).
5. Joining on the same LAN: press **Refresh rooms**, select the room from the list and supply its password if necessary.
6. If automatic discovery is blocked, enter the host's **LAN IPv4:47554**, for example **192.168.1.24:47554**, then select Join.
7. In the editor, the green Online status bar displays the room and current participant count. The host can explicitly send a full project snapshot using **Sync full project**. **Leave Room** disconnects without deleting your local project.

## Network requirements

- TCP **47554**: direct room connection, all edits/initial project data.
- UDP **47553**: LAN broadcast room discovery, not an Internet directory.
- Windows may prompt the host to allow Stick on private networks. Both PCs should be on the same LAN without client isolation.
- You can test two app instances on the same Windows PC by using **127.0.0.1:47554**.
- Outside the LAN, guests must know the host's accessible public IPv4 address, and the host's router must forward TCP 47554 to its Windows PC. This is **not** automatic NAT traversal.
- Connections behind **CGNAT**, firewalls, isolated guest Wi-Fi, or different networks without port forwarding can fail. No worldwide room catalogue is available without a discovery/rendezvous facility.

## Safety, privacy and current limitations

- Traffic is **not end-to-end encrypted**. Use a temporary, unique room password; do not share private/sensitive project files with people you do not trust. The nonce challenge avoids transmitting the literal password, but this is *not* a substitute for authenticated encryption or secure Internet access.
- Host may receive each collaborator's public or LAN IP address. Sharing a public host IP exposes that connection endpoint to participants.
- The host sends its current project when guests join and serializes **newly committed strokes** across connected artists. Vector and brush strokes support simultaneous additions within the existing layer/frame structure.
- **NOT yet synchronized automatically:** layer creation/reordering/names, erasing/editing existing strokes, transforms, undo/redo, independent keyframe edits, media/audio, text editing, and timeline operations. The host's explicit full-project resync overwrites the clients' local copy; save a backup first if collaborators have unsynchronized changes.
- All users should run the same Stick version. The room is closed when the host leaves.
- Networking is not a secure public server. Hardening (encrypted transport, bandwidth limits, malicious-client resistance, conflict resolution, reconnect, and relay/IPv6) is future work.

## Required Windows acceptance checks

- [ ] Host drawing room, join from second Windows PC over LAN using discovery.
- [ ] Join by manual IPv4, wrong-password rejection, full-room rejection.
- [ ] Draw on both PCs; each new stroke appears on both, exactly once, same frame/layer.
- [ ] Repeat on separate animation frames, confirm inherited frame content.
- [ ] Verify guests don't overwrite the host project when joining; save locally as a new file.
- [ ] Host syncs a snapshot explicitly; ensure guests receive it.
- [ ] Disconnect client then host; ensure both keep local project work and ports are released.
- [ ] Check Windows firewall prompts and failures with discovery blocked.
- [ ] Test local projects created with old Stick versions and standard save/reopen.
- [ ] Confirm no attempt is made to contact an official Stick server.

The GitHub Actions unit test and release build checks do not replace this two-PC GUI/network test.

## Room codes and roster (protocol v2)
- The host displays a copyable eight-digit **LAN room identifier**. Guests on the same LAN refresh the list, select the room or enter its code. For remote networks a code by itself **cannot** discover the host without a rendezvous service; use public reachable IPv4:47554 and port forwarding.
- Host and guest advertise display names and local, unverified progression levels. Each connected guest is assigned a distinct UI accent from the per-room palette. This is NOT cloud identity, anti-cheat or Google account linking.
- A new room now hosts the currently open project rather than wiping it to a blank canvas. Save/Export local copies via the normal Stick file actions. Other operations besides committed strokes are NOT fully synchronized.
- Protocol v2 is intentionally incompatible with older Stick online beta builds. All participants must upgrade to this version.
