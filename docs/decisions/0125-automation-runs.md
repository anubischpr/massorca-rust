# 0125: Automation runs (AUTO-03, AUTO-04, AUTO-06, AUTO-07, first version)

Status: accepted, 2026-11-26. Automations run on their schedule on the host that keeps them, can be run now, and keep a
run history with each run's outcome and result.

## Context

Decision 0123 stored automation definitions and left their runs to AUTO-03. The inventory (docs/inventory/automations.md)
asks for a schedule engine with missed runs and retention (AUTO-03), run now and run history (AUTO-04), the agent launch
(AUTO-06) and run completion with output and usage (AUTO-07). Decision 0082 gives one engine for routines and
automations; its claim, zone and occurrence rules are reused unchanged, and the Bot Teams routine timer is the model for
the timer, the claim and the sweep at start.

## Decision

- D1 Owner. The service that keeps an automation runs it. In this product the core always runs as a service, so
  schedules keep running while no window is open; the desktop-only rule "no window, no run" does not carry over. A
  remote client never runs a host's automation itself: it asks that host with `automation.run_now`.
- D2 Store. Migration step `automation_runs`, appended after `automations`. One row per run: its automation (`ON DELETE
  CASCADE`, so removing an automation removes its history), a number, the trigger (`schedule` or `manual`), the
  occurrence it stands for, how many occurrences it folds, its state, the agent's outcome, its session and workspace, a
  reason with its code, the result text, token counts, and its start, end and creation times. The state CHECK lists
  every AUTO-04 status, including the ones this version never writes (`pending`, `skipped_precheck`,
  `skipped_needs_interactive_auth`), so later versions need no table rebuild.
- D3 Numbers. A run's number is one more than its automation's highest, given in the insert. Retention keeps the newest
  runs, so the highest number is never pruned and a number is never given twice.
- D4 Timer. A check every 60 seconds and one at start (AUTO-03). Only enabled automations whose stored next run has
  passed are read. A claim is the compare-and-swap of decision 0082 on `next_run_at`; it stores the claim's runs and
  `last_run_at` in the same transaction, so two overlapping checks never start one occurrence twice. Each automation is
  claimed and started before the next one is claimed. A schedule that no longer compiles is skipped with a warning and
  tried again at the next check, so a repaired one resumes; nothing is recorded for it. The service starts the timer and
  the run listener beside the routine timer, and stops them on the same signal.
- D4a Edits. An edit, enable or disable writes the definition without `next_run_at` or `last_run_at`, which only a claim
  moves. When the schedule changes (or the automation is enabled or disabled), the next run is moved by a
  compare-and-swap on the stored next run, read again on each miss, so a claim made between the edit's read and its
  write is never undone and its occurrence never runs twice.
- D5 Missed runs. The latest due occurrence runs once when it is no later than its grace (720 minutes when unset) plus
  two checks of slack, as pv96 allows; otherwise it is recorded `skipped_missed` with `automation.run_missed`. Earlier
  occurrences a claim skips are folded into one `skipped_missed` row with their count. Nothing missed is replayed.
- D6 No project. An automation without a project (its project was removed) records `skipped_unavailable` with
  `automation.no_project` and starts nothing, whether scheduled or run now. A manual run records the same row and
  returns it, so the person sees why.
- D7 Run now. `automation.run_now` (permission `workspace`) starts a run at once and does not move the schedule. It
  returns the run as recorded: `dispatched` with its session, `dispatch_failed` with the reason, or
  `skipped_unavailable`. `automation.runs` (permission `read`) lists one automation's runs newest first by number, 20 a
  page by default and 100 at most, with `before` and `next_before` as the cursor.
