# 0034: Terminal scrollback and the attach snapshot

Status: accepted, 2026-11-21.

## Context

A client that attaches to a running terminal must see what a person sitting
at it would see: the current screen, colours, cursor, input modes and some
history above it (TERM-07). A client that only lost its connection for a
moment should get exactly the bytes it missed, nothing more (TERM-05,
TERM-06). The old app kept a parsed model per terminal with a 1000-row live
window because deeper models for 100 terminals exhausted memory, and sent a
rendered snapshot on reattach rather than a raw replay.

## Decision

- **Both a model and a ring.** Each terminal keeps:
  - a parsed screen model, the `vt100` crate (0.18), sized like the PTY,
    with **1000 rows** of scrollback;
  - a raw byte ring of the **newest 1 MiB** of output, indexed by absolute
    byte offset (the first byte the PTY ever produced is offset 0, kept as a
    `u64`).
- **Resume when possible, snapshot otherwise.** An attach that names an
  offset still in the ring gets the bytes from there (exact, no duplicates).
  Any other attach gets a snapshot, then output from the snapshot's offset.
- **The snapshot** is a byte stream of escape sequences, so every client
  feeds it to its own emulator: a full reset, the scrollback rows (oldest
  first), the visible screen with attributes (`vt100`'s formatted output),
  each grid's scroll region, origin mode and saved cursor, the character set and
  autowrap, then cursor position and visibility and the input modes (application
  cursor and keypad, bracketed paste, mouse mode and encoding, alternate
  screen). The title, which `vt100` does not keep, is tracked from OSC 0 and 2
  by the model's callbacks and sent in the `Terminal` record, not inside the
  bytes. A full-screen program in the alternate screen redraws itself after
  the client's resize, as in the old app.
- **Atomicity.** The reader thread applies one chunk to the model, the ring
  and every attachment under one lock, so a snapshot or a ring read taken
  under that lock sits exactly between two chunks. The offset in the snapshot
  is therefore exact. A resize takes the same lock to change the model and the
  attachments before it resizes the PTY, so output drawn at the new size is
  parsed at the new size and queued after its `Resized`.
- **Attachments never slow the shell.** Each attachment queues at most
  **2 MiB** of payload (the 1 MiB a resume can carry from the ring plus 1 MiB
  of live output; resume, snapshot and output bytes all count). When the
  queue is full the attachment is marked lagging, its queue is dropped, and
  it gets a new snapshot when it next reads, **at most one per 250 ms** for a
  client that keeps falling behind. The PTY reader never blocks on a client.
  A terminal accepts at most **16 attachments**; a further attach fails with
  `terminal.limit_reached`.
- **Cursor-position queries.** While no client is attached, the service
  answers `ESC [ 6 n` from the model's cursor. ConPTY sends that query when a
  terminal starts and renders nothing until it is answered; an attached
  client's emulator answers it instead (TERM-15).
- **No disk history in Phase 2.** Nothing is written to disk: no checkpoints,
  no output log, no cold restore (TERM-08, TERM-09 are later). Terminal
  output is never logged.

## Alternatives

- **`alacritty_terminal` as the model.** The same engine as the GPUI view
  (Zed pins it), so a snapshot would be read by the engine that wrote it. It
  has no serializer to escape sequences; we would write and maintain one
  covering every cell attribute and mode. The snapshot is an escape stream
  either way, so parity between the service's model and a client's emulator
  matters little. Revisit if `vt100`'s output proves lossy.
- **`avt`.** Has `dump()`, but it explicitly omits scrollback.
- **Raw replay only.** Simple, but a screen built from an arbitrary tail of
  bytes is wrong whenever the tail starts mid-sequence or after the screen
  was drawn, and full-screen programs cannot be replayed from a tail.
- **Model only, no ring.** Every reconnect would repaint the whole screen
  and history, and a client could not tell what it had already shown.
- **Pause the shell for slow clients** (the old app's flow control). Keeps
  every byte for a slow reader, but one stalled phone would freeze a build in
  a terminal the desktop is watching. A snapshot is a correct recovery.

## Consequences

- Memory per terminal is bounded: the model (1000 rows plus the screen), 1 MiB
  of ring and up to 16 attachment queues of 2 MiB each, plus the snapshot an
  attach starts with.
- A snapshot is still serialized under the terminal lock, so building one
  pauses that terminal's reader for its duration; the 250 ms pacing bounds how
  often a lagging client can cause that.
- A client offline for more than 1 MiB of output gets a snapshot instead of
  the missed bytes; history older than 1000 rows is gone.
- Durable history and cold restore can be added later from the same reader
  without changing the wire: they only change where a snapshot comes from.
