# 0078: A pairing scope without program execution

Status: not adopted (owner direction, 2026-11-22). The design below is kept
only as a record; no slice of it will be built.

A paired device is the owner themself. As in Claude Code, Codex and
Antigravity, every tool is usable from the first moment, and restrictions
come from the agent's permission mode (plan, always ask, automatic), not from
the device. A per-device level between `read` and `workspace` is therefore
more machinery than the product needs. Pairing defaults to full access, all
scopes including host admin; the existing scopes stay in the code as an
optional, advanced restriction and are not a product feature to extend. The
one part that shipped is the git rule for reads by a caller without
`workspace` (PR #202): it is cheap and protects a device the owner
deliberately paired with reduced scopes.

## Context

0071 rules that `workspace` on a host is equivalent to acting as the host user
there: a `workspace` device can run any program, so it can unwrap the
user-scope DPAPI vault key on Windows and read every file agents wrote. The
vault gating is defence in depth, not a boundary against such a device.

A real boundary needs a scope whose holder cannot make the host run a program
it chose. This decision defines it.

### Every path to a program the caller chooses

Paths a paired `workspace` device has today (main at the time of writing):

| # | Path | Where | Why it runs a chosen program |
|---|---|---|---|
| 1 | `terminal.create` with `shell.program`/`shell.args` | `server/src/features/terminals.rs:30-36`, `:95-98`; `protocol/src/methods/terminals.rs:47-56` | Spawns that program directly. |
| 2 | `terminal.create` with no shell, then `terminal.input` | `terminals.rs:52-57` | The default shell runs whatever is typed. |
| 3 | `terminal.create` `cwd` and `env` | `terminals.rs:93`, `:177-181`; `protocol/.../terminals.rs:51-56` | `cwd` may be absolute anywhere; `env` can set loader and interpreter variables. |
| 4 | `terminal.input` to an agent run | `terminals.rs:52-57` | Agent shell escapes, for example Claude's `!` prefix, run a command with no approval. |
| 5 | `agent.start` `args` and `env` | `protocol/src/methods/agents.rs:194-199`; `core-agents/src/sessions.rs:404-405` | Args such as `--settings`, `--mcp-config` or a permission-bypass flag pick programs; `env` is applied last. |
| 6 | `agent.credentials.import`, `agent.accounts.import` from a host directory | `server/src/features/agents.rs:128-133`, `:163-168`; `protocol/.../agents.rs:375-381` | Copies files such as `.claude.json` (`core-agents/src/catalog.rs:281`) into the agent home; that file can declare MCP servers, which are programs the agent starts. |
| 7 | `agent.accounts.check` | `agents.rs:184-189`; `core-agents/src/account_check.rs:167-207` | Runs the kind's program against a home the caller may have filled through row 6. |
| 8 | Agent home settings | 0073 (`agents/homes/claude/.claude/settings.json`) | Hooks in that file run code as the user. Reached through rows 6 and 12. |
| 9 | Git operations that run hooks | `core-git/src/git.rs:162-181` sets no `core.hooksPath`; `commit.rs:77`, `branch.rs:134`, `merge.rs:36`, `remote.rs:89-100`, `:150`, `worktree.rs:113`, `index.rs:111` | `pre-commit`, `commit-msg`, `post-checkout`, `post-merge`, `pre-rebase`, `post-rewrite`, `pre-push` run from the repository's hooks directory. |
| 10 | Git config that names a program | same runner | `core.fsmonitor` (even on `git.status`, a `read` method: `server/src/features/git/status.rs:20`), `core.sshCommand`, `filter.*.clean/smudge`, `diff.external`, `diff.*.textconv`, `merge.*.driver`, `credential.helper`, `gpg.program`, `remote.*.uploadpack/receivepack`, `include.path`. |
| 11 | `project.add` and `workspace.create` with a host path | `server/src/features/projects.rs:23-27`; `protocol/src/methods/projects.rs:26-34`; `protocol/src/methods/workspaces.rs:46` | Points MassOrca at any directory, including one with hooks or config from rows 9 and 10. |
| 12 | Prompts (`agent.start` prompt, `agent.prompt`, `agent.delegate`, `bring_back` with `prompt`) | `agents.rs:44-49`, `:72-77`, `:100-105`, `:114-119` | The agent runs tools. With approval prompts answered (row 4) or switched off by its launch, that is any program the prompt describes. |
| 13 | Extension install, enable, remove, and `dir:`/`file:` inspect | `server/src/features/extensions.rs:59-79`, `:126-130` | Already `host-admin`. No change. |
| 14 | Workspace setup scripts | none in this repository | Not implemented. Any future one is a row of this table. |

Agent kinds are read from built-ins, the catalogue and `agents/kinds/*.json`
in the profile (`core-agents/src/catalog.rs:74`). No device method writes
that folder, so the set of kinds is the owner's choice.

## Decision

**A new scope, `operate`.** Wire name `operate`, `Permission::Operate`, bit
16. The boundary is: *the device chooses no program and approves no
program.* It is not "no code runs". An agent started by an `operate` device
still runs code, and does whatever its own permission policy lets it do
without asking.

`workspace` implies `operate`: `Principal::holds(Operate)` is true for a
holder of either. Methods that an `operate` device may call are registered
with `Permission::Operate`; the rest stay at `workspace` or above.

### What `operate` allows

- Everything `read` allows: lists, status, diffs, logs, watches, boards.
- `agent.start` and `agent.delegate` of an **operate kind**, on a registered
  workspace, with no `args` and no `env`. `read_only`, title, size and
  `request_id` stay allowed.
- `agent.prompt`, `agent.resume`, `agent.stop`, `agent.forget`,
  `agent.bring_back`, within the prompt rule below.
- `terminal.attach` to a terminal that belongs to an agent run, as a viewer:
  no `cols`/`rows`, `answers_queries` forced off. `terminal.kill` and
  `terminal.close` of agent-run terminals.
- Git: branch create, switch, rename, delete; stage, unstage, discard;
  commit; fetch, pull, push, abort; all within the git rule below.
- `workspace.create` without `path`, `workspace.remove`.
- Bot Teams writes (teams, team projects, bots, tasks), `secret.request`.

### What `operate` denies

- `terminal.create`, `terminal.input`, `terminal.resize`, and attach to a
  terminal not owned by an agent run.
- `agent.start` and `agent.delegate` with `args` or `env`, or of a kind that
  is not an operate kind.
- `agent.credentials.import`, `agent.credentials.remove`, every
  `agent.accounts.*` write and `agent.accounts.check`.
- `project.add`, `project.remove`, and `workspace.create` with `path`.
- `agent.hook` (a run's own callback; a device has no run token).
- Everything already at `host-admin` or `device-admin`.

**Operate kinds.** A kind definition gains an `operate` block:
`{"allowed": true, "escape_prefixes": ["!", "/"]}`. A kind without it, or
with `allowed: false`, cannot be started by an `operate` device. A kind may
set `allowed` only when its launch arguments keep the agent's approval
prompts on. The built-in kinds get the block after a review of each one; a
profile kind file gets it only if the owner writes it.

**Approvals stay with the owner.** An `operate` device cannot send terminal
input, so it cannot answer an agent's approval prompt. A run that asks waits
(`activity: waiting`) until the owner or a `workspace` device answers.
Approving a tool call is choosing a program, so a structured approval method
for `operate` is out of this decision.

**Prompt rule.** For an `operate` caller, a prompt is refused with
`permission.denied` when it starts, after leading blanks, with one of the
kind's `escape_prefixes`. For a kind whose prompts are not sent as bracketed
paste, every line is checked, because each line feed submits a line. Control
characters are already refused (`core-agents/src/launch.rs:95-102`), so a
prompt cannot close a bracketed paste and type outside it. A task's text that
later becomes a prompt passes the same check when the task is created by an
`operate` device.

**Git rule.** The rule depends on the caller. A caller that holds
`workspace`, and the owner, keep today's behaviour unchanged: their commits,
merges, checkouts and pushes run the repository's hooks and configured
programs, as plain git does, because pre-commit tooling depends on it. A
caller without `workspace` (`read` today, `operate` later) must never cause a
program the repository names to run:

- Reads ship first, as their own PR ahead of the slices below, because a
  `read` device can trigger row 10 today. Every git process for such a
  caller runs with `--no-pager`, an empty `core.fsmonitor` (Git 2.25 reads the
  value as a hook path, so `false` would run a program named `false`; an empty
  value turns the monitor off in 2.25 and in 2.36+), `core.hooksPath` pointing
  at an empty folder in the owner-only profile, an empty `diff.external` and
  `credential.helper`, and `log.showSignature=false`. Every
  `filter.*.clean/smudge/process` and `diff.*.textconv/command` set in the
  repository's own config files (`git config --local`, and `--worktree` when
  `extensions.worktreeConfig` is on, both with `--includes`) is overridden
  with an empty value, and such a filter is made optional. `status`,
  `diff-files` and `diff-index` get `--ignore-submodules=dirty`, so git never
  runs a status inside a submodule under the submodule's own config. Every
  transport is closed (`protocol.allow=never`, each named protocol and each
  `protocol.*.allow` the repository sets overridden with `never`), lazy fetch
  is off where git knows `GIT_NO_LAZY_FETCH` (2.44+), and a partial clone
  (`extensions.partialClone` or a true `remote.*.promisor`) is refused, since
  a read that needs a missing object would fetch through a transport the
  repository names. Filters
  in user or system config keep running, so a Git LFS set up there still
  works. A run that cannot be made safe (the hooks folder is missing or not
  empty, the config cannot be read, a driver name contains `=`) is refused
  with `git.unsafe_for_caller`, and so is any mutation or remote command
  under this runner. Every option exists in Git 2.25; the baseline CI job
  runs the tests on Git 2.25.5.
- Before a mutation by an `operate` caller (slice 4), the service reads the repository and worktree config
  (`git config --local --list --show-origin`, and `--worktree`). If any key on
  row 10's list is set there, or `include.path`/`includeIf.*` is set at all,
  the call is refused with `permission.denied` naming the key.
- If the effective hooks directory holds a hook the operation would run, the
  call is refused naming the hook. Skipping it silently would commit or push
  work the owner's checks never saw.
- An `operate` caller's mutations then run with the same overrides as reads,
  so a hook or config added between the check and the run does nothing.

User and system git config stay trusted: they are the owner's files, and no
device method writes them.

### Interactions with the vault (0071, PR #160)

- `operate` is below `host-admin`, so its starts, resumes and delegations get
  no vault variables and the reply carries `secrets_withheld`.
- Runs marked `holds_secrets` stay as #160 rules them: attach, prompt, resume,
  delegate and bring-back need `host-admin`. `operate` adds nothing there.
- What `operate` changes is the trust boundary. Rows 1-11 are closed for an
  `operate` device, so it cannot run the code that unwraps the vault key.
  Row 12 is bounded by the operate kind's approval prompts, which the device
  cannot answer.

### Wire and compatibility

- **Existing devices keep their scopes.** 0041 says scopes never change after
  pairing; a `read`+`workspace` device stays one. To narrow it, the owner
  revokes it and pairs again with `operate`.
- **Pairing.** `PairingCreateParams` gains optional
  `role: "standard" | "operate" | "admin"` (`serde(other)` maps an unknown
  value to a refusal). `admin: true` keeps working. `operate` gives
  `["read", "operate"]`. CLI: `massorca pair offer --operate`, which conflicts
  with `--admin`. The desktop's offer dialog in Settings shows the three roles.
- **Default role.** New offers stay `standard` (`read`+`workspace`) until
  `operate` is built and validated. After that, the desktop pairing screen
  defaults to `operate` and offers full `workspace` as an explicit choice,
  explained in one line: "can run programs as you on this computer". The CLI
  gets the same default in the same PR (slice 5).
- **Older servers.** Params carry no `deny_unknown_fields`, so an older
  server ignores `role` and makes a standard offer. The client therefore
  checks `PairingCreated.scopes` and, if `operate` was asked for and the reply
  holds `workspace`, does not show the offer and says the host must be
  updated. The unshown offer expires unused.
- **Downgrade.** `Scopes::from_names` already gives nothing for an unknown
  name (`server/src/permission.rs:69-82`), so an `operate` device on an older
  build holds `read` only. It fails closed.
- **Older clients.** They show `operate` as a raw scope name in
  `device.list` and `device.whoami`. A client that hides actions by scope
  sees no `workspace` and hides terminals, which is correct.
- **Shown as.** CLI `device list` and the desktop's device rows label it
  "Operate (agents, no terminals)".
- **Errors.** Every refusal is `permission.denied`, which every client
  already handles. The message names what was refused and the scope that
  allows it, for example "device X may not pass agent args; needs workspace".
  The one new code is `git.unsafe_for_caller`, for a git read that cannot be
  made safe (see the git rule); older clients show it as a plain error.

## Residual risks

- **The agent's own policy is the gate for row 12.** An operate kind runs
  tools its policy auto-allows, for example reading files, without asking.
  A prompt can steer it to read anything that policy permits, including
  files outside the workspace if the policy allows that. Writing a git hook
  or config usually needs approval; if it does not, the git rule still stops
  the hook from running on a device-triggered operation.
- **A bypass launch voids the scope for that kind.** If an owner marks a kind
  `allowed` while its launch skips approvals, `operate` on that kind is
  equivalent to `workspace`. The review of built-in kinds and a test on each
  built-in definition guard this; owner-written kinds are the owner's call.
- **Owner approvals.** An owner who approves whatever a waiting run asks
  turns a device's prompt into a program. The run's view shows who started it.
- **Git internals.** The refused key list follows git's documented
  program-running keys at the time of writing. A new git release can add one.
  The empty `core.hooksPath` and the config check cover today's set.
- **Config written between the check and the run.** The repository's config
  is read in one git process and the command runs in the next. A local
  writer of that config (an agent working in the worktree) could add a
  driver in that gap. A read-only device cannot write it on its own.
- **Out of scope:** OS-level isolation of the vault or of agent runs (a
  separate user, AppContainer, a sandbox), per-workspace scopes, per-device
  limits on which kinds may run, and an approval method for `operate`.

## Implementation plan

Order: the git rule for reads by callers without `workspace` lands first, as
its own PR, because it closes a hole `read` devices have today. The five
slices follow as small PRs, each reviewed. Zeroing the plain copies of
secrets in the launch path comes after them.

| Slice | Content | Size |
|---|---|---|
| 1 | `Permission::Operate`, implication from `workspace`, wire name, `Scopes::OPERATE`; re-register the allowed methods with `Operate`. | ~150 lines |
| 2 | Parameter checks: agent `args`/`env`, operate kinds, `workspace.create` `path`, attach viewer rules, agent-run-only attach. Kind `operate` block and the built-in reviews. | ~250 lines |
| 3 | Prompt rule for start, prompt, delegate, bring-back and task text. | ~120 lines |
| 4 | Git rule for `operate` mutations: config and hooks checks, then the overrides reads already use. | ~200 lines |
| 5 | `role` on `pairing.create`, CLI `--operate`, client reply check, desktop dialog and labels, docs. | ~250 lines |

Tests, each through the service with a paired `operate` device:

- `terminal.create`, `terminal.input` and `terminal.resize` are refused with
  `permission.denied`; attach to a plain terminal is refused; attach to an
  agent-run terminal works and ignores `cols`/`rows`.
- `extensions.install` is refused (unchanged).
- `agent.start` of an operate kind with no args or env starts; the same with
  one arg, or with one `env` entry, is refused; a kind without the block is
  refused; the reply carries `secrets_withheld`.
- `agent.prompt` with a leading `!` to a kind declaring it is refused; the
  same text from a `workspace` device is delivered.
- `agent.credentials.import`, `agent.accounts.check` and `project.add` are
  refused.
- `git.commit` in a repository with an executable `pre-commit` is refused and
  the hook did not run; with `core.fsmonitor` in `.git/config` it is refused;
  `git.status` and `git.diff_file` from a `read` device run no configured
  fsmonitor or clean filter (shipped with the read PR).
- A `workspace` device still commits with its hooks running (no regression).
- A `read`+`workspace` device paired before the change keeps both scopes.
- `pairing.create` with `role: "operate"` gives `["read", "operate"]`; against
  a server that ignores `role`, the client refuses to show the offer.
- Every built-in kind marked `allowed` has no permission-bypass argument.
