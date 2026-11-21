# Automations, bot teams, orchestration and artifacts: functional inventory

Prefix: `AUTO`. Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits), read-only worktree. Baseline before pv96: MassOrca Electron app at custom/integration `f23a48ca8ca`. Each feature that pv96 changes has a `Source: pv96 (unmerged)` line.

## Summary

MassOrca lets a person schedule agent runs. An automation stores a prompt, an agent, a destination (a project, an existing workspace, or a fresh worktree per run) and a schedule. When a run is due, the host that owns the automation starts a terminal with that agent and the prompt, watches it until it finishes, and keeps a bounded history with output and, where available, token usage. Automations are owned by one host: the desktop app, a paired runtime server, or an SSH target bridged through the desktop. A separate group covers Hermes and OpenClaw cron jobs, which MassOrca lists and controls without owning them.

Three coordination layers sit above single agents. Bot teams are a roster of named bots with a task board per team, where each started task runs in a background chat session. In pv96 a team also has a Conference room where bots reply and requests are routed by one host query, projects, team memory, routines on their own scheduler, a Calendar, and a web address that starts a task from this computer. Orchestration is a set of lightweight Runs, Tasks, Dispatches, worker agents, messages, decision gates and cross-server federation, driven by agents through the `orca` CLI. Delegation from chat lets an agent ask the person which agent should run a child session, then starts it. Artifacts are HTML or Markdown files shared to the signed-in MassOrca account and returned as links, gated by a setting that is off by default.

Note on depth: the orchestration and bot-team areas are large (over 25,000 lines in the runtime orchestration folder alone). Behaviour here comes from specs, type definitions, store schemas, module comments and the core paths named below. Most RPC handler bodies were not read. pv96 changes 130 files under `src/main/bot-teams`, 77 under the orchestration folder and 46 under `src/main/delegated-session` (test files included, against custom/integration). Its rules come from shared modules, store code, module headers and the paths in the pv96 read list, not from a full read of every handler.

## Owner direction for the new product (2026-11-12)

The owner took decisions in the design session that change this domain. They are recorded in `docs/design/screen-inventory.md` (rows D-SH-10, D-WS-29, D-WS-30, D-AG-13, D-AG-14, D-AG-23 to D-AG-25, X-DL-01 and answered open questions 6, 7 and 8). The feature sections below keep the observed current behaviour and add a separate "Owner direction for the new product" line where the decision applies.

- **Bot Teams is a core mode**, switched from the activity rail, next to Workspaces mode. A team has bots, a Conference room that turns stated requirements into tasks on the team's Board, Routines, Files and Memory. See [bot-teams.md](bot-teams.md) for a reading guide.
- **Orchestration and delegation are in the first version**, as one core engine shared by Bot Teams (the lead bot delegates by competence) and Workspaces (a coordinator coding agent launches child agents, sequential or parallel, with worktree handoff). Child agents show as a tree inside the parent session or the Conference room.
- **Where a delegated child runs (child placement rules, 2026-11-12):** (1) a delegated child session gets its own worktree by default; (2) it may use the current worktree only when the child only reads (research, review, explain), when the workspace is a plain folder without git, or when the user picks it by hand on the delegation card; (3) two sessions never write in the same worktree at the same time, so if a child writes in the current worktree the parent waits until it finishes (sequential delegation, not parallel); (4) parallel launches always give each child its own worktree, with no option. See AG-26, AUTO-17, AUTO-20 and AUTO-32.
- **Routines and Automations are one core scheduling engine with two presentations**: Routines for non-technical users in Bot Teams, Automations for software engineers in Workspaces. A Calendar on the activity rail shows both on one timeline.

As a result, AUTO-01 to AUTO-04, AUTO-06 to AUTO-08, AUTO-12 to AUTO-18, AUTO-20, AUTO-21, AUTO-28 and AUTO-30 are proposed for the first version. AUTO-28 to AUTO-31 are new features that the current app does not have.

## pv96 delta (unmerged)

- Changed: AUTO-12, AUTO-13, AUTO-14, AUTO-15, AUTO-19, AUTO-20, AUTO-28, AUTO-29, AUTO-30 and AUTO-31. Each has a `Source: pv96 (unmerged)` line.
- New: AUTO-32 to AUTO-37.
- Removed: none. The legacy `orchestration.run` loop is still registered.
- Unchanged in pv96: the automation engine. `src/main/automations`, `src/shared/automation*`, the automation RPC methods and the automation CLI show no difference from custom/integration, so AUTO-01 to AUTO-11 keep their text. AUTO-16 to AUTO-18 changed in store and mailbox files, but their behaviour was not traced, so their text stands. AUTO-21 (depth limit) has no file change. AUTO-22 to AUTO-27 were not traced in this pass; AUTO-25 to AUTO-27 are artifacts, outside this delta.
- Owner direction lines are unchanged. Where pv96 differs from them, the feature's `Source` line says so: routines run on their own scheduler, not the Automations engine (AUTO-28); web address calls reach only this computer (AUTO-29); Integrations is still a placeholder (AUTO-12); the routing query does not use Jev and no team has a lead role (AUTO-13).

## Source directories read

All paths are relative to the reference repository root.

- `src/main/automations/`: all non-test files (scheduler service, dispatch, run target resolution, refusals, precheck, run completion and usage, retention, external managers, Hermes/OpenClaw readers, owner migration). Several were read only by their header comments.
- `src/main/persistence/scheduling-automations/`: definition, run and owner operations (signatures and derivation rules; not every function body).
- `src/main/startup/main-process-automations.ts`, `src/main/runtime/runtime-automation-controller.ts` (grep and validation rules), `src/main/runtime/orca-runtime-automation-operations.ts` (orchestration DB path).
- `src/main/runtime/rpc/methods/automations.ts`, `automation-schemas.ts`, `bot-teams.ts`, `bot-team-room.ts`, `artifacts.ts`, `runtime-artifact-controller.ts`: method names and main handler logic.
- `src/main/runtime/rpc/methods/orchestration/`: file listing and names; headers of `gates/gates.ts`, `federation/federation-attachment-observation.ts`, `delegation/`.
- `src/main/runtime/orchestration/`: `types.ts`, `coordinator.ts` (structure and options), `coordinator-escalation-triage.ts`, `preamble.ts` (headers), `lifecycle-reconciliation.ts` and `worker-output-archive.ts` (comments), `orchestration-schema-version-skew.ts`, `db/bot-teams/`, `db/delegation/`, `db/dispatch-context/` (circuit breaker), `db/dispatch-depth.ts`, `db/schema/` (table and migration names).
- `src/main/bot-teams/bot-team-task-start.ts`.
- `src/main/artifacts/`: `artifact-publisher.ts` (first 120 lines), `artifact-cloud-service.ts` (first 140 lines), `artifact-cloud-config.ts` and `artifact-cloud-request.ts` (names), share-record and create-intent store headers.
- `src/main/delegated-session/`: router, request service, answer and lifecycle headers, task classifier and model tiers (headers), agent ranking and access-mode helpers (headers).
- `src/main/ssh/ssh-remote-cli-host-passthrough.ts` (artifact input forwarding and environment), `ssh-remote-orchestration-send.ts` (comments), `src/main/ssh/` names only for the orchestration relay.
- `src/shared/`: `automations-types.ts`, `automation-execution-target.ts`, `automation-owner-ref.ts`, `automation-owner-key.ts`, `automation-owner-conflict.ts`, `automation-list-scope.ts`, `automation-run-retention.ts`, `automation-precheck.ts`, `automation-schedules.ts` (headers), `automation-schedule-parsing.ts` (exports), `bot-team-request-route.ts`, `bot-team-lead-selection.ts`, `artifacts.ts`, `artifact-sharing-gate.ts`, `artifact-cli-bridge.ts` (names), `nested-worker-depth.ts`, `rpc-contract/bot-team-params.ts`, `rpc-contract/bot-team-room-params.ts`, `rpc-contract/automation-params.ts` (names).
- `src/cli/specs/automations.ts`, `specs/artifacts.ts`, `specs/orchestration.ts`, `specs/orchestration-worker-specs.ts`: full command lists, usage, notes.
- `src/cli/handlers/artifacts.ts` (preflight, remote input, sharing errors); `handlers/automations.ts` and `automation-handler-flags.ts` (grep only).
- `src/cli/bundled-skill-guides.ts`: grep only, to confirm that the bundled `orca-cli` skill teaches agents the automation and artifact commands.
- Persistence migration helpers that move automations between hosts: `src/main/persistence/migrating-orcad-catalog/orcad-source-automation-state.ts` (transfer rules only).
- `src/main/ipc/automations.ts`: grep only, for the precheck IPC name.

