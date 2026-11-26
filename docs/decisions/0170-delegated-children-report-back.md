# 0170: Delegated children report back

Status: accepted, 2026-11-25.

## Context

- Inventory AG-26 (first version) asks that a child's result returns to the parent chat, that a child gets its own
  worktree by default, and that the current worktree is used only for read-only work, a plain folder, or a manual pick
  on the delegation card, "and then the parent waits": two sessions never write in the same worktree at once
  (owner direction, 2026-11-12, rules 2 and 3).
- Before this decision a parent learned that its child finished only by polling `agent status` or `agent tree`. A
  writing child of a plain folder shared it with a parent that kept going (decision 0161), and no call put a writing
  child in the parent's worktree.
- The reference, pv96 at commit d3c6311a531 (`src/main/delegated-session/delegation-lifecycle.ts`,
  `delegation-notices.ts`), posts a status row to the parent chat when a child starts, fails to start, or finishes
  (`Child session "<title>" finished: <first 280 characters of the result>`), always, with no opt-out. A row is posted to
  a parent whatever its state and stays in its history. pv96 lets a folder child write while the parent keeps going,
  which the owner's rule 3 replaces.

## Decision

- **Finished.** A child's run is finished from its first `done` report until the next prompt typed into it or its
  next `working` report; the service keeps this on the live run. A later `waiting` (Claude's idle notification, about
  60 s after a turn) and a repeated `done` do not change it. A kind that never reports `done` (Antigravity, Gentle
  Shell, a chat run) counts as finished only when its run ends.
- **The notice.** When a delegated child's run becomes finished, or its run ends while it was not finished, the service
  queues a prompt on the parent's queue (decision 0084):
  `MassOrca: child session <id> finished: <outcome>. Read its result with massorca agent status <id> --json, or bring it back with massorca agent bring-back <id>.`
  - `outcome` is `done`, `failed` or `cancelled` from the turn's report, or, for a run that ended mid-turn or before
    any turn, `stopped` (killed), `interrupted` (a turn was cut) or `exited` (with its code when not 0).
  - It carries no text a caller chose: neither the child's result, unlike pv96, nor its title, which a caller with
    workspace scope sets. Only the id, the outcome and the fixed instructions reach a parent that may hold secrets.
  - It is queued under the status lock that records the end, so whoever sees the end sees the notice. Its request id is
    `delegation-notice:<child>:<end>`, so each end is queued once. A run whose finished turn was told is not told
    again when it ends. A store error while checking for an earlier notice is logged, and no notice is queued. It is
    queued as host-admin: the service writes it and
    it holds no secret, so a parent run holding vault secrets (decision 0071) still takes it.
  - The queue's rules deliver it: when the parent is `idle` or `done`, never into a question, behind earlier prompts.
  - A parent with no live run (stopped by the person, or exited) gets the notice paused (`paused_stopped`), as the
    person's stop pauses a queue: it is not typed into a resumed run until the person prompts, sends it or resumes the
    queue. This keeps pv96's "the parent keeps the notice" while never typing into a parent the person stopped. A
    notice queued while the parent's run was live is not paused by a later exit (only the person's stop pauses), so it is
    typed into the resumed run at its first `idle`, like any queued prompt.
  - No opt-out: pv96 has none. A parent kind with no queue (no status mechanism) or a full queue gets no notice; the
    service logs it and the person still sees the child's end in `agent tree`.
  - A run the service marks `service_stopped` at its next start is not told: the parent's run ended with the service
    too, and its queue is paused by the restart (decision 0084, D5).
- **The parent waits.** A writing child (not read-only) whose run is live and not finished, working in its parent's
  own workspace, holds the parent:
  - the parent's queue holds, each waiting prompt shown with the new reason `child_writing`;
  - `agent.prompt` to the parent without `force` is refused with `agent.busy`, naming the child in `data.child`;
  - `force` and `agent.queue.send_now` still type, as they do past a busy agent (decision 0084, D6): the person's
    explicit call. Keys a person types into the parent's terminal are not intercepted.
  - The hold lifts when the child becomes finished or its run ends; the parent's queue is then drained, so what waited
    and the child's notice arrive in queue order. A child that is prompted again holds the parent again.
  - Only direct children count; a grandchild holds its own parent.
- **Manual pick.** `agent.delegate` takes `same_workspace` (CLI `--same-workspace`): a writing child of a worktree
  workspace runs in the parent's worktree instead of a new one, and its start `note` and record `notes` say "The child
  works in its parent's worktree, and the parent waits until it finishes." It is refused with
  `request.invalid_params` together with `name`. While another writing session of the same delegation tree, other
  than the parent, works unfinished in that worktree (a sibling, or a grandchild), or an ancestor of the parent has a
  live run there, finished or not, it is refused with `agent.workspace_busy`, naming that session in `data.session`, so
  two sessions never write in one worktree at once. Only the parent waits for its children; rather than make every
  ancestor wait, a grandchild is refused the pick while the root runs in that worktree. A resume of a writing child is
  refused by the same rule, so an exited child cannot come back beside a sibling.
  In a plain folder it changes nothing (the child already shares the
  folder). The field is omitted when false, so an older service never sees it; an older service given it ignores it and
  gives the child its own worktree.
- **One writer per tree.** In a worktree, which keeps one writing session at a time, sessions of the same delegation
  tree do not count as other writers on a start or a resume (`agent.workspace_busy`); any session outside the tree
  still does. A plain folder takes several writers anyway (decision 0169), so there only the parent wait applies: the
  refusals of a second `same_workspace` child and of a resume beside a tree writer are for worktrees. A chat child
  (decision 0101) has no hook run, so it holds its parent until its run ends, and a chat parent waits for its writing
  child as a terminal one does.

## Consequences

- A parent agent learns its child finished without polling, and reads the result only through `agent status` or
  `agent bring-back`, which it already knows from the delegation instructions (decision 0161).
- A notice is a turn of the parent: tests and callers that bring a child back right after it finishes wait for the
  parent to take the notice first (`agent.parent_busy` otherwise).
- In a plain folder the parent no longer keeps going while a writing child works (0161's consequence is replaced).
- AG-26's first-version rows on the service side are covered; the card that offers the manual pick is a client's.
