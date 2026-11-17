# Stick Online — friends over the Internet and LAN

Stick is **hosted on the creator's own computer**. It listens on TCP 47554 for collaborators and uses UDP 47553 to discover rooms. This implementation uses protocol v3 and synchronizes document edits via the host. There is **no central Stick account, global room directory or server operated by Stick**.

## Play with friends on different Internet connections (recommended)

1. Each person installs [Tailscale for Windows](https://tailscale.com/download) and signs in. Tailscale is **optional third-party software**, not bundled with Stick.
2. The host can **share only their computer** with a friend's Tailscale account, or invite that friend to the same tailnet. Do not share passwords or accounts. Review access policies before inviting people to a wider network. [Official sharing instructions](https://tailscale.com/docs/features/sharing).
3. In Stick, choose **Online → Draw Together / Animate Together → Create Room**. The creator's PC stays the host. Keep Stick and Tailscale running.
4. The host can copy the **Friends 100.x.y.z:47554** button at the top of the editor and send the address to their invited friend. This is a private Tailscale IP, **not** the router's public IP.
5. The friend chooses **Online → Join Room**, clicks **Refresh rooms**, and selects the room if it is discovered. If no room appears, paste the host's **100.x.y.z:47554** into the Host IP field and press Join. Use the room password if required.
6. Tailscale establishes a private encrypted path and can use its own DERP relay when direct NAT traversal fails. Stick itself still uses TCP and does **not** implement additional application-level encrypted transport. You do **not** need port forwarding for the Tailscale route.

**Important:** An eight-digit Stick room code resolves only among rooms **actually discovered** on the LAN or accessible tailnet peers. It is not a worldwide invite service. Use the host's Tailscale address when discovery fails.

## Same home / LAN

- Host creates a room; friend selects it from the LAN discovery list or uses host's **LAN IPv4:47554**.
- TCP **47554** carries all edits and the joining snapshot.
- UDP **47553** advertises rooms using LAN broadcast or unicast to visible Tailscale peers.
- Two instances on the same PC can test `127.0.0.1:47554`.
- Both ends must run compatible Stick protocol v3 builds.

## Troubleshooting

- **Windows error 10061 / actively refused:** there is no accepting server at the chosen IP and TCP port, or software is blocking it. Confirm the room is open on the host, the IP is correct, and that inbound **TCP 47554** is allowed for Stick from trusted devices.
- **192.168.x.x or 10.x.x.x works at home but not from your friend's network:** that address is LAN-only. Use the **100.x** Tailscale host IP instead. Public IPv4 port forwarding is an advanced alternative only if the host actually has a reachable public address.
- **Tailscale not connected:** install it, sign in, connect and make sure the friend is permitted to reach the host device. Press **Refresh rooms** after it is running.
- **Room not listed but direct address works:** UDP **47553** discovery may be blocked. If desired, allow UDP discovery traffic **only from trusted LAN/Tailscale devices**; manual IP joining does not need the UDP discovery port.
- **Connection times out:** Tailscale peer visibility, access policies and host firewall may prevent reaching TCP 47554. Do not switch off Windows Firewall globally.
- **Host leaves:** the room closes; each client keeps its local project. This is not a cloud-synced permanent room.

## Collaboration limitations / الأمان

- The host serializes edits, including drawing, bucket fill, erasing, layers, keyframes, frame content, text and transforms.
- Concurrent edits to the same array position can still conflict. The host can use **Sync full project** to restore participants to the same document.
- External image/audio **file contents are not transferred** through Stick; absolute local asset paths are deliberately suppressed. Saving and reopening a legacy project needs its original files.
- There is no dedicated reconnect/replay service, global player directory or end-to-end transport encryption **inside Stick**. Tailscale encrypts its transport when used; plain LAN/direct TCP is not independently encrypted by Stick.
- Keep a unique password for private rooms, limit device sharing to trusted friends and review what tailnet access you grant.

## مطلوب اختبار Windows بين جهازين حقيقيين

- [ ] Host Tailscale connected, guest joined from a different ISP; verify drawing and all document edits.
- [ ] Direct guest join using the host's 100.x address when UDP discovery is blocked.
- [ ] Test room listing over Tailscale when UDP discovery is allowed.
- [ ] Test password rejection, capacity, disconnect, reopened room and LAN behavior.
- [ ] Test erasing, adding/deleting layers, animation keyframes, and explicit host resync on both PCs.
- [ ] Test selection transform at zoom 25%, 100%, 400% and each of its four corner handles.
- [ ] Confirm old project save/reopen and Previous Versions rollback remain functional.

GitHub Actions compilation/unit tests do **not** replace this acceptance test.
