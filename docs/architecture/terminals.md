# Terminals

How the service runs terminals in Phase 2: shells in pseudo-terminals that
keep running while clients come and go. Each choice has a record in
`docs/decisions/` (0030 to 0035); this page shows how they fit together.
Behaviour to match is `docs/inventory/terminals.md` (TERM-xx).

## Shape

```
 client A (CLI attach)     client B (desktop)        client C (phone)
        |  attach stream          |                        |
        +-------------------------+------------------------+
                                  | terminals/1 + binary-frames/1
 +------------------------ massorca service ---------------------------+
 |  server: terminal.* handlers  ->  TerminalManager (core-terminal)    |
 |      Terminal { id, workspace, state, size, exit }                   |
 |        reader thread: PTY bytes -> screen model + byte ring + fanout |
 |        waiter thread: child exit -> state exited                     |
 |        Pty: ConPTY (Windows) | Unix pty (Linux, macOS)              |
 |        ProcessTree: job object (Windows) | session (Unix)            |
 +---------------------------------------------------------------------+
```

The service owns every terminal. A terminal belongs to one workspace. Any
number of clients attach to it; none of them owns it. A client leaving, by
detaching or by losing its connection, never stops a terminal.

## Crate

`crates/core-terminal` (package `massorca-core-terminal`) has no protocol or
IO beyond the PTY and the child processes:

| Module | Role |
|---|---|
| `pty` | spawn a program in a pseudo-terminal, read, write, resize, wait (0030) |
| `process_tree` | terminate everything the terminal started (0031) |
| `shell` | pick the default shell and build the environment (0032) |
| `screen` | the parsed screen model, scrollback and the attach snapshot (0034) |
| `ring` | the raw output ring with absolute byte offsets (0034) |
| `manager` | `TerminalManager`: create, list, attach, input, resize, kill, close (0033) |

The service maps protocol requests onto `TerminalManager` and does nothing
else; a later separate terminal host process (see 0033) can wrap the same
crate without changing it.

## Lifecycle and states

A terminal reports one of three states, the shared vocabulary for anything
that runs:

| State | Meaning |
|---|---|
| `live` | the process is running, proven by the host that runs it |
| `unverifiable` | the host cannot currently prove either way (a remote host out of contact, Phase 3); never treated as death |
| `exited` | the process is gone and reaped; `exit` says how |

A local terminal is `live` from spawn until its process is reaped, then
`exited` with `exit: { code?, signal?, at_ms, reason }`. `reason` is `exited`
(it ended on its own), `killed` (a `terminal.kill` or `terminal.close`) or
`service_stopped`. A signal death has a `signal` and no `code`. On Windows the
console event that ends a program on a stop (`STATUS_CONTROL_C_EXIT`) reads as
the signal `Interrupt`, so a stop reads the same on every platform. An exited terminal stays listed, with its screen
attachable for reading, until `terminal.close`; at most 64 exited terminals
are kept, the oldest dropped first.

Terminals live in the service's memory. Stopping the service kills them,
and they are not restored on the next start (0033).

## Output, scrollback and attach

Every byte the PTY produces gets an absolute offset: the first byte is
offset 0. The reader thread, under one lock per terminal:

1. feeds the bytes to the screen model (a `vt100` parser with 1000 rows of
   scrollback),
2. appends them to the raw ring (the newest 1 MiB),
3. hands them, with their offset, to each attachment.

An attach is a stream. It starts in one of two ways:

- **resume**: the client sends `since`, the offset it has read up to. If the
  ring still holds that offset, the service sends the bytes from `since`
  onward. Nothing is lost or repeated.
- **snapshot**: otherwise (a first attach, or `since` too old), the service
  sends a redraw of the current state: scrollback rows, the visible screen,
  cursor, colours and input modes, as escape sequences any VT emulator
  accepts. Output then continues from the snapshot's offset.

