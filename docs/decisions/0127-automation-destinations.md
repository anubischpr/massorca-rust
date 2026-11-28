# 0127: Automation destinations (AUTO-02, first version)

Status: accepted, 2026-11-26. An automation says where its runs work: the project's own place, a new worktree for each
run, or one existing workspace. A run that cannot start there is recorded with the reason.

## Context

Decision 0125 gave every run one worktree kept per automation for a git project, or the folder's workspace for a folder
project, chosen by the project's kind and not by the automation. The inventory (docs/inventory/automations.md, AUTO-02)
asks for an explicit destination: an existing workspace, or a new worktree per run inside a project, with a fixed
sentence for each refusal and folded history rows. Decision 0125 left destinations and the folding of repeated
unavailable rows for this version.

## Decision

- D1 Kinds. An automation's destination is `project_default` (the place decision 0125 chose), `project_worktree_per_run`
  (a new worktree for each run, titled with the automation's name, from an optional base branch) or
  `existing_workspace` (one workspace, a worktree or a folder: every run goes there).
- D2 Store. Migration step `automation destinations`, appended after `automation_runs`: columns `destination` (default
  `project_default`), `destination_workspace_id`, `destination_path` and `base_branch` on `automations`. Rows stored
  before the step read as `project_default`, so their runs keep today's place (pinned by a migration test). The
  workspace is an id with no foreign key: a removed workspace is seen at run time, not by cascade. The path is kept from
  when the destination was set, so a moved folder is noticed.
- D3 Protocol. Capability `automation-destinations/1`, registered by the server. The `destination` object is optional on
  create and update (`automation.update` replaces it when given), and always present on the automation view, with the
  workspace's name while it exists. An absent destination on create is `project_default`. A destination kind this build
  does not know decodes as `Unknown`. An older peer that omits the field keeps the project's own place.
- D4 Checks at create and update. An existing workspace must exist, belong to the automation's project, be ready and
  have its folder present; its id is stored, so an id, a name or a path given in the request resolves to the id.
  A per-run worktree needs a git project. A base branch, when set, must name a commit of that repository (its ref is
  resolved as `git` does for a base, and then checked); an unknown one is refused with
  `automation.base_branch_unknown`, and a blank one with `automation.destination_invalid`. Changing the project of an automation whose
  destination is not `project_default` needs its destination given again, since the old one names the old project.
  Refusals are `automation.destination_invalid`, or the workspace's own `workspace.not_found`.
- D5 Refusals. A run is refused before its row is written, both for a timer occurrence and for `automation.run_now`:
  - `automation.destination_removed`: the workspace is gone or being removed. "The destination workspace was removed, so
    the run did not start."
  - `automation.path_changed`: the workspace's folder is missing or its path differs from the path set at setup. "The
    destination path changed or is missing, so the run did not start."
  - `automation.repository_detached`: a per-run worktree's project folder is missing or no longer a git repository. "The
    repository is no longer attached to this host, so the run did not start."
  - `automation.no_project` (decision 0125): the project was removed. "The automation has no project, so the run did not
    start."
  A refused run is recorded as `skipped_unavailable` with its code and sentence, and the schedule still moves on.
  A workspace the service cannot reach for another reason (a lost connection, an unknown error) is not a refusal: the run
  starts and reports its own error, so loss of contact is never read as the workspace being gone.
- D6 Folding. A refusal that repeats the newest run's refusal of the same automation, with the same code and from the
  same trigger (a timer occurrence or a manual run), folds into that row: its count grows, its time becomes the latest
  occurrence, and no new number is given, so no number is reused. A refusal from the other trigger keeps its own row.
  The same rule folds the repeated `automation.no_project` rows that decision 0125 left as one row each. A run in between,
  or a different code, starts a new row.
- D7 Launch. An existing workspace is used as it is and is not announced as made. A per-run worktree is created through
  `workspace.create` (the core-workspace create path, with its setup and unique names): the automation's name as the
  name, `unique_name` so a taken name becomes `name-2`, the base branch when set, and the request id
  `automation.run-worktree:<run id>`, so a retry finds the worktree of its first try. The project's own place is
  unchanged (decision 0125, D8). Runs of a per-run destination are not removed afterwards (see the gaps).
- D8 Timer and run now. Both call the same refusal check (`features/automations/destination.rs`), in the timer's claim
  and in `automation.run_now`. The check reads the service's own records and the folder on this host, and starts no
  agent. A destination that breaks between the check and the launch is recorded `dispatch_failed` with the workspace's
  error; it is not folded, since the row is already written as dispatching.
- D9 Events and retention are unchanged (decision 0125, D13 and D14).

## CLI

- D10 `massorca automations create` takes `--workspace <WORKSPACE>` (an existing workspace: id, name or path),
  `--new-worktree` (a new worktree for each run), and `--base-branch <BRANCH>` (needs `--new-worktree`). `edit` takes the
  same three, and changes the destination only when one is given. Both refuse the flags on a host without
  `automation-destinations/1`, saying the host is too old.
- D11 Default. A create with no destination flag uses the MassOrca workspace the current directory is in, found with
  `workspace.current` on this computer (not for a remote `--host`). Its project is used when `--project` is not given,
  and `--project`, when given, must match it (the service refuses another). When no workspace resolves and no
  destination flag is given, the create keeps the project's own place when `--project` is given, and otherwise fails
  with `usage.missing_project`.
- D11a Capability gate. The enclosing-workspace default is looked up only when the host offers
  `automation-destinations/1` and the paths are local (`default_allowed`). An older host keeps the old behaviour: create
  needs `--project`, and no destination is sent.
- D12 Output. `show` prints `destination:`; `list` adds a destination column after the id. A workspace that no longer
  exists is shown by its id as "no longer exists".

## Desktop

- D13 Editor. The editor offers three choices: the project's own place, a new worktree per run (with an optional base
  branch), and an existing workspace, chosen from a select of the workspaces the shell already holds. Each label names
  its project, because the project field is free text, so the select lists all projects' workspaces rather than only the
  automation's project; the service refuses a workspace of another project. A stored workspace that no longer exists is
  shown as "(removed)" in the select.
- D13a Edits. An edit sends the destination only when the person changed it or the project (the service needs it again
  then). An untouched destination is omitted, so a stored workspace that is gone does not block a prompt-only edit, and
  an unknown destination kind from a newer peer survives a rename. A new automation always sends one.
- D13b Older hosts. Without `automation-destinations/1` the editor hides the picker behind a note and sends no
  destination, on create and edit alike.
- D14 Page. Each automation's line names its destination beside the project and the next run.

## Not in this version

- Session reuse for existing-workspace automations (AUTO-06) and the `--fresh-session` switch.
- Remote and SSH destinations, host ownership and fencing (AUTO-08, AUTO-09): the destination check runs on this host only.
- Removing the worktrees that per-run destinations leave behind; a per-run destination keeps each run's worktree.
- A select limited to the automation's own project. The project field is free text, so the select cannot follow it
  without a rebuild on every keystroke; the service enforces the project instead (D13).
- The check and the launch are not atomic: a race between them is recorded `dispatch_failed`, not as a refusal (D8).
