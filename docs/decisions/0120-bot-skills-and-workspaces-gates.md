# 0120: Bot skills and the team Workspaces gates

Status: accepted, 2026-11-25.

## Context

AUTO-12 (bot team roster, editing and team access) had two gaps. The Add bot dialog (D-AG-33, screens 4.5) has a
Skills field, "chips with a picker; none by default", but a bot had no skills. And a team's Workspaces option had one
reader, the team folder in Workspaces. In pv96 the option also gates connecting a project to code, the project's
memory from Engram, and the edit rights over a project's code. This decision gives a bot skills and gates the
equivalents that massorca-rust has.

## Decision

### Bot skills

- A bot keeps an ordered list of skill names, as pv96 does. The list is stored in a new table `bot_skills (bot_id,
  position, skill)`, added as a new migration step. It is keyed by bot and position, a name appears once per bot, and
  removing the bot removes its skills.
- `Bot` has an optional `skills` field, left out when it is empty. `team.bot_add` takes an optional `skills` list.
  `team.bot_update` takes an optional `skills` list that replaces the whole list; an empty list clears it, and leaving
  it out keeps the list. Names are trimmed, and blanks and repeats are dropped. More than 50 (`MAX_BOT_SKILLS`) is
  refused with `request.invalid_params`. An older host ignores the field, and an older client reads a bot without it.
- The CLI takes `--skills a,b` on `team bot add` and `team bot update`; on update it replaces the list, and
  `--skills ""` clears it. `team bot list` adds a last column with the skills joined by commas, or `-`. The add, update
  and remove lines end with "with skills ..." when the bot has skills.
- The desktop Add and Edit bot forms show the chosen skills as chips, each with a remove button. Below them is a picker
  of the skills this computer's service discovered (`skills.discover`, the list behind Marketplace > Skills) that are
  not chosen yet. Skills are not checked against the discovery, so a stored name that is no longer discovered stays as
  a chip. The Edit bot form sends the list only when it changed.

### Workspaces gates

The host enforces each gate and refuses with the new code `team.workspaces_off`. The message names the team and the
command that turns the option on. The gates only narrow what a call may do.

| pv96 gate | massorca-rust equivalent | Enforced |
| --- | --- | --- |
| Connect to code: the host refuses to list or link a Workspaces project, and the code-source choice is hidden | `team.project_add` and `team.project_update` refuse a non-blank `source_project` or a `code_source` | service: `features/teams/team_project.rs`; desktop: the project page's link control is off with the reason, and New project already hides the code source; CLI: prints the refusal |
| Project edit rights: only the owning team, with Workspaces on, edits a project's code link | the same check on `team.project_update`; a project belongs to one team | as above |
| Project memory from Engram | none: team memory has only team and bot scopes, and nothing reads Engram | not built |

- Clearing a link (`source_project: ""`) stays open with Workspaces off, so a team that turned the option off can still
  drop a link it made earlier. Name, goal and brief edits are not gated.
- `code_source` was already refused without Workspaces, but with `request.invalid_params`; it now uses
  `team.workspaces_off`.
- pv96 does not gate where a task runs. A task of a project that kept a link from before still runs in the linked code.

## Not built

- Project memory from Engram. There is no Engram integration and no project-scoped team memory to gate.
- A per-host skill picker: the picker lists this computer's service's skills, which is the host the teams page reads.