### pv96 additions (read at `d3c6311a531`)

- Odd docs: `odd/tasks/bot-teams.md`, `odd/tasks/bot-teams-pv91-qa.md`, `odd/tasks/child-bring-back.md` (all read), and the orchestration parts of `odd/tasks/delegated-child-sessions.md` (decisions, progress and follow-ups).
- `src/main/bot-teams/`: routine scheduler and host, webhook listener and endpoint, request query and flow, conversation answers, owner selection, reassign, teammate delegation, memory read, Engram project name, task versions, finish report (headers and key constants).
- `src/main/runtime/orchestration/db/bot-teams/` and `db/delegation/` (store file names and the routine store), `db/contract-constants.ts` (`SCHEMA_VERSION`), `db/schema/migrate-v46` to `migrate-v61` (file names).
- `src/main/runtime/rpc/methods/bot-team*.ts` and `orchestration.ts` (method names and registration).
- `src/shared/bot-team-*.ts` (task status, board column, request classification, routine schedule, folder name, owner selection), `src/shared/child-bring-back.ts`, `src/shared/rpc-contract/bot-team-params.ts`.
- `src/main/delegated-session/`: `delegation-jev-tier.ts`, the constants in `delegation-agent-ranking.ts`, `child-bring-back*.ts`.
- `src/cli/specs/orchestration-worker-specs.ts` (team commands and `ask-user`) and `src/cli/handlers/bot-teams-admin.ts` (command paths only).
- `src/renderer/src/components/calendar/` (sources and page), `components/settings/BotTeamsIntegrations.tsx`, and `components/bot-teams/` (names and the Workspaces and project gating lines only).
- `src/main/index.ts` (scheduler start) and `mobile/src/bot-teams/` (names only).

## Not read / low confidence

Not read:
- Renderer UI for automations, orchestration, delegation and artifacts (out of scope; only names and calls seen through the RPC layer). For bot teams, pv96 renderer code was read only for the Calendar sources, the Integrations copy and the gating lines.
- Mobile client, except bot team file and screen names in pv96. Not read in pv96: the artifact paths (AUTO-25 to AUTO-27), the Marketplace apps service and the Jev decision client.
- Body of most orchestration RPC handlers: worker start, release, stop, worker read, messaging send and check, federation, gates, runs (only names and headers).
- Mailbox pointer delivery internals, the full preamble text, the worker transcript readers, and the orchestration DB migrations (only table names and migration file names).
- Coordinator decomposition loop body (`coordinator.ts` beyond its options and structure).
- Delegation launch, answer and lifecycle bodies (not traced in pv96 beyond the files named in the pv96 read list); the `delegation-options.ts` card builder.
- Precheck runner body (command execution details) and the run completion watcher body (comments only).
- Runtime terminal run observer body (constants and comments only).
- Artifact cloud service and publisher beyond the first lines; share-record and create-intent store bodies; cloud-side artifact expiry duration (outside this repo's read scope).
- The `botTeams.delete` RPC handler body and any on-disk cleanup of a deleted team's folder.
- Session reuse mechanism for existing-workspace automations (only validation and CLI text read).
- Tests were not read, except as named in the file list.

Low confidence:
- How an SSH-targeted automation is actually executed (scheduler owner `ssh_bridge`, dispatch through the desktop renderer). Inferred from the owner derivation and the dispatch path. Behaviour when the SSH connection is down during a run was not verified.
- Whether the desktop runtime keeps running after the window closes. The automation scheduler is documented as desktop-main-process only, but process lifetime after the last window was not confirmed.
- Desktop precheck trigger: the renderer appears to request it over IPC after a dispatch request. Not traced end to end.
- Session reuse: the CLI says later runs go to the previous live session. The code that resumes it was not found.
- Purpose of the `orchestration.partyLocation` RPC.
- Whether any shipped UI still calls the legacy coordinator-loop RPC (`orchestration.run`, `orchestration.runStop`). pv96 still registers both RPCs; no UI caller was searched for.
- Resolved for pv96: `workspacesEnabled` has readers (see AUTO-12).

## Cross-cutting behaviour

Where automations run:
- Desktop-owned automations (local projects) are scheduled by the desktop main process. Each due occurrence asks the renderer to launch an agent in a terminal. With no window attached, the occurrence is recorded as unavailable.
- Runtime-owned automations (projects on a paired MassOrca server) are executed only by a headless server process. The desktop client mirrors them in the list and refuses to run them.
- SSH-owned automations are scheduled by the desktop that holds the SSH connection (medium confidence).

When the app is closed:
- Desktop schedules do not run while the app is closed. On the next start, the latest due occurrence runs once if it is within the grace window, otherwise it is recorded as missed. Earlier occurrences are never replayed.
- A headless server keeps running the schedules it owns while it is up.
- Bot Team routines, the web address listener and task naming start from the main process once its runtime is ready (`src/main/index.ts`). Whether the standalone host starts them was not verified.

Remote hosts:
- Ownership is fenced to a host incarnation (paired runtime revision, SSH registration generation). A removed-and-re-added host is detected and its automations are refused until re-adopted.
- Orchestration state lives in the runtime's own database. Remote workers are reached through federation (cross-server relay with acknowledgements) or the SSH-hosted CLI relay. Transport loss marks a remote worker as unverifiable, not exited.
- Artifacts always publish from the signed-in desktop account, including when the agent runs on an SSH host.

## Features

### AUTO-01 Scheduled automation definitions

- What the user does: creates an automation with a name, prompt, agent, schedule, timezone and destination, from the app or from `orca automations create | edit | show | list | remove`. Can enable or disable it. Removing deletes the automation and its run history.
- Schedule forms: presets (hourly, daily, weekdays, weekly), a 5-field cron expression, or an RRULE string. Cron expressions that can never occur are rejected. A missed-run grace setting is optional.
- Rules: an automation with no resolvable project cannot be created. A repeated create with the same client creation key returns the existing record. An update that does not move the destination keeps the stored host target. An edit that changes what a dispatch depends on is caught at launch ("The automation changed before this run could launch.").
- Edge cases: records that lack owner metadata are classified on load (see AUTO-08). Clearing extra arguments is done with an empty value on edit.
- Persisted: name, prompt, agent, extra arguments, precheck, run and source context, destination and workspace mode, schedule, timezone, start time, enabled flag, next and last run times, grace minutes, owner fields, timestamps.
- Depends on: AUTO-02, AUTO-03, AUTO-05, AUTO-06, AUTO-08; Workspaces; Agent launch; Settings (agent availability).
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): one core scheduling engine with two presentations: Automations in Workspaces mode, for software engineers (D-AG-14), and Routines in Bot Teams mode, for non-technical users (AUTO-28). The Calendar (AUTO-30) shows both on one timeline. Proposal: first version.

