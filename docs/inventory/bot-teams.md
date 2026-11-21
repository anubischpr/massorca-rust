# Bot Teams: reading guide

Status: draft for owner review, 2026-11-12.
Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits).

Bot Teams is a core mode of the new product, next to Workspaces mode. The owner decided this on 2026-11-12 in the design session (see `docs/design/screen-inventory.md`, rows D-SH-10, D-WS-29, D-WS-30, D-AG-13, D-AG-24, D-AG-25, X-DL-01 and answered open questions 6, 7 and 8).

This page gathers the Bot Teams features in one place. The features keep their IDs in [automations.md](automations.md) (prefix `AUTO`), because Bot Teams shares the delegation engine with Workspaces mode and, by the owner's direction, the schedule engine. Each feature there separates the behaviour observed in pv96 from the owner's direction for the new product, and a `Source: pv96 (unmerged)` line marks what pv96 changes.

## What the owner decided

- **A core mode.** One button in the activity rail switches between Workspaces and Bot Teams. In Bot Teams mode the sidebar lists the teams. Each team has a Conference room and a Board, and a Projects section with Files, Memory and Routines.
- **The Conference room.** A chat with the team's bots. It detects the requirements the user states and creates tasks on the team's Board. A composite requirement goes to the lead bot, which delegates the parts by competence. A request to schedule a task for later becomes a routine.
- **The Board.** The team's tasks. It is the Bot Teams source of tasks, and the Tasks page on the activity rail lists Board tasks next to tracker items.
- **Delegation.** One core engine serves both modes. In Bot Teams the lead bot delegates by competence; in Workspaces a coordinator coding agent launches child agents. Child agents show as a tree inside the Conference room or the parent session.
- **Where a delegated child runs (2026-11-12).** (1) a delegated child session gets its own worktree by default; (2) it may use the current worktree only when the child only reads (research, review, explain), when the workspace is a plain folder without git, or when the user picks it by hand on the delegation card; (3) two sessions never write in the same worktree at the same time, so if a child writes in the current worktree the parent waits until it finishes (sequential delegation, not parallel); (4) parallel launches always give each child its own worktree, with no option. In Bot Teams these rules meet the team folder, which may not be a git repository: there, writing children run one at a time. Whether that is acceptable for a lead bot that delegates several parts is an open question (open-questions.md).
- **Routines.** The simplified Bot Teams form of an automation, on the same schedule engine as Workspaces automations. The Calendar on the activity rail shows both.
- **Workspaces in a team.** A team can turn on Workspaces to reach Workspaces depth inside Bot Teams.

## How pv96 differs from the owner's direction

pv96 builds most of the Bot Teams mode (unmerged). The points below are where it differs from the owner's direction, or where it still behaves as custom/integration did. Each one links to the feature that holds the detail.

- **Bots reply in the room.** The custom/integration app never posted bot messages. pv96 posts a greeting from All, one bot's answer to a question, finish reports, and a typing line while a bot works. (AUTO-13)
- **Requests are classified by one host query, and there is no lead role.** A message with `@Name` of a bot or a team skips the query. Otherwise one query returns a bot task, a composite task, a task for another team, no team, or a conversation, and an unclear request gets a choice card. The owner asked for a lead bot chosen per request; pv96 has no lead. The least busy matching bot owns a task, and the bot that takes a composite request hands the other parts to their bots. The routing query runs on the Settings default agent, not on Jev. (AUTO-13, AUTO-20)
- **Tasks have a done state.** Statuses are Not started, Working, Waiting for you, Stopped, Stuck and Done, shown in Todo, In Progress, In Review and Done. Stopped is a status of its own. A finish the host never reports is not seen by a later answer in the room. (AUTO-14)
- **`workspacesEnabled` has readers.** While a team's Workspaces option is on, its folder shows in the Workspaces list, and Connect to code, project memory from Engram and project edit rights depend on it. Depth inside a team is still partial: no Versions on the phone, no bundled Git, and deliverables from linked worktrees are partial. (AUTO-12, AUTO-35, AUTO-36)
- **Routines run on a separate scheduler.** Desktop and phone have routines (daily, weekly, monthly or once), on the creator's time zone, with pause and run history. The owner asked for one scheduling engine shared with Automations; pv96 has a second scheduler that reuses only the occurrence math. The Calendar shows both kinds of run. (AUTO-28, AUTO-30)
- **Web address calls reach only this computer.** The endpoint is a loopback listener with one key per address and a limit of 6 calls a minute. Exposure beyond this computer and the hardening under Settings are not built. (AUTO-29)
- **Team files and memory exist in part.** Files list and open as read-only text. Memory holds team and per-bot lines that bots read when a task starts, and Engram memory for linked projects. The owner has not defined what memory holds. (AUTO-31, AUTO-36)
- **Integrations is still a placeholder.** The Settings pane lists four apps, Connect is off, and the pane says connecting apps is not available yet. (AUTO-12)
- **Child work comes back by merge.** A finished delegated child's commits can be merged into the parent worktree, under ask, auto and full policies. Bot Teams passes full. (AUTO-32)

## Features

