# Agents

How the service runs coding agents in workspaces (Phase 4, milestone M3).
Each choice has a record in `docs/decisions/` (0050 to 0057); this page shows
how they fit together and is the reference for the wire. Behaviour to match
is `docs/inventory/agents.md` (AG-xx) and `agent-status.md` (STAT-xx).

## Shape

```
 massorca agent ... (CLI)        desktop / phone (later)
        |  agents/1 (+ terminals/1 for attach)
 +---------------------------- massorca service ------------------------------+
 |  agent.* handlers -> Agents (core-agents)                                  |
 |     kinds: built-in JSON + <profile>/agents/kinds/*.json  (0050)           |
 |     launch plan: program, args, env, hook files  -> TerminalManager (0051)  |
 |     status store: execution (from terminal) + activity (from hooks) (0052) |
 |     delegation: child workspace via Workspaces, merge via core-git (0055)  |
 |  core-store: agent_sessions                                                |
 +----------------------------------------------------------------------------+
        ^  agent.hook (local socket)
        |
  agent CLI in its terminal --(hook)--> massorca agent hook <event> (0053)
```

## Profile files

```
<profile>/agents/
  kinds/*.json            declared kinds (owner `user`)
  homes/<kind>/           credential home per kind (0056)
  runs/<session>/         generated hook settings for a run (0053)
```

## Kind declaration

```json
{
  "id": "fake",
  "name": "Fake agent",
  "icon": "bot",
  "program": ["C:/path/to/massorca-fake-agent.exe"],
  "args": [],
  "prompt": { "mode": "argv", "separator": false, "paste": "plain", "submit": "\r" },
  "session_ref": { "assign": ["--session-id", "{ref}"], "learn": "session_id" },
  "resume": ["--resume", "{ref}"],
  "status": {
    "mechanism": "env-command",
    "events": {
      "start": "idle", "prompt": "working", "ask": "waiting",
      "stop": "done", "fail": "done:failure"
    },
    "result_field": "result"
  },
  "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"], "strip_env": [] }
}
```

- `prompt.mode`: `argv` | `flag` (with `flag`) | `none`. `separator` puts `--`
  before the text. `paste`: `plain` | `bracketed`.
- `session_ref.assign`: args added at the first launch with a generated
  UUID as `{ref}`. `learn`: payload field that carries the agent's own id.
- `resume`: args for a new run of an exited session; absent means not
  resumable.
- `status.mechanism`: `claude-settings` | `codex-notify` | `env-command` |
  `none`. `events` values: `working`, `waiting`, `idle`, `done` (outcome
  `success`), `done:<outcome>`, `ignore`.
- `credentials`: optional. `files` empty means import is unsupported.

## Session record (`AgentSession`)

| Field | Meaning |
|---|---|
| `id` | UUID v7 |
| `kind`, `workspace_id`, `title?`, `read_only` | as started |
| `parent_id?`, `brought_back_at_ms?` | delegation (0055) |
| `agent_ref?` | the agent's own conversation id |
| `terminal_id?` | the current or last run's terminal |
| `created_at_ms` | |
| `execution` | `{ state: live|unverifiable|exited, exit?: { code?, signal?, reason, at_ms } }` |
| `activity?` | `{ state: working|waiting|idle|done, outcome?, source: hook|process|output, at_ms }` |
| `result?` | the last turn's result text (at most 16 KiB) |

## Wire

Capability `agents/1`. Types in `crates/protocol/src/methods/agents.rs`.

| Method | Permission | Params | Result |
|---|---|---|---|
| `agent.kinds` | read | — | `{ kinds: [{ id, name, icon, owner, installed, program_path?, resumable, status_mechanism, credentials: { supported, imported }, problems }] }` |
| `agent.start` | workspace | `workspace`, `kind`, `prompt?`, `args?`, `env?`, `read_only?`, `title?`, `cols?`, `rows?`, `request_id?` | `AgentSession` |
| `agent.list` | read | `workspace?` | `{ sessions }`, newest first |
| `agent.show` | read | `session` | `AgentSession` |
| `agent.stop` | workspace | `session`, `force?` | `AgentSession` |
| `agent.forget` | workspace | `session` | `AgentSession` (must be exited) |
| `agent.prompt` | workspace | `session`, `text`, `force?` | `{ receipt: "handed_to_terminal", bytes }` |
| `agent.resume` | workspace | `session` | `AgentSession` |
| `agent.hook` | workspace | `session`, `token`, `event`, `payload?` | `null` |
| `agent.watch` | read | `workspace?`, `session?` | stream: `snapshot { sessions }`, then `changed { session }` |
| `agent.delegate` | workspace | `parent`, `prompt`, `kind?`, `name?`, `read_only?`, `token?`, `request_id?` | `AgentSession` (the child) |
| `agent.tree` | read | `session` | `{ root: AgentSession, children: [AgentSession] }` |
| `agent.bring_back` | workspace | `child`, `merge?` (default true), `prompt?` (default true), `token?` | `{ child, merged, merge_commit?, prompt_delivered, note? }` |
| `agent.credentials.import` | workspace | `kind`, `from`, `replace?` | `{ kind, home, files: [{ name, bytes }] }` |
| `agent.credentials.show` | read | `kind` | `{ kind, home, exists, files: [{ name, present, bytes?, modified_at_ms? }] }` |
| `agent.credentials.remove` | workspace | `kind` | `{ kind, removed }` |