- D8 Launch (AUTO-06). A run starts the automation's agent through `agents.start`, the start tasks use, with the
  automation's prompt and name as its title, and a request id per run (`automation.run:<run id>`) so a retry finds its
  session. Where it runs: a git project gets one worktree kept for the automation (named as a task's worktree is, from
  the automation's name and six characters of the random end of its id; made on its first run, request id
  `automation.worktree:<id>`), so runs never write in the person's own checkout and worktrees do not pile up; a folder
  project runs in its folder's workspace, made when it has none and shared by several writers. A second run while the
  first still writes in the same worktree is refused by the one-writer rule and recorded `dispatch_failed`.
- D9 Rights. A run gets no vault secrets and starts in its agent's default permission mode. An automation records no
  creator, so no run may claim host-admin rights; giving secrets to runs waits for a definition that records an admin
  creator, as routines do (decision 0071).
- D10 Completion (AUTO-07). A listener follows the agents domain, reads every session at start and after a lag, and
  ends the dispatched run of a session: `completed` with the agent's outcome and result when it reports Done from a run
  the host can see; `dispatch_failed` with `automation.run_exited` when the agent exits without Done; `dispatch_failed`
  with `automation.run_stranded` when the agents domain forgets the session. An unverifiable session leaves the run
  `dispatched`: loss of contact is never evidence that the agent ended. A Done that landed before the run named its
  session is settled right after the start. The listener reads every session at start and after a lag, and then fails
  with `automation.run_stranded` every dispatched run whose session the store no longer has, since its Forgotten event
  was missed; a run whose session exists stays, whatever that session shows. At the next start, a run still
  `dispatching` whose agent session exists (found by its request id) is attached to it and settled from it: the
  service stopped after the agent started and before the start was saved. Any other run still `dispatching` is failed
  with `automation.run_interrupted`. A `dispatched` run is kept and settled from its session.
- D11 Output. A run keeps the agent's last result text, which the agents domain caps at 16 KiB; the host keeps at most
  16,384 characters, dropping older text from the front. The full terminal snapshot of pv96 is not kept in this
  version.
- D12 Usage. Agent sessions report no token usage yet, so every run shows usage `unavailable` with the reason "the agent
  session reports no token usage". The token columns are stored for when they do.
- D13 Retention. The newest 100 final runs per automation are kept (pv96's number); runs in progress are never
  evicted. Pruning happens after each claim, each manual run and each run that ends.
- D14 Events. Topic `automation.changed`, whose id is the automation, fires when a run is recorded, starts or ends, so
  clients read the history and the last and next runs again. A worktree or folder workspace made for a run is
  announced with `workspace.created`.
- D15 Protocol. Capability `automation-runs/1`, beside `automations/1`. New fields are optional and new enums decode an
  unknown value as `Unknown`, so older and newer peers keep working. An older host answers the new methods as unknown;
  the CLI says the host is too old.
- D16 Next run. Now that runs advance it, an automation shows its stored next run instead of one computed at read time.
- D17 Calendar. With `include_done`, the Calendar shows an automation's completed runs from its kept history, placed
  at the occurrence they stand for; failed, missed and skipped runs are not shown as done (decision 0124, D3 amended).

## CLI

- D18 `massorca automations run <AUTOMATION> --now` runs it now and prints the run; `--now` is required, as for
  routines. `massorca automations runs <AUTOMATION> [--limit <N>] [--before <NUMBER>]` lists its runs. Both take
  `--json`.

## Desktop

- D19 The Automations page (artboard `ws-automations.dc.html`, D-AG-14) gives each automation Run now and Runs. Runs
  opens its run table: started, status, duration and the result or reason. The table is read when it opens, after a
  run now, and when `automation.changed` arrives, which also reads the list again for its last and next runs.

## Not in this version

- Extra agent arguments and session reuse (AUTO-06): the owner direction asks for plain launch first. They need new
  definition fields and destinations (AUTO-02).
- The precheck (AUTO-05), host ownership and fencing (AUTO-08), remote and SSH scheduling (AUTO-09), run terminal
  retention (AUTO-10). Destinations, per-run worktrees and folding repeated `skipped_unavailable` rows (AUTO-02) are
  decision 0127.
- Token usage (no session reports it) and the plain-text output snapshot beyond the result text.
