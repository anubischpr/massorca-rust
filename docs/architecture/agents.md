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
  homes/<kind>/           0056 credential home; moved to accounts/ on first access (0059)
  accounts/<kind>/<id>/   credential home per account (0059)
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
  "credentials": { "home_env": "FAKE_AGENT_HOME", "files": ["token.json"], "strip_env": [] },
  "permission_modes": [
    { "id": "careful", "label": "Careful", "args": ["--mode", "careful"] },
    {
      "id": "wild", "label": "Wild", "args": ["--mode", "wild"], "bypass": true,
      "live_switch": { "type": "input", "text": "/mode wild" }
    }
  ],
  "default_permission_mode": "careful"
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
- `credentials`: optional. `files` empty means import is unsupported. `files` are plain
  names (no separator, `..`, drive or absolute path). Every declared file is required by
  import: the declaration has no optional flag.
- `permission_modes` and `default_permission_mode` (decision 0083): optional together. Each
  mode has the vendor's `id` and `label`, `args` that select it at launch (placed after the
  kind's own `args`), an optional deterministic `live_switch` typed into a running session,
  and `bypass` when it lifts the vendor's prompts or sandbox. Ids are unique; the default is
  one of them and never a bypass mode; a bypass mode has its own `args` or `live_switch`; a
  mode with neither can only be the default. The built-in modes and their sources are in
  decision 0083.

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
| `agent.kinds` | read | — | `{ kinds: [{ id, name, icon, owner, installed (deprecated: = detected), detected, package: { state, id?, version? }, program_path?, resumable, status_mechanism, credentials: { supported, imported }, problems, permission_modes?: [{ id, label, description?, bypass, live_switch }], default_permission_mode? }] }` |
| `agent.start` | workspace | `workspace`, `kind`, `prompt?`, `args?`, `env?`, `read_only?`, `title?`, `cols?`, `rows?`, `request_id?` | `AgentSession` |
| `agent.list` | read | `workspace?` | `{ sessions }`, newest first |
| `agent.show` | read | `session` | `AgentSession` |
| `agent.stop` | workspace | `session`, `force?` | `AgentSession` |
| `agent.forget` | workspace | `session` | `AgentSession` (must be exited) |
| `agent.prompt` | workspace | `session`, `text`, `force?` | `{ receipt: "handed_to_terminal", bytes }` |
| `agent.resume` | workspace | `session` | `AgentSession` |
| `agent.hook` | workspace | `session`, `token`, `event`, `payload?` | `null` |
| `agent.watch` | read | `workspace?`, `session?` | stream: `snapshot { sessions }`, then `changed { session }` |
| `agent.delegate` | workspace | `parent`, `prompt`, `kind?`, `name?`, `title?` (default: the first line of `prompt`, at most 80 characters), `read_only?`, `token?`, `request_id?` | `AgentSession` (the child), plus `note?` |
| `agent.tree` | read | `session` | `{ root: AgentSession, children: [AgentSession] }` |
| `agent.bring_back` | workspace | `child`, `merge?` (default true), `prompt?` (default true), `token?` | `{ child, merged, merge_commit?, prompt_delivered, note? }` |
| `agent.credentials.import` | workspace | `kind`, `from`, `replace?` | `{ kind, home, files: [{ name, bytes }] }` |
| `agent.credentials.show` | read | `kind` | `{ kind, home, exists, files: [{ name, present, bytes?, modified_at_ms? }] }` |
| `agent.credentials.remove` | workspace | `kind` | `{ kind, removed }` |
| `agent.accounts.list` | read | `kind` | `{ accounts: [{ id, label, state, identity?, active, projects, origin }] }` |
| `agent.accounts.add` | workspace | `kind`, `label?` | `{ account, terminal_id }`: starts the sign-in |
| `agent.accounts.import` | workspace | `kind`, `from`, `label?` | `{ result: imported \| failed \| not_found, account?, reason? }` |
| `agent.accounts.activate` | workspace | `kind`, `account`, `project?` | the account's summary |
| `agent.accounts.remove` | workspace | `kind`, `account` | `{ kind, account, removed }` |
| `agent.accounts.check` | workspace | `kind`, `account?` | the account's summary, after the check |

Errors: `agent.not_found`, `agent.kind_not_found`, `agent.not_installed`,
`agent.workspace_busy`, `agent.not_live`, `agent.busy`,
`agent.prompt_invalid`, `agent.resume_unavailable`, `agent.hook_refused`,
`agent.delegate_needs_git`, `agent.depth_exceeded`, `agent.parent_busy`,
`agent.parent_dirty`, `agent.merge_conflict`, `agent.already_brought_back`,
`agent.credentials_unsupported`, `agent.credentials_exist`,
`agent.credentials_invalid`, `agent.account_not_found`.

