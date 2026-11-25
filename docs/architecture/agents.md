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
 |     kinds: KindPackages + <profile>/agents/kinds/*.json  (0050, 0059)     |
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
  runs/<session>/         generated hook settings for a run (0053); `starting` while a start is unsettled
```

## Kind packages (decision 0059)

The catalog (`catalog.rs`) is read again on every call from two sources, in this order:

1. **Packages.** `AgentsConfig.kind_packages` is an `Arc<dyn KindPackages>` (`packages.rs`). Its
   `kinds()` returns one `PackageKind { kind_id, package_id, version, state, decl }` per kind:
   `state` is the protocol's `PackageState` (`available`, `enabled`, `disabled`), and `decl` is the
   declaration's JSON text, or an error that names the package and the cause, never a path. The
   default, `BuiltInPackages`, returns the four compiled-in declarations as `enabled` packages
   `massorca.<kind>` at the crate version, for `core-agents`'s own tests. The service uses
   `ExtensionKindPackages` (`crates/server/src/kind_packages.rs`), which reads the profile's
   lockfile through `package_agent_kinds` on every load; an unreadable lockfile lists the built-ins
   as `unknown` with a declaration error. A kind from a
   `massorca.*` package has owner `core`; any other package's kind has owner `ext:<package id>`.
   A declaration error, a declaration that does not parse, or one whose id is not the package's
   `kind_id` is listed in `problems`, so the kind is listed and never started.
2. **Profile files.** `<profile>/agents/kinds/*.json`, owner `user`, with no package: they count as
   enabled. A package's kind always wins over a file that reuses its id, as a built-in did.

`agent.kinds` reports each kind's `package: { state, id, version }`; a profile file's has no id or
version.

**Gating.** A kind whose package is not `enabled` is refused with `agent.kind_not_enabled`, data
`{ kind, package, state }`, and a message that says to install or enable it in the Marketplace.
A package whose state cannot be read (state `unknown`, as when the extensions lockfile cannot be
read) is refused with the same code, and its message names that read failure instead, since
installing or enabling cannot fix it. A package whose files change under the read because it is
being updated is read again from the new lockfile; one whose lockfile keeps changing is listed with
the problem that it is being updated (`extension.busy`), not that its files were tampered with.
The service reuses its last read of the packages while the lockfile's bytes are unchanged, since a
read hashes every installed package's files; any lockfile write reads them again, and a read that
found an unusable package is not kept. Files changed without a lockfile write are seen at the next
lockfile change or service start.

**Removal guard.** The service registers the agents' remove guard on the extensions at start
(decision 0062): disabling, removing, or updating a package to a version that drops one of its
kinds is refused with `agent.busy` while a session of one of the package's kinds is live or
resumable (an exited run with a recorded end and a known ref). The message and its data
`{ package, kinds, sessions }` name those sessions; `agent.forget` frees one.
The check runs right after the kind is found and before its declaration's problems and its
program are checked, on `agent.start`, `agent.delegate` (and every other start through
`Agents::start`), `agent.resume`, and `agent.accounts.add`, `import` and `activate`.
`agent.accounts.check`, `agent.accounts.remove`, and the vendor sign-out on removal or on a
discarded sign-in keep working on a disabled kind.

**Removal cleanup** (decision 0098). Next to the guard, the service registers the agents' remove
cleanup, which runs only when a removal asks for `delete_agent_homes` (`extension remove
--delete-agent-homes`). The cleanup runs after the package's files are deleted, so the extensions
read the kinds the package provided from its manifest before deleting them, and pass them to the
cleanup with the package id (`RemovedPackage`). A manifest that cannot be read gives no kinds, and
the removal reports that their accounts were left; an unvouched first-party id gives none.
`Agents::remove_package_accounts` then, for each of those kinds that no other installed package
still provides, deletes under the homes gate every account row of the kind (with it, its active mark
and project choices), its directory under `<profile>/agents/accounts/<kind>/`, any account directory
left without a row, the hidden folders an interrupted import or replace left there, the
`accounts/<kind>/` folder itself, and the kind's 0056 home with its hidden siblings. The deletions go
through the account-home helpers, which refuse a link instead of following it. A package outside
`massorca.*` never cleans a kind the built-in catalogue declares: that kind is skipped and reported.
An account busy with a sign-in, a decision, a removal or a live run is left. What was not deleted is
the cleanup's failure in the removal result, with the code of its first problem. No sign-out runs:
an account a MassOrca sign-in made keeps its login valid at the vendor until the person signs out
there.

**Used kinds.** `used_kinds(&Store, profile_dir) -> Result<BTreeSet<String>, AgentError>` (public from core-agents)
is every kind the profile has used, for the extension runtime's removal rules: the kinds of
`agent_sessions`, `agent_accounts` (pending sign-ins included), `agent_account_history` and
`bots` (the store query `used_agent_kinds`), and the directories under
`<profile>/agents/accounts/` and `<profile>/agents/homes/` that are real directories, never links,
named by a valid kind id. A store that cannot be read, or a kind directory that exists but cannot be
listed, is an error (`internal`), so the caller retries instead of treating a used kind as unused; a
directory that does not exist has no kinds.

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
  resumable. `resume_transcript` (`{ env?, dir, glob }`) says where the CLI saves one
  conversation per ref, and `resume_not_found_pattern` is a regular expression over a failed
  resumed run's output that means the CLI found none; both need `resume` (decision 0160).
- `strip_inherited_env`: variable names, each once, that no run of the kind and none of its
  account commands may inherit from the service (decision 0160).
- `instructions`: optional, `{ "args": [...] }` with `{text}` exactly once and no other
  placeholder (decision 0161). `supported_when: { args, contains }` declares a support probe:
  the kind's program run with `args`, without an account home and with the names an account
  command strips removed, for at most 5 s; the channel is used only when it exits 0 and its
  output contains `contains`. The result is kept per program path and modified time, so the
  probe runs once per installed CLI in a service. A run whose CLI fails the probe starts without
  the text, and its session's `notes` say so. A run gets MassOrca's delegation instructions through these
  arguments, after the hook arguments and before the session ref, the caller's arguments and
  the prompt, on a first run and on a resume, while `agents.delegation_depth` leaves room for
  a child below its session. The built-in `claude` declares `["--append-system-prompt",
  "{text}"]` with the probe `--help` containing `--append-system-prompt`; the other built-in kinds declare none and get no text.
- `status.mechanism`: `claude-settings` | `codex-notify` | `env-command` |
  `none`. `events` values: `working`, `waiting`, `idle`, `done` (outcome
  `success`), `done:<outcome>`, `ignore`.
- `credentials`: optional. `files` empty means import is unsupported. `files` are plain
  names (no separator, `..`, drive or absolute path). Every declared file is required by
  import unless `optional` lists it (decision 0097). `state` declares how the CLI's own state
  file is built and repaired (decision 0099).
- `permission_modes` and `default_permission_mode` (decision 0083): optional together. Each
  mode has the vendor's `id` and `label`, `args` that select it at launch (placed after the
  kind's own `args`), an optional deterministic `live_switch` typed into a running session,
  and `bypass` when it lifts the vendor's prompts or sandbox. Ids are unique; the default is
  one of them and never a bypass mode; a bypass mode has its own `args` or `live_switch`; a
  mode with neither can only be the default. `may_fall_back` marks a mode the vendor may
  start another mode in without failing. `default_permission_mode` names the mode the vendor
  CLI starts in by itself; it is informational, may differ by CLI version, and is never shown
  as a session's mode. The built-in modes and their sources are in decision 0083.
- `model` and `effort` (decisions 0094, 0096): optional. Each is `{ "flag": "--model" }` (the
  value follows the flag) or `{ "args": ["-m", "{value}"] }` (`{value}` is replaced by the value
  and appears exactly once in the arguments): exactly one of the two. A flag starts with `-` and
  holds no spaces, `=` or control characters. A start's `model` or `effort` is passed after the
  mode's `args` and before the caller's `args`. A kind without the option refuses it with
  `agent.option_unsupported`, naming the kind. A value is 1 to 128 letters, digits, `-`, `_`, `.`,
  `:` or `/`, never starting with `-`.
- `model.choices` lists the models the picker shows, in order, each `{ id, label, default?: true,
  efforts? }`. An empty list means the account's models come from the vendor CLI. `efforts`
  absent means every level of the kind; `[]` means the model takes no effort; a listed id must be
  one of the effort's levels. `effort.levels` lists `{ id, label }` in order and `effort.default`
  is one of them. With levels declared, a start's effort must be one of them; with a model that
  lists efforts, it must be one of those (`request.invalid_params`, naming the allowed ids).
  Without levels, any value that passes the format rule is passed on. Ids are unique, at most one
  choice is the default, and no label is empty.

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
| `permission_mode?` | the mode chosen for the current or last run (decision 0083); absent when no mode was chosen (the vendor's own default) or the kind declares none |
| `mode_source?` | `vendor_default` (no mode flag was passed) or `chosen`; absent when the kind declares no modes |
| `mode_confirmed` | true only when the chosen mode was passed as launch arguments, no caller arguments followed, and its vendor never falls back from it; false (omitted) otherwise |
| `pending_permission_mode?` | a mode `agent.set_mode` stored for the next resume; the running mode is unchanged |
| `delegation?` | `{ writing_child: own_worktree|shared_folder|unavailable, read_only_child, reason? }`: the children the session could delegate now (decision 0161). `read_only_child` is false only at the depth limit; `reason` says why a writing child is unavailable |
| `notes?` | plain-word notes (decision 0161): on a writing child of a plain folder, "This workspace has no Git, so the child works in this folder."; on a run whose CLI failed the instruction channel's probe, "Delegation instructions were not given: this <kind name> CLI does not support <flag>." |

## Wire

Capability `agents/1`, and `agent-modes/1` for permission modes (decision 0083). Types in
`crates/protocol/src/methods/agents.rs`.

| Method | Permission | Params | Result |
|---|---|---|---|
| `agent.kinds` | read | — | `{ kinds: [{ id, name, icon, owner, installed (deprecated: = detected), detected, package: { state, id?, version? }, program_path?, resumable, status_mechanism, credentials: { supported, imported }, problems, permission_modes?: [{ id, label, description?, bypass, live_switch, may_fall_back }], default_permission_mode?, model?: { choices?: [{ id, label, default?, efforts? }] }, effort?: { levels?: [{ id, label }], default? } }] }` |
| `agent.start` | workspace, or none for a session with no project | `workspace?`, `kind`, `prompt?`, `args?`, `env?`, `read_only?`, `title?`, `cols?`, `rows?`, `request_id?`, `permission_mode?`, `model?`, `effort?` | `AgentSession`. An absent `workspace` needs capability `agent-no-project/1`; `model` and `effort` need `agent-options/1`. The client refuses either before sending when the service does not offer it. |
| `agent.list` | read | `workspace?` | `{ sessions }`, newest first |
| `agent.show` | read | `session` | `AgentSession` |
| `agent.stop` | workspace | `session`, `force?` | `AgentSession` (also ends the runs of the sessions delegated from it, 0055) |
| `agent.forget` | workspace | `session` | `AgentSession` (must be exited) |
| `agent.release` | workspace | `session` | `AgentSession`, marked `released` (must be exited; a live run is refused with `agent.busy`; capability `agent-release/1`, decision 0090) |
| `agent.retain` | workspace | `session`, `retained` | `AgentSession` with its `retained` flag (capability `agent-release/1`, decision 0090) |
| `agent.archive` | read | `session` | `{ session, released, text?, truncated, withheld? }`: the archived output of a released session, withheld from callers below host-admin scope for a run holding secrets (capability `agent-release/1`, decision 0090) |
| `agent.prompt` | workspace | `session`, `text`, `force?` | `{ receipt: "handed_to_terminal", bytes }` |
| `agent.resume` | workspace | `session`, `permission_mode?` | `AgentSession` |
| `agent.set_mode` | workspace | `session`, `permission_mode`, `force?` | `{ applied: live \| next_resume, session: AgentSession }` (capability `agent-modes/1`) |
| `agent.hook` | workspace | `session`, `token`, `event`, `payload?` | `null` |
| `agent.watch` | read | `workspace?`, `session?` | stream: `snapshot { sessions }`, then `changed { session }` |
| `agent.delegate` | workspace | `parent`, `prompt`, `kind?`, `name?`, `title?` (default: the first line of `prompt`, at most 80 characters), `read_only?`, `token?`, `request_id?`, `permission_mode?` (the vendor's default when absent, never the parent's) | `AgentSession` (the child), plus `note?` |
| `agent.tree` | read | `session` | `{ root: AgentSession, children: [AgentSession] }` |
| `agent.bring_back` | workspace | `child`, `merge?` (default true), `prompt?` (default true), `token?` | `{ child, merged, merge_commit?, prompt_delivered, note? }` |
| `agent.credentials.import` | workspace | `kind`, `from`, `replace?` | `{ kind, home, files: [{ name, bytes }] }` |
| `agent.credentials.show` | read | `kind` | `{ kind, home, exists, files: [{ name, present, bytes?, modified_at_ms? }] }` |
| `agent.credentials.remove` | workspace | `kind` | `{ kind, removed }` |
| `agent.accounts.list` | read | `kind` | `{ accounts: [{ id, label, state, identity?, active, projects, origin }] }` |
| `agent.accounts.add` | workspace | `kind`, `label?` | `{ account, terminal_id }`: starts the sign-in |
| `agent.accounts.sign_in` | workspace | `kind`, `account` | `{ account, terminal_id }`: starts a sign-in again to an existing account |
| `agent.accounts.import` | workspace | `kind`, `from?`, `label?` | `{ result: imported \| failed \| not_found, account?, reason?, updated? }`. `updated` names the existing account an import of the same login updated (decision 0098). An absent `from` copies each file from where the kind's CLI keeps it and needs capability `agent-import-detect/1`; the client refuses it before sending when the service does not offer it (decision 0097). |
| `agent.accounts.import_candidates` | read | `kind?` | `{ candidates: [{ kind, ready, folder?, files: [{ name, required, found, path?, env? }] }] }`. Capability `agent-import-detect/1` (decision 0097). Below `workspace` scope `folder` and `path` are omitted. |
| `agent.accounts.activate` | workspace | `kind`, `account`, `project?` | the account's summary |
| `agent.accounts.remove` | workspace | `kind`, `account` | `{ kind, account, removed }` |
| `agent.accounts.check` | workspace | `kind`, `account?` | the account's summary, after the check |

Errors: `agent.not_found`, `agent.kind_not_found`, `agent.kind_not_enabled`, `agent.not_installed`,
`agent.workspace_busy`, `agent.not_live`, `agent.busy`,
`agent.prompt_invalid`, `agent.resume_unavailable`, `agent.resume_not_found`, `agent.hook_refused`,
`agent.delegate_needs_git`, `agent.depth_exceeded`, `agent.parent_busy`,
`agent.parent_dirty`, `agent.merge_conflict`, `agent.already_brought_back`,
`agent.credentials_unsupported`, `agent.credentials_exist`,
`agent.credentials_invalid`, `agent.account_not_found`, `agent.mode_unknown`,
`agent.mode_unavailable`.

Event bus topic: `agent.changed` (id = session id).

## Permission modes

Decision 0083. When no mode is chosen, no mode flag is passed and nothing is typed: the vendor
CLI starts in its own default, exactly as if the person ran it, and the session records
`mode_source: vendor_default` with no `permission_mode`. The kind's `default_permission_mode`
is informational: it names that vendor default for display only, the vendor's real default may
differ by CLI version, and it must never be shown as a session's mode. A `vendor_default`
session is labelled "the CLI's default". A flag is passed only
for a chosen mode. An id the kind does not declare, or any id for a kind that declares none,
is refused with `agent.mode_unknown` (data: `kind`, `mode`, `known`). A chosen mode with
launch `args` is passed after the kind's own arguments; it is `mode_confirmed` unless the mode
is marked `may_fall_back` or the caller passed arguments after `--`, which may override it. A
chosen mode with only a live switch is typed at the run's first `idle`, before a held first
prompt, and stays unconfirmed; for a kind that reports no `idle` it is refused with
`agent.mode_unavailable`. A delegated child gets the mode its delegation names or the vendor's
default, never its parent's, so a bypass mode is never inherited. A delegation that carries a run
token (an agent asking) is refused with `agent.mode_unavailable` when it names a bypass mode:
a bypass mode is chosen by the person, through `agent.start`. Bypass modes are not gated
by device scope: the method's own permission applies.

`agent.set_mode` changes a session's mode. While its run is live and the mode declares a live
switch, the switch is typed into the run's terminal once the run is ready (as a prompt waits,
`agent.busy` otherwise), and the reply says `applied: live`; the session's mode is the new one,
`chosen`, with `mode_confirmed` false, since no hook confirms it. A switch does not count as a
typed prompt, because it starts no turn for a report to answer. A kind with no status mechanism
cannot say whether its agent is in the middle of a prompt, so a live switch into it needs
`force: true` and is refused with `agent.mode_unavailable` without it. While a switch is
typed the run is marked as switching, and every other prompt or switch, forced or not, is
refused with `agent.busy` until the write is done. Otherwise the mode is stored as
`pending_permission_mode` for the next resume (`applied: next_resume`), and the running mode
and its confirmation are left as they are. A mode a resume could not apply (only a live switch,
on a kind that reports no `idle`) is refused with `agent.mode_unavailable`; a session whose
kind cannot resume it, and which has no live switch to apply the mode now, with
`agent.resume_unavailable`. Every change is written to the session row and announced as
`agent.changed`.

A resume runs in the mode it names, which also ends any wait; else in the pending mode, unless
that is a bypass mode: a pending bypass is used only by a resume that names it, and until then
the run gets the vendor's default and the bypass stays pending; else in the last chosen mode
unless that is a bypass mode; else in the vendor's default. A bypass mode is never carried into
a new run without being named again.

## No project (decision 0094)

A conversation without a project starts with `agent.start` and no `workspace`. Its folder is
`<conversations folder>/<session id>`, where the conversations folder is the setting
`conversations.folder` (default `<home>/MassOrca/Conversations`, made on first use and refused
when it is not an absolute path). Each session gets a folder of its own because a vendor CLI
keeps its history per folder, so two sessions must never share one.

A folder workspace needs a project, so the conversations folder is registered under a hidden
project (`projects.conversations = 1`, migration 0094 in `core-store`). `project list` and every
other project listing leave it out, so a conversation is never a project. The workspace has no
worktree and no branch. A conversation's folder is an ordinary folder workspace, so stop,
resume, list and the rest of the session commands work as for any session.

The folder is created when the first session starts in it. A start that fails after its folder
was made keeps the folder, the same as a failed delegated start keeps its child workspace.

## Run environment

On top of the terminal environment (0032): `MASSORCA_BIN`, the kind's
home variable when the profile has a home for it (0056), minus the kind's
`strip_env` in that case, then the profile vault's secrets for the workspace's
project (0071), then the caller's `env`, then the run's own identity,
`MASSORCA_AGENT_SESSION`, `MASSORCA_AGENT_TOKEN` and `MASSORCA_AGENT_KIND`, which
the caller cannot override. The kind's `strip_inherited_env` names are stripped
from every run, unless the launch sets them (decision 0160). `PATH` starts with the
folder of `MASSORCA_BIN`, in front of the caller's or the vault's `PATH` when set,
else the inherited one, and is not added again when already first, so the agent can
run `massorca` (decision 0161).

The service builds every terminal's environment without the run identity it may
have inherited (when the service was started from inside an agent run), so
neither a plain terminal nor a run started directly carries another session's
identity. The account commands (check, sign-in, sign-out) strip the
credentials' and accounts' `strip_env`, the kind's `strip_inherited_env` and
the run identity names (decision 0160). A vault secret named `MASSORCA_*` or
like the home variable fails the launch with `agent.secrets_unavailable`; one
named in `strip_env` is set, not emptied. Only a run whose caller holds HostAdmin gets
vault variables; such a run and its children are marked `holds_secrets`, and
their terminals, prompts, resume and delegation then need HostAdmin (0071).

Limitation: a terminal's environment (0032) can add variables but not remove
inherited ones, and `core-terminal` is not changed in this phase. Until it can,
a stripped variable (from `strip_env` or `strip_inherited_env`) is set to the empty string instead of removed. Agents treat
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

A first prompt for a kind whose `prompt.mode` is `startup-query` is one launch argument, `<flag>=<text>`,
after the kind's arguments and the caller's (0054). The agent takes it as its startup query and submits
it itself once its own composer is ready, so MassOrca neither waits for `idle` nor types the text later:
no prompt is held on the run, and the kind's status is not needed for it. The `flag` is required and holds
no `=`; the text keeps the limits of every mode (8 KiB, no control character except tab, line feed and
carriage return). Later prompts of such a kind are typed as for any other kind. The contract is the Hermes
startup query of pv96 (`hermes chat --query=<text>`, decision 0054 amendment). No built-in kind uses it yet.

`agent.resume` (0054) starts a new run of an exited session, in the same session: the kind's resume
arguments, with the agent's ref in place of `{ref}`, in a new terminal in the workspace root. The
one-writer rule (0051) applies. The session keeps its id, its ref, its title and its last result; its
exit is cleared, and its activity is absent until the new run reports. The caller's arguments and
environment of the first start are not kept, so a resumed run gets the kind's own arguments and the
resume arguments only. The run is registered under the status lock before its program starts, as in
`agent.start`, so its first hook report is accepted. Refusals: `agent.busy` while the run is live;
`agent.resume_unavailable` when the kind is no longer declared, declares no resume form, or the
session's ref is not known yet (a Codex session learns its ref at a turn's end), or its last run has
no recorded end (no terminal and no exit; decision 0058); `agent.resume_not_found` when the
session's `resume` state says so (below); `agent.not_installed`
when no program of the kind resolves; `agent.workspace_busy` when another session writes in the
workspace.

### Resume state (decision 0160)

Every session view carries `resume: { available, code?, reason? }`, set by the service
(`core-agents::resume_state`); an older peer sends none.

- `agent.resume_unavailable` when the kind is no longer declared, is invalid, declares no
  resume form, or the session's ref is not known.
- `agent.busy`, reason "the session is running", while its run is live.
- `agent.resume_not_found`, reason "the agent saved no conversation for this session", when an
  exited session's kind declares `resume_transcript` and no file matches its `glob` with the ref
  in place of `{ref}`. The folder is the account home the last run used (recorded as
  `run-account` in the run directory), else the `env` variable's value when it is an absolute
  local path, else `dir` with `{home}` from `USERPROFILE` or `HOME`. Only metadata is read, and
  links and junctions are followed. Only a search that completed and matched nothing counts as
  missing: an unreadable folder, a folder of more than 10,000 entries, a ref that is not a plain
  file name, or no home to look under leaves the session available. A found transcript is kept
  in memory until a run starts or exits; a missing one is looked for at each view, and `agent
  list` reads the kind catalog once. The built-in `claude` checks `projects/*/{ref}.jsonl` under
  `CLAUDE_CONFIG_DIR` or `{home}/.claude`. `dir` and `glob` parts are plain names: `..`, `.`, a
  second root or a drive is a declaration problem.
- The same code when a resumed run exited early with a failure (before any hook report and
  within 10 seconds of its start) and the last 4 KiB of its output match the kind's
  `resume_not_found_pattern` (`claude`: "No conversation found with session ID"). A later exit
  is never classified. The run directory keeps the ref (`resume-not-found`); the session stays
  exited, and its exit is recorded as it was. A transcript check that finds the file drops the
  mark.

`agent.resume` refuses a session whose state is `agent.resume_not_found` before it starts a run.

### Crash-safe start

A start or resume is recorded before its program starts and settled after: the service writes an
empty `starting` file in the run's directory, starts the terminal, records the run in the session
row (a new row for `agent.start`, the new terminal for `agent.resume`), and only then removes the
file. A service that dies in between leaves the file. Its program does not outlive it (0051: a job
object per terminal on Windows, the session on Unix).

The next service start settles every marked directory before it serves. With no session row, the
start never reached its row: the directory, which may hold the run's hook token and launch files, is
deleted, so a retry with the same `request_id` starts afresh instead of finding a session with no
run. With a row, the start or resume was recorded and only the mark is removed; the row is marked
stale like any other run of the previous service.

The CLI tests reach both points through `MASSORCA_TEST_CRASH_AT` (`agent-start-spawned`,
`agent-resume-spawned`), which makes the service exit there at once, as a crash would. The crash
points exist only with core-agents' `test-hooks` feature, which only the CLI's dev-dependencies
enable, so a release build carries neither the points nor the variable;
`crates/core-agents/tests/test_hooks_gate.rs` fails when any normal or build dependency enables it.

## Prompt queue (decision 0084)

A session's queue holds the prompts a person queued while its agent was busy, in position order, in the
`agent_prompt_queue` table. The service keeps it, so it survives a client restart and serves every client.

- `agent.queue {session, text, request_id?}` queues a prompt and returns it. A `request_id` the session already queued
  returns the prompt it queued then.
- `agent.queue.list {session}` returns `{session, items}`, head first.
- `agent.queue.edit {id, text}` and `agent.queue.remove {id}` change or remove a waiting or failed prompt and return it.
  A prompt being sent is refused with `agent.busy`.

A `QueuedPrompt` has `id`, `session`, `position` (from 1), `text`, `state` (`waiting`, `sending`, `failed`), `reason`
and `created_at_ms` / `updated_at_ms`. The reason of a waiting prompt is `busy`, `asking`, `paused_stopped`,
`paused_restarted` or `behind_failed`; the reason of a failed prompt is why its delivery failed. Constants:
`QUEUE_REASON_*` in `massorca_protocol::methods::agents`.

Refusals: `agent.queue_already_delivered` for a request id the session delivered recently (the last 100, decision 0084,
D11); `agent.queue_unavailable` for a kind that declares no status mechanism (or is not declared), so it has no
queue; `agent.queue_full` past 100 prompts; `agent.queue_not_found` for an unknown prompt id; `agent.prompt_invalid` for
a text `agent.prompt` would refuse. Capability `agent-queue/1`. Every change publishes `agent.queue_changed` with the
session id; `agent.watch` streams do not carry queue items.

Delivery: the head of a queue is typed when its agent turns `idle` or `done`, through the prompt guards (merging,
switching, readiness, typed since the last report). One prompt goes per step, so the next waits for the next report. A
queue holds while the agent asks the person something (reason `asking`), and `agent.queue.send_now` is refused then. A
prompt that cannot be typed becomes `failed` with its reason and holds everything behind it; nothing retries, and
`remove` or `send_now` clears it. A stop by the person pauses the waiting prompts (`paused_stopped`); a service restart
pauses them (`paused_restarted`) and fails a prompt being sent. An accepted `agent.prompt`, `agent.queue.send_now` or
`agent.queue.resume` lifts the pauses; a resumed run does not deliver paused prompts by itself.
`agent.queue.send_now` types one prompt at once with force, ahead of the queue, and refuses while the run is switching a
mode or a merge holds its workspace. Calls: `agent.queue.send_now {id}` returns the prompt as it was, and
`agent.queue.resume {session}` returns the queue as it is after the lift.

## Release and retain (decision 0090)

`agent release` archives the newest 256 KiB of a run's terminal text (scrollback and visible rows, without escape codes),
then closes and frees the terminal and keeps the record, marked `released`. Only an exited run can be released; a live
one is refused with `agent.busy`. The archive lives in `agent_session_archives` and is read with `agent.archive`. When the
output cannot be read the terminal is kept and the release fails with `agent.archive_failed`. A release clears the retain.

`agent retain` is a durable flag on the session (`retained`). It keeps the session's exited terminal out of the eviction
that `max_exited` applies: the terminal manager counts only unpinned exited terminals and never forgets a pinned one. Every
later run's terminal is pinned when it starts. It does not block a release.

`agent forget` deletes the record, its terminal, its files and its archive. `agent resume` of a released session starts a
new run and drops the archive of the run before. The reference and deviations are in `docs/decisions/0090-agent-release-and-retain.md`.

## Delegation

`agent.delegate` and `agent.bring_back` follow decision 0055. Details the record leaves open:

- The result of `agent.delegate` is the child's `AgentSession` with an optional `note` beside its fields
  (`AgentDelegated`), so a reader that expects a session reads it unchanged. A writing child's note says that the
  parent's uncommitted changes were not carried over.
- Folder children (decision 0161): a writing child of a plain folder workspace runs in the parent's folder, with no
  copy, and its `note` and `notes` say "This workspace has no Git, so the child works in this folder." The
  sessions under the same delegation root count as one writer there: `agent.workspace_busy` does not refuse a writing
  start or resume of one of them because another writes in the folder, while any other writing start is refused as
  before. `agent.delegate_needs_git` remains for a worktree workspace with no branch.
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
massorca agent start <kind> [--workspace <ws>] [--model <m>] [--effort <e>] [--prompt <text>] [--read-only] [--title <t>] [-- <args>...]
massorca agent list [--workspace <ws>]
massorca agent status [<session>] [--watch] [--until <working|waiting|idle|done|exited>] [--timeout <secs>]
massorca agent stop <session> [--force]
massorca agent forget <session>
massorca agent release <session>
massorca agent retain <session> [--off]
massorca agent archive <session>
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
massorca agent accounts sign-in <kind> <account> [--no-attach]
massorca agent accounts import <kind> [--from <dir>] [--label <l>]
massorca agent accounts candidates [<kind>]
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

## Import sources (decision 0097)

`core-agents::sources`. A kind's `credentials` may declare, per file, where its CLI keeps it:

```json
"optional": [".claude.json"],
"sources": [
  { "file": ".credentials.json", "env": "CLAUDE_CONFIG_DIR", "dir": "{home}/.claude" },
  { "file": ".claude.json", "env": "CLAUDE_CONFIG_DIR", "dir": "{home}" }
]
```

`dir` is the default folder template; `windows`, `macos` and `linux` replace it on that platform. The only
placeholder is `{home}`, read from `USERPROFILE` on Windows and `HOME` elsewhere (what the vendor CLIs use), and from
the platform when that variable is unset or not absolute. When `env` names a variable set to an absolute path in the
service's environment, the file is looked for in that folder instead. A file not in `optional` is required; without
`optional` every declared file is. The kind's problems list a source or optional entry that is not a declared file,
a file named twice, an `env` that is not a variable name, and a template that neither starts with `{home}` nor is an
absolute path. The built-in `claude` declares `.credentials.json` (required) in `{home}/.claude` and `.claude.json`
(optional) in `{home}`, both moved by `CLAUDE_CONFIG_DIR`; `codex` declares `auth.json` (required) and `config.toml`
(optional) in `{home}/.codex`, moved by `CODEX_HOME`.

`agent.accounts.import_candidates` reports, per kind, each file's path and whether the import would accept it: a
regular file, not a link, at most 1 MiB (a state file: its rule's `max_source_bytes`), judged from `symlink_metadata`
alone. It never opens a file. `ready` means
every required file was found. `agent.accounts.import` without `from` reads each file from its own path, with every
check of Credential import; a folder import applies the same optional rule.

## Account state file (decision 0099)

`core-agents::state_file`. Some CLIs keep their own state beside the login in the folder the home variable names:
with `CLAUDE_CONFIG_DIR` set, Claude Code reads `.claude.json` there too, and without `hasCompletedOnboarding` it opens
its first-run setup, which ends in a browser sign-in, while `auth status` (which judges only the credentials) still
says signed in. A kind's `credentials.state` declares how that file is made:

```json
"state": {
  "file": ".claude.json",
  "keep": ["oauthAccount", "theme", "mcpServers"],
  "set": { "hasCompletedOnboarding": true },
  "max_source_bytes": 16777216
}
```

`file` is a declared file; `keep` and `set` name plain top-level keys (letters, digits, `_`, `-`, `.`), each once and in
only one of them; `max_source_bytes` (default 1 MiB) is between 1 byte and 64 MiB. The kind's problems list each
breach.

- Import, from a folder or detected sources, builds the state file and never copies it raw. The source is read with
  the checks of Credential import but with `max_source_bytes` as its cap, parsed, and only the `keep` keys present are
  written, then the `set` keys. Everything else, project history included, stays behind. A missing source gives a file
  with the `set` keys only. A source that is not a JSON object, or over its cap, refuses the import when the file is
  required; when it is optional (Claude), the file gets the `set` keys only, and the import's `reason` and the file's
  `reason` say why. A state file alone, built from nothing, is nothing to import.
- Before every account check (`agent.accounts.check`, and the checks that decide an import or a sign-in) and before
  every launch on an account home, the service puts the `set` keys back under `credentials::lock_homes`: it creates the
  file when it is missing and merges the keys when they are absent or differ, keeping every other key, writing a new
  owner-only sibling and renaming it over the file. A link or anything but a regular file is never followed or
  written through. A state file that cannot be repaired (not a JSON object, over its cap, a link) is left as it is;
  the check reports `needs_sign_in` and a launch is refused with `agent.credentials_invalid` naming the file, so the
  check and the session never disagree. Existing accounts are repaired this way without a new import.
- Only `claude` declares a rule. Folder trust prompts are out of scope.

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
- **One account per login** (`Agents::duplicate_of`, decision 0098): a new account whose check
  reports an identity another account of the kind already has (compared trimmed and without case,
  as pv96 compares a duplicate add's email) updates that account instead of adding one. Under the
  homes gate, `credentials::update_account_files_locked` writes each declared credential file the
  new account holds into the existing account's home, each to a new owner-only sibling renamed over
  the old file (a link is refused). Nothing else in that home changes: an agent home also holds
  transcripts, history and settings, and a declared file the new account lacks is kept. The new
  account's home and row are then deleted, and the existing account's state and identity are
  refreshed. The existing account keeps its id,
  label, origin, active mark and project choices. An import answers `imported` with that account and
  `updated: <its id>`; a sign-in's end announces both accounts. When the existing account is busy (a
  live run, a removal, a sign-in or a decision of its own, or a sign-in undecided in the store), the
  new account is deleted as before: an import fails naming the existing account, and a sign-in is
  announced as `conflict`. Nothing is ever signed out here: the vendor sign-out would end the session
  both accounts share. A sign-in again whose login turns out to be another account's is stored as
  `conflict`, keeping its recorded identity. A kind's accounts in WSL are not told apart from the host's yet:
  there is no WSL account scope.
- **Sign-in again** (`sign_in.rs`, `Agents::accounts_sign_in`, `agent.accounts.sign_in`): signs an
  existing account in once more, in its own home, with the same login and environment as a new
  sign-in (both go through `create_login_terminal`). The account is kept whatever the login ends
  with, and never deleted nor signed out: it existed before the sign-in, so its end runs
  `decide_signed_in_again`, not `decide_marked`. That decision stores the state the account's
  check found, with the check's identity: `signed_in` with the identity, else `expired`,
  `needs_sign_in`, `conflict`, or `unknown` when the check cannot tell (a kind without a check, or
  one that cannot run, is `unknown`). Its active status and its project choices do not change, and
  the decision announces the account once. It is refused before anything starts, with the codes of a sign-in:
  an unknown account, or one of another kind, is `agent.account_not_found`; one being removed,
  signing in, decided, or in use by a live run is `agent.busy`; one whose sign-in is undecided in
  the store is `agent.busy`; one without a usable home is `agent.credentials_invalid`.
  **Crash safety**: the account is marked pending in memory only (`account_busy().pending`), never
  in the store (`sign_in_pending`). The startup sweep decides only the store's undecided sign-ins,
  so a restart during the login leaves the row as it was, and the account's next check decides
  its state. The mark is the terminal's id until the terminal ends, then the decision's empty mark.
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
  does it count as the kind's history. `not_found` means a required file is missing; the reason
  names each one and where it was looked for. A missing optional file (`credentials.optional`) is
  skipped. Without `from`, each file is read from where the kind's CLI keeps it (see Import sources). An import a crash left undecided is decided at the next start, the same way.
- **Removal**: three steps. Under the gate, a busy account (a live run, an open sign-in, or a removal
  already running) is refused, and the account is marked removing. Outside the gate, the kind's
  `accounts.logout` runs only for an account whose origin is `sign_in`: an imported or migrated login
  is the person's, so the service never signs it out; its files are only deleted. Under the gate
  again, the home and the row are deleted. A sign-in the service discards (see Sign-in) is signed out
  the same way before its home is deleted, since the service made that login.
- **Migration and startup**: `startup_sweep` runs once before requests are served. It first settles
  the old homes a replace set aside (`.<name>.old-<id>` in `homes/` and `accounts/<kind>/`, decision
  0098): one whose home is missing, because a crash came between the two renames, is renamed back,
  and one whose home exists is deleted. Then it deletes a
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
| `claude` | `claude` | `CLAUDE_CONFIG_DIR` | `claude auth login` (confirmed) | `claude auth status --json`, exit 0 signed in, 1 not (confirmed); `loggedIn: false` needs a sign-in, identity from `email` and `orgName` (decision 0098) | `claude auth logout` (confirmed) |
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

Declared accounts for `claude` are `login: ["auth", "login"]`, `check: { args: ["auth", "status",
"--json"], signed_in_exit: 0, identity_json: ["email", "orgName"], signed_in_json: "loggedIn" }` and
`logout: ["auth", "logout"]` (decision 0098). A check's `identity_json` reads its output as a JSON
object and joins the listed top-level string fields that are present with ` / `, so one email in
two organisations is two logins; it is declared instead of `identity_pattern`. `signed_in_json`
names a top-level field whose `false` means the check needs a sign-in, whatever its exit code. The
previous app reads `claude auth status --json` and its `email` field; the fields were not run
here.
Below `workspace` scope the identity is masked, as every account identity is. No `expired_pattern`
is declared, because none is documented. Without it, an expired login probably reads as
`needs_sign_in`, not `expired`; this is not verified.

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
