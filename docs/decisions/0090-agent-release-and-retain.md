# 0090: Agent release and retain

Status: accepted, 2026-11-24.

## Context

An exited run keeps its terminal, and the output that terminal holds, until the terminal manager forgets it. The manager
keeps at most `max_exited` (64) exited terminals and forgets the oldest first. Nothing lets a person keep the output of a
finished run and free its terminal on purpose. `agent forget` deletes the record with its output (0051), which is too
much when the output is still wanted.

## Reference

The reference is the pv96 desktop, read-only at commit `d3c6311a531`. Its release and retain are per supervised worker
of the orchestration layer, not per agent session. The source files are:

- `src/cli/specs/orchestration-worker-specs.ts:96-120`: `worker-release` closes the terminal of a settled worker and keeps
  an output archive readable. `worker-retain` records a durable user exception, which a later explicit release clears.
- `src/main/runtime/orchestration/worker-terminal-ownership.ts:85-92`: only a `succeeded` or `failed` worker is
  releasable. `worker-terminal-release.ts:43-50` refuses any other unsettled worker.
- `src/main/runtime/orchestration/worker-output-archive.ts:22-23, 189-217`: the terminal archive is bounded to
  `TERMINAL_ARCHIVE_MAX_CHARS = 262_144` characters, newest kept. `:82-84, 147-155`: a capture that fails keeps the
  terminal (`archive_failed`).
- `worker-terminal-release.ts:72-85`: an explicit release of a retained worker clears the retain and releases it.
- `worker-terminal-ownership.ts:113-122`: a retained worker is listed as `retained`, not `reclaimable`.

## Decision

- `agent release <SESSION>` on an exited run archives the newest 256 KiB of the terminal's text, closes and frees the
  terminal, clears the retain, and keeps the record marked released. The text is the screen's scrollback and visible rows
  without escape codes. A live run is refused with `agent.busy`. A second release changes nothing. When the terminal's
  output cannot be read, the terminal is kept and the release fails with `agent.archive_failed`. A terminal the manager
  no longer has has no output left, so it archives an empty text.
- The archive bound is `ARCHIVE_MAX_BYTES = 262_144` bytes of UTF-8, cut on a character boundary. pv96 keeps 262,144
  characters. The number is kept for parity, and the unit is bytes because that is what the store holds. The newest output
  is the evidence a person reads back, so the head is what goes.
- `agent archive <SESSION>` shows the archive. A run that holds vault secrets shows its text to host-admin callers only,
  as its result is (decision 0071); other callers see `withheld`.
- `agent retain <SESSION> [--off]` sets a durable user flag. Its only effect is that the session's exited terminal is not
  evicted by `max_exited`. The manager counts only unpinned exited terminals, and never forgets a pinned one. Every later
  run's terminal is pinned when it starts. Retain does not block a release, and an explicit release clears it.
- `agent forget` deletes the record, its terminal, its generated files and its archive, as before.
- `agent resume` of a released session starts a new run. The archive of the run before is dropped, and the session is no
  longer released.

## Deviations from pv96

- pv96's retain protects a worker from automatic reclaim. Here no automatic reclaim exists, so retain protects the terminal
  from the eviction that does happen, `max_exited`.
- The archive is the screen text, not a provider transcript. pv96 prefers the transcript. Sessions here have no transcript
  reader, so the terminal is the only source.
- A release of a session whose terminal was already evicted archives an empty text, because pv96's case of a missing
  terminal does not arise there. A retain set after eviction cannot restore the terminal.

## Consequences

- A retained session's exited terminal does not count toward `max_exited`, so the memory it holds is bounded by the user's
  retains, not by the cap. Release or forget frees it.
- Migration step 25 adds `agent_sessions.retained`, `agent_sessions.released_at` and the `agent_session_archives` table.
  Its number is renumbered at push time when `main` has moved.
- Protocol: `agent.release`, `agent.retain` and `agent.archive`, under capability `agent-release/1`; error
  `agent.archive_failed`.
