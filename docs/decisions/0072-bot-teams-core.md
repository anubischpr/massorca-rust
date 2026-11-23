# 0072: Bot Teams core

Status: accepted, 2026-11-22.

## Context

Bot Teams is a core mode next to Workspaces (inventory `bot-teams.md`). The
desktop draws the sidebar, the Conference room and the Board (screens 4.1 to
4.3) from sample data. The service has none of it.

The first-version items that need the service are:

- **AUTO-12:** teams and their bots;
- **AUTO-14:** the Board, its tasks and the agent session that runs each task;
- **AUTO-33:** team projects, which have their own Board and Brief.

The agents domain (decisions on the `phase4/*` branches) owns agent sessions
and their status store. Its activity states are Working, Waiting, Idle and
Done, and a Done carries an outcome: success, failure, cancelled or
interrupted. Bot Teams must reuse that store, not keep a second one.

The documents disagree on the Board's columns:

- the inventory, describing pv96: Todo, In Progress, In Review and Done;
- the screen spec and the desktop: Open, Running, Waiting for you and Done.

The screen spec is the newer source, so it wins.

## Decision

### Entities

- **Team:**
  - id, name (unique) and purpose;
  - character, one of six shapes (cosmetic);
  - access, which is `full` by default;
  - `workspaces`, a flag;
  - its folder workspace, created on the first task start;
  - created time.
