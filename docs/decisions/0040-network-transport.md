# 0040: Network transport

Status: accepted, 2026-11-21.

## Context

0002 planned WebSocket binary messages over TLS for the network and left the
final record to Phase 3. The headless server (VPS), a remote desktop and the
Android app reach the service over the network. Android uses the same Rust
`client` crate through UniFFI, so whatever the CLI speaks, the phone speaks.
Phones change networks and sleep; servers restart for upgrades.

## Decision

**Stack.** TCP, then TLS 1.3 (`rustls` with the `ring` provider, through
`tokio-rustls`), then WebSocket (`tokio-tungstenite`, RFC 6455) on the path
`/massorca/v1`. We run TLS ourselves and hand the TLS stream to tungstenite,
so the same certificate checks (0041) apply to every client and no system or
web root store is consulted. `ring` avoids the CMake and NASM toolchain that
`aws-lc-rs` needs on Windows and builds for Android.

**Frames.** One WebSocket binary message carries exactly one frame body of
0002: the kind byte, then the body. The `u32` length prefix is dropped
because WebSocket already delimits messages. Text messages are a protocol
error and close the connection. Everything above the frame (handshake,
requests, streams, stream-bytes frames of 0035) is shared with the local
socket; the connection code does not know which transport it runs on.

**Limits.** Frame limit 1 MiB on the network (0002's default), announced in
`welcome.max_frame_len`; tungstenite's message and frame limits are set to the
same value so an oversized message is refused before it is buffered. At most
128 network connections at a time; over that, a new connection is closed as
soon as it is accepted, before the TLS handshake, so it costs no handshake
work. Of those, at most 32 may be unauthenticated (accepted but not yet past
`welcome`), and at most 4 from one address. Before `welcome` the server reads
one frame of at most 64 KiB (the `hello`) and nothing else, so an
unauthenticated peer cannot fill the inbound queue. A connection must finish
TLS, the WebSocket upgrade and `hello` within 10 seconds.

**Keep-alive.** Both sides send a WebSocket ping every 20 seconds. A side that
receives nothing at all (no frame, ping or pong) for 60 seconds closes the
connection. tungstenite answers pings while reading, so a busy peer still
answers.

**Reconnect and resume.** There is no session resume on the wire. A dropped
connection ends its streams (0002); the client reconnects with a fresh
handshake and re-subscribes. Terminals already resume from an output offset
(`terminal.attach since`, 0035), which covers the case that matters. The
client library reconnects with backoff (250 ms doubling to 30 s, reset after
60 s connected); the one-shot CLI simply reports the failure.

**Local socket unchanged.** The local endpoint keeps its owner-only socket and
`u32` framing (0003). Only the network listener is new.

## Alternatives

- **QUIC (`quinn`).** Handles network changes on phones better, but UDP is
  blocked on many networks and the proxy and debugging story is weaker.
  WebSocket over TLS passes everywhere HTTPS does. Can be added later as a
  second transport under the same frames.
- **Raw TLS with the `u32` framing.** Simpler, but does not pass HTTP reverse
  proxies or `tailscale serve`, and browsers cannot speak it if a web client
  ever returns.
- **Application-level encryption without TLS (the old app).** The old app
  rejected TLS because React Native could not pin a self-signed certificate.
  Our phone uses Rust and `rustls`, so the standard protocol is available and
  we do not invent one.
- **HTTP/2 or gRPC.** Rejected in 0002 already.

## Consequences

- One dependency set (`rustls`, `tokio-rustls`, `tokio-tungstenite`) serves
  server, CLI, desktop and phone.
- The frame codec gains a "one body per message" form next to the
  length-prefixed one.
- Long transfers must fit 1 MiB frames; stream-bytes payloads are already
  capped at 64 KiB.