### AUTO-02 Destinations and workspace modes

- What the user does: picks where a run happens: an existing workspace (one worktree or folder, every run goes there), or a new worktree per run inside a project. The destination is given by repository selector, workspace selector, project plus host, or project host setup.
- CLI default: when no destination is given, the enclosing MassOrca worktree is used if one resolves from the current directory.
- Rules: the destination must be set up on its host and in state `ready`; the repository must still be attached to that host; the project path must match the stored setup. Otherwise the run is refused with a fixed sentence per cause (not set up anymore, setup in another state, target no longer matches, repository no longer attached, path changed, host no longer registered). Refusals are recorded as run history rows, folded when they repeat.
- New-per-run worktrees are created with the run title and an optional base branch. Setup for those worktrees follows the normal worktree setup flow (not read).
- Session reuse is only valid for existing-workspace automations (see AUTO-06).
- Depends on: Workspaces (git worktrees and folders), SSH hosts, project host setups.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): the shared engine needs destinations for automations and routines alike. Proposal: first version.

### AUTO-03 Schedule engine, missed runs and retention

- What the user gets: runs fire on schedule without user action, provided the owning host is running and (for desktop) the app is open.
- Mechanics: a tick every 60 seconds, plus a pass at startup and when the desktop window reports ready. Only enabled automations whose next run time has passed are evaluated.
- Latest occurrence only: each evaluation considers the most recent occurrence. A long outage produces at most one run, never a backlog.
- Missed-run policy: run once if within the grace window. Default grace is 720 minutes when not set. The occurrence is recorded as `skipped_missed` when it is older than grace plus two ticks.
- Window closed while the app runs: due occurrences are recorded as `skipped_unavailable` with a fixed sentence ("No MassOrca window was available to launch the automation."). Repeats fold into one row with a count. Those occurrences are not replayed later.
- Unevaluable schedule: one broken automation never blocks the others. Its failure is recorded once and retried each tick, so a repaired schedule resumes on its own.
- Drift: on start, automations whose stored next run time disagrees with their schedule are reported (diagnostics path; user visibility not verified).
- Retention: the last 100 final runs per automation are kept. Runs still in progress are never evicted.
- Depends on: AUTO-04, AUTO-07, AUTO-09; Settings (none beyond the owner host).
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): this engine is shared by Automations, Routines and the Calendar. In the new product the core always runs as a service, so the current rule that desktop schedules pause while the app is closed comes from the Electron process model and need not carry over (a design decision, not taken here). Proposal: first version.

### AUTO-04 Run now and run history

- What the user does: runs an automation immediately (`orca automations run <id>`), and reviews its history (`orca automations runs`, paged by limit and cursor, newest first).
- Rules: a manual run is dispatched at once and is not subject to the precheck. A manual run refused for host or owner reasons is still written to history as `skipped_unavailable`, so the person sees why.
- History rows carry a unique run number that is never reused after pruning, a title, status, workspace display name, terminal identity, precheck result, output snapshot and usage summary.
- Run statuses: pending, dispatching, dispatched, completed, skipped_precheck, skipped_missed, skipped_unavailable, skipped_needs_interactive_auth, dispatch_failed.
- Depends on: AUTO-03, AUTO-07, AUTO-08.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): each routine run is kept in its history (D-AG-25). Proposal: first version.

### AUTO-05 Precheck gate

- What the user does: sets an optional shell command, with a timeout, that must succeed before a scheduled run starts (for example, "a PR is waiting").
- Rules: applies to scheduled runs only, never to manual runs. Exit code 0 continues. Any other outcome (non-zero exit, timeout, launch error) records `skipped_precheck` with a formatted reason. Timeout is clamped between 1 second and 10 minutes.
- Output: standard output and error captured, each truncated to 4,000 characters, kept on the run.
- Execution: local runs execute in the destination folder on the owner host. SSH runs execute over the SSH connection. On timeout, the whole process tree is killed (Windows walks the tree).
- Depends on: AUTO-02, AUTO-03; SSH hosts.

### AUTO-06 Agent launch for runs, extra arguments and session reuse

- What happens: each run opens a terminal, starts the chosen agent with the saved prompt and the run title, and leaves it running. Desktop runs are launched through the renderer. Headless runs use the runtime's terminal launch.
- Agents: the set of terminal agents MassOrca knows (the `--provider` flag accepts those ids).
- Extra arguments: appended after the host's default agent arguments on each fresh run. Allowed only for Claude, Codex, CodeBuddy, Cursor, Grok and OMP, and only a safe subset (for example model, effort, add-dir). Cannot be combined with session reuse.
- Session reuse: only for existing-workspace automations. Later runs are submitted to the previous live session of that automation when it is still available. `--fresh-session` turns it off. Setting reuse on a new-per-run automation is refused ("Session reuse requires an existing workspace target.").
- Depends on: Agent launch, Terminals, AUTO-02.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): routines and automations launch agents through the same path. Proposal: first version, with plain launch first.

### AUTO-07 Run completion, output and usage

