# 0052: One agent status store in the service

Status: accepted, 2026-11-21.

## Context

The old app kept status per pane where the agent ran: in the desktop for
local panes, in a relay for SSH panes (`docs/inventory/agent-status.md`,
STAT-xx). Its vocabulary was `working`, `blocked`, `waiting` and `done`, with
`blocked` and `waiting` both meaning "needs the person". The plan's rules
say agent status lives in one store on the execution host that every UI
reads, that the host executing the work owns its status, and that losing
contact is never proof a process died. Terminals already use `live` /
`unverifiable` / `exited` for anything that runs (0033).

## Decision

- **Two separate axes, never merged into one word.**
  - **Execution**: `live` / `unverifiable` / `exited`, exactly the terminal
    vocabulary and meaning, read from the session's current terminal. No
    synonyms (`running`, `dead`, `stopped`) appear on the wire or in the CLI.
    A session that never ran or whose run ended reports `exited` with the
    terminal's `exit` record (`code`, `signal`, `reason`, `at_ms`).
  - **Activity**: what the agent is doing, from its own reports:
    - `working`: a turn is running;
    - `waiting`: the agent stopped to ask the person (a permission or a
      question; the old `blocked` folds in here);
    - `idle`: the agent is ready and no turn has run since it started or
      resumed;
    - `done`: the last turn ended. It carries `outcome`: `success`,
      `failure`, `cancelled` or `interrupted`.
    Activity is absent when nothing has been reported (a kind with no
    status mechanism, or before its first report). Absent is not a fifth
    state and clients show it as "no status".
- **Every activity value carries its source and time**: `source` is `hook`
  (the agent's hooks, 0053), `process` (derived by the service from the
  run's exit) or `output` (structured output, reserved for structured chat
  adapters; not produced in Phase 4); `at_ms` is when the service recorded
  it. Clients can always say why a session shows what it shows.
- **The service derives only what it saw.** When a run exits while its
  activity is `working` or `waiting`, the activity becomes `done` with
  outcome `interrupted`, source `process`. Nothing else is inferred: a
  prompt handed to a terminal does not make a session `working` (0054).
- **One store, many readers.** The store is in the service's memory, keyed
  by session id, with the last activity and result written to the session
  row on each change (so a restart shows the last known activity, marked
  with its time). Readers:
  - `agent.list` / `agent.show` return the current record;
  - `agent.watch` is a stream: first a `snapshot` of the selected sessions,
    then a `changed` item with the full record after every change, in the
    order the store applied them; the subscription is taken before the
    snapshot so nothing falls between;
  - the event bus gets `agent.changed` with the session id, so `events/1`
    subscribers and later automations share the same source.
  A watcher that falls behind is dropped to a fresh `snapshot`, like a slow
  terminal attachment.
- **Exits are observed by the service.** Terminals expose no exit
  notification yet, so the store sweeps the terminals of live sessions every
  200 ms and applies an exit once, in order with hook reports. When the
  terminal crate exposes an exit notification, the sweep goes.
- **Remote execution hosts.** The store lives in the service of the host
  that runs the agent. A client connected to a remote service reads that
  host's store. A client that loses its connection shows its last
  observation as `unverifiable`, never `exited`, until it reconnects and
  takes a fresh snapshot. A hook that cannot reach its service never changes
  execution state; activity simply stays at its last value.
- **No freshness expiry in Phase 4.** Each activity carries `at_ms`; a client
  may dim old values. Expiry rules (the old 30-minute rule) come with
  notifications, where they matter.

## Alternatives

- **One combined state** (`running`, `waiting`, `exited`...). It mixes "is
  the process there" with "what is it doing" and makes `unverifiable`
  ambiguous.
- **Keep `blocked` apart from `waiting`.** Both meant "needs the person" in
  every reader of the old app.
- **Status in each client.** Breaks the rule that the execution host owns
  status, and two clients could disagree.

## Consequences

- The desktop's agent status component (`crates/ui`) maps these values to
  glyphs; "error" and "unreachable" in the design are a client's rendering
  of `done/failure` and of a lost connection, not states of the store.
- A 200 ms sweep runs while any agent is live; it reads in-memory terminal
  state only.
