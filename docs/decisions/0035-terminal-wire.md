# 0035: Terminal methods, the attach stream and binary frames

Status: accepted, 2026-11-21.

## Context

Clients create terminals, attach to them, type, resize and stop them over the
protocol of 0002. Output is bulk binary data that is not always valid UTF-8.
0002 already says bulk binary data gets its own frame kind behind a
capability instead of base64 inside JSON. Clients reconnect after a network
drop and must neither lose nor repeat output. The phone and the GPUI desktop
come later and must not need a different stream.

## Decision

**Capabilities.** `terminals/1` holds the methods. `binary-frames/1` enables
frame kind `0x02`; the service offers it in `welcome`, and a client lists it
in `hello` to say it can read it. `terminal.attach` is a byte-stream method, so
the dispatcher refuses it with `stream.binary_frames_required` before the
handler runs when the client did not offer `binary-frames/1`. Every client we
ship goes through the Rust `client` crate, so one output path is enough.

**Frame kind `0x02`, stream bytes.** Body: kind byte `0x02`, `stream` as u64
big-endian, then an opaque payload whose meaning belongs to the stream. It is
generic, so a later file transfer can use it. The frame length limit of 0002
applies; terminal payloads are at most 64 KiB plus the header.

**Terminal payload.** A tag byte, `offset` (u64 big-endian), then the bytes.
Tag `0x00` is output starting at `offset`; tag `0x01` is a part of the
snapshot announced by the preceding `snapshot` event (its `offset` is the
snapshot's). Unknown tags are ignored by readers.

**Methods** (types in `crates/protocol/src/methods/terminals.rs`):

| Method | Permission | Notes |
|---|---|---|
| `terminal.create` | `workspace` | `workspace` selector; `shell { program, args }` overrides the default shell (the program the terminal is, never text typed into it); `cwd` relative to the workspace root or absolute; `env` added to the environment; `cols`/`rows` default 120x40; `request_id` makes a retry return the first result |
| `terminal.list` | `read` | optional `workspace` filter; live and exited terminals |
| `terminal.show` | `read` | one terminal |
| `terminal.attach` | `workspace` | stream, result `{ stream }` like every stream; `since` resumes from an offset; `cols`/`rows` resize first |
| `terminal.input` | `workspace` | `data` is base64 of the bytes to write, at most 1 MiB decoded per call |
| `terminal.resize` | `workspace` | the terminal's single size; the last resize wins and every attachment hears `resized` |
| `terminal.kill` | `workspace` | ends the process tree; `force` skips the grace period |
| `terminal.close` | `workspace` | kills if live, then forgets the terminal |

Attach uses `workspace`, not `read`, because terminal output routinely
contains secrets. Detach is the generic `cancel` of the attach stream.

**Records.** `Terminal { id, workspace_id, state, title?, cols, rows, pid?,
shell, cwd, created_at_ms, exit? }` with `state` one of `live`,
`unverifiable`, `exited` (plus the `Unknown` fallback of 0002) and
`exit { code?, signal?, reason, at_ms }`, `reason` one of `exited`, `killed`,
`service_stopped`.

**Attach events** (JSON `event` frames, `data.kind` tagged): `attached
{ terminal, mode: resume | snapshot, offset }`, `snapshot { offset, cols,
rows, len }`, `resized { cols, rows }`, `exited { terminal }`. After `exited`
the service ends the stream.

**Errors.** `terminal.not_found`, `terminal.exited` (input or resize on an
exited terminal), `terminal.spawn_failed` (with the OS error in `message`),
`terminal.input_too_large`, `terminal.invalid_size`, `terminal.limit_reached` (the live-terminal limit of 0033, or the
attachments-per-terminal limit of 0034).

**Evolution.** Everything is additive inside `terminals/1`: new optional
params (a phone's smaller scrollback, a startup command, a shell kind), new
event kinds and new payload tags, all of which old readers ignore. A change
to the meaning of a field gets `terminals/2`.

## Alternatives

- **Base64 output inside JSON events.** One path for every client, but a
  third more bytes and an encode and decode on every chunk of a busy
  terminal, against 0002's stated direction.
- **Output as JSON text (lossy UTF-8).** Breaks on any non-UTF-8 byte and on
  multi-byte characters split across reads.
- **A per-terminal connection** (as the old host's stream channel). Two
  sockets per client and a second handshake; the stream id already separates
  terminals on one connection.
- **Acknowledgement-based flow control** (the old renderer delivery). The
  attach queue plus snapshot recovery (0034) bounds memory without a client
  round trip per chunk.
- **Input as binary frames.** Typing is small; JSON requests keep a
  per-keystroke result and error, and stay readable in a frame log.

## Consequences

- The server and client crates learn one more frame kind, read and written
  next to JSON frames on the same connection, in order.
- A client that cannot read binary frames can still create, list, type into
  and kill terminals; it just cannot attach.
- A browser client later reads the same frames from WebSocket binary
  messages.

## Amendment, 2026-11-28: `terminal.show` names the PATH source (TERM-17)

`terminal.show` returns a `TerminalDetails`: the terminal's fields at the top level, as in the
table above, plus `path_source`, which is `registry` or `inherited` (decision 0032, amendment).
A service from a newer build may send another value, which reads as `unknown`. The key is
optional on the wire, so a client that predates it decodes the terminal alone, because unknown
keys are ignored. `terminal.create`, `terminal.list` and the other methods are unchanged.