- Completion detection: after dispatch, a run stays `dispatched` until the agent reports done through its own status for that terminal (hooks, titles or terminal-escape status). For agents that do not report status, an idle pane counts as done only after a two-minute start grace and only if no shell reported "command not found" for the agent's command.
- Observation budget: the watch is bounded in wall-clock time. Surviving runs are re-attached after an authority restart, once the terminal surface is ready. A run whose terminal cannot be found after that grace is closed with a truthful "stranded" reason. It is never reported as completed.
- Agent exit without completion (headless path): recorded as `dispatch_failed` with "The agent exited without reporting completion."
- Output: a plain-text snapshot of the run's output. Headless keeps the most recent 256K characters (older output is dropped from the front). The snapshot is stored with the run.
- Usage: for finished runs, token usage is read from Claude and Codex session data within the run's time window. Status is `known` or `unavailable`, with reasons: not finished, provider unsupported, remote usage unavailable, usage not enabled, scan failed, no matching session, ambiguous session. Cost is an API-equivalent estimate.
- Depends on: Terminals (status and idle detection), Agent launch, Usage tracking (Claude and Codex), AUTO-03.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): run history on the Routines and Automations pages needs an outcome. Proposal: first version, with completion taken from agent status rather than heuristics.

### AUTO-08 Host ownership, fencing and orphans

- What the user sees: each automation belongs to one host (the desktop, a paired MassOrca server, or an SSH target). Automations whose host is gone show in an orphan bucket with the reason.
- Owner model: an owner is the authority (desktop, or a runtime environment with its pairing revision) plus a host selector (this host, or an SSH target with its registration generation). Changing or running an automation from a client must echo the owner it saw. Mismatches return typed conflicts: owner changed, SSH target removed, fencing required, invalid destination. Old clients have their owner captured at the protocol boundary.
- Orphans: SSH target no longer registered; target removed and re-registered under the same id (generation changed); workspace spanning more than one host. Orphans never run. The list returns an orphan count.
- Migration on load: generations are stamped on live SSH targets. Automations are adopted or classified as owned, orphan or ambiguous. Running it again changes nothing.
- Re-adoption: when an SSH target is removed and re-added under the same id, its automations and runs are moved to the new id, and the captured generation is rewritten to the new registration.
- Persisted host filters use stable references, not incarnation fields.
- Depends on: SSH hosts (AUTO-09), Runtime environments (pairing), Project host setups.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): the Routines page names the host the team runs on. Because the core runs both on the desktop and on a server, each schedule needs exactly one owner host. Proposal: first version.

### AUTO-09 Remote and SSH automation scheduling

- Scheduler owner by project host: local project, scheduled by the local host; SSH project, scheduled by the bridge owner (the desktop with the SSH connection); paired runtime project, scheduled by that runtime only.
- Server behaviour: a headless server process executes runtime-owned automations and the local ones stored on it. Fresh worktrees and launches are done without a renderer. Runtime-owned runs are allowed only when the server is in headless mode and the automation is owned by a remote host service.
- Desktop behaviour: the desktop lists runtime-owned automations and refuses to run them with: "Remote-server automation scheduling is not available from this MassOrca client yet. Run this automation on the remote server or update MassOrca when durable remote scheduling is available."
- Transfer: during an orcad source migration, disabled SSH-owned automations can move to a new owner when the target id, generation and projects match. Enabled ones do not.
- Depends on: AUTO-08, AUTO-02; SSH hosts; Runtime environments; orcad migration.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): not named by the owner's decisions. Proposal unchanged: later.

### AUTO-10 Headless run terminal retention

- What the user gets: on a headless server, run terminals do not pile up. A finished run's terminal closes after a 10-minute grace. The newest three terminals per automation stay viewable.
- Rules: a failed or still-running run's terminal is closed only when its shell is proven alone at a prompt, since the agent may still be alive. A terminal someone typed into, is viewing, or whose use cannot be determined is never closed. On the desktop, the window closes run terminals on completion instead.
- Depends on: AUTO-07; Terminals.

### AUTO-11 External Hermes and OpenClaw cron managers

- What the user does: sees Hermes and OpenClaw cron jobs (from other agent tools) in the Automations view, on the local machine or on a connected SSH target. Can pause, resume, run now, and delete jobs. Hermes jobs can also be created and edited (prompt and schedule are required; the name defaults from the prompt).
- Data: read from each tool's own files and CLI. MassOrca does not store these jobs. Run history comes from Hermes output files and its session database. Badge counts are cached.
- Rules: probes run only for scopes the person selected and yield to automation list and mutation traffic. Leaving a host's scope cancels its probes. Lookup of a command on PATH times out after 5 seconds; mutations after 30 seconds. An older SSH relay without the method returns a message asking the person to reconnect the target.
- OpenClaw: pause and resume map to its disable and enable verbs. Create and edit are not supported.
- Depends on: SSH hosts, the Hermes and OpenClaw CLIs.
- Proposal: drop. These are other tools' schedulers. Their value is in importing or migrating them, which is a separate decision.

### AUTO-12 Bot team roster and Bot Teams mode

- Source: pv96 (unmerged). Teams and bots can be edited after creation, in the form used to add one. A bot's model and effort are picked from its job unless set by hand under advanced options. The team's Workspaces option now has readers: it shows the team folder in Workspaces and gates Connect to code, project memory from Engram and project edit rights. A new team starts with full access, as before.
- What the user does: creates a named team with a purpose, a character (one of six shapes, cosmetic), an optional "workspaces" flag and initial bots. Each bot has a name, a job description and a terminal agent. Adds or removes bots, and changes the team's access mode.
- Rules: a bot's agent must be a known terminal agent. A new team always starts with full-access mode. Deleting a team removes its bots, tasks, messages and workspace link from the store.
- Gaps: the Workspaces option gives a team folder and project links only. A full Workspaces flow inside the team (phone Versions, deliverables from linked worktrees, finish reports) is partial in pv96.
- Depends on: AUTO-14, Agent launch, Settings (access modes).
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): Bot Teams is a core mode of the product, switched from the activity rail (D-SH-10). In that mode the sidebar lists the teams, each with its Conference room and Board, plus a Projects section with Files, Memory and Routines (D-WS-29). Bots are added on the team page (D-AG-13). A bot's job description is its competence, which the lead bot uses to delegate (AUTO-13). A team can turn on Workspaces to reach Workspaces depth inside Bot Teams, which gives the stored but unread `workspacesEnabled` flag a meaning. Proposal: first version (was later).

### AUTO-13 Conference room: requirements become Board tasks (current app: bot team room and request routing)