Event bus topic: `agent.changed` (id = session id).

## Run environment

On top of the terminal environment (0032): `MASSORCA_AGENT_SESSION`,
`MASSORCA_AGENT_TOKEN`, `MASSORCA_AGENT_KIND`, `MASSORCA_BIN`, the kind's
home variable when the profile has a home for it (0056), minus the kind's
`strip_env` in that case, then the profile vault's secrets for the workspace's
project (0071), then the caller's `env`. A vault secret named `MASSORCA_*` or
like the home variable fails the launch with `agent.secrets_unavailable`; one
named in `strip_env` is set, not emptied. Only a run whose caller holds HostAdmin gets
vault variables; such a run and its children are marked `holds_secrets`, and
their terminals, prompts, resume and delegation then need HostAdmin (0071).

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
  workspace whose run then fails to start is kept, and the error names it (`data.workspace`). A retry with the same
  `request_id` finds the workspace its first call created, by that request id, and reuses it whatever its name.
- A `request_id` is unique across sessions: a retry returns its first child only to the same parent. One that belongs
  to a call for another session is `request.invalid_params`. The parent and its token are checked before the retry is
  answered.
- `<parent workspace>-<n>` skips a number whose workspace name, branch or directory is already taken.
- Authorization: inside a run (`MASSORCA_AGENT_SESSION` set) the CLI always sends the run's own token, whatever
  session the call names. The service accepts a token only when it is the live run token of the parent: the named
  parent for `agent.delegate`, the child's parent for `agent.bring_back`. Anything else is `agent.hook_refused`, so a
  child cannot delegate as its parent or bring itself back. Residual risk: a caller outside any run sends no token and
  is treated as a person, and an agent that clears its environment can do the same. That gives it nothing new, since it
  can already run any command as the service's user.
- `agent.tree` of a child returns its parent as the root.
- `agent.bring_back` of a session that was not delegated is `request.invalid_params`. A child is finished when its run
  has exited, or its activity is `done` and no prompt was typed since. Bring-backs are serialised. The git work goes
  through a `RepoOps` port that the service implements with `core-git`.
- A merge never runs unlocked. The service takes the parent workspace's git mutation lock (0010) for the whole call; a
  parent workspace that is not a git worktree, is missing or is being removed refuses the bring-back before anything
  runs.
- Just before the merge, under the start gate, the parent workspace is checked again: another live writing session
  there is `agent.workspace_busy` (its id in `data.session`). Under the status lock the parent's activity is read again:
  `working`, or a prompt it has not answered yet, is `agent.parent_busy`. The workspace is then listed as merging until
  the merge ends. Meanwhile a start or resume there, and any prompt to a session there (even with `force`), are `agent.busy`.
- The parent's worktree must be clean and have no rebase, merge, cherry-pick, revert or mailbox apply stopped in it,
  read from git's own state files in that worktree's git directory; otherwise `agent.parent_dirty` says which.
- The merge takes `refs/heads/<child branch>`, so a tag of the same name is never merged instead. A merge that fails
  for any reason other than a conflict (a refusing hook, a missing identity) is aborted too when it left a merge in
  progress, and HEAD is checked to be where it was.
- The child is marked brought back after the merge. If the mark fails after a merge, the error carries the merge commit
  (`data.merge_commit`), so a retry that is refused does not hide that the merge happened.