Errors: `agent.not_found`, `agent.kind_not_found`, `agent.not_installed`,
`agent.workspace_busy`, `agent.not_live`, `agent.busy`,
`agent.prompt_invalid`, `agent.resume_unavailable`, `agent.hook_refused`,
`agent.delegate_needs_git`, `agent.depth_exceeded`, `agent.parent_busy`,
`agent.parent_dirty`, `agent.merge_conflict`, `agent.already_brought_back`,
`agent.credentials_unsupported`, `agent.credentials_exist`,
`agent.credentials_invalid`.

Event bus topic: `agent.changed` (id = session id).

## Run environment

On top of the terminal environment (0032): `MASSORCA_AGENT_SESSION`,
`MASSORCA_AGENT_TOKEN`, `MASSORCA_AGENT_KIND`, `MASSORCA_BIN`, the kind's
home variable when the profile has a home for it (0056), minus the kind's
`strip_env` in that case, then the caller's `env`.

Limitation: a terminal's environment (0032) can add variables but not remove
inherited ones, and `core-terminal` is not changed in this phase. Until it can,
a stripped variable is set to the empty string instead of removed. Agents treat
an empty key as missing in practice, but a program that checks only for the
variable's presence still sees it. Removing it belongs to the terminals chain.

Session selectors (`agent.show`, `stop`, `forget`, and the CLI) take the full
id or a unique id prefix of at least 8 characters, resolved by the service; a
prefix shared by several sessions is refused with `selector.ambiguous`. UUID v7
ids start with their creation time, so sessions started close together need a
longer prefix.

`agent.forget` of a session whose run is live is refused with `agent.busy`. Until
`agent.prompt` arrives, a first prompt for a kind whose `prompt.mode` is `none` is
refused with `agent.prompt_invalid` instead of waiting for `idle`.

## Status store

`core-agents` applies every status change under one lock: a start, a hook report, a
run's exit and a forget. Each change is written to the session row (activity, outcome,
source, time, result and a learned `agent_ref` in one update) and then announced, in
that order, to the service, which publishes `agent.changed` on the event bus and an item
to every `agent.watch` stream. The live runs and their hook tokens are held in memory
only; the activity itself is read from the row, so a restart shows the last known value.

- `agent.hook` checks the token of the session's live run, then maps the event through
  the kind's `status.events`. A `done` report takes `result_field` from a JSON payload
  (truncated to 16 KiB on a character boundary; a `done` without it clears the result);
  any accepted report sets `agent_ref` from the `session_ref.learn` field when none is
  set. A payload over 64 KiB is refused with `request.invalid_params`.
- Exits: a sweep checks the terminals of live runs every 200 ms; `agent.stop` applies
  the exit it gets back from the terminal at once. A run that exits while `working` or
  `waiting` becomes `done` / `interrupted` with source `process`, also for runs found
  still live in the store when the service starts.
- `agent.watch` subscribes before it reads the snapshot. A watcher that falls more than
  1024 changes behind gets a fresh `snapshot`; one whose connection reads too slowly
  ends with `stream.overflow` on the client, like any stream. A forgotten session sends
  no `changed` item.

A run whose terminal is gone without an exit the service saw (closed with
`terminal close`, or dropped from the terminal manager's list of exited
terminals) is recorded as `exited` with reason `exited` and no code, at the time
it was noticed.

## CLI

```
massorca agent kinds
massorca agent start <kind> --workspace <ws> [--prompt <text>] [--read-only] [--title <t>] [-- <args>...]
massorca agent list [--workspace <ws>]
massorca agent status [<session>] [--watch] [--until <working|waiting|idle|done|exited>] [--timeout <secs>]
massorca agent stop <session> [--force]
massorca agent forget <session>
massorca agent attach <session>
massorca agent prompt <session> <text> [--force]
massorca agent resume <session>
massorca agent hook <event> [<payload>]
massorca agent delegate [--from <session>] --prompt <text> [--kind <k>] [--name <n>] [--read-only]
massorca agent tree <session>
massorca agent bring-back <child> [--no-merge] [--no-prompt]
massorca agent credentials import <kind> --from <dir> [--replace]
massorca agent credentials show <kind>
massorca agent credentials remove <kind>
```

## Security

An agent runs arbitrary commands as the service's user, like a terminal.
Hook reports need the run's token (0053); delegation and bring-back from
inside a run need it too. Credential files are copied, never read into
logs or output (0056). The prompt text, hook payloads and result text are
never logged.
