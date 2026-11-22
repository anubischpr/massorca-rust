# 0053: How agent status is produced

Status: accepted, 2026-11-21.

## Context

The old app took status from hooks of 27 agents, posted to a loopback HTTP
listener with a per-start token, and from parsing terminal titles and
screens for agents without hooks (`docs/inventory/agent-status.md`). It also
accepted an unauthenticated OSC 9999 escape in pane output. Screen parsing
written from memory of how an agent paints breaks silently when the agent's
UI changes.

## Decision

- **Status comes from the agent's own structured signals only**: its hooks,
  or (later) its structured output. MassOrca never infers activity from
  what an agent paints. If a rule that reads terminal output is ever added
  for an agent without hooks, it must be written and tested against a
  captured transcript of that agent's real output stored in the repository
  (with any personal data replaced), never from memory. Escape sequences in
  terminal output are never accepted as status.
- **One hook entry point: `massorca agent hook <event> [payload]`.** Hooks
  run this command; it connects to the service over the profile's local
  socket and calls `agent.hook` with the session id and token from its
  environment, the event name and the payload (the trailing argument if
  given, else stdin when stdin is not a terminal; at most 64 KiB, JSON
  expected but kept opaque). It always exits 0 within its 5 s budget and
  prints nothing on success: a status problem never blocks or breaks the
  agent (hooks fail open). Failures are written to the CLI log.
- **A per-session token.** `agent.start` creates a random 128-bit token,
  stored only in the service's memory and handed to the run in
  `MASSORCA_AGENT_TOKEN`. `agent.hook` with an unknown session or a wrong
  token is refused (`agent.hook_refused`) and changes nothing. The token
  binds a report to the run that received it; a resume gets a new token.
- **The declaration maps events to activity.** Each kind's `status.events`
  table maps hook event names to `working`, `waiting`, `idle`, `done` (with
  an outcome) or `ignore`. Unknown events are ignored. A `done` event may
  carry the turn's result text, taken from a payload field the declaration
  names (the fake agent sends `result`; Claude's field is declared only
  once it is checked against a captured Stop payload, until then a Claude
  turn ends without result text). The agent's own conversation id is learned from a payload
  field the declaration names (`session_id` for Claude, `thread-id` for
  Codex) when it is not assigned at launch.
- **Installing hooks is a mechanism, chosen per kind (0050):**
  - `claude-settings`: the service writes a settings file under
    `<profile>/agents/runs/<session>/` whose `hooks` run
    `MASSORCA_BIN agent hook <Event>` for `SessionStart`,
    `UserPromptSubmit`, `PreToolUse`, `Notification` and `Stop`, and passes
    it with `--settings <file>`. Nothing in the user's own Claude settings is
    changed.
  - `codex-notify`: the service passes
    `-c notify=["MASSORCA_BIN","agent","hook","turn-complete"]`; Codex calls
    it with the JSON payload as the last argument at each turn end, so Codex
    reports `done` only (turn-end authority).
  - `env-command`: nothing is installed; the agent is expected to call
    `MASSORCA_BIN agent hook <event>` itself. The fake agent uses this, as
    can any program written for MassOrca.
  - `none`: no activity; execution state only.
  Generated files hold no credentials; they are deleted with the session.
- **Hooks are installed per run, never globally.** Nothing is written to the
  user's home or to the agent's global config, so removing MassOrca leaves
  no hook behind and two profiles never see each other's hooks.

## Alternatives

- **A loopback HTTP listener** (the old design). A second listener and port
  to secure, when the service already has an authenticated local socket and
  the `massorca` binary is at hand.
- **Screen or title parsing** for agents without hooks. Fragile and
  untestable without captured output; allowed later only under the
  transcript rule above.
- **Hook events as a direct write to a file the service tails.** No
  authentication and no ordering with other changes.

## Consequences

- Each hook event costs one short `massorca` process and one local
  connection; Claude fires several per turn, well within budget.
- An agent whose hook command fails (service stopped, profile mismatch)
  keeps working and simply stops reporting.
- Offline spooling of events (the old app replayed them) is not built; a
  service that is down has no live runs to report about anyway (0051).
