# Stick cross-network online — optional Remote Bridge

Status: **experimental implementation branch — not merged into stable main yet**  
Date: 2026-11-18

## Goal

Let two Stick users on different Internet networks use the existing Drawing/Animation room protocol without requiring LAN, Tailscale, ZeroTier, router port forwarding, or a public IPv4 address.

The current networking remains untouched and available:

- LAN discovery
- direct IP / DNS TCP
- Tailscale / ZeroTier
- current Stick room code/password/invites

The new component is intentionally a **separate downloadable executable**:

`StickOnlineBridge.exe`

It tunnels the existing Stick TCP session; it does not replace or reinterpret the collaboration packets.

## Implemented architecture

Iroh is used as the encrypted peer-to-peer transport.

```
HOST PC
Stick.exe :47554 <-- localhost TCP --> StickOnlineBridge.exe
                                      |
                                encrypted QUIC
                           direct NAT traversal first
                                      |
                              relay fallback if needed
                                      |
GUEST PC                              |
Stick.exe --> 127.0.0.1:47555 --> StickOnlineBridge.exe
```

Iroh's N0 preset provides address lookup, NAT traversal and production relay fallback. The remote endpoint ID authenticates the peer; relay infrastructure only forwards encrypted Iroh traffic and cannot decode the QUIC payload.

Official references:
- https://docs.rs/iroh/latest/iroh/
- https://docs.rs/iroh/latest/iroh/endpoint/presets/struct.N0.html
- https://github.com/n0-computer/iroh

Iroh is dual licensed MIT / Apache-2.0. Stick does not copy Iroh source code; it consumes the published Rust crate as an optional dependency.

## Host flow

1. Open a normal Stick room. Stick keeps listening on TCP 47554 as it does today.
2. Run:
   `StickOnlineBridge.exe host`
3. The bridge binds an Iroh endpoint, waits until its relay/discovery path is online, and prints:
   `STICK_REMOTE_CODE=stick-remote://<endpoint-id>`
4. Share only that remote code with the guest.
5. Keep Stick and the bridge open.

The bridge only connects inbound remote bridge streams to `127.0.0.1:47554`.

## Guest flow

1. Run:
   `StickOnlineBridge.exe join stick-remote://<endpoint-id>`
2. The bridge listens only on:
   `127.0.0.1:47555`
3. In Stick's existing Direct Join box enter:
   `127.0.0.1:47555`
4. Enter the normal Stick room password if the host configured one.

The existing Stick password challenge, protocol version validation, capacity limits, host moderation and project synchronization all still execute unchanged over the tunnel.

## Security properties

- Remote transport is authenticated/encrypted by Iroh QUIC/TLS.
- The bridge listens on loopback only; it does not expose a new public Windows TCP listener.
- The host endpoint ID is not the Stick room password.
- Stick's current password proof remains the application-level room authorization.
- No Cloudflare/API credentials, server secrets or account tokens are embedded in either executable.
- The bridge does not persist project contents or audio.
- Public relay infrastructure can observe connection metadata, but Iroh documents that relay servers cannot decode endpoint traffic.
- Existing Stick packet-size and project validation still applies because payload bytes reach the normal Stick TCP server unchanged.

## Build / packaging

The bridge is feature-gated:

`cargo build --release --features remote-bridge --bin StickOnlineBridge`

Stable Stick builds remain WGPU/egui as before. CI on this branch adds:

- normal Stick tests / Clippy / Release build;
- bridge-specific tests;
- bridge Release build;
- `StickOnlineBridge.exe` in the Windows artifact and, after eventual merge, as a separate GitHub Release asset;
- SHA-256 entries for both executables.

## Why this supersedes the initial Cloudflare WSS design for phase 1

The earlier WSS/Durable Object design requires deploying and operating a central service/account before users can connect. The optional Iroh bridge can establish remote connectivity through the library's production discovery/relay infrastructure without a Stick-owned cloud credential.

A future Stick-owned WSS/WebRTC service is still an option if:
- public relay dependency becomes unacceptable;
- service policy/cost changes;
- Stick needs a global room directory rather than explicit invitations;
- controlled infrastructure / telemetry / abuse handling becomes necessary.

## Required real-device acceptance tests

Do **not** call this production ready until all of these pass:

- two separate Windows PCs;
- two genuinely separate Internet connections/NATs (for example home Wi-Fi + mobile hotspot);
- host bridge reaches READY and guest bridge connects;
- Drawing room: strokes, erasing, fill, cursor sync;
- Animation room: frames/keyframes/project edits;
- correct room password succeeds and incorrect password is rejected;
- kick/rename/color moderation propagates;
- disconnect/reconnect;
- at least one setup where Iroh reports/uses a relayed path because direct NAT traversal is not available;
- long idle room followed by edits;
- LAN/direct/Tailscale routes still work when bridge is not used.

## Explicit non-claims

CI can prove that the bridge builds and unit tests pass. CI cannot prove two real NATs can communicate. Until the real-device matrix above is performed:

- do not label the add-on “stable”;
- do not auto-install it for all users;
- keep it optional;
- keep all legacy online methods visible.


## Alternative fallback: Radmin VPN Assistant

A second optional helper is now built on this branch:

`StickRadminAssistant.exe`

This path is intentionally independent from the Iroh bridge. It is for users whose NAT/provider setup still prevents the experimental direct/relay bridge from working reliably.

Radmin VPN is third-party software and is **not bundled** with Stick. Its official site currently describes Radmin VPN as completely free and provides the workflow Create network -> Network name/password -> Join network. The helper only:

- detects the Radmin virtual adapter and its 26.x IPv4 address;
- shows the exact `<Radmin-IP>:47554` address for Stick Direct Join;
- checks the host with a single ping when requested;
- copies the Stick Direct Join address to the Windows clipboard;
- opens the official Radmin download page when the adapter is absent.

The assistant never disables Windows Firewall, never stores the Radmin network password, and never downloads/redistributes Radmin binaries.

Real acceptance test remains mandatory: two Windows PCs on different Internet connections must join the same Radmin network and then connect in Stick using Direct Join.
