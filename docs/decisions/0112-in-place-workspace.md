# 0112: In-place workspace

Status: accepted, 2026-11-26.

## Context

Decision 0008 makes every git workspace a linked worktree with its own row. The main worktree is never adopted, created
or removed. A person who works in a project's own checkout cannot give that checkout a workspace of its own: agent
sessions and terminals there have no row, and a new worktree and branch are the only way in.

## Decision

- `workspace.create` takes `in_place: bool`, `false` by default and omitted from the wire when `false`. It is offered
  under capability `workspace-in-place/1`. The CLI flag is `workspace create <PROJECT> --in-place`, and NAME is optional
  with it.
- With `in_place`, the service registers the project's own checkout as the workspace. The row's path is that checkout,
  its kind is `worktree` for a git project (`folder` for a folder project), and the store column
  `workspaces.in_place` marks it. The column is added by the last migration step with default `0`, so every existing
  workspace is not in place.
- A git project has one in-place workspace. A repeat call returns the same row, and nothing changes. For a folder
  project, `in_place` returns the existing folder workspace at the project path, or registers one there.
- Its name is the project's name unless one is given. Its branch is read from the checkout on each call, because the
  person may switch branches. `created_branch` is `false` and `name_generated` is `false`, so `workspace.rename` never
  renames it.
- `in_place` with `branch`, `base`, `existing_branch`, `refresh_base`, `path`, `unique_name` or `name_generated` is
  refused with `request.invalid_params`. The service never switches the person's checkout.
- No setup script runs for an in-place workspace, because no worktree is made.
- Removal only forgets the row. It never runs `git worktree remove`, never deletes a branch, never touches a file, and
  never runs the archive script (decision 0095). The dirty and ignored checks do not apply, since nothing is deleted.
  The refusals that protect linked worktrees (the main worktree, a locked one, a dirty one) are unchanged for them.

## Consequences

- An older client never asks for `in_place`, and never sees it unless it reads the optional `in_place` field of a
  `Workspace`. The field is omitted when `false`, so the wire is unchanged for every workspace that is not in place.
- A client must not treat the removal of an in-place workspace as the removal of a worktree: the directory and the
  branch stay.
