# 0087: Terminal wait

Status: accepted, 2026-11-23.

## Context

Inventory row CLI-11 (`docs/inventory/cli.md`) asks for `terminal wait`: a script or an agent starts a command in a
terminal and must block until it exits, without polling `terminal show`. The service keeps exit records (0031, 0033) but
offers no call that waits for one. The reference app has `terminal wait --for exit|tui-idle` (read-only, commit
d3c6311a531, `src/cli/handlers/terminal.ts`).

## Decision

- `terminal.wait` is a new method under the capability `terminal-wait/1`. Params: `terminal`, `condition`, optional
  `timeout_ms`. Result: `terminal`, `condition`, `satisfied`, `status`, optional `exit`. It adds no stream and no
  opcode. The CLI refuses a service that does not offer the capability and names it.
- The only condition is `exit`. The reference's `tui-idle` decides readiness from what an agent CLI paints, which needs
  rules checked against captured transcripts. It is left out until those exist. An unknown condition is refused with
  `request.invalid_params`.
- One call waits at most 60 seconds (`MAX_WAIT_MS`). When the cap passes first the result is `satisfied: false`, with
  the state `live` or `unverifiable`. The CLI sends the wait again until its own `--timeout` (default 300 seconds) ends.
- An exited terminal answers at once with its exit record. A terminal closed while a wait is on it answers
  `terminal.gone`: `close` marks the terminal before it kills it, so the kill is not reported as an exit.
- An unverifiable terminal never satisfies a wait. A lost connection during a CLI wait is `terminal.unverifiable`
  (exit 4), never `exited`.
- At most 64 waits run at once; a further wait is refused with `terminal.limit_reached`. Each wait holds one thread of
  the blocking pool for its duration.
- The CLI exits 0 once the condition holds and prints the exit code. The process exit status is not the terminal's
  exit code, so `terminal wait && terminal send` works as it does in the reference. A timeout or a closed terminal
  exits 1 with `terminal.wait_timeout` or `terminal.gone`.

## Consequences

- Scripts that need the `tui-idle` condition wait for a later decision, with rules backed by captured transcripts.
- Each waiting client holds a thread for at most a minute per call. The 64-wait cap keeps the rest of the terminal work
  responsive.