- **Bot:** id, team, name (unique within the team), job (its competence), agent
  kind, an optional model and effort, and a `lead` flag. At most one bot per team
  is the lead. Whether a team must have a lead is still open (inventory, "Not
  decided"), so a team may have none.
- **Project:** id, team, name (unique within the team), goal and brief. Its
  code source is out of scope here.
- **Task:**
  - id, team, an optional project, title and body;
  - an optional bot;
  - origin: `room` or `cli`;
  - `session`: the agent session id, set only after the prompt was delivered;
  - `outcome`: the latched final state;
  - `error`: why the last start failed;
  - created and updated times.

### Task state comes from the agent session

A task stores only its own lifecycle, never a copy of the agent's activity.

| Task | Session | Shown as | Column |
|---|---|---|---|
| not started | none | Not started | Open |
| start failed | none, `error` set | Not started, with the error | Open |
| started | no activity reported yet | Working | Running |
| started | Working | Working | Running |
| started | Waiting | Waiting for you | Waiting for you |
| started | Idle | Waiting for you | Waiting for you |
| started | Done, success | Done | Done |
| started | Done, failure | Stuck | Waiting for you |
| started | Done, cancelled or interrupted | Stopped | Waiting for you |
| started | execution exited, no Done | Stopped | Waiting for you |
| started | execution unverifiable | the last state shown, marked "Last known state" | unchanged |
| any | session forgotten | the latched `outcome` | per the outcome |

The session is read in process through `massorca_core_agents::Agents::show`
and started with `Agents::start` (with a `request_id`, so a retried start is
idempotent). Changes arrive as `agent.changed` on the event bus. Bot Teams
never installs the agents domain's single listener. An unverifiable execution
means "cannot tell", never "dead" (AGENTS.md, SSH use case).

- When the agents domain publishes `agent.changed` for a task's session and
  the session is Done, the task latches that outcome in `outcome`. The Board
  then keeps Done after the session is forgotten. This is the task's own
  result, not a second status store.
- "Waiting for you" collects everything that needs the owner. Stuck and
  Stopped show their own word in the card's status line.
- `task.start` runs as follows:
  1. It refuses a task that already has a session.
  2. It creates the team's folder workspace if there is none.
  3. It starts the bot's agent through the agents domain's in-process API,
     passing the workspace, the agent kind and the prompt. The prompt gives the
     bot its name, its team and its job, then the task text.
  4. It records the session only once delivery succeeded. A failed delivery
     leaves the task Not started, with `error` set (AUTO-14).
- `task.done` marks a task done by hand, for work that finished outside an
  agent.
- `task.reopen` clears `outcome`, `error` and the session link, so the task
  returns to Open and can be started again. It refuses while the linked session
  is still running.

### Methods and events

The capability is `teams/1`.

- **Teams:** `team.create`, `team.list`, `team.show`, `team.update` and
  `team.rm`. Removing a team that has a running task is refused unless `force`
  is given. Removing a team removes its bots, projects and tasks, and leaves
  agent sessions to the agents domain.
- **Bots:** `team.bot_list`, `team.bot_add`, `team.bot_update` and
  `team.bot_rm`. A bot that owns open tasks is refused unless `unassign` is
  given.
- **Projects:** `team.project_list`, `team.project_add`,
  `team.project_update` and `team.project_rm`.
- The two list methods take `{ team }` and return one team's bots or projects
  in the order they were added.
- **Tasks:** `task.create`, `task.list` (filter by team, project, bot or
  column), `task.show`, `task.update` (title, body, bot, project), `task.start`,
  `task.done`, `task.reopen` and `task.rm`.
- **The Board:** `team.board`, given `{ team, project? }`. It returns the four
  columns in order, each with its cards: task id, title, bot, the state word and
  the time.
- **Events:**
  - `team.changed`, with the team id, for a team, bot or project change;
  - `task.changed`, with the task id, when a task is created, updated, started,
    finished, done, reopened or removed. The service also publishes it on every
    `agent.changed` of a linked session, which can change the task's state or
    column.

### CLI

```
massorca team create <name> [--purpose ...] [--character ...]
massorca team list | show <team> | rm <team> [--force]
massorca team bot list <team> | add <team> <name> --agent <kind> --job ... [--lead]
massorca team bot rm <team> <bot> [--unassign]
massorca team project list <team> | add <team> <name> [--goal ...]
massorca team board <team> [--project <project>]
massorca task create <team> <title> [--bot <bot>] [--project <project>] [--body ...]
massorca task list [--team ...] [--column ...] | show <task>
massorca task start <task> | done <task> | reopen <task> | rm <task>
```

## Out of scope

- Conference room routing (AUTO-13): it classifies requests and creates tasks
  from them.
- Routines (AUTO-28) and the Calendar (AUTO-30).
- Team files and memory (AUTO-31).
- Moving a task to another team (AUTO-34).
- Deliverables (AUTO-35).

These later items call `task.create` and the methods above.

## Consequences

- The Board's state cannot drift from the agent, because it is read from the
  agent session.
- `task.start` depends on the agents domain landing on main. Teams, bots,
  projects, tasks and the Board land first, and `task.start` follows in its
  own slice once agent sessions are on main.

## Amendment 2026-11-22: Team memory

A memory line belongs to a team. It is in the team scope, or in one bot's scope
of that team. A line written during a task names the task and has the source
`task`. A line the owner wrote has the source `person`. Each line keeps its
creation time.

A line is at most 500 characters. Surrounding spaces are removed first, and a
blank line is refused. The count is in Unicode scalar values. The owner lists
and removes lines. Bots and the owner add them.

A task start reads 40 lines per scope: the newest 40 team-scope lines, and the
newest 40 lines of the bot that starts the task. The store provides this read
as `memory_for_start`. Task start does not read it yet. The next PR, which adds
`task.start`, puts these lines in the bot's prompt.

Methods:

- `team.memory_list` takes `{ team, bot? }`. Without `bot` it lists every line
  of the team. With `bot` it lists only that bot's lines. Newest first.
- `team.memory_add` takes `{ team, bot?, text, task? }`. A given `task` makes
  the line a `task` line. The task must belong to the team.
- `team.memory_rm` takes `{ team, line }` and returns the removed line.

Adds and removes publish `team.changed` with the team id. Refusals use
`request.invalid_params` for a blank or long text, `team.not_found`,
`bot.not_found`, `task.not_found`, and `team.memory_not_found`.

CLI:

```
massorca team memory list <team> [--bot <bot>]
massorca team memory add <team> <text> [--bot <bot>] [--task <task>]
massorca team memory rm <team> <line>
```

Team files and the Engram link stay out of this change (AUTO-31).

## Amendment 2026-11-22: a team project's source

A team project can now name the Workspaces project its code comes from. This
amends the "code source" that the core decision left out of scope.

- **The link.** `source_project` holds the Workspaces project id. While that
  project exists, the wire also returns `source_project_name` and `source_kind`
  (`git` or `folder`). All three are left out when no project is linked.
- **How it is set.** `team.project_add` and `team.project_update` take an
  optional `source_project`: a Workspaces project id, exact name or path. The
  Workspaces domain resolves it. An unknown project is refused with
  `project.not_found`, and nothing changes.
- **How it is cleared.** `team.project_update` with an empty `source_project`
  clears the link. Leaving the field out keeps the link.
- **When the Workspaces project is removed.** The link is cleared. The column
  is a foreign key with `ON DELETE SET NULL`, so no dangling id is kept. The
  team project keeps its id, name, goal, brief and Board.
- **Storage.** Migration step 15 adds `team_projects.source_project_id`. Rows
  from before it read as no link.
- **CLI.** `massorca team project add|update <team> <name> --source <project>`
  sets the link. On update, an empty `--source` clears it. `team project list`
  shows the linked project's name in a fourth column, or `-`.
- **Not in this change.** Starting a task on a linked project comes in the next
  PR. A git project gets one worktree per task. A folder project runs the task
  in the folder itself. Read-only tasks run in the main checkout.

## Amendment 2026-11-22: where a task runs

The workspace domain lets one agent write in a workspace at a time
(`agent.workspace_busy`). A team's bots must still be able to work at once.

- **A task with no project:** the bot works in the team's folder workspace,
  `<workspaces root>/bot-teams/<team id>`. Every team folder belongs to one
  folder project named "Bot teams", made on the first start. The team's
  workspace is stored on the team and made again if it was removed.
- **A team folder has several writers.** This is a property a folder
  workspace can have and a worktree cannot. A folder has no git index to
  corrupt, and the team's bots seeing each other's files is the point of a
  team folder. Every other workspace keeps one writer and
  `agent.workspace_busy`. pv96 ran a team's bots in one shared folder in the
  same way.
- **A busy bot:** a bot works on one task at a time. Another of its tasks still
  running blocks a second start, and so does one whose session cannot be
  verified. That start is refused with `bot.busy`. The task stays Open with the
  reason in `error`, and nothing is queued. Starts are serialised, so two
  starts for one bot cannot both find it free.
- **A task on a team project that is a plain folder:** it runs in that folder,
  with several writers, as in pv96.
- **A task on a team project that is a git repository:** the bot works in its
  own git worktree for that task. That worktree is made through the path
  delegation uses for child agents, and the result comes back as a branch. A
  task that only reads runs in the main checkout, as in pv96.
- **What is not built yet:** a team project can name its code source, but
  `task.start` does not use that link yet, so no task reaches either project
  case today. Both come in a follow-up.
- **Who owns the run:** the host that runs the agent owns the run. A card
  follows its session in the `live`, `unverifiable` and `exited` terms. An
  unverifiable session counts as running, and losing contact never moves a
  card to Done or to a failure.
- **Who starts the run:** the run is started with the scope of the caller of
  `task.start`, never as the service. It gets the vault's variables only when
  that caller holds host-admin scope, as `agent.start` does (decision 0071).
  A paired device with standard scopes starts a run without them.
- **Delivery:** the built-in kinds take the prompt on their command line, so
  the prompt is delivered once the agent starts, and only then is the session
  linked. A kind whose prompt mode is `none` gets its prompt at its first idle,
  and the agents domain drops that prompt after 120 s. Such a kind's task can
  show Running with a prompt it never received.
- **Error code:** a task and a Board card carry `error_code` next to `error`.
  It is the protocol code of the failed start, such as `agent.not_installed`,
  `bot.busy` or `agent.credentials_invalid`, so a client can show its own
  sentence. The field is optional and additive. A start that links a session
  clears it, and so does `task.reopen`.
- **Targeted task writes:** `task.update` writes only the title, body, bot and
  project, so it never overwrites a session link that a start wrote after it
  read the task. `task.done` and `task.reopen` write only if the task still has
  the session they read. Otherwise they are refused: with `task.running` when
  the task now runs, and with `task.changed` ("the task changed; try again")
  otherwise.
- **Waiting for a start:** `task.rm` and a bot change in `task.update` wait for
  a start in progress, then apply their usual check. A task whose session runs
  cannot be removed or change bot; both are refused with `task.running`.
- **Start attempt:** a start stores an attempt id on the task before it starts
  the agent, and the agent request is keyed by it. The id is cleared once the
  attempt is settled: the session is linked, or the start failed with nothing
  left running. A retry after a start that left a run behind uses the same id,
  so it finds that run instead of starting a second one.
- **Stop on failure:** when a start spawned a session but cannot link it, it
  stops that session at once, and records the error on the task if the task
  still exists. A session that is not linked never keeps running.
- **A taken team path:** the "Bot teams" project must be a folder project. When
  `<workspaces root>/bot-teams` was already added as another kind of project,
  the start is refused with `team.folder_taken`, recorded on the task like
  other failed starts. Nothing is written inside that project.
- **Older hosts:** `task.start` joined `teams/1` without a new capability, so a
  host from before it answers `method.not_found`. `massorca task start` shows
  that answer as "this host is too old to run tasks; update it".
- The bot's model and effort are not passed yet, because `agent.start` has no
  field for them.
- **Done latch:** a task whose session reports Done with an outcome keeps that outcome in `outcome` once the host can see the run; an unverifiable run latches nothing, so the Board keeps Done after the session is forgotten. Every change of a linked session publishes `task.changed`, and a session that finished before `task.start` linked it latches at the link.
