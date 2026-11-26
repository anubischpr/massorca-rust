# 0131: Port forwards and advertised port URLs

Status: accepted, 2026-11-26.

## Context

REM-16 asks for manual port forwards: a local port on the loopback address carried to a port on a remote host. REM-17
asks to see the host's listening ports and open dev-server URLs locally. The reference forwards over its SSH transport
and matches URLs printed in SSH terminals to a local forward. Here a host is reached over the paired WebSocket
connection (0040), `host.ports` already lists the terminals' listening sockets on the host (0081), and stream-bytes
frames (`0x02`, `binary-frames/1`) carry terminal output from service to client. No client sent stream bytes yet: the
service dropped them.

A forward lets a paired device reach a port on the host that may listen on loopback precisely so that nothing else can
reach it: a database, a dev server, an admin page.

## Decision

- D1 Capability and methods. Capability `port-forward/1` with four methods. `host.forward` (stream) opens a forward to
  one port and keeps it while its stream is open: its first event is `opened` with the forward (`id`, `port`, optional
  `address` and `local_port`, `device_id`, counters); when `host.forward_stop` stops it, a `stopped` event comes before
  the end. `host.forward_connect {forward}` opens one TCP connection to that port and carries it on a duplex stream.
  `host.forwards` lists the open forwards of every caller. `host.forward_stop {forward}` stops one and every
  connection it carries. A forward is tied to the connection that opened it: only that connection may connect through
  it, and it ends when that connection ends.
- D2 Wire shape. No new frame kind: the tunnel rides on stream-bytes frames of the connect stream, in both directions.
  A duplex stream is the only kind whose client-sent bytes the service reads; bytes for any other stream are still
  dropped. Each payload is one tagged chunk: `0x00` data (1 to 32 KiB), `0x01` end of data, `0x02` acknowledgement
  (`u32` big endian, a count of data chunks). The connection's normal `end` and `cancel` close the stream.
- D3 Old peers. A client calls the methods only when the service offers `port-forward/1` (and `binary-frames/1`);
  otherwise it refuses on its side with `service.capability_missing`, before sending anything. An older service would
  silently drop the client's stream bytes, so they are never sent to one. An older client never calls the methods, and
  `host.ports` only gains optional fields.
- D4 Permission. Opening, connecting and stopping need `host-admin`; listing needs `read`. Running a terminal needs
  `workspace`, and a `workspace` device can already reach a loopback port from a shell, but a forward turns that into a
  standing listener on another machine with no program on the host to see or stop. `host-admin` is the class for
  host-level exposure (listeners, extensions). Paired devices get every class by default (0041), so only a deliberately
  reduced device (`pair offer --standard`) is refused, with `permission.denied`.
- D5 Targets. Only loopback addresses on the host. Without an address the service tries `127.0.0.1`, then `::1`, since
  dev servers bind `localhost` to either. Any other address is refused with `forward.target_not_loopback`; no option
  widens it. Nothing accepting the connection is `forward.connect_failed`, answered to the connect request, so the
  client closes its local socket.
- D6 Local side. `massorca host forward NAME PORT` listens on `127.0.0.1` and the same port unless `--local` (0 picks a
  free port, printed in the first line) or `--bind` say otherwise; a non-loopback `--bind` is the user's explicit
  choice and prints a warning. The forward is foreground: it ends with the command (Ctrl+C), when stopped on the host,
  or when the connection is lost (exit 4). Saving forwards per host and restoring them on reconnect is the desktop's
  part of REM-16 and is not built here.
- D7 Backpressure and close. Each direction may have at most 16 data chunks unacknowledged; a chunk is acknowledged once
  written to the receiving socket, so a slow reader slows the sender and no queue grows without limit. The service hands
  client bytes to a stream without waiting; a client that ignores the window fills the stream's queue and the stream
  ends with `stream.inbound_overflow`. End of data half-closes: the receiver shuts its socket's write side and keeps
  reading. The tunnel ends when both sides sent end of data, either side fails or cancels, the forward stops, or it is
  idle. Closing either socket therefore closes the other.
- D8 Limits. 16 forwards per connection and 64 connections per forward at a time (`forward.limit`); the CLI closes a
  65th local connection at once. A tunnel that carries nothing either way for 30 minutes is closed. Connecting to the
  target times out after 10 seconds.
- D9 Logging. The service logs each forward's open and close with its id, port, device, connection count and byte
  counts, and a tunnel's protocol or idle end. Payload is never logged.
- D10 Ports and URLs. Each `host.ports` entry gains `url`: the last `http(s)` URL with that port that its terminal printed
  (a loopback or wildcard host wins over another, so `Local:` wins over `Network:`), else `http://` and `connect_host`;
  `url_printed` says which. `forwarded` is true while an open forward targets the port. A printed URL is terminal
  output and may carry a secret (Jupyter prints `?token=`), so its query and fragment are always dropped, and a caller
  that may not read the terminal gets the guessed URL instead: reading needs `workspace`, plus `host-admin` when the
  terminal holds vault secrets (0071). The sample is shared between callers, so this is applied per call. `host forward` prints the local
  URL as the printed one with its host and port replaced by the local listener.

## Alternatives

- Data in JSON requests (`host.forward_write` with base64): no new client-to-service byte path, but a third more bytes,
  and requests run concurrently on the service, so writes must wait for each answer: one round trip per chunk.
- A new frame kind for client bytes: decoders drop unknown kinds silently (AGENTS.md), and kind `0x02` already names a
  stream; a capability-gated meaning for it on duplex streams needs no new decoder.
- A shared window per connection instead of per tunnel: one slow local reader would stall every forward.
- Allowing non-loopback targets behind an option: it makes the host a relay into its network. Not needed for dev
  servers; a later decision may add it.

## Consequences

- The client's outbound queue carries frames, not only messages, so it can send stream bytes.
- `massorca-net` holds the tunnel (`splice`), shared by the service and the client, so the desktop and the phone can
  carry forwards with the same code.
- `HostPort` gains three optional fields (`url`, `url_printed`, `forwarded`), omitted when empty.