- Source: pv96 (unmerged). Bots now reply in the room, and one host query classifies each request. custom/integration posted nothing from bots and routed only by `@Name` or to a team's only bot.
- What the user does: posts a message in a team's Conference room. A message with `@Name` of a bot or a team goes there with no query. A greeting gets one reply from All. Any other message gets one short query on the Settings default agent (Jev is not wired into this query in pv96). The query returns a task for one or more bots, a composite task, a task for another team, no team, or a conversation. A composite task goes to the least busy bot it names, which hands the other parts to their bots. A conversation gets one answer from one bot, under its own name, of at most 1,200 characters. An unclear request gets a choice card: create a bot or team, send it to an existing one, or keep it in this team. A request for later goes to Todo, and a request with a day and time becomes a one-time routine (AUTO-28). The composer's `@` menu lists the team's bots, other teams, projects and files.
- Task creation: the first line of the message, without the mention, becomes the task title (80 characters max). The full message is the prompt. By default the task starts at once. With start off, it waits in Todo.
- Rules: bots post in the room as themselves or as All. A reply to a quoted message keeps the quoted line as its context and never asks which project it is for. The room shows a typing line while a bot works, and a reply task is not listed as a task there. Files sent with no words stay in the room, and the person can choose to save them only.
- Owner selection: when several bots match, the one with the fewest active tasks owns the task (`bot-team-owner-selection.ts`). A team has no lead role; the custom/integration lead helper is gone.
- Proposal for the custom/integration behaviour (before the owner's direction): drop. pv96 replaces that behaviour, so the owner direction below sets the proposal.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): the Conference room is a chat with the team's bots (D-AG-24). It detects the requirements the user states and creates tasks on the team's Board (AUTO-14). A composite requirement goes to the lead bot, which delegates the parts by competence (AUTO-20, shown as a delegation tree, X-DL-01). When the user asks to schedule a task for later, the room creates a routine (AUTO-28). Differences from the current app: bots reply in the room (today they never post), tasks come from detected requirements instead of the first message line, and routing is by competence through a lead bot instead of `@mention` or the only bot. Proposal: first version (was drop).

### AUTO-14 Team Board, bot tasks and background sessions

- Source: pv96 (unmerged). Tasks have the states Not started, Unknown, Working, Waiting for you, Stopped, Stuck and Done, shown in Todo, In Progress, In Review and Done columns. Done is read from the owner session's finished turn and kept on the host; a turn cut off by a stop is Stopped, and a failed turn is Stuck. A waiting task can be given to another bot (AUTO-34), and a finished task's team folder is saved as a version (AUTO-36).
- What the user does: starts a waiting task, or a task created from the room. The task runs in its owner bot's background agent session.
- Mechanics: the first start creates a folder workspace under the home directory (`MassOrca Bot Teams/<team id>`) in a project group named "Bot teams". It starts a structured chat session for the owner bot in that workspace, with the team's access mode, but does not activate it. The prompt tells the bot its name, team and job, then gives the task text. The task is linked to the session only after delivery succeeds.
- Failure: if the prompt is not delivered, the task stays in Todo with an error and can be started again. A started task cannot be started a second time.
- Gaps: none for the task state (see Source). A finish that the host never reports is not seen by a later answer in the room.
- Depends on: Workspaces (folder workspaces), Agent launch (structured chat sessions), AUTO-12, AUTO-13.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): the team Board (D-WS-30) is the Bot Teams source of tasks, and the Tasks page on the activity rail lists Board tasks next to tracker items (D-WS-24; assumed in the design, A25). The current gap matters now: a Board needs a done state, which the current store lacks. Proposal: first version (was later).

### AUTO-15 Orchestration Runs and their state

- Source: pv96 (unmerged). The database is at schema 61. Migrations v46 to v61 add the bot team tables (room, projects, memory, routines and their runs, web addresses, delegation links, task origins and returns) and their columns. The legacy RPC `orchestration.run` and `orchestration.runStop` are still registered.
- What agents do: create or bind a lightweight Run (`orchestration run-create`, `run-use`, `run-current`, `run-list`, `run-show`). Tasks, dispatches and messages belong to a Run. Group addresses reach only the live dispatches of that Run.
- Reset: `orchestration reset` clears one explicit scope (all state, tasks, or messages).
- State store: one SQLite database per runtime in the app's user data folder. Tables cover Runs, tasks, dispatch contexts, messages, gates, questions, worker dispatches, federation relay items and attachments, mutation receipts, delegation requests, bot teams and legacy adoption records. Migrations run from version 2 to 61 in pv96.
- Retired path: `orchestration run` and `run-stop` (also `coordinator-start` and `coordinator-stop`) perform no effects in the CLI and point to the current skill. The RPC `orchestration.run` still starts an in-process coordinator loop: it decomposes work, dispatches up to four tasks at once, polls every two seconds, and allows one active loop per runtime. Who calls that RPC was not verified.
- Proposal for the current behaviour (before the owner's direction): the Run concept is later. The legacy loop is a drop.
- Depends on: AUTO-16 to AUTO-19; Agent launch; Multi-server federation (AUTO-22).
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): orchestration is in the first version as one core delegation engine shared by Bot Teams (the lead bot delegates by competence) and Workspaces (a coordinator coding agent launches child agents, sequential or parallel, with worktree handoff). Child agents show as a tree inside the parent session or the Conference room, not in a floating panel (X-DL-01). Proposal: the Run concept is first version; the legacy loop (AUTO-24) stays a drop.

### AUTO-16 Tasks and dispatch to terminals

- What agents do: create, list and update tasks (statuses: pending, ready, dispatched, completed, failed, blocked). Dispatch a task to an existing terminal with `orchestration dispatch --task <id> --to <handle>`. Dry-run and preamble preview are available. Inspect the dispatch context with `dispatch-show`.
- Preamble: a dispatch sends the worker instructions that name its identity and task, and the return path for messages. Redispatch text tells the worker to keep its shell open and wait for the next message.
- Circuit breaker: three failed dispatch attempts for one task mark it `circuit_broken` and the task failed. Escalations count as failures. Below the threshold, the task returns to pending for another try.
- Retry safety: each attempt has its own dispatch identity. Messages from an old attempt cannot complete a retry. Mutations carry request ids, and `request-show` reports whether one already took effect.
- Preflight: a dispatch can warn when the target worktree is behind its remote.
- Depends on: AUTO-17, AUTO-18, Terminals.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): delegated parts become tasks dispatched to workers, in both modes (X-DL-01). Proposal: first version.

### AUTO-17 Worker lifecycle

- What agents do: start a worker with `orchestration worker-start`, from a task id or from spec text. The worker runs on the Run's home host or on a connected MassOrca server chosen by saved environment name. Agent is given by id or by an existing terminal handle.
- Placement: `current`, an existing worktree selector, a new child worktree, or a new top-level worktree. New worktrees take a name, repository, base branch, display name, comment and setup mode (run immediately, or wait for setup before the agent gets input). Current and existing worktrees never rerun setup.
- Kind: a structured chat worker when the setting for new agent tabs is chat or `--chat` is set. Otherwise a terminal worker. The start receipt says which one ran and why.
- Access modes: default, full-access, auto, accept-edits, plan, read-only. They apply only to structured workers created by the start. An agent that cannot apply a mode gets the safest mode it declares. Model and effort options are per agent; effort requires a model.
- Reading and ending: `worker-show`, `worker-read` (transcript, local or remote), `worker-list`, and stop, abandon, release (archives output and frees the terminal) and retain. The output archive keeps a bounded tail of raw terminal output.
- Depends on: Workspaces (worktrees), Terminals, Agent launch, AUTO-16, AUTO-22.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): in Workspaces mode a coordinator coding agent launches child agents, sequential or parallel, with worktree handoff (X-DL-01). Proposal: first version; remote worker servers (AUTO-22) stay later.
- Owner direction for the new product (2026-11-12, child placement rules from the design session): (1) a delegated child session gets its own worktree by default; (2) it may use the current worktree only when the child only reads (research, review, explain), when the workspace is a plain folder without git, or when the user picks it by hand on the delegation card; (3) two sessions never write in the same worktree at the same time, so if a child writes in the current worktree the parent waits until it finishes (sequential delegation, not parallel); (4) parallel launches always give each child its own worktree, with no option. The observed placement choices (`current`, an existing worktree, a new child worktree, a new top-level worktree) stay, but `current` is limited to rule 2, and a worker launched in parallel always gets its own worktree. Proposal unchanged.