- The prompt to the parent is one line: control characters in the child's result, line breaks included, become spaces,
  and at most 4 KiB of it is quoted, so it passes the prompt rules (0058).

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
massorca agent delegate [--from <session>] --prompt <text> [--kind <k>] [--name <n>] [--title <t>] [--read-only]
massorca agent tree <session>
massorca agent bring-back <child> [--no-merge] [--no-prompt]
massorca agent credentials import <kind> --from <dir> [--replace]
massorca agent credentials show <kind>
massorca agent credentials remove <kind>
massorca agent accounts list <kind>
massorca agent accounts add <kind> [--label <l>] [--no-attach]
massorca agent accounts import <kind> --from <dir> [--label <l>]
massorca agent accounts activate <kind> <account> [--project <p>]
massorca agent accounts remove <kind> <account>
massorca agent accounts check <kind> [<account>]
```

## Credential import

`core-agents::credentials` (0056). Import reads every declared file before writing
anything: each must be a regular file, checked with `symlink_metadata` and opened without
following a link (`O_NOFOLLOW` on Unix, `FILE_FLAG_OPEN_REPARSE_POINT` on Windows, where
every reparse point counts as a link), and at most 1 MiB as read from the open handle. The
new home is written into a temporary sibling `homes/.<kind>.import-<id>` (`0700`, files
`0600` and created new on Unix; on Windows created with a protected DACL for the owner and
SYSTEM, which the files inherit) and renamed into place after its DACL is checked again.
On Windows `agents` and `homes` are created the same way, and restricted when they already
exist. Homes imported before this fix keep their old access until they are imported again
with `--replace`. On Windows import holds `agents`, `homes` and the new home open without
delete sharing, from before their access changes until the swap is done, so none of them can
be renamed or replaced by a junction meanwhile; the new home is moved through its own handle. Each pinned directory
must be owned by the service's user, its token's default owner (the Administrators group for an
elevated administrator) or SYSTEM, compared as binary SIDs; any other owner refuses the import with
`agent.credentials_invalid`. The directories it creates or restricts (`agents`, run files included)
are owned by the user, elevated or not, with a DACL for the user and SYSTEM, so elevated and
non-elevated runs read each other's. An `agents` or `homes` owned by Administrators, left by an
older elevated run, is refused by a non-elevated run with that reason: run MassOrca elevated once,
which makes it the user's, or take ownership of it. With
`replace` the old home is first renamed aside to `homes/.<kind>.old-<id>`, put back if the
swap fails (the error names where it is when that fails too), and deleted after it succeeds.
Half-built `.import-` leftovers of an interrupted import are deleted by the next import of
that kind; an `.old-` leftover may be the only copy of a login, so it is never deleted: the
next import of the kind is refused with `agent.credentials_invalid` naming it
(`data.leftover`) until the owner moves it. `agents`, `homes` and the home itself must not
be links; show and remove check all three and refuse a link instead of following it.
Imports and removals in one service run one at a time (`credentials::lock_homes`). The
service refuses `import --replace` and `remove` with `agent.busy` while any session of the
kind has a run that has not exited. Declared file names must also not be Windows device
names (`CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9`, `LPT1`-`LPT9`, with any extension) or end
in a dot or a space.

## Accounts (decision 0059)

A kind can have several accounts on one host, and one of them is active. An account is a
credential home at `<profile>/agents/accounts/<kind>/<id>/`, built with the 0056 rules
(`credentials.rs`: owner-only directories, pinning, link refusal, the homes gate). Its row in
the store (`agent_accounts`, with `agent_account_active` and `agent_account_choices`) holds the
label, the identity the check captured, and the state. Ids are canonical lower-case UUIDs.

- **Run resolution** (`accounts::resolve_home`, under `lock_homes`, for start and resume alike):
  the project's choice, else the active account, else no home variable. The home is taken only
  as a plain directory; a link is refused with `agent.credentials_invalid`. A run records its
  account in memory, and `agent.accounts.remove` is refused with `agent.busy` while a live run
  uses it.
- **Sign-in** (`sign_in.rs`): the empty home and its row (origin `sign_in`) are created under the
  gate, and the account is pending until its terminal ends. A service-owned terminal runs the
  kind's `accounts.login` arguments with the home variable set and `strip_env` applied (set to
  empty, as in 0056). The terminal is listed under `agent-accounts`, which is no workspace. A
  terminal still open after 15 minutes (`sign_in::SIGN_IN_TIMEOUT`) is closed with a graceful kill,
  and then decided as an ended one is. The limit is read from `MASSORCA_SIGN_IN_TIMEOUT_MS` when
  the service starts: a test hook, whole milliseconds clamped to 100 ms through the default. When
  the terminal ends, `decide_sign_in` decides the account from the check and its declared files
  (`sign_in::sign_in_outcome`), holding the account's decision mark until the outcome is stored:
  - `signed_in` keeps the account decided, with the check's own identity, and clears its mark;
  - a check that cannot tell keeps the account decided as `unknown`, and clears its mark, while its
    declared files exist. A check cannot tell when the kind has no check, or when it did not finish
    (a timeout, or a program that could not start). The account can then be activated;
  - any other verdict (`expired`, or another exit) deletes the account, and so does a check that
    cannot tell when no declared file is left. Before a sign-in is deleted, the kind's
    `accounts.logout` runs against the home when it is declared (see Removal).
  The startup sweep decides the same way, except that a check that cannot tell keeps the account
  pending, as `unknown`, so the next start decides it again (see Migration and startup). A check of
  an undecided account (its store mark is set) decides it the same way, as a sign-in's end does. An
  undecided account is not the kind's history until a decision keeps it (decision 0059). A row that
  is already gone is left alone, with no event.
- **Pending and removing accounts**: an account whose sign-in is open in this service, or whose
  sign-in or import is being decided, refuses removal, activation and checks with `agent.busy`. An
  account whose store mark (`sign_in_pending`) is still set, which a startup sweep or an interrupted
  import leaves undecided, refuses activation with `agent.busy` too, also after a restart, until a
  check decides it. Removal of such an account is allowed, and runs the sign-out as for any sign-in.
  An account that never signed in refuses activation. Activation holds the homes gate from its checks
  to its write, so a removal that begins meanwhile cannot delete the row under it. Removal marks the
  account as removing under the gate, so no run starts on it while the sign-out runs.
- **Devices below `workspace`**: a paired device that does not hold `workspace` still gets every
  `agent.accounts.*` answer without the account's `identity`, gets `agent.credentials.show` with an
  empty `home` and refusals that name no path, and gets sign-in terminals from `terminal.list` and
  `terminal.show` with an empty `cwd`, since those paths are the account's home. The owner and
  `workspace` devices see all of them.
- **Check** (`account_check.rs`): it runs with the home variable set and the account commands' stripped
  names removed (`credentials.strip_env` and `accounts.strip_env`, decision 0059 amendment 2026-11-22), for at most 10 s. Its whole
  process tree is stopped at the deadline (a Job Object on Windows, a process group on Unix), and
  output is read for at most one second after that, so a child that keeps the pipes open cannot
  hold the gate. At most 64 KiB of output is kept, which is matched and never logged or returned.
  An expired pattern match means `expired`; the signed-in exit means `signed_in`, with the identity
  pattern's first group as the identity; another exit means `needs_sign_in`; a check that does not
  finish means `unknown`. A captured identity is kept only when it is at most 256 characters with no
  control character. A different identity than the recorded one is `conflict`, and the recorded
  identity stays. The declaration has no way to name an offline failure, so a non-zero exit that is
  neither the signed-in exit nor an expired match stays `needs_sign_in`; a kind that cannot tell
  declares no check. An ordinary account keeps what its check finds: the state is recorded and
  nothing else changes. A check of an undecided account (its store mark is set) decides it instead,
  as a sign-in's end does (see Sign-in). A check of an account whose sign-in is open, or being
  decided, is refused with `agent.busy`; a check that decides an undecided account by signing it out
  answers `agent.account_not_found`, since the account is gone.
- **Import** copies the declared files into a new account (origin `import`), which stays undecided
  until its check decides it: a check that reports the login is not signed in deletes the new
  account without a sign-out, and the result is `failed`; any other result keeps it, and only then
  does it count as the kind's history. `not_found` means none of the declared files is in the
  source. An import a crash left undecided is decided at the next start, the same way.
- **Removal**: three steps. Under the gate, a busy account (a live run, an open sign-in, or a removal
  already running) is refused, and the account is marked removing. Outside the gate, the kind's
  `accounts.logout` runs only for an account whose origin is `sign_in`: an imported or migrated login
  is the person's, so the service never signs it out; its files are only deleted. Under the gate
  again, the home and the row are deleted. A sign-in the service discards (see Sign-in) is signed out
  the same way before its home is deleted, since the service made that login.
- **Migration and startup**: `startup_sweep` runs once before requests are served. It deletes a
  `migrated` row whose home is gone only while 0056's home is still there (a crash between recording
  the row and moving the home); moves 0056's home of each kind into an account; adopts an account
  directory that has no row as `migrated` (active only when its kind has no active account); and
  decides each sign-in or import still undecided (see Sign-in and Import): a sign-in whose check cannot
  tell stays pending for the next start. Only a row marked pending (`sign_in_pending`, set when `add`
  or an import creates it, cleared when a decision keeps the account) is undecided. A `needs_sign_in` state on
  an imported or migrated account is only a state: the sweep never discards it. The row is recorded
  before the move, and its active mark too when the kind has none, so a crash after the move leaves
  the kind on that account. A move that fails deletes the row again. The reads `agent.kinds`,
  `agent.accounts.list` and `agent.credentials.show` never move a home; a start or a resume still
  moves it as a fallback.
- **No silent fallback** (decision 0059): a kind that has ever had an account (the history record
  `agent_account_history`, written by a trigger when an account is inserted decided, or when its
  undecided sign-in or import is kept, and kept when the accounts are removed) and has no active
  account refuses to start with `agent.credentials_invalid`, until an account is activated. Only a
  kind that never had an account uses the vendor's usual home. A refused sign-in or import never
  writes the record.
- **Terminals and restarts**: the service owns its terminals. A graceful stop kills every live
  terminal (`TerminalManager::shutdown`, called from `service.rs`), and on Windows each terminal's
  job object kills its processes when the service's handle closes. A restart therefore starts with
  no sign-in terminal running from the previous service on those paths. On Unix, after a crash, a
  login process can outlive the service; the sweep decides a sign-in by its files and check and
  does not wait for that process.
- **Labels**: at most 256 characters with no control character, refused with
  `request.invalid_params`. The CLI prints labels and identities without control characters.
- **Event**: `agent.accounts_changed` is published on the event bus with the kind as its id. The
  bus carries no payload, so a client reads the accounts with `agent.accounts.list`. Every account
  mutation announces itself once: an added account (`needs_sign_in`); a sign-in decision (kept,
  left undecided, or refused and deleted); a check that changes the state; an import (kept, or
  refused and deleted); an activation (the account made active, or chosen by a project); a removal
  (the removed account, with the state it had); and a migration or adoption. A refused sign-in or
  import is announced with the state its check found. An import that creates no account announces
  nothing.
- **Origin**: `origin` is `sign_in`, `import` or `migrated`, from the row's origin column. A value
  this build does not know, or a summary from a peer that omits the field, reads as `unknown`.
- **Declaration**: `accounts` takes `login` (argument list; empty means no sign-in), `check`
  (`args`, `signed_in_exit`, `expired_pattern`, `identity_pattern`), `logout` (arguments) and `strip_env`
  (variable names removed from the check, sign-in and sign-out, in addition to `credentials.strip_env`;
  agent runs keep their environment). They
  go to the resolved program and are never run at declaration time. Patterns are compiled with
  the `regex` crate when the kind loads. The regexes are written by the owner of the profile and run
  on the service, as its programs do; they are not a trust boundary for other users. Only `claude`
  declares accounts among the built-ins; see "Built-in sign-in commands" below.

Not in this part: `agent.accounts.progress` (URL and code patterns), `agent.options`,
`agent.usage`, `agent.models`, and the vault environment provider.

## Built-in sign-in commands

A built-in kind gets an `accounts` block only when every command in it is confirmed from a primary
source (the vendor's documentation, the CLI's own `--help`, or, for Gentle Shell, the owner's
product source). Confirmed on 2026-11-22; the programs were run only with `--help` and `--version`
under an empty temporary home.

| Kind | Program | Home variable | Login | Check | Logout |
| :- | :- | :- | :- | :- | :- |
| `claude` | `claude` | `CLAUDE_CONFIG_DIR` | `claude auth login` (confirmed) | `claude auth status`, exit 0 signed in, 1 not (confirmed) | `claude auth logout` (confirmed) |
| `codex` | `codex` | `CODEX_HOME` (documented for file storage only) | `codex login` exists; not confirmed to honour `CODEX_HOME` | `codex login status` exists; exit code and output not documented | `codex logout` exists; not confirmed to honour `CODEX_HOME` |
| `antigravity` | `agy` | none; credentials are in the OS keyring | no `login` subcommand; sign-in runs when `agy` is first launched | none; no non-interactive status | `/logout` inside the prompt only |
| `gentle-shell` | `gentle-shell` | `PI_CODING_AGENT_DIR` (as declared today) | `/login` slash command inside its terminal UI; no command-line form | none | none |

Sources:

- Claude: `code.claude.com/docs/en/authentication` (read 2026-11-22). Browser sign-in, with a code
  pasted into the terminal when the browser shows one; with `CLAUDE_CONFIG_DIR` set, credentials
  are kept under that directory on Linux and Windows. `code.claude.com/docs/en/cli-reference` (read
  2026-11-22): `claude auth status` exits 0 when logged in and 1 when not; its JSON carries
  `configDirectory` (Claude Code 2.1.268 or later). Local `claude --help` and
  `claude auth {login,status,logout} --help` on 2.1.268.
- Codex: `developers.openai.com/codex/auth`, which redirects to `learn.chatgpt.com/docs/auth` (read
  2026-11-22). Lists `codex login`, `codex login --device-auth`, `codex login status` and
  `codex logout`; `CODEX_HOME` holds `auth.json` in file storage mode; storage modes are file,
  keyring, auto and ephemeral. The docs state no exit code or identity for `login status`, and do
  not say whether the commands honour `CODEX_HOME`. Local `codex --help` and the `login`,
  `login status` and `logout` helps on 0.159.2.
- Antigravity: `antigravity.google/docs/install?tab=cli` (read 2026-11-22). Sign-in starts on first
  launch of `agy`; credentials live in the OS keyring; `/logout` signs out; no status command or
  credential-home variable is documented. Local `agy --help` on 1.5.11 lists no login or logout
  subcommand.
- Gentle Shell: the owner's product source, read in the previous app's repository (read-only). Its
  sign-in table lists a `/login` slash command for the Gentle Shell runtime (internal id `pi`) with
  no command-line form, and the credential variable in the launch specs. The binary is not
  installed on this machine, so it was not run.

Declared accounts for `claude` are `login: ["auth", "login"]`, `check: { args: ["auth", "status"],
signed_in_exit: 0 }` and `logout: ["auth", "logout"]`. No `expired_pattern` or `identity_pattern` is
declared, because neither is documented. Without an `expired_pattern`, an expired login probably
reads as `needs_sign_in`, not `expired`; this is not verified.

`claude` also declares `accounts.strip_env` (decision 0059, amendment 2026-11-22). The check, the
sign-out and the sign-in terminal judge the account's home only, so these variables are removed from
their environment in addition to `credentials.strip_env`. The sign-in terminal can only add variables,
so it sets them to empty. The list is the authentication inputs in Claude's documentation, read
2026-11-22:

- `code.claude.com/docs/en/authentication`, "Authentication precedence" and "Anthropic profiles and
  federation credentials": `ANTHROPIC_AUTH_TOKEN`, `ANTHROPIC_API_KEY`, `CLAUDE_CODE_OAUTH_TOKEN`,
  `ANTHROPIC_PROFILE`, `ANTHROPIC_FEDERATION_RULE_ID`, `ANTHROPIC_ORGANIZATION_ID`, the provider
  selectors `CLAUDE_CODE_USE_BEDROCK`, `CLAUDE_CODE_USE_VERTEX` and `CLAUDE_CODE_USE_FOUNDRY`, and
  `ANTHROPIC_IDENTITY_TOKEN_FILE`, which the page names as a Workload Identity Federation variable.
  The page sends the WIF variables to the WIF reference, which was not read.
- `code.claude.com/docs/en/env-vars`, rows whose description says they supply or select an
  authentication credential: `CLAUDE_CODE_OAUTH_REFRESH_TOKEN`, `ANTHROPIC_WORKSPACE_ID`,
  `ANTHROPIC_FOUNDRY_API_KEY`, `ANTHROPIC_FOUNDRY_AUTH_TOKEN`, `AWS_BEARER_TOKEN_BEDROCK`,
  `ANTHROPIC_AWS_API_KEY`, `CLAUDE_CODE_CLIENT_CERT`, `CLAUDE_CODE_CLIENT_KEY` and
  `CLAUDE_CODE_CLIENT_KEY_PASSPHRASE`.

Excluded: `CLAUDE_CODE_OAUTH_SCOPES` (a scope list, not a credential), `ANTHROPIC_BASE_URL` and the
endpoint variables (they route requests, they do not authenticate), and the `CLAUDE_CODE_SKIP_*_AUTH`
switches. The environment-variable reference page was longer than one fetch: the first fetch covered its
first 100000 characters, and the rest was searched for the variables above, not read in full.

Limits, not verified: an empty value may not count as unset for every variable, so the sign-in terminal
may still see a set but empty variable; and the list covers the variables named in the documents read,
not every variable a future Claude release may read.

## Follow-ups

- Agent sessions (runs) keep the vendor credential variables; whether they strip `accounts.strip_env` is open (decision 0059, amendment 2026-11-22).
- The sign-in terminal sets stripped names to empty because a terminal can only add variables;
  removing them needs environment removal in `core-terminal`.
- A check failure caused by being offline is read as signed out: a kind declaration has no way to
  name it yet.

## Security

An agent runs arbitrary commands as the service's user, like a terminal.
Hook reports need the run's token (0053); delegation and bring-back from
inside a run need it too. Credential files are copied, never read into
logs or output (0056). The prompt text, hook payloads and result text are
never logged.
