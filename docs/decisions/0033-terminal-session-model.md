# 0033: Terminal session model

Status: accepted, 2026-11-21.

## Context

The Phase 2 exit is: open a terminal from the CLI, disconnect, reconnect and
find it alive with its output. In the old app a separate per-user terminal
host process owned the PTYs so that app restarts and updates did not kill
them (TERM-04, TERM-19). Here the core already runs as a long-lived service
that outlives every client (0003). Status vocabulary across the product is
`live` / `unverifiable` / `exited`, and losing contact with a host is never
proof that its work died.

## Decision

- **The service owns terminals, in its own process.** A `TerminalManager`
  (crate `core-terminal`) holds them in memory. Clients never own a terminal;
  closing a client, detaching or losing a connection leaves it running.
- **A terminal belongs to one workspace.** It is created with a workspace
  selector, starts in the workspace root unless `cwd` says otherwise, and is
  listed and filtered by workspace. Removing a workspace kills its terminals
  (TERM-27; wired when workspace removal and terminals meet in the service).
- **Identity.** A UUID v7 string, never reused. A `request_id` on create
  makes a retried create return the first terminal instead of a second one.
- **States.** `live` from spawn until the process is reaped, then `exited`
  with `exit { code?, signal?, reason, at_ms }`. `unverifiable` is reserved for
  hosts the service cannot currently reach (remote, Phase 3); a local
  terminal reports it only when its wait fails, because the service is the
  parent of the process and its wait is proof. A failed wait is reported as
  `unverifiable`, never as `exited`: the terminal stays listed, counted as
  live, attachable, and accepts input, resize and kill, until a later wait
  reaps it.
- **Exited terminals stay listed** until `terminal.close`, so a client that
  reconnects sees how a terminal ended and can still attach to read its last
  screen. At most 64 exited terminals are kept; the oldest are forgotten
  first.
- **Several clients per terminal.** Every attachment receives all output;
  input from any attachment is written in arrival order; the terminal has one
  size and the last resize wins. Nobody is evicted (the old host's
  "new attach detaches the earlier one" rule is dropped: the CLI and the
  desktop can watch the same terminal).
- **Kill.** `terminal.kill` ends the whole process tree (0031): a graceful
  request first, then forced after 5 s, or forced at once with `force`.
  `terminal.close` kills if needed and forgets the terminal.
- **Service stop ends terminals.** `service.shutdown`, Ctrl+C or SIGTERM kill
  every terminal (graceful with a 2 s grace, so it fits the service's 5 s
  shutdown budget) and
  nothing is restored on the next start. If the service process dies
  abruptly, the operating system ends the trees: the job object closes on
  Windows, the PTY hangs up on Unix (0031).
- **Limits.** At most 256 live terminals per service; size clamped to 2..=1000
  columns and 1..=500 rows; input at most 1 MiB per call.

## Alternatives

- **A separate terminal host process** (the old design). Terminals would
  survive a service restart or update, at the cost of a second process
  lifecycle, a second endpoint, identity checks and adoption rules
  (TERM-04, TERM-19). The service already outlives clients, which is what
  Phase 2 needs. `core-terminal` has no dependency on the service, so the
  same crate can move behind a host process later if service updates must
  keep terminals.
- **Terminals owned by the client that created them.** Breaks the phase exit
  and the headless server.
- **Persisting terminal rows in SQLite now.** Without cold restore there is
  nothing to do with them after a restart except show dead entries.

## Consequences

- Restarting the service (an update, `service restart`) ends every terminal.
  `service stop` and `service restart` should say how many terminals they end
  once the CLI has terminal commands.
- Memory is bounded per terminal (0034) and by the live-terminal limit.
