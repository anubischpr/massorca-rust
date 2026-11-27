# 0115: Initialize git on a folder project

Status: accepted, 2026-11-25.

## Context

The desktop's "No Git" dialog needs an Initialize Git action for a folder project (WS-32, GIT-04). The Electron app
(pv96, commit `d3c6311a531`) has the same action as `repo.initGit`, implemented in
`src/main/runtime/runtime-folder-git-initialization.ts`. Its behaviour, read on that commit:

- `git init` with git's own default branch name (no `-b`, no `init.defaultBranch` override), then
  `git commit --allow-empty -m "Initial commit"`. No `.gitignore` is written.
- When the commit reports that no identity is configured, it retries once with the fixed identity
  `-c user.name=Orca -c user.email=orca@localhost`. A commit that still fails removes the `.git` directory it created, so
  the folder is left as it was.
- It refuses a project that has folder workspaces (`has_folder_workspaces`), and reuses an existing `.git` instead of
  running `git init`.

A worktree branches from a commit, so a repository without one cannot host workspaces.

## Decision

- D1 Method and surface. `project.init_git` takes `{ project }`, selected as `project.show` selects, and returns the
  updated Project. It is offered under capability `project-init-git/1` and needs `workspace` permission. It publishes
  `project.changed`. CLI: `massorca project init-git <PROJECT>`.
- D2 Steps, under the mutation lock that add, update and remove already hold. A project that is already git is refused
  with `project.already_git`. So is a folder that has a `.git` entry, and a folder that git reports as inside a
  repository (`git rev-parse --show-toplevel` succeeds, which covers a folder that is itself a repository). Then
  `git init`, then the empty first commit `Initial commit`. The commit uses the configured identity; when git reports no
  identity, it retries once with the fixed identity `user.name=MassOrca`, `user.email=massorca@localhost`. The name
  differs from pv96's `Orca` because this product is MassOrca. Last, the store row becomes kind `git` with the
  workspaces root that `project.add` would give a project of that name (one helper, so the layout and the default root
  match).
- D3 Rollback. When a step fails, the `.git` directory this call created is removed, and nothing else is touched. Git
  writes some object files read-only, which Windows will not delete, so their flag is cleared first.
- D4 Folder workspaces stay as they are: listed, shown and removable, with their paths unchanged. pv96 refuses such a
  project; this decision does not, because the request was that existing folder workspaces keep working.
- D5 Refusals. Git missing or too old uses the existing `git.unavailable` and `git.too_old` codes. Git refusing the init
  or the first commit uses `git.failed`, with git's message, after the rollback in D3. A folder whose directory is gone
  uses `path.not_found`. An unknown project uses `project.not_found`.
- D6 Concurrency. Two calls cannot both initialize one folder: the second waits for the lock and then finds a git
  project.
- D7 Workspaces root. The project gets the default root of a new repository of its name, even when `project.add` gave
  the folder project another root.

## Consequences

- The first commit is empty and holds no files, so untracked files stay untracked, and worktrees can branch from HEAD.
- The branch is git's default name for a new repository (`init.defaultBranch`, otherwise git's own default).
- A `.git` entry that git does not accept stays a refusal (`project.already_git`) until someone removes it by hand. This
  happens only when the rollback could not delete it.
- The first commit runs the user's hooks and signing settings, as pv96 does. A commit that fails for another reason
  rolls the init back, so the folder stays a folder and the refusal carries git's message.
