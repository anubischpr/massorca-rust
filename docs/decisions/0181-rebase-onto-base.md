# 0181: Rebase onto base

Status: accepted, 2026-11-27.

## Context

A worktree's branch falls behind the base its workspace was created from, and until now the only way to bring it up to
date was a rebase run by hand in a terminal. `git.pull --mode rebase` rebases onto the branch's upstream instead, which
is a different commit once the branch has been pushed or when it has none. The desktop and the CLI need one operation
that rebases a worktree onto its base, with the refusals and the conflict handling the other git mutations already have.

## Decision

- Method `git.rebase_base { workspace, refresh? }`, capability `git-rebase-base/1`. Its result is
  `{ outcome: rebased { head } | up_to_date | conflict { paths } }`; an outcome a newer peer adds decodes as `unknown`.
- The base is resolved as `workspace create` resolves it: the workspace's recorded base, else the project's, else the
  repository default. The create path and the rebase share one resolver (`Git::base_or_default`).
- `refresh` fetches the base's remote-tracking branch first, with the create path's fetch helper and its 30-second
  bound. A failed fetch refuses the rebase, so nothing is rebased onto a copy the person asked to refresh.
- Refused before git runs: a folder workspace, a workspace being removed, an in-place checkout (`in_place`, decision
  0112) and the main worktree, a detached HEAD, a merge or rebase already in progress, and local changes. The rebase
  runs under the workspace's mutation lock.
- The rebase is `git rebase <base>`, which Git 2.25 supports. A base already in the branch's history is up to date and
  nothing runs. A rebase that stops on conflicts is aborted here, so the worktree is as it was; the result lists the
  conflicted paths.
- No new notification is published. `git.watch` reports the HEAD and ref changes a rebase makes, as it does for every
  other git command that moves them.
- CLI: `massorca git rebase-base -w <WORKSPACE> [--refresh]`. A conflict exits 1 with `git.conflicts`, as `git pull`
  does for its conflicts.

## Consequences

- No schema change: the base comes from rows that already exist.
- A rebase rewrites the branch's commits, so a branch that was already pushed needs a forced push afterwards. The
  method never forces anything; `git.push` forces only with a lease the person names.
- The abort on conflict is part of the method, so a caller never has to run `git.abort` after a conflict outcome.
