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
| `agent.delegate` | workspace | `parent`, `prompt`, `kind?`, `name?`, `read_only?`, `token?`, `request_id?` | `AgentSession` (the child), plus `note?` |
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

`agent.forget` of a session whose run is live is refused with `agent.busy`. A first prompt for
a kind whose `prompt.mode` is `none` waits for the run's first `idle`; see Prompts.

## Prompts

`agent.prompt` (0054) types text into a live run's terminal as one terminal input: the text,
then the kind's submit sequence. Plain text is typed as keystrokes, each line break as Enter
(`\r`), the same as the desktop's paste. Bracketed text sits between the paste markers and keeps
its line breaks. The text is at most 8 KiB and holds no control character except tab, line feed
and carriage return (decision 0058): NUL, the escape character, DEL, the other C0 controls and the
C1 controls (U+0080 to U+009F) are refused with `agent.prompt_invalid`, at launch and in
`agent.prompt`, in both paste modes. Because the escape character is refused, no paste marker can
appear inside the text. A run that has exited is `agent.not_live`.

A run takes a prompt when its activity is `idle` or `done`, or when its kind has no status
mechanism. Absent activity, meaning nothing reported yet, is not ready; `working`, `waiting`
and unknown states are not either. A run that has taken a prompt since its last hook report is not
ready either (decision 0058): the flag is set when a prompt is accepted or a held first prompt is
taken, and the next accepted hook report clears it, so two prompts never land on the same idle. A
kind with no status mechanism never sets it, since nothing reports back, so it stays always ready.
Otherwise the call is `agent.busy`, unless `force` is set.
The readiness check and the copy of the kind's input rules happen under the status lock. The
write happens after the lock is released: a write can wait for the agent to read, and the
agent's own hook reports need the lock to return.

The receipt is `{ receipt: "handed_to_terminal", bytes }`, where `bytes` counts every byte
written to the terminal, the paste markers and the submit sequence included. It never says the
agent read them; the activity turns `working` only when the agent's hooks report it.

A first prompt for a kind whose `prompt.mode` is `none` waits for the run's first `idle` report
(0054). The prompt is held in memory on the run from `agent.start`; the report that makes the run
`idle` types it on a thread of its own, so the report returns at once. Decision 0054 leaves open
what happens when no `idle` comes, so two bounds apply: the held prompt is dropped when the run
exits first, and when no `idle` arrives within 120 s of the start (`FIRST_PROMPT_WAIT`). A drop is
logged with the session id only. The session record does not show a drop: its activity stays as it
was, normally `no status`. A kind with no status mechanism, or one that maps no event to `idle`,
cannot wait for one, so `agent.start` refuses such a first prompt with `agent.prompt_invalid`
before anything runs. A forced `agent.prompt` is typed at once even while a first prompt waits; the
held prompt still follows at `idle`.

`agent.resume` (0054) starts a new run of an exited session, in the same session: the kind's resume
arguments, with the agent's ref in place of `{ref}`, in a new terminal in the workspace root. The
one-writer rule (0051) applies. The session keeps its id, its ref, its title and its last result; its
exit is cleared, and its activity is absent until the new run reports. The caller's arguments and
environment of the first start are not kept, so a resumed run gets the kind's own arguments and the
resume arguments only. The run is registered under the status lock before its program starts, as in
`agent.start`, so its first hook report is accepted. Refusals: `agent.busy` while the run is live;
`agent.resume_unavailable` when the kind is no longer declared, declares no resume form, or the
session's ref is not known yet (a Codex session learns its ref at a turn's end), or its last run has
no recorded end (no terminal and no exit; decision 0058); `agent.not_installed`
when no program of the kind resolves; `agent.workspace_busy` when another session writes in the
workspace.

## Delegation

`agent.delegate` and `agent.bring_back` follow decision 0055. Details the record leaves open:

- The result of `agent.delegate` is the child's `AgentSession` with an optional `note` beside its fields
  (`AgentDelegated`), so a reader that expects a session reads it unchanged. A writing child's note says that the
  parent's uncommitted changes were not carried over.
- The kind is checked before the child workspace is created, so an unknown or missing kind leaves no workspace. A child
  workspace whose run then fails to start is kept; a retry with the same `request_id` reuses it.
- `<parent workspace>-<n>` skips a number whose workspace name, branch or directory is already taken.
- A token that does not match the parent's live run is refused with `agent.hook_refused`, in `agent.delegate` and
  `agent.bring_back`.
- `agent.tree` of a child returns its parent as the root.
- `agent.bring_back` of a session that was not delegated is `request.invalid_params`. A child is finished when its run
  has exited, or its activity is `done` and no prompt was typed since. Bring-backs are serialised, and the parent
  workspace's git mutation lock (0010) is held for the whole call. The git work goes through a `RepoOps` port that the
  service implements with `core-git`. The prompt to the parent is one line: control characters in the child's result,
  line breaks included, become spaces, and at most 4 KiB of it is quoted, so it passes the prompt rules (0058).

## Status store

`core-agents` applies every status change under one lock: a start, a hook report, a
run's exit and a forget. Each change is written to the session row (activity, outcome,
source, time, result and a learned `agent_ref` in one update) and then announced, in
that order, to the service, which publishes `agent.changed` on the event bus and an item
to every `agent.watch` stream. The live runs and their hook tokens are held in memory
only; the activity itself is read from the row, so a restart shows the last known value.
A row read before the lock is read again under it, so an exit never turns a `done`
report that landed in between into `interrupted`. `agent.start` registers the run and
holds the lock from before its program starts until its row exists, so the run's first
report waits instead of being refused. The exit sweep checks each live run on its own and
logs one that fails, by session id and error code, without stopping.

- `agent.hook` checks the token of the session's live run, then maps the event through
  the kind's `status.events`. A `done` report takes `result_field` from a JSON payload
  (truncated to 16 KiB on a character boundary; a `done` without it clears the result);
  any accepted report sets `agent_ref` from the `session_ref.learn` field when none is
  set. A payload over 64 KiB is refused with `request.invalid_params`.
- Exits: a sweep checks the terminals of live runs every 200 ms; `agent.stop` applies
  the exit it gets back from the terminal at once. A run that exits while `working` or
  `waiting` becomes `done` / `interrupted` with source `process`, also for runs found
  still live in the store when the service starts. An exit is recorded only while the row still
  names the terminal the run ran in (decision 0058): an end that arrives after a resume moved the
  session onto a new terminal changes nothing, and does not end the resumed run.
- A terminal is closed after the status lock is released: when a start or a resume fails after its
  run was registered, and in `agent.forget`. A close can wait for the terminal's kill grace, and
  the status of the other sessions must not wait with it.
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
