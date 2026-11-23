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
- **A task on a team project that is a plain folder:** it runs in that
  folder, with several writers. The project's folder workspace at its own path
  is reused, or made when it has none, and is marked as shared.
- **A task on a team project that is a git repository:** the bot works in its
  own git worktree, made through the same Workspaces entry delegation uses for
  a child agent. The Workspaces domain picks the base: the project's base
  branch once projects have one, otherwise the main checkout's `HEAD`. The
  main checkout's uncommitted changes are not carried, and the task prompt
  says so. The result is that worktree's branch; the owner merges it.
- **Worktree names:** as in pv96, the worktree and its branch are named after
  the task title: a lowercase ASCII slug of at most 40 characters, then `-`
  and 6 characters of the start attempt (`child-<6>` when the title has no
  letters or digits). Every start makes a new worktree; a retry of the same
  pending attempt finds its own.
- **Cleanup:** none. `task.done`, `task.reopen`, `task.rm` and removing the
  team leave the worktree and its branch. The owner removes them with
  `workspace rm`, which keeps a branch that is not merged.
- **Where the result is:** a task and a Board card carry optional
  `workspace_id` (where the last start ran) and `branch` (set only for a task's
  own worktree). Migration step 17 adds both columns.
- **No source:** a task with no project, or whose project links no source or a
  removed one, runs in the team folder.
- **Read-only tasks: deferred.** pv96 runs a task that only reads in the main
  checkout. Here a git project's main checkout has no workspace row, so no
  agent can run there yet. That needs a Workspaces change of its own.
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
- **Waiting for a start:** `task.rm`, `task.done`, `task.reopen`, a bot change
  in `task.update`, and `agent.resume` take one start gate. Each one checks
  whether the task runs and writes under that gate, so no start links and no
  resume revives a session between the check and the write. A task whose
  session runs cannot be removed, finished, reopened or change bot; each is
  refused with `task.running`. The gate is one for every team, so starts in
  different teams also wait for each other. Starts are short, so this is
  accepted for now.
- **A task finished during its start:** the link is written only while the task
  has no session and no outcome. When it finds an outcome, the start stops the
  session it spawned and records `task.changed` on the task.
- **Start attempt:** a start stores an attempt id on the task before it starts
  the agent, and the agent request is keyed by it. The id is cleared once the
  attempt is settled: the session is linked, or the start failed with nothing
  left running. A retry after a start that left a run behind uses the same id,
  so it finds that run instead of starting a second one.
- **Stop on failure:** when a start spawned a session but cannot link it, it
  stops that session at once, and records the error on the task if the task
  still exists. A session that is not linked never keeps running. A start that
  fails after the spawn point is treated the same way: the session is found by
  its agent request id and stopped. When that stop fails, the attempt is kept.
  The bot then stays busy while that run lives, and a retry names the same
  run. A read that fails after a successful link is reported to the caller
  and never stops the linked run.
- **Known gap:** the agents domain stores the request id after the terminal
  spawns. A service crash between the two lets a retry start a second process.
  That is the agents domain's start order and is tracked there.
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

## Amendment 2026-11-23: routines