A client that attaches declares whether it is a terminal emulator that answers
the cursor-position request `ESC [ 6 n` (`answers_queries`, 0036). Only a
declared emulator can hold a query: while one is attached the service waits
for its answer, and otherwise the service answers at once, as it does while
nobody is attached, so a log tail or a script never stalls the shell. The
service answers a held query itself when another client attaches or when every
emulator that received it has detached; a late reply to an answered query is
dropped (0034). ConPTY and PowerShell stall until it is answered.

The CLI's `terminal attach` is the first interactive client, in
`crates/cli/src/terminal_attach.rs`. It declares `answers_queries`: the user's
terminal answers the program's queries and the CLI forwards those replies. On
Unix it forwards raw stdin bytes, so the cursor-position answer reaches the
program: crossterm's Unix reader drops that reply. On Windows raw console reads
carry no arrow keys, so key events are turned back into the bytes a terminal
sends. `terminal screen` replays one snapshot through a `vt100` parser of the
same scrollback size and prints the rows as text; it declares nothing.

Attachments never slow the shell. Each attachment has a bounded queue; one
that falls behind (a slow or stalled client) has its queue dropped and gets a
fresh snapshot when it catches up. Memory per terminal is therefore bounded
by the model, the ring and the queues, whatever the clients do.

## Wire

Capability `terminals/1`. Terminal output uses binary frames, which need
`binary-frames/1` negotiated by both sides (0035). Full types are in
`crates/protocol/src/methods/terminals.rs`.

| Method | Params | Result |
|---|---|---|
| `terminal.create` | `workspace`, `shell?`, `cwd?`, `env?`, `cols?`, `rows?`, `title?`, `request_id?` | `Terminal` |
| `terminal.list` | `workspace?` | `{ terminals }` |
| `terminal.show` | `terminal` | `Terminal` |
| `terminal.attach` | `terminal`, `since?`, `cols?`, `rows?`, `answers_queries?` | stream (below) |
| `terminal.input` | `terminal`, `data` (base64) | `null` |
| `terminal.resize` | `terminal`, `cols`, `rows` | `Terminal` |
| `terminal.kill` | `terminal`, `force?` | `Terminal` |
| `terminal.close` | `terminal` | `Terminal` |

Detaching is the protocol's generic `cancel` of the attach stream; a dropped
connection detaches all of its attachments.

The attach stream carries JSON events and binary items, in order on one
connection:

| Item | Kind | Meaning |
|---|---|---|
| `attached` | JSON event | first item: the `Terminal`, the mode (`resume` or `snapshot`) and the offset output continues from |
| `snapshot` | JSON event | a redraw follows: `offset`, `cols`, `rows`, `len`; the client resets its emulator and feeds the next `len` snapshot bytes |
| snapshot bytes | binary, tag `0x01` | part of the redraw |
| output bytes | binary, tag `0x00` | PTY output starting at the offset in the item |
| `resized` | JSON event | the terminal's size changed (any client may resize) |
| `exited` | JSON event | the process ended; the stream then ends |

A binary item is a frame of kind `0x02`: `stream` (u64, big-endian), then
the stream's payload. A terminal payload is a tag byte, then `offset` (u64,
big-endian), then the bytes. Clients keep `next offset` per terminal: an
output item whose bytes all lie before it is skipped, one that starts past
it is a gap (never expected; the client re-attaches without `since`).

## Security

A terminal runs arbitrary commands as the service's user. `terminal.list`
and `terminal.show` need the `read` permission class; everything else,
attach included (output carries secrets), needs `workspace`. In Phase 2 the
only caller is the local owner, so nothing here widens who can reach the
service.
Terminal input and output are never logged, at any level.

## For later surfaces

- **GPUI desktop**: feeds snapshot and output bytes into its own emulator
  (Zed's `alacritty_terminal`); it needs nothing beyond this stream.
- **Android**: the same stream through the Rust client and UniFFI. A phone
  can attach with a smaller scrollback later through an optional attach
  field; old services ignore it and send the full snapshot.
- **Remote hosts (Phase 3)**: `unverifiable` exists for them from the start.