### AUTO-18 Agent-to-agent messages

- What agents do: send a message (`orchestration send`) with a subject, body, type, priority, thread id and optional payload. Types: status, dispatch, worker_done, merge_ready, escalation, handoff, decision_gate, question, heartbeat. Priorities: normal, high, urgent.
- Addressing: a Run, a dispatch, a terminal handle, or a group (`@all`, `@idle`, provider groups such as `@codex`, `@worktree:<id>`). A Run's group excludes its own coordinator. Groups need quoting in PowerShell.
- Completion: `worker_done` needs an outcome (succeeded or failed) and settles exactly the task and dispatch it names. Rejections carry codes: unknown task, unknown dispatch, task and dispatch mismatch, inactive dispatch, stale dispatch. Duplicate settlements are detected. `heartbeat` and `worker_done` cannot target groups.
- Reading: `orchestration check` or `inbox` lists unread messages for the caller. `reply` answers by message id. Delivery is tracked by a delivery contract (direct legacy, current, audit only), and restart reconciliation of pending deliveries exists (its details were not read).
- Depends on: AUTO-16, AUTO-17; Terminals (handles).
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): children report back to the lead bot or the coordinator, and the delegation tree shows each child's status (X-DL-01). Proposal: first version.

### AUTO-19 Decision gates and blocking questions

- Source: pv96 (unmerged). `orca orchestration ask-user` asks the person a multiple-choice question in the caller's chat and waits for the answer, through the question card that chat agents share. Gates are unchanged.
- Decision gates: a coordinator or worker creates a gate that blocks a task until it is resolved. Gates are pending, resolved or timed out. They can be listed and resolved from the CLI.
- Blocking questions: an agent can ask the coordinator a question and block until it is answered (`orchestration ask`). Questions are stored as threads and answered with `orchestration reply`.
- Not read: how a person resolves gates and questions in the app UI.
- Depends on: AUTO-16, AUTO-18.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): not named by the owner's decisions. Proposal unchanged: later.

### AUTO-20 Delegation from chat with agent choice

- Source: pv96 (unmerged). A bot hands work to a teammate with `orca orchestration team-delegate --bot <name> --text <task> [--team <team>]`. The teammate's agent is the bot's own, so no card appears; the child starts through the same lifecycle and works in its own folder, not the project's main checkout. With `--team`, the task is filed in another team. An optional Jev advisor, when wired, picks the tier from the one-line summary only; the keyword classifier is the fallback.
- What the user does: an agent in a MassOrca chat asks to delegate a piece of work (`orchestration delegate` with a one-line summary of up to 160 characters, the prompt, an optional title, and `--writes` if it will edit code). The call returns at once with a request id. A card appears in the chat asking which agent should run the child, or Automatic.
- After the choice: the child starts as a new worktree when it writes in a git workspace. In a folder workspace it runs in place, with a warning. The child inherits the parent's access mode and never a wider one. When the child finishes, its result is posted back into the chat, with start, failure and finish notices.
- Automatic routing: the summary is classified into a tier by Jev when Jev is wired, otherwise by keyword rules and a write flag. High-risk work raises the tier to a floor of 4. Agents are ranked by the tighter of their 5-hour and weekly usage windows; the 30-day allowance binds only when neither is known. A stale or missing reading counts as 60% and is labelled, and an agent at 90% or more is listed but not picked. The top eligible agent wins, and its model and effort are chosen from a per-tier table, with effort clamped where the model does not support it. If no agent is eligible, the request fails with `no-eligible-agent`.
- Request states: pending, starting, started, failed, cancelled, finished. Restart handling: an answered request that never started fails; a child that finished while nothing listened is closed on reconcile.
- Refusals (no effects applied): the parent chat is not held by this host, or no agent able to run a chat is usable.
- Depends on: Chats, Agent launch, Usage tracking (rate limits), AUTO-16, AUTO-17.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): delegation is in the first version. For Bot Teams the lead bot picks children by competence (each bot's job). For Workspaces the coordinator picks child agents. The current usage-based Automatic routing (ACC-26) is not required by the decision and can follow. Proposal: first version (was later).
- Owner direction for the new product (2026-11-12, child placement rules from the design session): the child placement rules apply to delegation from chat and to `team-delegate`: (1) a delegated child session gets its own worktree by default; (2) it may use the current worktree only when the child only reads (research, review, explain), when the workspace is a plain folder without git, or when the user picks it by hand on the delegation card; (3) two sessions never write in the same worktree at the same time, so if a child writes in the current worktree the parent waits until it finishes (sequential delegation, not parallel); (4) parallel launches always give each child its own worktree, with no option. The observed rule "in a folder workspace it runs in place, with a warning" stays allowed by rule 2, but a writing child there now makes the parent wait. The Bot Teams case (a team folder that may not be a git repository, with a lead bot delegating parts that could run in parallel) is an open question in open-questions.md, not resolved here. Proposal unchanged.

### AUTO-21 Nested worker depth limit

- What the user gets: a control on how deep workers may dispatch further workers. Root coordinators are depth 0 and their workers depth 1. The default maximum is 1, so a worker cannot dispatch sub-workers. A refused worker is told to complete the task itself.
- Setting: Settings, Orchestration, "Nested worker depth". Invalid values clamp to at least 1.
- Depends on: AUTO-16; Settings.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): the depth limit is a setting in the agents section of settings (D-ST-02). Proposal: first version.

### AUTO-22 Multi-server federation of runs and workers

- What the user gets: a Run whose coordinator lives on one MassOrca server and whose workers run on other connected servers (the "run home" and "worker servers"). Paired by peer fingerprint.
- Operations: federation setup, attach start, pull, import, acknowledge, read, read output, show, release, stop, and a fleet snapshot of worker status.
- Mechanics: store-and-forward relay. Items travel home-bound or worker-bound, each with a sequence number and an acknowledgement. Imports are idempotent. Malformed peer pages fail as orchestration errors. Peers negotiate capabilities and are cached.
- Remote worker states: unattached, missing, identity changed (the terminal's process differs from the recorded one), live, exited, unverifiable. Transport loss is unverifiable, never exited. Only the execution host can prove an exit.
- Fleet snapshot reasons: missing status, stale status, future status, restored but unconfirmed.
- Version skew: a schema-skew module lists the columns and versions that older databases may lack. Its exact peer checks were not read.
- Depends on: AUTO-17, AUTO-18; Pairing of MassOrca servers (other domain); SSH hosts (for the SSH relay in AUTO-23).
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): not named by the owner's decisions. Proposal unchanged: later.

