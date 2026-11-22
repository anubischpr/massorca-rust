# 0047: Where the Phase 3 code lives

Status: accepted, 2026-11-21.

## Context

Phase 3 adds TLS, WebSocket, pairing, device storage, Tailscale and SSH
bootstrap. Other sessions change `server`, `client` and `protocol` at the same
time (terminals, git), so Phase 3 must add, not reshape.

## Decision

| Crate | Phase 3 content |
|---|---|
| `net` (new, `massorca-net`) | TLS identities (key, certificate, pin), the pinned server verifier and the device client verifier, the WebSocket frame transport, the network connector. No service logic. |
| `core-remote` (new, `massorca-core-remote`) | Tailscale status, SSH bootstrap through the system `ssh`, the systemd user unit. Named in the PLAN layout already. |
| `core-store` | one migration adding the `devices` table and its queries |
| `protocol` | the `devices/1` and `network/1` methods, the auth payloads and error codes |
| `server` | the network listener, pairing offers, the device principal and its scopes, revocation of live connections |
| `client` | connecting to a host over the network, the client host list |
| `cli` | the `pair`, `device`, `host` and `server` groups and the global `--host` |

The connection code of the server and the client is made generic over a frame
transport (a reader and a writer of frames), so the local socket and the
WebSocket share every line above framing. Phase 3 branches start from the
Phase 2 terminal chain because that chain already changes the same transport
code; they are rebased onto `main` as it lands.

## Consequences

- `net` is the only crate that touches cryptography, which keeps the
  security review in one place.
- `server` and `client` depend on `net`; `net` depends only on `protocol`.
