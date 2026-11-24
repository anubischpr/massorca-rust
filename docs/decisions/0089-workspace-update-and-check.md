# 0089: Workspace update and check

Status: accepted, 2026-11-24.

## Context

Folder workspaces could be added, listed, shown and forgotten (CLI-35, WS-44), but not renamed, commented or checked.
pv96 has `folder-workspace update` and `folder-workspace path-status`, which are the reference for these commands.

## Decision

- Capability `workspace-update/1` offers `workspace.update`, permission `workspace`. Params: `workspace` (id, name or
  path), and optional `name` and `comment`. A field left out keeps its value; an empty `comment` clears it. A request
  with neither is refused with `request.invalid_params`. The result is the updated `Workspace`.
- A rename changes the stored name only. The directory and the branch keep the names they were made with. The name
  follows the create rules (`workspace.name_invalid`), and a name another workspace of the project holds is refused
  with `workspace.name_taken`.
- The comment is a short status line in a nullable column, `workspaces.comment`, added by the last migration step. It
  appears on `Workspace` only when set.
- The service publishes `workspace.updated` with the workspace id after an update.
- Capability `workspace-check/1` offers `workspace.check`, permission `read`. The result carries `id`, `name`, `kind`,
  `path`, `exists`, `readable`, `registered`, `ok` and `reason`. `registered` says whether git still lists a worktree;
  it is absent for a folder and when git cannot answer. `reason` names the first check that failed. A failed check is a
  report, not an error, so the command exits 0 with `ok` false. Nothing is changed.
- Pin and archive are not added, because the store has no field for either. Groups and move remain missing (WS-44).

## Consequences

- Both methods have their own capability. A connection that was not offered a capability is refused the method, so a
  client calls them only against a service that offers them.
- `comment` is an optional field of `Workspace`; an older client ignores it.
- The desktop does not act on `workspace.updated` yet, so a rename made from the CLI appears there after the next
  resync.
