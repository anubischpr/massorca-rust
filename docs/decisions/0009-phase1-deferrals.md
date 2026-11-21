# 0009: Phase 1 deferrals

Status: accepted, 2026-11-21.

## Context

Phase 1 builds the service core, the CLI and the first project and workspace
features. The architecture names more than that. Each item below is left out on
purpose. The table says where it lands, so the Phase 1 code keeps the seam the
item will plug into, and nothing is forgotten.

## Decision

| Deferred | Lands in | Seam kept in Phase 1 |
|---|---|---|
| Capability-changed event: a change event when a capability is offered or withdrawn | M2 | Capabilities are registered and withdrawn through handles (`CapabilityRegistry`); events go through the event bus |
| Devices and authentication: a device principal, credentials in `hello.auth`, pairing | Phase 3 | `Principal` is non-exhaustive and set per connection at the handshake; `hello` already carries `auth` |
| Extensions as principals and the extension host: packages, dev links, sandboxing | M6 | `Owner::Extension` and the reserved `ext.` prefix are enforced at registration; `extensions/` is reserved |
| Network transport: WebSocket over TLS with a pinned certificate | Phase 3 | Framing is transport-neutral; `welcome` carries `max_frame_len` |
| The workspace deferrals in 0008: branch prefixes, generated names and their retirement, base-ref fallback order, nested layout options, setup scripts, terminal and agent guards on removal, squash-merge detection, id aliases after a folder rename, remote hosts | A later phase, each with its own tests | 0008's model adds columns and options, not a new model |

The handshake records what the client offered in `hello` and hands it to
handlers as `RequestCtx::client_capabilities`. No behaviour depends on it yet.

## Consequences

- A reviewer can check a deferred item against this table instead of the
  architecture documents.
- When an item moves to another phase, this table changes in the same change.