### AUTO-23 Orchestration over SSH hosts

- What the user gets: workers and messages work on an SSH host through the `orca` CLI installed there, with the same verbs as local ones.
- Mechanics: the remote command receives the SSH bridge credential and the host identity (kind, target id, connection incarnation, attachment), so lifecycle decisions are attributed to that SSH host. A remote command never claims a local agent session identity. If the remote install is broken, a fallback path keeps the lifecycle flags so payloads stay valid. Transcript reads over SSH are bounded to a scan window.
- Depends on: SSH hosts, AUTO-17, AUTO-18.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): not named by the owner's decisions. Proposal unchanged: later.

### AUTO-24 Legacy coordinator loop (orchestration.run and runStop)

- What it is: an in-process loop that takes a spec, breaks it into tasks, dispatches workers and merges results. It is reachable through the RPC layer. The CLI commands for it are retired and now only point to the skill.
- Proposal: drop. The lightweight Run, Task and worker commands replace it. Keeping the loop would mean keeping its state rules, which is the largest part of the coordinator code.

### AUTO-25 Artifact sharing commands

- What the user does: `orca artifacts share <file>` publishes an HTML or Markdown file to the signed-in MassOrca account and returns a link. `update <file>` republishes a file shared earlier from the same profile, keyed by its path. `unshare <file>` deletes the artifact tied to that path. `list` pages the account's artifacts. `delete <id>` (alias `rm`) deletes by id.
- Limits: content up to 10 MB. The CLI envelope is capped at 800 KB, so larger files fail on the way in. The cloud accepts up to 11 MiB request bodies.
- Remote use: artifact commands reject remote selection flags (`--host` and similar). Artifacts always use the signed-in desktop account. When an agent runs on an SSH host, the desktop reads the file and forwards its content and an SSH-scoped source key to the remote CLI, which then publishes through the desktop account.
- Expiry: each artifact carries an expiry time that is refreshed when it is republished. The duration is set by the cloud service (not read).
- Depends on: AUTO-26, AUTO-27; Cloud account (other domain); SSH hosts.

### AUTO-26 Artifact sharing gate and account rules

- What the user gets: a global setting that turns publishing on or off. Off by default. When off, `share` and `update` are refused with a sharing-disabled error and next steps. The CLI reads the setting first; the service checks it again on each call.
- Never gated: list, unshare and delete remain available when publishing is off, so a person can audit and revoke old links.
- Account rules: a profile not linked to a cloud account cannot publish. Token states can be reconnect-required or unconfigured, each with a message. Switching accounts during a request aborts it with "The signed-in MassOrca account changed while the artifact request was running." An explicit API URL is accepted only under certain configurations.
- Depends on: Settings; Cloud account and profile (other domain).

### AUTO-27 Artifact records, idempotency and recovery

- What happens: the desktop keeps a local record per profile and file path (slug, edit-token fingerprint, expiry). Up to 10,000 records are kept, newest first. A removed artifact removes its record.
- Create journal: a create is written to a pending-intent store before the request is sent, so a retry after a crash sends the same body. Up to 32 pending creates are kept. Client errors other than 408, 409, 425 and 429 discard the intent. Transient failures keep it.
- Ordering: operations on one file are queued, per source key. If the content differs from the pending create, the flow updates the created artifact instead of creating a second one.
- Depends on: AUTO-25; Local profile storage.

### AUTO-28 Routines (Bot Teams presentation of automations)

- Status: new feature. The current app has no routines; this is owner direction for the new product (D-AG-25, open question 8 answered).
- Source: pv96 (unmerged). Routines run on desktop and phone: per team and bot, daily, weekly, monthly or once, at a time on the creator's clock, with pause and resume, a run history, and one task per run. A host check every 30 seconds claims the due runs; a run missed while the host was asleep happens once, on wake. A bot can schedule a one-time routine from the room (AUTO-13). The routine scheduler keeps its own tables and reuses only the occurrence functions of AUTO-01, so the one-engine direction is not met in pv96.
- What the user does: opens a team's Routines page from the team's Projects section. The header shows the team's name and New routine. A routine starts a task in the team on a schedule, and each run is kept in its history. A routine can also be created by a bot in the Conference room when the user asks to schedule a task for later (AUTO-13). The footer names the host the team runs on.
- Rules: a routine is the simplified Bot Teams form of an automation (AUTO-01). In pv96 the automation scheduler (AUTO-03) does not run it, and its run history is its own. Every routine also appears on the Calendar (AUTO-30). New routine opens the Bot Teams editor, not the shared schedule editor.
- Empty states: no routines yet; routines listed.
- Depends on: AUTO-01, AUTO-03, AUTO-04, AUTO-12, AUTO-13.
- Proposal: first version.

### AUTO-29 Web address (webhook) triggers

- Status: new feature, owner direction for the new product (D-AG-25). The current app was not seen to have one.
- Source: pv96 (unmerged). The endpoint is mounted on its own listener at 127.0.0.1, on port 47391 or the next free port up to 47395: `POST /hooks/<team>/<id>` with `Authorization: Bearer <key>`. Each address has one key, shown once when made or replaced. A call with a missing or wrong key is refused with 401. Bodies over 16 KiB are refused, and an address accepts 6 calls a minute. A valid call starts one task titled by its first line (80 characters at most). Other paths answer 404. When no port is free, the page says the host does not take calls yet.
- What the user does: adds a web address to a team on the Routines page. Calling that address starts one task in the team.
- Rules: the owner's reference has no form for a web address; pv96 builds one, with the bot chosen when it is made. A web address is a network entry point, so pv96 uses the per-address key and the call limit. Exposure beyond this computer and any extra hardening are not defined.
- Empty states: no web addresses yet; web addresses listed.
- Depends on: AUTO-28; the network listener (server-and-pairing domain).
- Proposal: later, until the form and its security rules are designed. The owner may want it sooner, because the Routines page shows the section.

### AUTO-30 Calendar of routines and automations

- Status: new feature, owner direction for the new product (D-AG-23, D-SH-12, open question 8 answered).
- Source: pv96 (unmerged). The Calendar page shows automation runs and Bot Team routine runs. Routine runs come from the host in windows of up to 62 days, and each run is placed by its instant on the routine's zone. Done runs are hidden by default, with a saved switch to show them.
- What the user does: opens the Calendar from the activity rail and sees every routine and automation on one timeline, with a mode filter that separates Bot Teams routines from Workspaces automations. Opening a routine goes to its Routines page; opening an automation goes to the Automations page (assumed).
- Rules: the Calendar reads the shared schedule engine. It shows routines whether they were created on the Routines page or by a bot in the Conference room. Android placement is not designed.
- Depends on: AUTO-01, AUTO-03, AUTO-28.
- Proposal: first version.

