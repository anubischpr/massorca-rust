# 0126: Team project code source

Status: accepted, 2026-11-26.

## Context

AUTO-33 (team projects: Board, Brief and code source) records where a project's code is, as "Existing repository" or
"New folder with git". Decision 0072 (amendment 2026-11-23) keeps that record only: nothing is cloned, created or
initialised with git, and a task runs where the linked Workspaces project says, never where the code source does.
The owner ruled on 2026-11-26 that this rule stands, so this decision covers the parts of AUTO-33 that do not create
anything.

## Decision

### Clearing a code source

- `CodeSource::None` (wire `none`) clears a project's recorded code source. Clearing needs no Workspaces. Recording a
  kind still does, and is refused with `team.workspaces_off`.
- CLI: `massorca team project update TEAM PROJECT --code-source none` clears it. `add --code-source none` records none.
- An older host reads `none` as an unknown kind and refuses it with `request.invalid_params`, so clearing needs the
  newer host.

### One code source per project

- A project takes a linked Workspaces project or a `new_folder_with_git` kind, never both. `team.project_add` and
  `team.project_update` refuse the combination with `request.invalid_params` when the call sets `source_project` or
  `code_source`. A call that sets neither is not checked, so records written before this decision keep their state.
- `existing_repository` with a link is allowed. The link is the repository the tasks run in.

### Reading one project

- `team.project_show` (`PROJECT_SHOW`, read permission, capability `teams/1`) returns one project by id, then by exact
  name. It needs no Workspaces. An unknown team is `team.not_found`, an unknown project `team.project_not_found`.
- CLI: `massorca team project show TEAM PROJECT` prints the name, the goal, the brief when set, the code source (`-`
  when none), the linked Workspaces project (`-` when none) and the id. `team project list` already prints the code
  source.

### Desktop

- The project page has a "Code source" control with three segments, None, Existing repository and New folder with git,
  placed under the link control and drawn from the recorded code source. With Workspaces off only None can be picked,
  so a recorded source can still be cleared, as the service allows; the two kinds are disabled, and their tooltip and
  the sentence under the control say "Turn on Workspaces for this team in Edit team to connect its projects to code."
- The New project dialog's select offers the same three choices, None first. Its default stays Existing repository.
- No artboard covers the picker (`docs/design/desktop-conformance-audit.md`, line 819 says so). The page control matches
  the link control beside it, a segmented control whose state comes from the model on every draw.

## Not built: folder creation (open owner question)

Option A: the service creates the Git folder for a `new_folder_with_git` project under the team folder, and the
project's tasks run there. It would reverse the recorded-only rule of decision 0072 and need a `git init` in core-git.
It is not built. If the owner accepts it, a follow-up decision records the folder path, its migration and the task
routing.

## Consequences

- Removing a project never deletes files. Nothing creates a folder, so there are none to delete.
- A project that already holds both a link and a new-folder kind keeps both until an update sets one of them.
