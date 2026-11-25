# 0093: Agent credential guard and worktree listing

Status: accepted, 2026-11-24. Covers the agent credential guard (TERM-24) and the worktree listing (GIT-14).

## Agent credential guard (TERM-24)

### Context

TERM-24 says an unattended agent terminal never stops to ask for a git password. The service's own git calls already run with
`GIT_TERMINAL_PROMPT=0` and blank askpass helpers (`crates/core-git/src/git.rs`, `command`), but an agent's terminal did not
have the guard, so git run inside an agent session could wait for a credential prompt that nobody answers.

The reference is pv96 at commit `d3c6311a531`, read without checkout. Its decision is
`src/shared/terminal-git-credential-guard.ts`; the variables are defined in `src/shared/git-credential-prompt-env.ts`.

### Decision

- **Scope.** Every agent run's terminal gets the guard, one-shot and resumed runs included, because every run is planned by
  `crates/core-agents/src/launch.rs`. User terminals, sign-in terminals and `terminal create` keep the environment they inherit.
  pv96 applies the guard to unattended launches and recognised agent commands, and not to user terminals.
- **Settings.** The pv96 `gitCredentialPromptGuardEnv`: `GIT_TERMINAL_PROMPT=0`, `GCM_INTERACTIVE=never`, and
  `GIT_ASKPASS` and `SSH_ASKPASS` set to empty unless one is already configured. The git config protocol gets
  `credential.interactive=false` and `credential.guiPrompt=false` appended, but only when the protocol already in the
  environment is unambiguous. Otherwise only the scalar settings apply.
- **Order.** The guard is applied last, after the caller's and the vault's variables, so a caller cannot turn prompts back
  on. A configured askpass helper is kept, as pv96 keeps it.
- **Transport.** The launch plan sets `MASSORCA_INTERNAL_GIT_CREDENTIAL_GUARD=guard`. `build_environment`
  (`crates/core-terminal/src/shell.rs`) reads the request, applies the guard and never passes the variable to the program.
  pv96 carries its request the same way, in `ORCA_INTERNAL_TERMINAL_GIT_CREDENTIAL_GUARD_POLICY`, and deletes it before spawn.
- **Exposure.** A terminal records whether it runs guarded. `terminal show` and the `terminal.show` method report
  `git_credential_guard: true`, an optional field that is absent otherwise, and the CLI prints `git credential guard: on`.
- **SSH.** No `BatchMode` is set for terminals. pv96 sets `GIT_SSH_COMMAND=ssh -o BatchMode=yes` only for its own service git
  runner (`src/main/git/command-runner/git-process-env.ts`), not in the terminal guard.
- **Not done.** pv96 also adds the guard keys to `WSLENV` on Windows. The Rust terminal environment has no WSL bridge yet, so
  there is nothing to add to.

### Alternatives

- **Four variables in the launch plan.** Rejected. A plan cannot see the askpass helper or the git config protocol that the
  terminal inherits, so it would overwrite them. The terminal sees the final environment.
- **A typed field on the terminal spawn spec.** Rejected for now. Every literal construction of the spec would change,
  including the core-terminal test support. The variable keeps the change in the crates that own the behaviour and follows pv96.
- **Guard every terminal.** Rejected. TERM-24 and pv96 both keep user terminals on normal git behaviour.

### Consequences

- An agent that hits a credential prompt now gets git's error and continues, instead of waiting.
- `git_credential_guard` is optional, so an older client ignores it.
- The `credential.*` config part needs Git 2.31 or later, which is the first version that reads `GIT_CONFIG_COUNT` (checked in
  the git sources at tags v2.30.0 and v2.31.0). On the 2.25 baseline only the scalar settings apply.
- Passing `MASSORCA_INTERNAL_GIT_CREDENTIAL_GUARD=guard` to `terminal create` turns the guard on for that terminal. Nothing else
  changes.

## Worktree listing (GIT-14)

### Context

GIT-14 says the workspace list shows every linked worktree git knows for a repository, with locked and prunable state.
Before this change, `workspace list` reported unregistered worktrees only under the project's workspaces root, and the
lock and prune state came from git's porcelain output alone.

The reference is pv96 at `d3c6311a531`: `src/main/git/worktree-list-reader.ts` lists every linked worktree, and
`annotatePrunableByExistence` in the same file marks a worktree prunable when it is not locked, not already prunable, and
its folder gives `ENOENT`. Git's own rule is in `worktree.c` (`worktree_prune_reason`), and its listing passes no expiry
(`builtin/worktree.c`, `expire = TIME_MAX`, in `worktree list`).

Upstream sources, checked at tags `v2.25.0` and `v2.31.0`: the porcelain printer of 2.25 writes `worktree`, `HEAD`,
`detached` and `branch` lines and no `locked` or `prunable` line. 2.31 adds both. So on the 2.25 baseline the core fills
two fields itself:

- `locked`: the reason in each linked worktree's admin `locked` file, read as before (`crates/core-git/src/worktree_locks.rs`).
- `prunable`: when git printed none, the core marks a linked worktree that is not locked and whose folder does not exist
  (`crates/core-git/src/worktree_prune.rs`). The reason is git's wording, `gitdir file points to non-existent location`, so
  the same state reads the same on every version.

### Decision

- **Scope.** `workspace list` and `workspace.list` report every linked worktree of a project that no workspace row records,
  whether it sits under the project's workspaces root or elsewhere. The main worktree is never reported. Listing removes
  nothing.
- **Adoption.** A create at a worktree's path on the same branch adopts it. Adoption is decided by path and branch
  (`adopt_interrupted_worktree`), not by root, so the CLI hint says that and no longer names the workspaces root.
- **No new method.** `workspace.list` already returns `unregistered` with `locked` and `prunable`. Only the set of entries
  grows, so no capability is added and the wire is unchanged.

### Consequences

- On git 2.31 and later the probe adds nothing: git prints the state itself, and the probe only fills a missing `prunable`
  for a folder git would also report.
- The probe checks the folder, as pv96 does. Git checks the worktree's `.git` file. On the 2.25 to 2.30 baseline, a folder
  that exists without its `.git` file is therefore not reported as prunable.
- Worktrees made outside Massorca now appear under `unregistered`, with their lock and prune state. Nothing in this change
  removes them.