A routine is a schedule that starts a bot on a task. It belongs to a team and
names one of the team's bots. Its fields are a name, a prompt, a cadence (daily,
weekly on a weekday, monthly on day 1 to 28, or once on a date), a 24-hour time,
an IANA zone (the host's zone when unset), a paused flag and the next run. The
schedule itself is decided by `massorca-core-schedule` (decision 0082): the
occurrences, the daylight-saving rules and the claim. A routine adds its store,
its methods and its timer, and nothing else.

Runs:

- The host checks every 30 seconds, and once at start. A check claims each
  routine whose next run is due by a compare-and-swap on that next run, then
  starts that run before it claims the next routine. The claim writes the run
  and the run's task in the same transaction, so an occurrence runs once and a
  run never exists without the task it made. A shutdown signal stops the timer
  before any further check starts.
- A check that finds several missed occurrences claims only the latest; the
  earlier ones are dropped. Pausing clears the next run. Resuming schedules the
  first occurrence after now, so nothing missed while paused is made up.
- A run makes a task. Its title is the routine's name. Its body is the prompt,
  then a line that names the day the run stands for in the routine's zone, so
  "today" means that day. Its bot is the routine's bot. The task starts through
  the same start a person's Start uses.
- A run's task keeps the origin `cli`, the origin of a task a command made. A new
  origin value would be unknown to older clients, so none was added.
- A run records its task and any error with its code (`error_code`). A failed
  start keeps the task Open with its own error, and the run keeps the error too.
  No run is retried: a failed run waits for the next occurrence.
- A run is pending from its claim until its start records an outcome
  (`settled_at`). The next service start settles every run left pending from
  before it from what its task shows: a task whose session is linked started,
  so the run has no error; a task that kept a start error gives the run that
  error. Any other run is interrupted (`team.routine_run_interrupted`, "the
  service stopped before this run started"), such as one whose service crashed
  between the claim and the start: its task stays Open and is not started.
  Nothing retries the run, and the next occurrence runs as usual. A run whose
  outcome cannot be written is tried once more, then left pending for that
  settling, so its state is always in the store. A settling that fails is tried
  again before each 30-second check until one succeeds.
- A schedule that no longer compiles, such as a zone the database no longer
  knows, finishes the routine. The claim's run records the reason, and nothing
  retries it.

Removals:

- A bot's removal keeps its routines with no bot (`ON DELETE SET NULL`). Each
  run of such a routine records `bot removed: the routine has no bot to run`,
  makes no task and starts no agent. Updating the routine with another bot
  restores it.
- A team's removal removes its routines and their runs. Deleting a routine
  removes its runs and never its tasks.

Caller scope:

- A scheduled run acts with the scope of the caller that last added or updated
  the routine. `starts_as_admin` records whether that caller held host-admin
  scope: an add or an update by a host-admin caller sets it, and an update by any
  other caller clears it. Pause and resume leave it as it is. A scheduled run
  receives vault secrets (`SecretGrant::Provide`) only when it is set; otherwise
  it starts with `SecretGrant::Withhold`.
- A manual run (`team.routine_run`) receives vault secrets only when the caller
  and the routine are both host-admin, so a caller with less scope cannot widen
  what a routine may do.
- An edit, a pause or a resume writes only while the stored routine still equals
  the one it read; otherwise it reads the routine again and reapplies itself, at
  most five times, then answers `team.routine_changed`. Two concurrent changes
  both survive, and no change can restore a `starts_as_admin` another caller
  cleared or a schedule another caller set.

Zones:

- A routine without a zone runs in the host's zone. `Routine.zone_used` names
  that zone (its IANA name, or `UTC` when the host's zone cannot be read) so the
  zone a run uses is never silent. While the host's zone cannot be read, an add
  without a zone and an update that clears the zone are refused with
  `request.invalid_params`.

Methods (capability `teams/1`):

- `team.routine_list { team }` lists the routines in creation order.
- `team.routine_add { team, bot, name, prompt, cadence, time, weekday?, month_day?, run_date?, zone? }`.
- `team.routine_update { team, routine, ... }` changes only the fields given. An
  empty `zone` returns the routine to the host's zone.
- `team.routine_rm`, `team.routine_pause` and `team.routine_resume` take
  `{ team, routine }` and return the routine.
- `team.routine_runs { team, routine, limit? }` lists the runs newest first: 50
  when `limit` is unset, at most 500.
- `team.routine_run { team, routine }` makes one run now. It does not move the
  schedule.

Refusals use `request.invalid_params` for a schedule that core-schedule refuses,
a blank name or prompt, a cadence or weekday outside the known values, and a
one-time date that has passed. `team.routine_not_found`, `team.not_found` and
`bot.not_found` name the rest. Cadence and weekday values this build does not
know decode as `unknown`. Changes publish `team.changed` with the team id, and a
run that makes a task publishes `task.changed` for that task. An older host
answers `team.routine_*` with `method.not_found`, which `massorca team routine`
reports as "this host is too old for routines; update it".

Daylight saving follows decision 0082: a wall time that does not exist fires at
the end of the gap, and a repeated one fires once, at the earlier instant. The
server tests cover both in America/New_York.

Storage: migration step 18 adds `team_routines` and `team_routine_runs`, whose
runs keep `error_code` and `settled_at`. `RoutineRun.error_code` and
`Routine.zone_used` are optional fields an older peer omits.

CLI:

```
massorca team routine list <team>
massorca team routine add <team> <name> --bot <bot> --prompt <text> --time <HH:MM> --cadence daily|weekly|monthly|once [--weekday mon..sun] [--month-day <1-28>] [--date <YYYY-MM-DD>] [--zone <IANA>]
massorca team routine update <team> <routine> [the same options]
massorca team routine rm <team> <routine>
massorca team routine pause <team> <routine>
massorca team routine resume <team> <routine>
massorca team routine runs <team> <routine> [--limit <n>]
massorca team routine run <team> <routine> --now
```

Not in this change: one-time routines created from a Conference room (AUTO-13),
the Calendar (AUTO-30), the desktop Routines page and the phone, which still show
sample data, and moving routines onto the Automations engine (AUTO-01).

## Amendment 2026-11-23: team access

A team's access decides the permission mode its bots run in (decision 0083). It
has two values, as in pv96, and there is no per-bot mode.

- **`full`**: a task start names the first bypass mode (`bypass: true`) the
  bot's kind declares that a start can apply: one with launch arguments, or a
  live switch on a kind that reports idle. The agent then runs without asking
  for permission. A kind with no such mode, such as one whose only bypass mode
  is a live switch and that reports no status, runs in its vendor default
  instead of failing the start. `massorca task start` says so on that start,
  and the service log notes it for starts made elsewhere, such as routines.
- **`default`**: no mode is passed, and the vendor CLI starts in its own
  default, which may ask for permission.
- **New teams are `full`**, as in pv96: an unattended bot that stops to ask is a
  stuck task. Decision 0083 says a bypass mode is always named, so the create
  path names it every time. `massorca team create` and `team update` to `full`
  print "Full access: bots run without asking for permission", with the one
  command that turns it off. `team create --access default` makes a team without
  it.
- **Only a host-admin caller sets `full`.** A caller below host-admin scope,
  such as a standard paired device, makes `default` teams when it names no
  access, and an explicit `full` on `team.create` or `team.update` is refused
  with `permission.denied`. A full-access team's bots skip permission prompts,
  and runs of routines a host-admin set up also hold vault secrets (decision
  0071), so only a host-admin caller may combine the two.
- **Existing teams become `default`** (migration step 24). They were made when
  `full` changed nothing, so they must not gain bypass silently. `massorca team
  update TEAM --access full` turns it on.
- `team.create` without `access` still makes a `full` team, so an older client
  that sends none gets the new default without the sentence. `default` is a new
  wire value: an older client reads it as unknown.
- Not in this change: the desktop New team dialog and the team card, which will
  name the access and offer the change in a follow-up.

Decision 0083 is unchanged: a mode is still passed only when someone chose it.
Here the owner chooses it once for the team.
