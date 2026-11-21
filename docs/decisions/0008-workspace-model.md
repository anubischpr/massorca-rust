# 0008: Projects and workspaces in Phase 1

Status: accepted, 2026-11-20.

## Context

`docs/inventory/workspaces.md` describes projects (a git repository or a plain
folder, bound to one host) and workspaces (a git worktree with a branch, or an
existing folder). Phase 1 must create, list, show and remove both kinds from
the CLI. Much of the inventory (setup scripts, generated names, status board,
lineage) comes later; the model must not block it.

## Decision

- **Project.** `{id, kind: git | folder, path, name, workspaces_root,
  added_at}`. `massorca project add <path>` registers it; a git repository is
  detected with `git rev-parse --show-toplevel`. `workspaces_root` defaults to
  `<user home>/massorca/workspaces/<project name>` and is a project property,
  so a test profile never writes into the real home.
- **Workspace.** `{id, project_id, kind: worktree | folder, path, name,
  branch?, base_ref?, created_branch, state: ready | removing, created_at}`.
  Names are unique within a project.
- **Git is the truth for which worktrees exist.** The store keeps identity and
  metadata. `show` and `list` report a workspace whose directory is gone as
  `missing` instead of hiding it.
- **Create a worktree.** Branch defaults to the sanitized name; base defaults
  to the repository's `HEAD`. An existing local branch is used only with
  `--existing-branch`. Path is `<workspaces_root>/<name>` unless `--path` is
  given. Create carries an optional client request id; repeating the same id
  returns the first result instead of creating twice.
- **Create a folder workspace.** Registers an existing directory (the project
  path by default). No files are created.
- **Remove.** The workspace row is first set to `removing`, then git runs
  `worktree remove` (with `--force` only when the user passes `--force`), then
  the row is deleted. A dirty or locked worktree is refused with a typed code.
  The main worktree is never removed. A branch the workspace created is
  deleted with `git branch -d` (only if git proves it merged); otherwise it is
  kept and the result says so. A folder workspace is forgotten; its directory
  is never deleted.
- **Errors** are typed codes (`workspace.dirty`, `workspace.locked`,
  `workspace.not_found`, `workspace.name_taken`, `project.not_git`, ...), never
  classified from git's message text.

## Deferred (recorded so it is not forgotten)

Branch prefixes, generated names and their retirement, base-ref fallback
order, nested layout options, setup scripts, terminal and agent guards on
removal, squash-merge detection, id aliases after a folder rename, remote
hosts. Each lands with its phase and its own tests.

## Consequences

- Removal is resumable: running `rm` again on a `removing` workspace finishes
  the job.
- Adding the deferred behaviour adds columns and options, not a new model.
