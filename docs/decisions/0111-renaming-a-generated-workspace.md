# 0111: Renaming a generated workspace

Status: accepted, 2026-11-25.

## Context

pv96 starts a workspace under a placeholder name and renames its branch once the first agent work says what the
workspace is for. MassOrca names a workspace with a blank name after a creature (decision 0008), and the desktop
derives a name from the prompt. Neither name is one a person chose, but nothing recorded that, so nothing could tell a
placeholder from a typed name. This slice builds the service primitive only.

## Decision

- The store records whether the service chose a workspace's name: `workspaces.name_generated`, `0` or `1`, added by
  the last migration step with default `0`. Workspaces made before the step are therefore never renamed.
- Only two things set it at create: a blank name (a creature name), or `name_generated: true` on
  `workspace.create`, which a client sends for a name it derived itself (the desktop's prompt-derived names; the CLI's
  `--name-generated` for scripts and tests). That param is offered under `workspace-rename/1`. Neither applies with
  `branch` or `existing_branch`: that branch is the person's. `unique_name` alone never sets it, since a typed
  `--unique` name is still the person's. Conversations, delegation and Bot Teams workspaces stay `0`.
- `workspace.update` with a `name` clears it: the name is then a person's. `Workspace` carries `name_generated` only
  when it holds; `workspace show` prints it.
- Capability `workspace-rename/1` offers `workspace.rename`, permission `workspace`. Params: `workspace` (id, name or
  path), `name`, `from_generated` (must be `true`, else `request.invalid_params`), and optional `expected_branch`. The
  result is the updated `Workspace`, and the service publishes `workspace.updated` with its id.
- Under the mutation lock that create, update and remove hold, the call re-reads the row and refuses, in order:
  - a name a person gave: `workspace.name_not_generated`;
  - a workspace being removed: `workspace.busy`;
  - for a worktree, a detached HEAD: `git.detached_head`;
  - a worktree whose checked-out branch is not the recorded one, or not `expected_branch`: `workspace.branch_changed`;
  - a branch with an upstream configured, or a remote-tracking copy under any remote: `workspace.branch_published`. A
    git failure while checking is an error, never a pass.
- The new name follows the create rules and takes the first free of `name`, `name-2` .. `name-100`, from the same
  search a unique create uses: workspace names of the project and local and remote-tracking branches, case folded.
  The workspace's own name and local branch do not count, so a case-only change gets no suffix, and folders are not
  checked, because the folder is not moved. The branch checks run first; a name that resolves to the current one then
  changes nothing.
- A worktree's branch is renamed with `git branch -m` to the new name under the branch prefix, as at create; a
  case-only change goes through a temporary name, because a case-insensitive filesystem sees the two refs as one. The
  row's name and branch are then written together. If that write fails, the branch is renamed back. The flag stays
  `1`: the new name is still the service's.
- A folder workspace changes its name only.
- The worktree folder is never moved. Windows locks a running agent's working directory, and pv96 leaves the folder
  too. Running terminals and agents therefore do not block a rename.

## Consequences

- The lock serialises the service's own calls only. Git run outside the service (a push, an agent's commit or
  checkout) can change the branch between the checks and the rename; the checks narrow that window but do not close
  it.
- A crash between the branch rename and the row write leaves the row on the old branch. Later renames then refuse
  with `workspace.branch_changed` until the row is fixed by hand.

- Only service-chosen names are ever renamed, and only while the branch is still private to this computer.
- `name_generated`, the new method and the new codes are optional additions; an older client ignores them.
- The trigger and the name generation are a later slice: after the first agent work, a one-shot
  `Agents::generate_title` from the agent session produces the text, and the service calls this primitive with
  `expected_branch` set to the branch it saw.
- `massorca workspace rename <WORKSPACE> <NEW_NAME> --from-generated [--expected-branch B]` is the debug entry point.
