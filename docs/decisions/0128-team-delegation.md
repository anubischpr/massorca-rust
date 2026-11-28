# 0128: Team delegation

Status: accepted, 2026-11-26.

## Context

- Inventory AUTO-20 (Bot Teams part) asks that a bot hand work to a teammate with
  `team-delegate --bot <name> --text <task> [--team <team>]`, that the teammate's agent is the bot's own, that the
  child works in its own folder, and that with `--team` the task is filed in another team. Owner direction: the lead
  bot picks children by competence (each bot's job). Usage-based Automatic routing is not required.
- Child placement (owner, 2026-11-12) applies to team delegation too: a child gets its own worktree by default; it
  may share the current workspace only when it only reads, when the workspace is a plain folder, or when the person
  picks it. Children report back to the lead bot (AUTO-18).
- Decision 0055 defines the agent delegation this builds on (one writer per worktree, never a wider access mode).
  Decision 0170 defines the report-back for agent children. This decision covers bot children in Bot Teams.

## Decision

- **Call.** `team.delegate` (capability `team-delegate/1`) is called by a bot's agent run. The run sends its session
  and run token (`MASSORCA_AGENT_SESSION`, `MASSORCA_AGENT_TOKEN`). The token must match a live run
  (`permission.denied` otherwise), and the run's task names the parent. Only a task run can delegate, and only as its
  own bot. A call with no run, or a run with no task or no bot, is refused with `team.delegate_not_a_bot`. A call
  naming the caller's own bot is refused with `team.delegate_self`.
- **Depth.** A child sits one level below the task that delegated it. The task's depth is the number of links above
  it, and a child whose depth would exceed `agents.delegation_depth` (default 1, clamped 1 to 5, read on every call) is
  refused with `team.delegate_depth_exceeded`, whose data carries `limit`, `setting` and `depth`. At the default a
  child cannot delegate. A cycle cannot form: a child is always a new task, and storing an existing task as a child
  fails.
- **Idempotency.** `request_id` (optional) scopes a key to the parent task. A repeated call with a key the parent used
  returns the child the first call made and starts nothing. The CLI derives its key from the run, the task text, the
  bot, the team and the read flag, so a retried command keeps its key. The same text sent again from the same run is
  therefore the same call; a deliberate second delegation needs a different text.
- **Child task.** The child is a task in the target team (the caller's team, or `--team`) with the named or picked
  bot. The task and its link to its parent in `task_delegations` (one parent per child, with the read-only flag and
  the request key) are stored in one transaction.
- **Picking.** A bot named with `bot` is used as named. Without it, competence decides: each teammate's job and
  skills are split into words (lowercase, at least three letters, stop words removed), and the teammate that shares
  the most words with the task text wins. Ties go to the name that sorts first. In the caller's own team the caller
  is not a candidate. No shared word is refused with `team.delegate_no_bot`, so a task never lands on a bot by
  chance. The rule is deterministic and uses no usage data.
- **Placement** (the existing task start, `task_start`):
  - A writing child keeps the parent's project when it is in the same team, so it gets that project's own worktree
    (a git project) or linked folder (a folder project). Without a project it takes the team's folder, which is a
    plain folder.
  - A read-only child runs in the parent's workspace. A parent with none yet leaves it the team's folder.
  - A child that cannot start stays Open with the reason in `error`; the delegation stands and the call succeeds.
- **Read-only.** `--reads` is refused with `team.delegate_reads_other_team` for a team other than the parent's, because
  a read-only child shares the parent's workspace. A read-only child starts in its kind's plan mode, so it cannot edit
  the workspace it shares: the kind's plan mode is the first mode that is not a bypass and is named `plan` by its id or
  its label, in any letter case. A kind with no plan mode gives the child the vendor's default, which is never a bypass
  mode; the child then has no mode of its own and the decision does not promise it is read-only. The start goes through
  the agent layer's own mode check, without changes to core-agents.
- **Access and vault.** A writing child runs in its team's own mode, but never in a bypass mode when its parent runs in
  the vendor's default mode, or when its parent's bot has been removed (then it keeps the vendor default). A delegated
  child starts without the vault (`SecretGrant::Withhold`), so it never gets more scope than the run that delegated it.
- **Result back.** A delegated child's outcome is posted to its team's Conference room by its own bot (`Done: <title>`
  or the failure word), the same line a room task gets. The lead reads the whole tree with `team.delegation_tree`,
  `team.delegation_list` or `task show`, which list each child with its state.
- **Tree.** `team.delegation_tree` returns one task and its children (oldest first, each with its state and read-only
  flag). `team.delegation_list` returns every link of a team in one call, for the Conference room.
- **Surfaces.** CLI: `massorca team delegate --bot <name> --text <task> [--team <team>] [--reads]` runs only inside a
  bot's run; outside one it is refused before any connection. `massorca task show` lists the children. Desktop: each
  room task card lists its delegated children with their state.

## Not built (gaps)

- The parent does not wait for a writing child that shares its folder (placement rule 3). A writing child in a folder
  project or in the team's folder runs beside its parent, as those folders already allow for tasks.
- A delegated child's result is not sent as a prompt to the lead's run by itself; the lead reads it from the tree or the
  room. A child that sends `worker_done` (decision 0129) wakes a live lead run.
- Only a task run can delegate. A bot's reply run in the Conference room cannot yet.
- Automatic routing by usage (ACC-26) is not built.
- No end-to-end test covers a live bot run that calls `team delegate`: the fake agent cannot run the CLI. The run-token,
  placement, mode and outcome paths are covered by code review and the tests listed in the commit history.

## Migration

- Two steps: `task_delegations` (child task, parent task, read-only flag, creation time), with an index by parent, and
  then `request_id` with a unique index per parent. Removing either task removes its link. No existing row changes.

## Consequences

- A delegation is a task with a link, so it appears on the Board, in `task list`, and in the room like any task.
- Competence is a word match, a first version: it is explainable and repeatable, and a bot whose job uses other words
  is picked only by name.
- The depth limit is read by the server from the stored setting with the same clamp as core-agents; core-agents keeps
  its own parser private, so the two parsers must change together.
