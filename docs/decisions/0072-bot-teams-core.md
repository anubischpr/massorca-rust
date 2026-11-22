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
- **Bots:** `team.bot_add`, `team.bot_update` and `team.bot_rm`. A bot that owns
  open tasks is refused unless `unassign` is given.
- **Projects:** `team.project_add`, `team.project_update` and
  `team.project_rm`.
- **Tasks:** `task.create`, `task.list` (filter by team, project, bot or
  column), `task.show`, `task.update` (title, body, bot, project), `task.start`,
  `task.done`, `task.reopen` and `task.rm`.
- **The Board:** `team.board`, given `{ team, project? }`. It returns the four
  columns in order, each with its cards: task id, title, bot, the state word and
  the time.
- **Events:**
  - `team.changed`, with the team id, for a team, bot or project change;
  - `task.changed`, with the task id, when a task is created, updated, started,
    finished, done, reopened or removed. The service also publishes it when a
    linked session's `agent.changed` changes the task's column.

### CLI

```
massorca team create <name> [--purpose ...] [--character ...]
massorca team list | show <team> | rm <team> [--force]
massorca team bot add <team> <name> --agent <kind> --job ... [--lead]
massorca team bot rm <team> <bot> [--unassign]
massorca team project add <team> <name> [--goal ...]
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
- **What is not built yet:** a team project has no code source yet, so no task
  reaches either project case today. Both come with the follow-up that lets a
  team project name its repository or folder.
- **Who owns the run:** the host that runs the agent owns the run. A card
  follows its session in the `live`, `unverifiable` and `exited` terms. An
  unverifiable session counts as running, and losing contact never moves a
  card to Done or to a failure.
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
- The bot's model and effort are not passed yet, because `agent.start` has no
  field for them.