| ID | Feature | Proposed | Notes |
|---|---|---|---|
| AUTO-12 | Bot team roster and Bot Teams mode | first version | Teams and bots, editable after creation; per-team access (full access by default); bot model and effort picked from the job unless set; Workspaces option shows the team folder |
| AUTO-13 | Conference room: requirements become Board tasks | first version | Bots reply; one query routes each request; choice cards; no lead role; `@` menu for bots, teams, projects and files |
| AUTO-14 | Team Board, bot tasks and background sessions | first version | Statuses Not started, Working, Waiting for you, Stopped, Stuck and Done; Todo, In Progress, In Review and Done columns; reassign is AUTO-34 |
| AUTO-28 | Routines | first version | Built on desktop and phone with its own scheduler, not the Automations engine; one-time routines; creator's time zone |
| AUTO-29 | Web address (webhook) triggers | later | Loopback endpoint with a per-address key; reachable from this computer only |
| AUTO-30 | Calendar of routines and automations | first version | Built: one page reads both kinds of run; done runs hidden by default |
| AUTO-31 | Team files and team memory | later | Read-only file list; team and per-bot memory lines; Engram memory for linked projects; versions are AUTO-36 |
| AUTO-33 | Team projects: Board, Brief and code source | first version | New in pv96; the code source is recorded only, nothing is cloned |
| AUTO-34 | Move a waiting task to another bot or team | later | New in pv96; any bot of any team |
| AUTO-35 | Task deliverables returned to the asking room | later | New in pv96; the owner's return card is not built |
| AUTO-36 | File versions of team folders | later | New in pv96; desktop only |
| AUTO-37 | Room answers with web research or code reading | later | New in pv96; depends on each agent's search and read modes |

Shared engines that Bot Teams depends on:

| ID | Feature | Role in Bot Teams |
|---|---|---|
| AUTO-15 to AUTO-18 | Orchestration Runs, task dispatch, worker lifecycle, agent messages | Bot tasks and team delegation run on them. Schema 61 holds the bot team tables. |
| AUTO-20, AUTO-21 | Delegation with agent choice; depth limit | A bot hands work to a teammate with no card (`team-delegate`); an optional Jev advisor can pick the tier; the depth limit is unchanged |
| AUTO-32 | Bring back child session work | Brings a finished child's commits into the parent worktree; Bot Teams passes full |
| AUTO-01 to AUTO-04, AUTO-06 to AUTO-08 | Schedule engine and runs | Routines do not run on them in pv96; they reuse only the occurrence math (AUTO-28) |
| AG-15, AG-17 | Structured chat platform; Claude adapter | The Conference room is a chat; current bot tasks already run as structured chat sessions |
| SCM-22 | Unified task list with source filter | The Tasks page lists Board tasks |
| SET-29 | Secret vault | Secrets can belong to a team scope |

## Not decided

- Whether writing bot children may run in parallel in a team folder that is not a git repository; the child placement rules allow only one at a time there (open-questions.md, blocking item 27).

- What team memory holds (AUTO-31). pv96 stores team and per-bot lines and lets bots write them with `team-remember`; the content rules are not defined.
- Whether web address calls may come from beyond this computer, and which hardening is on by default (AUTO-29).
- Whether a team needs a lead bot. pv96 has none; the least busy matching bot owns a task (AUTO-13). The owner's direction asked for a lead bot, and Jev is not wired into the routing query in pv96.
- Whether routines move onto the Automations engine (AUTO-28).
- Android: pv96 has phone screens for teams, the room, routines and web addresses, none of them run on a device. The delegation tree and the Calendar placement are not designed for Android.

## Read for this pass

- Odd docs: `odd/tasks/bot-teams.md`, `odd/tasks/bot-teams-pv91-qa.md`, `odd/tasks/child-bring-back.md` (all read), and the orchestration parts of `odd/tasks/delegated-child-sessions.md`.
- Code at `d3c6311a531`, read only: `src/main/bot-teams/`, `src/main/runtime/orchestration/db/bot-teams/` and `db/delegation/`, `src/main/runtime/rpc/methods/bot-team*.ts`, `src/main/delegated-session/`, `src/shared/bot-team-*.ts`, `src/shared/child-bring-back.ts`, `src/cli/specs/orchestration-worker-specs.ts`, `src/cli/handlers/bot-teams-admin.ts`, the Calendar sources and gating lines in `src/renderer/src/components/bot-teams/`, `src/renderer/src/components/settings/BotTeamsIntegrations.tsx`, and `mobile/src/bot-teams/` (names only).
- Checked and unchanged: `src/main/automations/`, `src/shared/automation*`, the automation RPC and CLI.

## Not read / low confidence

- Not read: most RPC handler bodies, the orchestration migrations (names only), the Engram sync, the Marketplace apps service, the Jev decision client, the bring-back git flow beyond the merge call, and most renderer and phone components.
- Low confidence: the deliverable limits (10 files, 25 MiB) and the version size limits come from the odd docs and were not traced in code. Whether the standalone host starts routines was not verified.
- No real agent run was checked in this pass. Live results in the odd docs were not re-run.
