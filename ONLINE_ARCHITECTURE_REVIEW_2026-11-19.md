# Stick Studio Online Architecture Review — 19 Nov 2026

## Current architecture already in the repository

Stick already implements the useful core of two of the proposed approaches:

1. **Host + TCP/UDP**
   - The host opens the collaborative room.
   - TCP port 47554 carries the reliable room protocol and project events.
   - UDP port 47553 is used for LAN discovery.
   - Room codes are discovery locators, not global relay credentials.

2. **Virtual LAN / private mesh**
   - Tailscale discovery and direct 100.x addressing are integrated.
   - ZeroTier is supported as an optional manual private-network path.

This means the project does **not** need a networking rewrite just to let friends draw remotely.

## Changes adopted in this update

- Reworked Create/Join screens into icon-based connection cards.
- Added three clear connection paths:
  - LAN: automatic local discovery.
  - Tailscale: recommended private path between different Internet connections.
  - Direct / Tunnel: advanced host:port endpoint.
- All online icon cards use delayed hover help; the global UI tooltip delay is two seconds.
- Direct Join remains provider-neutral: any reachable TCP endpoint forwarding to local port 47554 can be pasted.
- Online Guide now explains the above paths and their limitations.

## Review of the external proposals

### tokio + broadcast + bincode
Potentially useful later, but not required for the current room size. The existing protocol already has a working host/session model. A migration should be done as a protocol-versioned refactor, not mixed into UI work.

**Future value:** lower thread count, cleaner async connection lifecycle, smaller packets if binary serialization replaces JSON.

### playit.gg
Useful as an external tunnel concept, but it should not be presented as a guaranteed free TCP solution. Current playit documentation marks generic TCP/TCP+UDP as premium. Stick therefore only documents it as an optional external provider.

Official site: https://playit.gg/

### ngrok
A TCP endpoint can forward Stick's TCP port. Current ngrok documentation says free-plan TCP endpoints require a valid payment method, and free-plan quotas apply.

Official TCP docs: https://ngrok.com/docs/gateway/endpoints/tcp

### WebRTC / matchbox_socket
Interesting long-term direction if Stick later wants NAT traversal without requiring Tailscale or an external tunnel. It would require signaling infrastructure and a second transport implementation, so it is deliberately not introduced in this patch.

### renet / QUIC
Could be useful if the protocol is split into:
- reliable project/edit messages;
- transient low-latency cursor/presence data.

That is a future protocol change and should increment Stick's collaboration protocol version.

## Recommended next networking phase

1. Keep TCP project synchronization as the reliable source of truth.
2. Move only transient cursor/presence traffic to a low-latency datagram channel if profiling shows TCP head-of-line blocking.
3. Add measured ping / packet timing to the online status bar before replacing transports.
4. If seamless Internet rooms become a priority, prototype WebRTC behind a feature flag while preserving Tailscale/LAN fallback.
5. Do not silently bundle or launch third-party tunneling executables; keep provider choice explicit to the user.

## Security notes

- Keep the Windows firewall enabled.
- Direct public endpoints expose the Stick host protocol to the Internet; room passwords are not a replacement for transport encryption.
- Tailscale/ZeroTier provide a private network layer and are preferable to directly exposing the host for normal friend sessions.