### AUTO-31 Team files and team memory

- Status: new feature, owner direction for the new product (D-WS-29, D-ED-13, D-AG-26). The custom/integration app kept a team folder workspace for bot tasks (AUTO-14) but showed no team files or memory.
- Source: pv96 (unmerged). Team files: the team folder lists with the host's file listing and opens as read-only text, with no locks and no editor. Memory: team lines and per-bot lines of up to 500 characters, the 40 newest per scope, read when a task starts. Bots write them with `orca orchestration team-remember`, and the Memory page lists and deletes them. With the team's Workspaces option on, the Memory page also lists Engram memories of a linked project. File versions are AUTO-36.
- What the user does: opens Files or Memory from a team's Projects section.
- Rules: the owner's reference shows only the entries. Team files likely reuse the file tree and the editor (assumed in the design). The owner has not defined what the memory holds. In pv96 bots can write to it with `team-remember`.
- Depends on: AUTO-12; the file explorer and editor (settings-and-files domain).
- Proposal: later, until the owner defines what team memory is.

### AUTO-32 Bring back child session work into the parent

- Status: new in pv96 (not in custom/integration). Part of the delegation engine, shared by Bot Teams and Workspaces.
- Source: pv96 (unmerged). When a delegated child finishes with commits, the parent can bring them back: the host merges the child's branch into the parent's worktree with `git merge --no-ff`. States include clean commits, a predicted conflict (only where Git supports `merge-tree --write-tree`), uncommitted work on either side, a parent that is working, conflicts still to resolve, and already brought back. The parent's access sets the policy: `ask` always asks, `auto` merges alone when the merge is clean and the parent is idle, and `full` never asks; it waits for the parent's turn to end. Bot Teams passes `full`. Nothing is stashed, pushed or committed automatically.
- What the user does: sees a notice in the parent chat with the commit and file counts and the branch, and can bring the work back, resolve conflicts with the agent, or abort the merge.
- Depends on: AUTO-20; git on the host; the agent-session side of delegated children (AG-26).
- Proposal: first version. The owner's orchestration direction names worktree handoff for child agents (AUTO-17).
- Owner direction for the new product (2026-11-12, child placement rules from the design session): with the child placement rules, writing children normally run in their own worktree (rule 1), so bring-back is the usual way their work returns. A child that wrote in the current worktree (rule 2, run sequentially by rule 3) needs no bring-back. Proposal unchanged.

### AUTO-33 Bot team projects: Board, Brief and code source

- Status: new in pv96 (not in custom/integration).
- Source: pv96 (unmerged). A team can keep projects, each with its own Board and Brief (a goal and sections; read-only on the phone). A task can be moved into a project, and the bots of a task read the project's goal and brief when the task starts. A project's code source is recorded as "Existing repository" or "New folder with git". It is recorded only: nothing is cloned or created yet, and it needs the team's Workspaces option.
- What the user does: creates a project in a team, opens its Board or Brief, moves tasks into it, and records its code source.
- Depends on: AUTO-12, AUTO-14; a linked Workspaces project for code.
- Proposal: first version. The owner's direction names a Projects section in the Bot Teams sidebar (D-WS-29).

### AUTO-34 Move a waiting task to another bot or team

- Status: new in pv96 (not in custom/integration).
- Source: pv96 (unmerged). A task that is Not started, Stopped or Stuck can be given to a bot in any team (`botTeams.reassignTask`). Its session and project link are dropped, and the new owner starts the task fresh with handover notes: that it was reassigned, whether it had started, and the old owner's last five posts about it in the room. The caller checks that the task is idle. A task that moves to another team runs in that team's folder with that team's access mode.
- What the user does: chooses Reassign on a waiting task, picks a bot in any team, and starts it.
- Depends on: AUTO-13, AUTO-14.
- Proposal: later. Not in the owner's recorded decisions; first-version routing covers new requests.

### AUTO-35 Task deliverables returned to the asking room

- Status: new in pv96 (not in custom/integration).
- Source: pv96 (unmerged). When a task that another team asked for finishes, the asking room gets one finish line. Its deliverables are the text files the task changed in its team folder or its worktree branch: up to 10 are copied into the room's attachments. Binary, oversized and extra files are named, not copied. A task run in a linked project's worktree returns what its branch changed. The owner's "send result" card is not built.
- What the user does: sees the finish line and the copied files in the asking room.
- Depends on: AUTO-14, AUTO-34; Workspaces worktrees.
- Proposal: later. The return card is not in the owner's decisions, and the copy rules have open cases.

### AUTO-36 File versions of team folders

- Status: new in pv96 (not in custom/integration). The storage choice is open: the owner reopened it after the first build.
- Source: pv96 (unmerged). Each team has a content-addressed version store. A version is saved when a task's turn finishes and when a file is restored. On desktop, a file's Versions card lists the earlier versions, with Compare (added, removed and changed marks) and Restore; a restore first saves the current state as a "Before restoring" version. The phone has no Versions. Size limits apply to what is kept.
- What the user does: opens a team file, sees its versions, compares two, and restores one.
- Depends on: AUTO-31, AUTO-14; the file explorer.
- Proposal: later. The owner reopened the storage choice; keep it out of the first version until it is decided.

### AUTO-37 Room answers with web research or code reading

- Status: new in pv96 (not in custom/integration).
- Source: pv96 (unmerged). A plain-conversation answer can use two extra reads. Web research runs when the answering bot's agent has a search mode; Gentle Shell and agents without one cannot. A read-only code read runs when the agent has a read-only code mode; for Codex and Antigravity it runs only where the host limits their MCP servers. Answers are plain text of up to 1,200 characters, or up to 3,000 with source links.
- What the user does: asks a question in the room and gets an answer that may cite web sources or the linked project's code.
- Depends on: AUTO-13; agent search and read modes (other domain); a linked project for code questions.
- Proposal: later. Not in the owner's direction, and it depends on each agent's search and read modes.

## Key open questions for the owner

The owner's decisions of 2026-11-12 answered the earlier questions here: Bot Teams is kept as a core mode, and automations and orchestration come in the first version. Remaining questions:
- Hermes and OpenClaw managers (AUTO-11) and the legacy coordinator loop (AUTO-24) stay proposed as drop.
- Whether web address triggers (AUTO-29) belong in the first version, and what team memory holds (AUTO-31).
- The first-version scope grew by 18 features in this domain. See [open-questions.md](open-questions.md), "Automations", for what the owner may want to trim.
- pv96 raises two owner questions: whether routines should run on the Automations engine, as the owner direction says (AUTO-28), and whether web address calls may come from beyond this computer, and which hardening is on by default (AUTO-29).
- Whether a team needs a lead bot, or the least busy matching bot is enough (AUTO-13).
