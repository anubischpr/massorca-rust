# 0010: Git operations surface

Status: accepted, 2026-11-21.

## Context

Phase 2 adds the working-tree side of git to the service: status, diffs,
staging, discarding, committing, branches, fetch, pull, push and history. The
inventory (`docs/inventory/git.md`) describes what the current product does.
GIT-02, GIT-05, GIT-06, GIT-07, GIT-08, GIT-09 and GIT-19 are first-version
features. GIT-11, GIT-21 and GIT-22 come next. Phase 1 already runs the user's
git binary through `core-git` with no shell, a timeout, a pinned locale and a
process-tree kill (0007, 0008).

Several clients and agents will run these operations on one workspace at the
same time. The same API must later run against a remote execution host
(0015).

## Decision

### One capability, one module per command group

- The capability is `git/1`. Each method takes a `workspace` selector, resolved
  as in 0008: by id, then by exact name, then by path. A folder workspace is
  refused with `git.not_a_repository`.
- Each command group has its own module in each layer:
  - `protocol::methods::git::<group>`;
  - `core-git::<group>`;
  - `server::features::git::<group>`;
  - `client` typed helpers;
  - `cli::git::<group>`.

  The groups are status, diff, log, index (stage, unstage, discard), commit,
  branch, remote (fetch, pull, push) and watch.
- Methods:
  - Status and diffs: `git.status`, `git.diff`, `git.diff_file`.
  - History: `git.log`.
  - Index: `git.stage`, `git.unstage`, `git.discard`.
  - Commits: `git.commit`.
  - Branches: `git.branches`, `git.branch_create`, `git.branch_switch`,
    `git.branch_rename`, `git.branch_delete`.
  - Remotes: `git.fetch`, `git.pull`, `git.push`.
  - In-progress operations: `git.abort`.
  - Notifications: `git.watch`, a stream (0012).
- Reads need the `Read` permission. Everything else needs `Workspace`.
- Each method has a command descriptor in the core table. `massorca git <verb>`
  maps one to one onto the methods.

### Paths on the wire

- Paths inside a workspace travel as UTF-8, relative to the workspace root,
  with `/` separators, exactly as git prints them with `core.quotepath=off`.
- A request path that is absolute, contains a `..` segment, or contains a NUL
  is refused with `git.path_outside_worktree` before git runs.
- Paths reach git as literal pathspecs (`--literal-pathspecs`), so glob
  characters in file names match only themselves.
- A path git reports as non-UTF-8 is still listed. It is marked
  `path_lossy: true`, and the methods that take a path refuse it.

### Status

- The server runs
  `git status --porcelain=v2 -z --branch --untracked-files=all` and
  `git diff --numstat -z` (worktree and `--cached`) to get line counts.
- The result lists:
  - each entry as staged, unstaged, untracked or conflicted, with its
    porcelain codes and line counts;
  - the branch, its upstream, and how far it is ahead of and behind the
    upstream;
  - the operation in progress: merge, rebase, cherry-pick or revert.
- The operation in progress is read from git's state files: `MERGE_HEAD`,
  `CHERRY_PICK_HEAD`, `REVERT_HEAD`, and the `rebase-merge` and `rebase-apply`
  directories. It is never read from `REBASE_HEAD` alone, which can linger.
- Every read runs with `GIT_OPTIONAL_LOCKS=0`. A read then never rewrites the
  index, which would wake every watcher (0012).

### Concurrency

- Mutations on one workspace hold a per-workspace operation lock in the
  service, so two clients cannot interleave a stage and a commit. Reads do not
  take the lock.
- Git's own `index.lock` remains the backstop for writers outside the service,
  such as a terminal or an agent. When git reports the lock taken, the result
  is `git.index_locked`.

### Error classification

0008 allowed reading git's message text in one place only. Mutations and
remote operations need more, because git reports a rejected push, a
non-fast-forward pull or an overwrite refusal only in text.

- Classification reads stderr under `LC_ALL=C`.
- Each pattern lives in one table in `core-git::classify`, and each pattern
  has a test that runs real git and produces it.
- Text that matches no pattern is not guessed at. It becomes `git.failed`,
  carrying the redacted stderr (0013).

### Not offered

- Hunk staging and stash: the inventory lists both as absent.
- Interactive rebase, cherry-pick, reset to a commit, tags, blame and
  submodule actions: absent in the inventory.
- `--no-verify` and signing options. Hooks always run.
- Provider behaviour (pull requests, forks, web links): a later phase, outside
  `git/1`.

## Consequences

- Every git feature is reachable from the CLI, so it can be tested and
  debugged end to end without a desktop client.
- Terminals and the network server can grow in parallel. Shared files (the
  feature lists, the command table, the CLI argument enum) only gain lines.
