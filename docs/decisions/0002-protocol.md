# 0002: Protocol, transport and framing

Status: accepted, 2026-11-20.

## Context

Every surface (CLI, GPUI desktop, Android, remote desktop) is a client of one
core service. Clients and servers update independently, so mixed versions are
the normal case, not an edge case. The local service is reached over IPC; the
headless server (Phase 3) and Android reach it over the network. Android uses
a Rust client core through UniFFI.

## Decision

**Transports.**

- Local: `interprocess` local sockets on top of tokio. Named pipes on Windows,
  Unix domain sockets elsewhere. The name is derived from the profile (see
  0004), so a client never needs a discovery file to connect.
- Network (Phase 3, direction only): WebSocket binary messages over TLS
  (`tokio-tungstenite` + `rustls`), the certificate pinned by fingerprint at
  pairing time instead of a public CA. Each WebSocket message carries exactly
  one frame body, so everything above framing is shared. The final network
  record is written in Phase 3.

**Framing (stream transports).** `u32` big-endian length, then the frame body.
The body's first byte is the frame kind; `0x01` is a UTF-8 JSON message. Any
other kind must be enabled by a negotiated capability first, because a peer
that does not know a kind cannot do anything sensible with it. Frames above
the negotiated limit (default 8 MiB local, 1 MiB network) close the
connection rather than being truncated.

**Serialization.** JSON with `serde`. Readers ignore unknown fields; writers
omit `None` fields. Every enum that crosses the wire decodes an unknown value
to an `Unknown` fallback; the full list is "Wire compatibility rules" in
`docs/architecture/core.md`. Bulk binary data (terminal output in Phase 2) gets its own
frame kind behind a capability rather than base64 inside JSON.

**Messages.** Every JSON message has a `type`:

| type | direction | purpose |
|---|---|---|
| `hello` | client to server | first message: protocol range, client name and version, capabilities, optional auth |
| `welcome` | server to client | chosen protocol version, server version, capabilities, connection id |
| `rejected` | server to client | sent instead of `welcome` when the handshake fails: `error` plus `supported` (the server's protocol range); the server then closes |
| `request` | client to server | `id` (client-chosen u64), `method`, `params` |
| `response` | server to client | `id` plus exactly one of `result` or `error` |
| `event` | server to client | `stream` id plus `data`: one item of a server-pushed stream |
| `end` | server to client | `stream` id, optional `error`: the stream is finished |
| `cancel` | client to server | `stream` id: stop a stream |

A stream is opened by an ordinary request whose result contains the stream
id. Streams die with the connection; clients re-subscribe after reconnecting.

**Versioning and capabilities.** `hello` carries `protocol: {min, max}`
(integers). The server picks the highest version both support and answers
`welcome`. When the ranges do not overlap it answers `rejected` with
`protocol.unsupported_version`; when `hello` is malformed or missing it answers
`rejected` with `protocol.handshake`. Either way `supported` carries the
server's own range so the client can report what to upgrade, and the server
closes after sending it. A client older than `rejected` decodes it as an
unknown message and sees the close. Inside one protocol
version only additive changes are allowed (new optional fields, new methods,
new capabilities). Features are grouped into named, versioned capabilities
(`service/1`, `workspaces/1`, `events/1`); `welcome` lists what the server
offers, and a client must not call methods of a capability it was not
offered. Extensions add capabilities through the same registry (no
hard-wired list in the dispatcher).

**Errors.** `{code, message, data?}`. `code` is a stable dotted string
(`workspace.not_found`, `request.invalid_params`, `method.not_found`), never a
number and never a message to be parsed. `message` is for humans. Clients
treat `method.not_found` as "this peer does not support it".

**Authorization room.** Every method declares a permission class
(`read`, `workspace`, `host-admin`, ...) in the method registry. Phase 1 only
has the local owner, who holds every class; the check is still made per
call, so Phase 3 adds callers without touching each handler.

**UniFFI.** The `protocol` crate is plain data (serde types, no IO). It avoids
types UniFFI cannot express (no `u128`, no generic public records, timestamps
as `i64` Unix milliseconds, ids as strings). `client-ffi` later wraps the
`client` crate and derives UniFFI records on protocol types behind a feature,
so the Android client and the server compile the same definitions.

## Alternatives

- **JSON-RPC 2.0 as is.** Close to this, but it has no notion of streams,
  handshake or capabilities; we would extend it anyway.
- **gRPC / tonic.** Strong typing and streaming, but HTTP/2 over named pipes is
  awkward, codegen complicates UniFFI sharing, and protobuf field numbering
  adds ceremony with little gain for one team owning both sides.
- **MessagePack or CBOR.** Smaller and binary-friendly, but harder to debug by
  eye, and the only bulk-binary case is handled by a dedicated frame kind.
- **Cap'n Proto.** Excellent evolution story, too heavy for the team size.

## Consequences

- Any frame can be logged and read by a person. `massorca` can offer a raw
  request command later at no cost.
- Additive evolution is the default; breaking changes need a new protocol
  version and a server that speaks both for a while.
- A Phase 2 binary frame kind must be capability-gated before use.
