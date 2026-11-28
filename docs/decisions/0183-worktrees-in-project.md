# 0183: Worktrees inside the project

Status: accepted, 2026-11-27.

## Context

Decision 0008 puts a project's worktrees under `workspaces.default_root` (`<user home>/massorca/workspaces`), in a
folder named after the project (`nested`) or directly in the root (`flat`). The owner wants them inside the project
instead, so they sit beside the code they come from and leave the home folder alone. This decision amends the Layout
paragraph of 0008.

## Decision

- **Layout.** `workspaces.layout` takes a third value, `in_project`, and it becomes the default. A git project added
  with it records `<project>/.worktrees` as its workspaces root, so a workspace is `<project>/.worktrees/<name>`.
  `nested` and `flat` stay as 0008 defines them.
- **What moves.** Nothing. The layout is read when a project is added (or when a folder project becomes a git project,
  decision 0115), and every project stores its root, so existing projects keep the root they were added with. The
  per-project `workspaces_root` still wins over every layout. No store change is needed.
- **The project's folder.** It is the main worktree the project registers at, so a project added from a linked worktree
  takes its main repository's `.worktrees`. A linked worktree of a bare repository is still refused (0008), so a bare
  repository has no in-project root.
- **Folder projects.** A folder project has no worktrees; under `in_project` it records the nested root, as before, and
  no `.worktrees` folder is made. If it is later turned into a git project, it takes `<folder>/.worktrees`.
- **Kept out of git.** When a worktree is created inside `<project>/.worktrees`, `/.worktrees/` is appended once to
  `<common git dir>/info/exclude`, which is local to the clone and never committed, and which linked worktrees share.
  The repository's `.gitignore` is never edited. The append is one function (`exclude_worktrees_folder` in core-git),
  so switching to `.gitignore` later is a change in one place.
- **Created lazily.** Adding a project creates nothing. `.worktrees` and the exclude line appear with the first
  worktree; removing the last worktree leaves both.
- **Windows path limit.** On Windows, a create whose worktree path inside `.worktrees` is longer than 260 characters is
  refused with `workspace.path_too_long` before git runs. The message names the path and says to set
  `workspaces.default_root` and `workspaces.layout`, or give the project a shorter `workspaces_root`. The check is a pure
  function of the path and a limit, so it is tested on every platform.
- **Our walkers skip it.** Project icon detection never reads a file inside `.worktrees`. Git views (`git status`,
  untracked listings) skip it through the exclude line.

## Consequences

- A project added after this change, with no setting stored, has its worktrees inside it. A person who wants the old
  place sets `workspaces.layout` to `nested`.
- The working-tree watcher of the main worktree (core-watch) still reports changes under `.worktrees` as working-tree
  changes of the main checkout; skipping that folder there is left to the crate that owns the watcher.
- `massorca project show` already prints the workspaces root, so the in-project root is visible there and in its JSON.
