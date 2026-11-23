# Inventory coverage

Status: measured 2026-11-22 on commit `fdc01881` (`origin/main`), against the 542 features of the [functional inventory](README.md). Generated content: regenerate it rather than editing rows by hand (see Method).

## Headline

| | Features | Done % | Done or in review % | Weighted % | Done incl. built replacements % |
|---|---|---|---|---|---|
| Whole inventory | 542 | 7.9% | 8.7% | 21.6% | 11.3% |
| [First version](first-version.md) | 190 | 19.5% | 21.3% | 48.6% | 25.1% |

Percentages exclude features marked `changed` or `dropped` from the denominator. **Weighted %** counts `done` as 1 and `partial` as 0.5. **Done or in review %** adds features held only in an open pull request. **Done incl. built replacements %** also counts as done the `changed` features whose replacement design is built on `main` (19 overall, 13 in the first version; detected from the row note, which says "built" and not "not built" or "in review").

Reading these numbers: the reviewers were strict. A feature whose main path works but misses one rule (an icon, a setup script, a phone surface) is `partial`, not `done`; read the note. Most new service work is reachable from the CLI only: the desktop app connects to the local service for workspaces, terminals and source control, but its sessions, agent status, Bot Teams and Marketplace screens still run on sample data, so those features stay `partial`. The secret vault stores and scopes secrets, but no stored value reaches an agent run yet. Bot Teams tasks cannot be started (`task.start` is not built).

Since `89bcad1`, the Phase 3 network chain (#66 to #70, #85) merged, so its features left `in review`: those served end to end became `done`, the rest `partial`, which is why "done or in review" fell while "done" and "weighted" rose. Two changes in the table below correct the earlier reading rather than follow new code: PLAT-09 (the newer-schema refusal already existed) and TERM-27 (removing a workspace does not stop its terminals).
## Changed since 89bcad1

The previous measurement was taken on commit `89bcad1`. Same method, same reviewers' rules; every status change below was checked against the code on `fdc01881`.

| Scope | Done % (89bcad1) | Done % (fdc01881) | Done or in review % (89bcad1) | Done or in review % (fdc01881) | Weighted % (89bcad1) | Weighted % (fdc01881) |
|---|---|---|---|---|---|---|
| Whole inventory | 4.8% | 7.9% | 10.5% | 8.7% | 14.6% | 21.6% |
| First version | 11.4% | 19.5% | 22.9% | 21.3% | 35.4% | 48.6% |
| M1 | 22.6% | 22.6% | 25.8% | 22.6% | 57.3% | 58.9% |
| M2 | 13.6% | 61.9% | 95.5% | 61.9% | 15.9% | 81.0% |
| M3 | 3.0% | 7.6% | 3.0% | 12.1% | 27.3% | 35.6% |
| M4 | 4.0% | 8.0% | 4.0% | 8.0% | 20.0% | 30.0% |
| M6 | 1.2% | 1.6% | 3.7% | 1.9% | 3.3% | 7.0% |

64 features changed status:

| From | To | Features | IDs |
|---|---|---|---|
| not started | partial | 27 | TERM-12, SCM-22, AG-26, AG-31, STAT-26, ACC-06, ACC-07, ACC-08, ACC-13, ACC-26, HIST-10, HIST-16, AUTO-15, AUTO-16, AUTO-23, AUTO-34, SKL-09, SKL-13, SKL-27, SKL-29, SKL-37, CLI-30, CLI-32, PLAT-09, PLAT-25, SET-04, SET-29 |
| in review | partial | 16 | TERM-02, TERM-26, REM-03, REM-04, REM-05, REM-06, REM-07, REM-23, REM-25, SRV-02, SRV-06, SRV-07, SRV-21, CLI-05, CLI-06, PLAT-26 |
| in review | done | 11 | REM-01, REM-11, REM-12, REM-14, REM-22, SRV-01, SRV-09, SRV-10, SRV-15, SRV-18, CLI-04 |
| not started | in review | 4 | ACC-03, ACC-04, ACC-05, SKL-42 |
| partial | done | 3 | ACC-01, ACC-02, HIST-07 |
| in review | changed | 1 | SRV-04 |
| not started | done | 1 | CLI-36 |
| partial | not started | 1 | TERM-27 |

## Statuses

- `done`: reachable through the CLI or the desktop app against the real service on `main`. Evidence: the command or protocol method, and one file that implements it.
- `partial`: some layer exists on `main` (for example the service method without a CLI or UI, a UI on sample data only, or only part of the feature's rules). The note says what is missing.
- `in review`: the feature, or the layer that completes it, exists only in an open pull request (number given).
- `not started`: nothing on `main` or in an open pull request.
- `changed`: a decision record or owner direction replaced the feature with a different design; the note says whether the replacement is built.
- `dropped`: a decision record or owner direction removed it.

## Totals

### Overall and first version

| Scope | Features | Done | Partial | In review | Not started | Changed | Dropped | Done % | Done or in review % | Weighted % |
|---|---|---|---|---|---|---|---|---|---|---|
| Whole inventory | 542 | 39 | 136 | 4 | 317 | 37 | 9 | 7.9% | 8.7% | 21.6% |
| First version | 190 | 34 | 101 | 3 | 36 | 14 | 2 | 19.5% | 21.3% | 48.6% |

### Per inventory area

| Scope | Features | Done | Partial | In review | Not started | Changed | Dropped | Done % | Done or in review % | Weighted % |
|---|---|---|---|---|---|---|---|---|---|---|
| [Workspaces and worktrees](workspaces.md) (WS) | 45 | 0 | 20 | 0 | 24 | 1 | 0 | 0.0% | 0.0% | 22.7% |
| [Terminals](terminals.md) (TERM) | 28 | 6 | 11 | 0 | 7 | 4 | 0 | 25.0% | 25.0% | 47.9% |
| [Git and source control](git.md) (GIT) | 31 | 10 | 8 | 0 | 10 | 3 | 0 | 35.7% | 35.7% | 50.0% |
| [Hosting providers and issue trackers](providers-and-trackers.md) (SCM) | 32 | 0 | 1 | 0 | 30 | 1 | 0 | 0.0% | 0.0% | 1.6% |
| [Agent integrations](agents.md) (AG) | 36 | 0 | 8 | 0 | 25 | 3 | 0 | 0.0% | 0.0% | 12.1% |
| [Agent status and hooks](agent-status.md) (STAT) | 26 | 3 | 5 | 0 | 11 | 2 | 5 | 15.8% | 15.8% | 28.9% |
| [Accounts, credentials and usage](accounts-and-usage.md) (ACC) | 26 | 2 | 11 | 3 | 9 | 1 | 0 | 8.0% | 20.0% | 30.0% |
| [Session history](session-history.md) (HIST) | 18 | 1 | 4 | 0 | 13 | 0 | 0 | 5.6% | 5.6% | 16.7% |
| [SSH and remote hosts](remote-hosts.md) (REM) | 27 | 5 | 7 | 0 | 8 | 5 | 2 | 25.0% | 25.0% | 42.5% |
| [Headless server and pairing](server-and-pairing.md) (SRV) | 22 | 8 | 5 | 0 | 4 | 4 | 1 | 47.1% | 47.1% | 61.8% |
| [Automations, bot teams and orchestration](automations.md) (AUTO) | 37 | 0 | 16 | 0 | 20 | 0 | 1 | 0.0% | 0.0% | 22.2% |
| [Browser pane and devices](browser-and-devices.md) (BRW) | 30 | 0 | 2 | 0 | 28 | 0 | 0 | 0.0% | 0.0% | 3.3% |
| [Skills, plugins and marketplace](skills-and-plugins.md) (SKL) | 44 | 0 | 5 | 1 | 30 | 8 | 0 | 0.0% | 2.8% | 6.9% |
| [Command-line interface](cli.md) (CLI) | 37 | 2 | 13 | 0 | 21 | 1 | 0 | 5.6% | 5.6% | 23.6% |
| [Cloud services](cloud.md) (CLD) | 18 | 0 | 0 | 0 | 17 | 1 | 0 | 0.0% | 0.0% | 0.0% |
| [Desktop app platform](app-platform.md) (PLAT) | 30 | 1 | 11 | 0 | 18 | 0 | 0 | 3.3% | 3.3% | 21.7% |
| [Settings, persistence and files](settings-and-files.md) (SET) | 30 | 1 | 7 | 0 | 21 | 1 | 0 | 3.4% | 3.4% | 15.5% |
| [Customisation and extensibility](customisation.md) (CUS) | 11 | 0 | 2 | 0 | 7 | 2 | 0 | 0.0% | 0.0% | 11.1% |
| [Prototypes and design system](prototypes.md) (PROTO) | 14 | 0 | 0 | 0 | 14 | 0 | 0 | 0.0% | 0.0% | 0.0% |

### Per milestone

Milestones as defined in `docs/WORKING-RULES.md`. M1 to M5 hold first-version features only; everything outside the first version is M6.

| Scope | Features | Done | Partial | In review | Not started | Changed | Dropped | Done % | Done or in review % | Weighted % |
|---|---|---|---|---|---|---|---|---|---|---|
| M1 Core and CLI: workspaces, terminals, git | 66 | 14 | 45 | 0 | 3 | 4 | 0 | 22.6% | 22.6% | 58.9% |
| M2 Headless server driven by the CLI | 25 | 13 | 8 | 0 | 0 | 4 | 0 | 61.9% | 61.9% | 81.0% |
| M3 Agents, status, delegation, credentials | 74 | 5 | 37 | 3 | 21 | 6 | 2 | 7.6% | 12.1% | 35.6% |
| M4 Desktop app, first-version scope | 25 | 2 | 11 | 0 | 12 | 0 | 0 | 8.0% | 8.0% | 30.0% |
| M5 Android app | 0 | - | - | - | - | - | - | - | - | - |
| M6 Extensions and the rest of the inventory | 352 | 5 | 35 | 1 | 281 | 23 | 7 | 1.6% | 1.9% | 7.0% |

## Method

1. Worktree at the measured commit; the open pull requests listed by `gh pr list --state open` at the same time.
2. Eight read-only reviewers, one per group of inventory areas, read the code (`crates/`), the CLI reference (`docs/cli.md`), the decision records (`docs/decisions/`) and the open pull request diffs, and gave each feature one status with evidence. Evidence or it is not done: a name alone was not enough, the reviewer had to open the implementing code; a screen drawn on sample data was not counted as done.
3. A script (`docs/inventory/coverage_assemble.py`) joined the rows with the inventory's README table and `first-version.md`, checked that every one of the 542 IDs appears exactly once, and computed the totals.
4. Milestone mapping (judgement, applied by prefix with listed exceptions): first-version features of WS, TERM and GIT, the core store (SET-01 to SET-04), `orca.yaml` scripts (CUS-01), core extension points (CUS-08), the local CLI groups (CLI-01, 03, 08, 09, 10, 11, 35) and core service hygiene (PLAT-07, PLAT-14, PLAT-18, PLAT-27) are M1; SRV, REM, CLD and CLI-04, CLI-05 are M2; AG, STAT, ACC, HIST, AUTO, profile import (SET-08), per-agent overrides (CUS-04) and the agent, automation and Bot Teams CLI groups (CLI-12, 14, 23, 24, 25, 32) are M3; every other first-version feature is M4. The inventory describes core behaviour, so no feature row is Android-only and M5 has no rows: Android reaches the M1 to M4 features through the same service.
5. To regenerate: repeat step 2 on a new commit, starting from the previous rows and re-checking every row the new commits could change, then run `python docs/inventory/coverage_assemble.py <repo-root> <rows-dir> <commit> <previous-commit>`; the last argument reads the existing `coverage.md` as the earlier measurement and writes the "Changed since" section. `<rows-dir>` may hold a `reading.txt` with the paragraph shown under the headline. Keep this method section in step with any change.

Limits: nothing was run; statuses come from reading code. The milestone mapping and the `partial` judgement involve reading the feature's intent, so two reviewers may differ on borderline rows. Work on local branches without an open pull request counts as not started.

## Features

| ID | Feature | Proposed | Milestone | Status | Evidence | Note |
|---|---|---|---|---|---|---|
| WS-01 | Add an existing git repository as a project | first version | M1 | partial | `massorca project add` / `project.add`, crates/core-workspace/src/projects.rs | Git repo registered at its toplevel, duplicate path and name refused (project.exists), name settable; desktop Add a project dialog calls the service. No project icon. |
| WS-02 | Add a folder as a plain-folder project | first version | M1 | partial | `project add --folder` / `project.add` kind folder, crates/core-workspace/src/projects.rs | Folder projects register (desktop auto-detects kind). No No Git marker on desktop, no warning before adding a non-git folder, no phone. |
| WS-03 | Create, clone or initialise a project | later | M6 | not started | none | No create-repo, clone or git init on main or in an open PR. |
| WS-04 | Per-project base branch and workspace location | first version | M1 | partial | `project add --workspaces-root` / `project.add` workspaces_root, crates/protocol/src/methods/projects.rs | Per-project workspace folder exists. No per-project base branch setting; base is per create (`--base`, default HEAD). |
| WS-05 | Create a git worktree workspace | first version | M1 | partial | `workspace create` / `workspace.create`, crates/core-workspace/src/create.rs | Worktree on a new branch from a base, registered, idempotent request id; desktop New workspace calls it. No setup result (setup scripts deferred, ADR 0008). |
| WS-06 | Workspace and branch naming | first version | M1 | partial | name rules in crates/core-workspace/src/names.rs; delegated child names in crates/core-agents/src/sessions/delegation.rs | Validated, not sanitised. Only delegated children get a parent-n suffix, from the parent name, not the chat (decision 0055). No branch prefix, generated names or accent folding (decisions 0008/0009). |
| WS-07 | Create from an existing branch or a PR/MR head | later | M6 | partial | `workspace create --existing-branch`, crates/protocol/src/methods/workspaces.rs | Existing local branch works. No PR/MR head or fork PR checkout. |
| WS-08 | Create from a work item (issue, PR/MR, Linear, Jira) | later | M6 | not started | none | No tracker search or link-based create. |
| WS-09 | Plain-folder workspaces | first version | M1 | partial | `workspace create <folder project> <name> --path DIR`, crates/core-workspace/src/create.rs | Folder workspace on an existing directory, with terminals and agents. No one-line row name. |
| WS-10 | Projectless chats | later | M6 | not started | crates/desktop/src/model/workspace_flow.rs (No project row) | Desktop menu has a No project row that creates nothing yet; no projectless chats or Chats group. |
| WS-11 | Project groups and nested repository import | later | M6 | not started | none | No project groups or nested repository import. |
| WS-12 | Workspace placement on disk | first version | M1 | partial | config `workspaces.default_root` + per-project workspaces_root, crates/core-workspace/src/projects.rs | Global root and per-project base exist; nested or flat layout options deferred (ADR 0008). |
| WS-13 | Setup script on workspace create | later | M6 | not started | docs/decisions/0008-workspace-model.md (deferred) | Setup scripts deferred; nothing built. |
| WS-14 | Copy and share files into new workspaces | later | M6 | not started | none | No copy of gitignored files or shared folder links. |
| WS-15 | Archive script before removal | later | M6 | not started | none | No archive script before delete. |
| WS-16 | Remove a workspace | first version | M1 | partial | `workspace rm` / `workspace.remove`, crates/core-workspace/src/remove.rs | Worktree and merged branch removal with per-refusal waivers (CLI flags, desktop waiver dialogs). Does not stop terminals or agents (deferred, ADR 0008); no delegated children. |
| WS-17 | Remove a project from MassOrca | first version | M1 | partial | `project rm` / `project.remove`, crates/core-workspace/src/projects.rs | Unregisters without touching files, but refuses a project that still has workspaces instead of dropping their records. |
| WS-18 | Auto-rename from the first agent message | later | M6 | not started | none | No auto-rename from first agent prompt. |
| WS-19 | Rename, pin, order, annotate and status | later | M6 | not started | crates/desktop/src/model/workspace_flow.rs (Renaming comes later) | No rename, pin, order, note or board status; desktop menu rows are disabled. |
| WS-20 | Sleep and wake a workspace | later | M6 | not started | none | No sleep or wake of a workspace. |
| WS-21 | Workspace lineage (parent and child) | first version | M1 | partial | `agent delegate` / `agent tree` in crates/cli/src/agent.rs; crates/core-agents/src/sessions/delegation.rs; parent_id in crates/core-store/src/agent_sessions.rs | Parent set by delegate (CLI or orchestration); agent tree lists children across worktrees. No manual link; the desktop sidebar shows no service sessions. |
| WS-22 | Links from workspaces to issues, PRs and MRs | later | M6 | not started | none | No issue/PR/MR links; providers are a later phase (decision 0010). |
| WS-23 | Base-branch drift and local base refresh | later | M6 | not started | none | No base drift display or local base fast-forward. |
| WS-24 | Worktrees created outside MassOrca: visibility and import | later | M6 | partial | `workspace list` unregistered entries, crates/core-workspace/src/unregistered.rs | Worktrees git registers under a project's workspaces root are reported as unregistered. No import, no show or hide per project, source or host. |
| WS-25 | Disk usage and stale-workspace cleanup | later | M6 | not started | none | No disk usage or stale-workspace cleanup. |
| WS-26 | Prepared checkouts for faster creates | later | M6 | not started | none | No prepared checkouts. |
| WS-27 | Ephemeral VM workspaces from environment recipes | later | M6 | not started | none | No VM-backed workspaces. |
| WS-28 | Windows runtime per project (Windows host or WSL) | later | M6 | changed | docs/decisions/0015-git-on-remote-execution-hosts.md | Decision 0015: WSL served by a service inside the distribution, paired like any host. Replacement not built: no WSL code or bootstrap on main; generic host pairing is merged (PRs 66-70). |
| WS-29 | Workspace records: persistence and live catalog | first version | M1 | partial | `workspace.list` + workspace.created/removed events, crates/server/src/features/workspaces.rs | Persisted in the store, live across clients by events (desktop store), missing dirs reported, git as truth. No manual row names or session references. |
| WS-30 | Profiles: switcher and moving projects between profiles | drop | M6 | partial | `--profile` / MASSORCA_PROFILE, crates/cli/src/args.rs | Profiles exist per CLI option (decision 0004). No switcher and no moving a project between profiles. Inventory ranks it drop, but no decision record. |
| WS-31 | No Git warning when adding a folder | first version | M1 | not started | crates/desktop/src/model/project_add.rs | Desktop sends project.add with no kind; a non-git folder is registered silently, no warning or Initialize Git choice. |
| WS-32 | No Git marker and Initialize Git on folder projects | later | M6 | not started | none | No No Git pill or Initialize Git action (only Needs Git reasons on disabled git menu rows). |
| WS-33 | One-line worktree sidebar (default layout) | first version | M1 | partial | sidebar tree, crates/desktop/src/model/sidebar.rs | Project, host and workspace rows come from the service; session rows only from sample data (service_view leaves sessions empty). No Show N more folding. |
| WS-34 | Session row actions and hover card | later | M6 | not started | none | No session row actions or hover card; session rows are sample data. |
| WS-35 | Child sessions listed under their parent across worktrees | first version | M1 | partial | child session depth, crates/desktop/src/model/sidebar.rs | Children indent only on sample data; service sessions are empty. The CLI agent tree lists children across worktrees (WS-21); the desktop does not show them yet. |
| WS-36 | Checklist card in agent chats | later | M6 | not started | none | No checklist card in agent chats. |
| WS-37 | Checklist tab in the floating workspace | later | M6 | not started | none | No Checklist tab in the floating workspace. |
| WS-38 | Open an existing session in the floating workspace | later | M6 | not started | none | No floating workspace session tabs. |
| WS-39 | Host selector in the floating workspace | later | M6 | not started | none | No host selector in a floating workspace; host rows in the sidebar are local or sample only. |
| WS-40 | Start a session in an existing branch or worktree from the create page | first version | M1 | partial | start page from service records, crates/desktop/src/model/start_page.rs; Open terminal in crates/desktop/src/shell/start_page.rs | Host, project and branch from real records. Open terminal works when connected (PR 110). Chat/Terminal switch is drawn disabled; no chat start until sessions land. |
| WS-41 | Prototypes in the Workspaces side panel | first version | M1 | partial | Prototypes nav row, crates/desktop/src/model/sidebar.rs | Sidebar has a Prototypes entry, but the service count is always 0 and no prototypes are listed. |
| WS-42 | Show bot teams filter in Workspaces | later | M6 | not started | none | No Show bot teams filter in Workspaces. |
| WS-43 | CLI: list and close a worktree's agent sessions | later | M6 | partial | `agent list --workspace` and `agent stop` / `agent forget`, crates/cli/src/agent.rs | A workspace's agent sessions can be listed and stopped by id under agent commands. No worktree sessions command, no chats. |
| WS-44 | CLI: folder workspace commands | first version | M1 | partial | `workspace create/list/show/rm` on folder projects, crates/cli/src/workspace.rs | List, add and stop tracking work. No groups, rename, move, archive, pin, comment or check. |
| WS-45 | Sidebar card layout choice | later | M6 | not started | none | No sidebar card layout choice (Density is a planned, inert settings row). |
| TERM-01 | Create a terminal in a workspace | first version | M1 | done | `massorca terminal create <WS> [--shell] [--cwd]` -> terminal.create; crates/core-terminal/src/manager.rs; crates/desktop/src/shell/work_panel.rs | Starts in workspace root or --cwd with default or chosen shell. Desktop entry point from PR 110 is on main (create_terminal in shell/work_panel.rs). |
| TERM-02 | Tabs and pane binding | first version | M1 | partial | Work panel strip lists service terminals and attaches on select: crates/desktop/src/shell/work_panel.rs; crates/desktop/src/model/work_panel.rs; crates/desktop/src/service/terminals.rs | Tabs reattach after restart through the service list (PR 110, now on main). Missing: sessions shown by reference from another workspace; no code for it on main. |
| TERM-03 | Split panes | later | M6 | not started | none: no split-pane code in crates/desktop or crates/ui terminal modules | Work panel shows one pane per tab on main (PR 110 merged). No open PR file list touches terminal panes. |
| TERM-04 | Terminal host and reconnection after UI restart | first version | M1 | done | terminal.attach; crates/core-terminal/src/manager.rs | Service owns terminals (0033), so closing a client or the desktop leaves them running; `terminal attach` reconnects. |
| TERM-05 | Attach, detach and multiple clients | first version | M1 | changed | docs/decisions/0033-terminal-session-model.md | Built: many clients attach at once, nobody is evicted; the old "new viewer takes over" rule is dropped. |
| TERM-06 | Output streaming and flow control | first version | M1 | changed | docs/decisions/0034-terminal-scrollback-and-snapshot.md | Built: bounded 2 MiB queues per attachment and a fresh snapshot for laggards instead of pausing the shell. |
| TERM-07 | Scrollback and screen restore on reattach | first version | M1 | done | terminal.attach (snapshot/resume); crates/core-terminal/src/screen.rs | Snapshot restores screen, cursor, colours, modes and 1000 rows; resume from 1 MiB ring by offset. |
| TERM-08 | Terminal history on disk | later | M6 | not started | docs/decisions/0034-terminal-scrollback-and-snapshot.md | 0034 says no disk history in Phase 2; deferred, not dropped. |
| TERM-09 | Cold restore after a lost host or crash | later | M6 | not started | docs/decisions/0034-terminal-scrollback-and-snapshot.md | No cold restore; service stop ends terminals and nothing is restored (0033). |
| TERM-10 | Shell detection and selection | first version | M1 | done | terminal.create shell / `--shell`; crates/core-terminal/src/shell.rs | Default pwsh then powershell on Windows, $SHELL then zsh/bash/sh on Unix; explicit --shell. Setting terminal.shell is registered in config keys but no code reads it. |
| TERM-11 | Shell integration: readiness, working directory, titles | first version | M1 | partial | Terminal.title from OSC 0/2; crates/core-terminal/src/manager.rs | Titles tracked; no readiness markers, startup-command barrier or working-directory tracking (0032 defers them). |
| TERM-12 | Busy state and close confirmation | later | M6 | partial | close_step in crates/desktop/src/model/work_panel.rs; crates/desktop/src/shell/work_panel.rs | Close asks when a terminal may still run (live or unverifiable). No busy state for a running command or agent on tabs. Was not started. |
| TERM-13 | Closing and killing terminals | first version | M1 | done | terminal.kill / terminal.close, agent.stop; crates/core-terminal/src/process_tree.rs | Job object on Windows, session kill on Unix; graceful then forced after 5 s (0031, 0033). |
| TERM-14 | Input, paste, resize and terminal modes | first version | M1 | done | terminal.input / terminal.resize, `terminal attach`; crates/cli/src/terminal_attach.rs | Raw input, resize and modes (bracketed paste, mouse, alt screen) kept in snapshot. Desktop pane from PR 110 is on main (crates/ui/src/components/terminal_surface/view.rs). |
| TERM-15 | Terminal query replies and colour answers | first version | M1 | partial | cursor query answering; crates/core-terminal/src/manager.rs; docs/decisions/0036-terminal-query-answering-clients.md | Service answers ESC[6n only when no declared emulator is attached. No colour (OSC 10/11) or other capability answers. PR 113 and 116 are on main. |
| TERM-16 | Terminal environment | first version | M1 | partial | build_environment; crates/core-terminal/src/shell.rs | TERM, COLORTERM, TERM_PROGRAM, MASSORCA_* ids and caller env set; no Marketplace variables. |
| TERM-17 | Windows terminals: ConPTY, PowerShell, PATH | first version | M1 | partial | ConPTY pty; crates/core-terminal/src/pty.rs | ConPTY and PowerShell default built; registry PATH merge deferred (0032), inherited PATH only. |
| TERM-18 | macOS and Linux platform handling | first version | M1 | partial | Unix session process tree; crates/core-terminal/src/process_tree.rs | Unix pty and process groups built; no evidence of macOS privacy-prompt or Linux long-path handling. |
| TERM-19 | Host updates and crash recovery | first version | M1 | changed | docs/decisions/0033-terminal-session-model.md | No separate terminal host: UI restarts keep terminals, but a service restart or update ends them (built as decided). |
| TERM-20 | Host log | later | M6 | changed | docs/decisions/0033-terminal-session-model.md | No host process; the service log (0006, logs/service.<date>.log) covers it. Built. |
| TERM-21 | Built-in terminal themes | first version | M1 | partial | TerminalColors tokens; crates/ui/src/components/terminal_surface/colors.rs | Dark and light palettes exist, but no terminal theme setting (config keys: terminal.font, terminal.scrollback, terminal.shell). Pane from PR 110 is on main. |
| TERM-22 | Theme import from Ghostty and Warp | later | M6 | not started | none | Desktop settings only show an "Import theme" row label (crates/desktop/src/model/settings_sections.rs); no importer. |
| TERM-23 | Per-workspace shell command history | later | M6 | not started | none | No per-workspace shell history. |
| TERM-24 | Credential guard and agent shell hooks | first version | M1 | partial | `agent hook`; crates/core-agents/src/launch.rs | Agent status hooks installed per run; git credential guard only on service git calls (crates/core-git/src/git.rs), not in agent terminal env. |
| TERM-25 | Terminal preview in the dashboard | later | M6 | not started | none | No dashboard terminal preview. |
| TERM-26 | Headless-server terminals | first version | M1 | partial | `service run --listen` (crates/cli/src/args.rs); crates/cli/src/service/foreground.rs; docs/decisions/0033-terminal-session-model.md | Listener from PR 69/70 is on main; terminals live in the service. Not seen: a terminal test over the network, or bot-team routine keep-alive (routines out of scope, 0072). |
| TERM-27 | Closing a workspace or worktree closes its terminals | first version | M1 | not started | none: no terminal handling in crates/server/src/features/workspaces.rs or crates/core-workspace/src | Removing a workspace does not stop its terminals; the terminal guard on removal is deferred (docs/cli.md). Was partial; no removal hook exists. |
| TERM-28 | WSL terminals (behaviour note) | later | M6 | partial | `terminal create --shell wsl.exe`; crates/core-terminal/src/shell.rs | 0032 makes WSL an explicit shell choice only; no distro picker or WSL integration. |
| GIT-01 | Git availability and version baseline | first version | M1 | partial | git discovery, crates/core-git/src/git.rs; git.unavailable mapping in crates/core-workspace/src/error.rs | Service checks git runs and maps a missing binary to git.unavailable; no present/missing/unknown report per host, no Marketplace install. |
| GIT-02 | Non-interactive git runner and credential policy | first version | M1 | done | GIT_TERMINAL_PROMPT=0, askpass off, ssh BatchMode, timeouts; crates/core-git/src/git.rs | Non-interactive runner with process-tree timeout and classified stderr (decision 0013). |
| GIT-03 | Repository detection and registration | first version | M1 | partial | `project add` / `project.add`, crates/core-workspace/src/projects.rs | Tells repo from folder via rev-parse --show-toplevel. A linked worktree resolves to its own toplevel, not to its main repository. |
| GIT-04 | Clone and initialize repositories | later | M6 | not started | none | No clone or init. |
| GIT-05 | Status with line counts and listing caps | first version | M1 | done | `massorca git status` / `git.status`, crates/server/src/features/git/status.rs | Staged, unstaged, untracked, conflicted with line counts and listing caps (decision 0011). Desktop Source control panel calls it (#115, crates/desktop/src/service/git.rs). |
| GIT-06 | File diffs for unstaged, staged and HEAD comparisons | first version | M1 | partial | `massorca git diff` + `git patch` / `git.diff_file`, crates/server/src/features/git/diff.rs; desktop DiffView in crates/desktop/src/model/source_control_diff.rs | Worktree, staged, HEAD and range diffs with binary sizes. No image previews (binary note only); no submodule pointer diff. Desktop DiffView merged (PR 115). |
| GIT-07 | Stage and unstage files | first version | M1 | done | `massorca git stage/unstage [--all]` / `git.stage`, `git.unstage`, crates/server/src/features/git/index.rs | One file, a selection or everything; desktop panel on main (#115). |
| GIT-08 | Discard changes | first version | M1 | done | `massorca git discard` / `git.discard`, crates/server/src/features/git/index.rs | Untracked delete and staged drop need explicit flags; paths outside the worktree refused (git.path_outside_worktree). |
| GIT-09 | Commit with a typed message | first version | M1 | done | `massorca git commit` / `git.commit`, crates/server/src/features/git/commit.rs | Typed message, hooks always run (decision 0010). Desktop commit box on main (#115). |
| GIT-10 | AI-generated commit messages | later | M6 | not started | none | No AI-generated commit messages. |
| GIT-11 | Branch list and checkout | later | M6 | done | `massorca git branches` / `git switch` (`git.branches`, `git.branch_switch`), crates/server/src/features/git/branch.rs | Local branch list and checkout, refusing overwrites. |
| GIT-12 | Rename branch and folder after first agent work | later | M6 | partial | `massorca git branch-rename` / `git.branch_rename`, crates/server/src/features/git/branch.rs | Manual branch rename only; no folder move and no trigger on first agent work. |
| GIT-13 | Create a worktree from a base branch | first version | M1 | partial | `workspace create --base` / `workspace.create`, crates/core-workspace/src/create.rs | New branch from a chosen base works. Base refresh and base-ref fallback order deferred (ADR 0008). |
| GIT-14 | Worktree listing and scanning | first version | M1 | partial | worktree porcelain parser, crates/core-git/src/worktree_porcelain.rs | Parses locked and prunable internally; users only see rows plus unregistered worktrees under the root, without locked or prunable flags. |
| GIT-15 | Worktree removal with safety checks | first version | M1 | done | `workspace rm [--force]` / `workspace.remove`, crates/core-workspace/src/remove.rs | Dirty refused unless forced, ignored files and orphans need their own waiver, unmerged branches kept (branch -d). |
| GIT-16 | Preserved branches and force-delete | later | M6 | partial | `workspace rm` kept branch + `git branch-delete --force`, crates/server/src/features/git/branch.rs | Branch kept on removal and force delete reports its commit, but no saved-commit check before force delete. |
| GIT-17 | Pre-warmed worktree checkouts | drop | M6 | not started | none | Not built. Inventory ranks it drop, but no decision record removes it. |
| GIT-18 | Sparse checkout and shared paths for new worktrees | later | M6 | not started | none | No sparse checkout or shared paths. |
| GIT-19 | Push, pull, fetch and upstream ahead or behind | first version | M1 | done | `massorca git push/pull/fetch` / `git.push`, `git.pull`, `git.fetch`, crates/server/src/features/git/remote.rs | Push to upstream (creates it), pull ff-only/merge/rebase, fetch with prune, ahead and behind in status. The desktop panel (#115) omits them. |
| GIT-20 | Rebase worktree onto base branch | later | M6 | not started | none | No rebase onto base; pull --mode rebase rebases only onto the upstream. |
| GIT-21 | Merge and rebase conflict state and abort | later | M6 | done | `git.status` operation + `massorca git abort` / `git.abort`, crates/server/src/features/git/remote.rs | Merge, rebase, cherry-pick and revert in progress shown; abort supported. |
| GIT-22 | History, branch and commit comparison | later | M6 | done | `massorca git log` + `git diff --from REV` / `git.log`, `git.diff` range, crates/server/src/features/git/log.rs | Paged history and branch-against-base comparison. |
| GIT-23 | Submodule status and pointer diffs | later | M6 | partial | `git.status` entry submodule flag, crates/protocol/src/methods/git/status.rs | Status marks submodule entries; no dedicated submodule status or pointer diff view. |
| GIT-24 | Ignore helpers and huge-folder suggestions | later | M6 | not started | none | No ignore helpers. |
| GIT-25 | Idle ref packing and repository maintenance | later | M6 | not started | none | No idle ref packing. |
| GIT-26 | Fork sync and fork remote setup | later | M6 | not started | docs/decisions/0010-git-operations-surface.md | Fork and provider behaviour is a later phase outside git/1. |
| GIT-27 | Remote file and commit web links | later | M6 | not started | docs/decisions/0010-git-operations-surface.md | Web links are provider behaviour, a later phase. |
| GIT-28 | Git on SSH hosts through the relay | later | M6 | changed | docs/decisions/0015-git-on-remote-execution-hosts.md; crates/cli/src/host/add_ssh.rs; `--host` on git in crates/cli/src/git/mod.rs | Replacement built: host add-ssh installs the service on a Linux SSH host, and the global --host runs git subcommands there. The SSH relay itself is not built (decision 0015). |
| GIT-29 | Git inside WSL distributions | later | M6 | changed | docs/decisions/0015-git-on-remote-execution-hosts.md | Replacement not built: no WSL code or bootstrap on main. Decision 0015 has a service inside the distribution, paired as any host; that transport is merged but unverified in WSL. |
| GIT-30 | Bring a delegated child's commits back into the parent branch | first version | M1 | changed | docs/decisions/0055-delegation-and-bring-back.md; crates/core-git/src/merge.rs; `agent bring-back` in crates/cli/src/agent.rs | Replacement built: agent bring-back merges with --no-ff and aborts on conflict. No ask/auto/full policy and no conflict prediction. Server agent.bring_back and CLI on main. |
| GIT-31 | One-click Git install on this computer | later | M6 | not started | none | No one-click git install. |
| SCM-01 | GitHub connection and diagnostics | first version | M4 | not started | crates/desktop/src/model/settings_sections.rs | Only a disabled "Connect" GitHub row in Settings (planned control, no backing); no gh auth, device code or diagnostics in any crate |
| SCM-02 | Per-repository GitHub account binding | later | M6 | not started | none | No account binding anywhere; no provider layer exists |
| SCM-03 | Hosting provider detection and repository identity | first version | M4 | not started | docs/decisions/0010-git-operations-surface.md | git.fetch/pull/push exist, but no remote parsing into host or owner, no fork awareness; 0010 puts provider behaviour in a later phase |
| SCM-04 | GitLab, Bitbucket, Azure DevOps and Gitea connection | later | M6 | not started | crates/desktop/src/model/settings_sections.rs | Only a disabled GitLab "Connect" row; Bitbucket, Azure DevOps, Gitea absent. extensions.md lists trackers as `itemSources` (built-in or extension, undecided) |
| SCM-05 | Jira site connection | later | M6 | not started | crates/desktop/src/model/settings_sections.rs | Only a disabled Jira "Connect" row; no service or protocol support |
| SCM-06 | Linear workspace connection | later | M6 | not started | crates/desktop/src/model/settings_sections.rs | Only a disabled Linear "Connect" row; no service or protocol support |
| SCM-07 | Branch review lookup and sidebar status | first version | M4 | not started | docs/decisions/0010-git-operations-surface.md | No PR or MR lookup, CI mark or browser link; 0010 defers pull requests and web links to a later phase |
| SCM-08 | Status refresh pacing and rate-limit handling | first version | M4 | not started | crates/desktop/src/model/settings_sections.rs | Only a disabled "API budget" row; no provider polling or rate-limit handling |
| SCM-09 | Create pull or merge request from a branch | first version | M4 | not started | none | No PR or MR creation in protocol, service, CLI or desktop |
| SCM-10 | Stacked GitHub pull requests | later | M6 | not started | none | No stacked PR support |
| SCM-11 | Checks and CI status details | later | M6 | not started | none | No checks or CI detail support |
| SCM-12 | PR and MR comments and review threads | later | M6 | not started | none | No PR comment or review thread support |
| SCM-13 | PR and MR file contents and viewed state | later | M6 | not started | none | No PR file contents or viewed state |
| SCM-14 | Reviewers and review state | later | M6 | not started | none | No reviewer support |
| SCM-15 | Merge, close, reopen, draft and title edits, conflict summary | later | M6 | not started | none | No merge, close, reopen or title edit of PRs |
| SCM-16 | GitHub auto-merge and merge queue | later | M6 | not started | none | No auto-merge or merge queue |
| SCM-17 | AI-drafted PR title and description | later | M6 | not started | none | No AI-drafted PR text |
| SCM-18 | GitHub and GitLab issue and MR browse | later | M6 | not started | docs/architecture/extensions.md | Not built; extensions.md section 10 plans GitHub issues and PRs as a built-in `itemSources` contribution |
| SCM-19 | GitHub and GitLab issue details, edits and comments | later | M6 | not started | none | No issue detail, edit or comment support |
| SCM-20 | Jira issue browse, detail and edit | later | M6 | not started | docs/architecture/extensions.md | Not built; Jira planned as an `itemSources` provider, built-in or first-party extension decided per batch |
| SCM-21 | Linear issue browse, detail and edit | later | M6 | not started | docs/architecture/extensions.md | Not built; Linear planned as an `itemSources` provider, built-in or first-party extension decided per batch |
| SCM-22 | Unified task drawer and source picker | first version | M4 | partial | crates/server/src/features/teams/task.rs; crates/cli/src/task/mod.rs; crates/desktop/src/shell/canvas.rs | task.* methods and `massorca task` on main (#140, 0072); Tasks page still a placeholder; Board shows sample teams without a service; no approval list, source list or WorkSpaces/Bot Team switch |
| SCM-23 | Agent access to Linear (CLI and RPC) | later | M6 | not started | none | No orca linear command or RPC |
| SCM-24 | Create workspace from an issue or PR | later | M6 | not started | none | Workspace create takes name and branch by hand; no issue or PR prefill |
| SCM-25 | Link and relink a workspace to a task | later | M6 | not started | none | No workspace-to-task link field |
| SCM-26 | Issue command template | later | M6 | not started | docs/decisions/0009-phase1-deferrals.md | No issue command template; workspace extras deferred by 0008 and 0009 |
| SCM-27 | GitHub Projects board (Tasks) | later | M6 | changed | docs/decisions/0001-extension-model.md | Owner direction in 0001: GitHub Projects becomes a first-party extension (board view plus itemSources) at M6; not built |
| SCM-28 | Self-hosted instance support | later | M6 | not started | none | No provider layer, so no self-hosted instances |
| SCM-29 | Remote and WSL execution of provider calls | later | M6 | not started | crates/core-remote/src/lib.rs | No provider calls to run remotely; SSH host bootstrap is on main (#68, core-remote) but there is no WSL code and no provider relay |
| SCM-30 | Star Orca on GitHub (upstream promotion) | drop | M6 | not started | docs/inventory/providers-and-trackers.md | Inventory proposes drop; no decision record or owner text confirms it, so lowered to not started |
| SCM-31 | Host-wide GitHub logins: list, switch active, remove | later | M6 | not started | none | No gh login listing, switching or removal |
| SCM-32 | Feedback reports published as GitHub issues | drop | M6 | not started | docs/inventory/open-questions.md | Inventory proposes drop but says owner to confirm; no decision record, so lowered to not started |
| AG-01 | Supported agent catalog, defaults and enable/disable | first version | M3 | changed | docs/decisions/0050-agent-kinds.md; docs/decisions/0059-agents-come-from-the-marketplace.md; `agent kinds`, crates/core-agents/src/catalog.rs | Kinds are data: 4 built-ins plus profile files, package state reported (0059). Package install needs core-extensions (PR #142, in review). No default, auto-pick order or hide switch |
| AG-02 | Install detection, refresh and installation help | first version | M3 | partial | `agent kinds` (installed, detected, package), crates/core-agents/src/resolve.rs, crates/cli/src/app.rs | Local PATH detection; detected split from installed. Paired hosts report their own kinds via --host. No WSL detection; no install-docs links (needs core-extensions, PR #142, in review) |
| AG-03 | CLI version probing and capability gates | later | M6 | not started | docs/decisions/0050-agent-kinds.md | 0050 says no version probe in Phase 4; no capability gates (no structured chats yet) |
| AG-04 | Terminal launch command construction | first version | M3 | changed | docs/decisions/0051-agent-sessions.md; `agent start`, crates/core-agents/src/launch.rs | Agent spawned directly as the terminal program (no shell, so no per-shell quoting); args and env from kind plus caller; built. CLI passes no env |
| AG-05 | Prompt delivery to terminal agents | first version | M3 | partial | `agent start --prompt`, `agent prompt`, crates/core-agents/src/prompt.rs, crates/core-agents/src/sessions.rs, crates/core-agents/src/kind.rs | All 4 built-ins take the first prompt at launch (argv or flag). Later prompts paste text plus Enter (#120, 0054). Startup-query mode missing: not in PromptMode, so no kind can use it |
| AG-06 | Draft (unsent) prompts | later | M6 | not started | - | No draft prompt seeding anywhere |
| AG-07 | Image and file attachments for terminal agents | later | M6 | not started | - | No attachment or @file handling for terminal agents |
| AG-08 | Launch intent API (agent.launch) and launch receipts | first version | M3 | partial | `agent.start`, `agent.prompt`, crates/server/src/commands/core_table.rs, crates/protocol/src/methods/agents.rs | Start in an existing workspace works (request_id retry); agent.prompt served since #120. No launch intent call, no new-workspace placement, no launch receipt (surface, prompt delivery, placement) |
| AG-09 | Creating a workspace with a startup agent | first version | M3 | not started | - | workspace create has no startup agent; only two steps (workspace create, then agent start) |
| AG-10 | Permission posture (Yolo / Manual) and access presets | first version | M3 | changed | docs/decisions/0050-agent-kinds.md | No permission-bypass switch; caller passes bypass flags explicitly via `agent start -- <args>` (built). No presets |
| AG-11 | Per-agent Command, Arguments and Environment settings | first version | M3 | partial | `<profile>/agents/kinds/*.json`, crates/core-agents/src/catalog.rs, crates/core-agents/src/credentials.rs | User kinds declare program and args; per-start args and env on agent.start; profile home env (0056). No standing override of built-ins; CLI start sends no env; no vault secrets |
| AG-12 | Workspace trust pre-writes | first version | M3 | not started | - | No trust-prompt pre-writes in core-agents |
| AG-13 | Resume, continue and fork of terminal sessions | first version | M3 | partial | `agent.resume`, `agent.credentials.import`, crates/core-agents/src/catalog.rs, crates/core-agents/src/credentials.rs | Resume runs the kind's resume form with the stored ref (#120, 0054); profile credential homes keep logins (#121, 0056). No fork of Claude or Codex. Several accounts per kind: PR #144 (in review) |
| AG-14 | Model, effort and mode selection for launches | first version | M3 | not started | - | No model/effort/mode selection; only raw caller args after `--` |
| AG-15 | Structured (native) chat platform | first version | M3 | not started | - | No structured chat; activity source `output` reserved (0052) |
| AG-16 | Chat capabilities (queue, steer, stop, rewind, compact, goals, approvals, questions, subagents, context usage) | later | M6 | not started | `agent.stop`, `agent.prompt`, crates/server/src/commands/core_table.rs | No chat controls. Terminal runs have only stop and typed prompts; no queue, steer, rewind, compact, goals, approvals or questions |
| AG-17 | Claude structured adapter | first version | M3 | not started | - | Claude runs only as a terminal agent; no structured adapter |
| AG-18 | Codex structured adapter | later | M6 | not started | - | Codex runs only as a terminal agent |
| AG-19 | Pi RPC adapter and Gentle Shell | later | M6 | not started | - | Gentle Shell is a terminal kind with no status; no RPC adapter |
| AG-20 | Antigravity stream-json adapter | later | M6 | not started | - | Antigravity is a terminal kind with no status; no stream-json adapter |
| AG-21 | ACP client (Grok, OpenCode, OMP) | later | M6 | not started | - | No ACP client |
| AG-22 | Chat attachments and image prompts | later | M6 | not started | - | No chat attachments |
| AG-23 | Model catalog service for chats | first version | M3 | not started | - | No model catalog |
| AG-24 | Terminal-backed chat view (transcript chat) | later | M6 | not started | - | No transcript chat view |
| AG-25 | Provider process supervision | later | M6 | partial | docs/decisions/0051-agent-sessions.md; crates/core-agents/src/sessions.rs | Agents run in service terminals: tree kill and end with the service (0031/0033). No headless provider children exist yet |
| AG-26 | Delegated (child) sessions | first version | M3 | partial | `agent.delegate`, `agent delegate`, crates/core-agents/src/sessions/delegation.rs | Built (#133, 0055): own worktree by default, read-only in the parent workspace, depth 1. Missing: desktop card and manual pick (ui DelegationTree is gallery-only); no automatic parent wait |
| AG-27 | AI text generation through agent CLIs | later | M6 | not started | - | No AI text generation through agent CLIs |
| AG-28 | Claude Agent Teams mode | later | M6 | not started | - | No Claude Agent Teams mode (PR 118 is a docs-only Bot Teams decision) |
| AG-29 | Agent process recognition in panes | later | M6 | not started | - | Sessions own their terminal, so MassOrca-launched agents are known; no recognition of agents in other panes |
| AG-30 | Per-agent startup quirks | later | M6 | not started | - | No per-agent startup workarounds |
| AG-31 | Bring back a child session's git work into the parent worktree | later | M6 | partial | `agent.bring_back`, `agent bring-back`, crates/core-agents/src/sessions/delegation.rs | Manual bring-back built (#133, 0055): --no-ff merge into the parent worktree, conflicts aborted, parent told by prompt. Missing: desktop notice and automatic bring-back under automatic or full access |
| AG-32 | Shared checklist card in agent chats | later | M6 | not started | crates/ui/src/components/session_summary_card.rs (CardKind::Checklist) | No checklist card in agent chats; the ui checklist kind is gallery-only on sample data, with no screen or agent data |
| AG-33 | Jev decision for delegated task tier | later | M6 | not started | - | No delegated tier decision service |
| AG-34 | Ask the person a question from a chat agent | later | M6 | not started | - | No question card |
| AG-35 | Results card: environment URLs and opened files | later | M6 | not started | crates/ui/src/components/session_summary_card.rs (CardKind::Results) | No Results card on any screen; the ui Results kind is gallery-only on sample data, with no screen or agent data |
| AG-36 | Design-system folder access for chat agents in linked worktrees | later | M6 | not started | - | No design-system folder grant |
| STAT-01 | Agent status states and the status row | first version | M3 | partial | `agent status` / `agent.show`, crates/cli/src/agent.rs, crates/cli/src/agent_text.rs | Four activity states and execution from the real service in the CLI; CLI names a child's parent (agent_text.rs). Desktop status row uses sample data (crates/desktop/src/data.rs); no agent calls |
| STAT-02 | Hook listener and provider event mapping | first version | M3 | done | `agent.hook` + `massorca agent hook`, crates/core-agents/src/status.rs | Claude settings hooks and Codex notify mapped per kind declaration; Antigravity and Gentle Shell have no mechanism yet |
| STAT-03 | Agent coverage and detection paths | later | M6 | done | `agent kinds --json` (status_mechanism), crates/core-agents/src/catalog.rs | Mechanism per kind only in JSON; the text table omits it |
| STAT-04 | Installing managed hooks into agent configs | first version | M3 | changed | docs/decisions/0053-agent-status-sources.md; crates/core-agents/src/launch.rs | Hooks installed per run (--settings file, -c notify), never into agent configs; built |
| STAT-05 | Removing managed hooks and per-agent switches | first version | M3 | dropped | docs/decisions/0053-agent-status-sources.md | Nothing is written to user configs, so there is nothing to remove; no per-agent switch |
| STAT-06 | Installer safety: atomic writes, backups, ownership and locks | first version | M3 | dropped | docs/decisions/0053-agent-status-sources.md | No user config writes, so no backups, ownership or locks needed |
| STAT-07 | Offline spool of hook events | later | M6 | dropped | docs/decisions/0052-agent-status-store.md | Hooks fail open; a hook that cannot reach the service changes nothing, and runs end with the service |
| STAT-08 | Claude usage line feed | later | M6 | not started | - | No usage line feed |
| STAT-09 | Title, screen and status escape fallback for agents without hooks | later | M6 | dropped | docs/decisions/0053-agent-status-sources.md | Status never inferred from painted output or escape sequences |
| STAT-10 | Interrupt inference (Escape and Ctrl+C) | later | M6 | dropped | docs/decisions/0052-agent-status-store.md | Nothing inferred beyond exit: a run exiting while working becomes done/interrupted (built); no Esc/Ctrl+C inference |
| STAT-11 | Claude subagents, background tasks and wake-ups | later | M6 | not started | - | No subagent or background-task tracking |
| STAT-12 | Transcript reads for last message, and Codex and Muse polling | later | M6 | not started | - | Result text from a declared hook payload field only (none declared for Claude); no transcript reads or polling |
| STAT-13 | Status store on the execution host and persistence | first version | M3 | done | `agent.watch` / `agent.list`, crates/core-agents/src/sessions.rs | Store in the executing service; activity and result persisted in agent_sessions rows |
| STAT-14 | Pane identity, closure and reuse rules | first version | M3 | changed | docs/decisions/0051-agent-sessions.md; crates/core-agents/src/sessions.rs | One session per service terminal; a closed or gone terminal is recorded exited, a resume gets a new terminal; built |
| STAT-15 | Stale rows and dead-process cleanup | first version | M3 | partial | exit sweep and service_stopped marking, crates/core-agents/src/sessions.rs | Dead-process cleanup built. 30-minute freshness deferred by 0052; no approvals list |
| STAT-16 | Remote host status over SSH | first version | M3 | partial | `massorca agent` with --host, crates/cli/src/app.rs, crates/cli/src/agent.rs | CLI agent commands route through --host to a paired host (app.rs, Commands::Agent). Desktop shows no agent data: it never calls the agent service; its status row is sample data |
| STAT-17 | Status during disconnect and reconnect | first version | M3 | partial | execution state `unverifiable`, crates/protocol/src/methods/agents.rs | Wire vocabulary exists; no client keeps last state as unverifiable and replays on reconnect |
| STAT-18 | WSL agent relay | later | M6 | not started | - | No WSL relay |
| STAT-19 | Attention and unread markers | first version | M3 | not started | - | No attention or unread markers |
| STAT-20 | Desktop notifications | later | M6 | not started | - | No desktop notifications |
| STAT-21 | Notification sounds | later | M6 | not started | - | No notification sounds |
| STAT-22 | Phone alerts and desktop-away gating | later | M6 | not started | - | No phone alerts |
| STAT-23 | Keep-awake while agents work | later | M6 | not started | crates/server/src/features/config_keys.rs (agents.keep_awake), crates/desktop/src/model/settings_sections.rs | Only a setting key and a planned desktop switch exist; nothing keeps the computer awake |
| STAT-24 | Rename workspace from first agent work | later | M6 | not started | - | No rename from first agent work |
| STAT-25 | Repository setup and archive hooks (orca.yaml) | later | M6 | not started | docs/decisions/0009-phase1-deferrals.md | Setup scripts deferred to a later phase; nothing built |
| STAT-26 | Delegated parent link on status rows | first version | M3 | partial | `agent.tree`, `agent tree`, crates/cli/src/agent_text.rs, crates/core-agents/src/sessions/delegation.rs | Delegation tree served (#133); CLI prints a child's parent. No paired client or desktop view groups children under their parent; parent_id is not read by any desktop, ui or client crate |
| ACC-01 | Claude Code sign-in and import | first version | M3 | done | `agent.credentials.import` in crates/server/src/commands/core_table.rs; CLI `agent credentials import claude --from <DIR>` (docs/cli.md); crates/core-agents/src/credentials.rs | Claude config-folder import works on main (0056). Sign-in is not on main; it is only in open #144. Import alone meets the "or" in the feature. |
| ACC-02 | Codex sign-in and import | first version | M3 | done | `agent.credentials.import` in crates/server/src/commands/core_table.rs; CLI `agent credentials import codex --from <DIR>`; crates/core-agents/kinds/codex.json declares auth.json and config.toml | Codex config-folder import works on main. Sign-in is not on main; it is only in open #144. |
| ACC-03 | Account registry and duplicate detection | first version | M3 | in review | #144 (open): `agent accounts add`, `list`, `activate`, `remove`, `check` in crates/cli/src/agent_accounts.rs; registry in crates/core-store/src/agent_accounts.rs and crates/core-agents/src/accounts.rs | Registry per kind exists only in #144. Its diff has no refusal for adding the same login twice and no host or WSL scope. WSL code is absent on main. |
| ACC-04 | Claude Code account switching | first version | M3 | in review | #144 (open): `agent accounts activate` in crates/cli/src/agent_accounts.rs and accounts_activate in crates/core-agents/src/accounts.rs, per kind or per project | Choosing an account per kind or per project exists only in #144. It has no per-host or per-WSL-distro scope, and WSL is absent on main. |
| ACC-05 | Codex account switching | first version | M3 | in review | #144 (open): the same `agent accounts activate` mechanism applied to the codex kind (crates/core-agents/src/accounts.rs) | Codex account choice exists only in #144. On main, Codex runs use the single profile home when it exists (crates/core-agents/src/launch.rs). |
| ACC-06 | Re-authentication and account removal | first version | M3 | partial | `agent.credentials.remove` (crates/server/src/commands/core_table.rs); CLI `agent credentials remove <KIND>` (docs/cli.md); crates/core-agents/src/credentials.rs | Removing the kind's home works and is refused with agent.busy while a run is live (docs/cli.md). No re-authentication on main; sign-in again and per-account removal are in open #144. |
| ACC-07 | Claude token refresh and refresh ownership | first version | M3 | partial | crates/core-agents/src/launch.rs sets the kind's home variable (CLAUDE_CONFIG_DIR) to the profile home when it exists; docs/decisions/0056-agent-credentials.md | The agent refreshes its own tokens in that home. MassOrca does not check or refresh logins on main (0056). The account check is only in open #144. |
| ACC-08 | Live-session protection during account changes | first version | M3 | partial | crates/server/src/features/agents.rs refuses credential import --replace and remove while a run of the kind is live (agent.busy); docs/cli.md | Live runs are protected during a home swap. There is no account switching on main yet; switching and its guards are in open #144. |
| ACC-09 | Claude usage and limits | first version | M3 | partial | UsageModel rings, crates/desktop/src/data.rs | Rail rings drawn from sample data only (data.rs says sample data now); no Claude usage reading in service or CLI |
| ACC-10 | Codex usage and limits | first version | M3 | partial | UsageModel rings, crates/desktop/src/data.rs | Same as ACC-09: sample rings only, no Codex usage source |
| ACC-11 | Codex reset credits | later | M6 | not started | none | No Codex reset credits anywhere |
| ACC-12 | Read-only sign-in detection for Cursor, Grok, Gemini and Kimi | later | M6 | not started | none | No Cursor, Grok, Gemini or Kimi sign-in detection |
| ACC-13 | Stored API keys and cookies for key-based providers | later | M6 | partial | `secret.set` (crates/server/src/commands/secret_table.rs); CLI `secret set` (crates/cli/src/secret.rs); crates/core-vault/src/vault/mod.rs | The vault stores an API key. No OpenCode Go or MiniMax provider, no usage reading, no cookies. No agent receives vault values: environment_for has no caller outside tests. |
| ACC-14 | Managed OpenCode and Devin accounts | later | M6 | not started | none | No OpenCode or Devin accounts |
| ACC-15 | Usage refresh cadence and caching | first version | M3 | not started | none | No usage source, so no refresh cadence or caching |
| ACC-16 | Usage and limits dashboard | later | M6 | partial | rail usage rings and popover states, crates/desktop/src/model/shell.rs | Rings and popover text exist on sample data; no switcher menus, no usage page, no real data |
| ACC-17 | Limit warnings | drop | M6 | not started | none | Proposed drop in docs/inventory/README.md, but no decision record on main; nothing built |
| ACC-18 | Automatic account rotation on limits | drop | M6 | not started | none | Proposed drop in docs/inventory/README.md, no decision record; nothing built |
| ACC-19 | Local token-usage analytics and cost estimates | later | M6 | not started | none | No local token or cost analytics |
| ACC-20 | Usage attribution to workspaces and sessions | later | M6 | not started | none | No usage attribution |
| ACC-21 | Credential at-rest protection | first version | M3 | partial | crates/profile/src/profile.rs (profile dir 0700 on Unix, 0004); crates/core-agents/src/credentials.rs (homes 0700, files 0600, owner-only DACL on Windows); crates/core-vault/src/key/mod.rs | Credential homes are owner-only and the vault key is OS-wrapped or kept in an owner-only file. Credential files are not encrypted (0056). The Windows profile-root ACL is in open #130. |
| ACC-22 | Keep refreshed sign-ins across app restart | first version | M3 | partial | crates/core-agents/src/paths.rs (home under the profile); crates/core-agents/src/launch.rs (home set at launch); crates/core-agents/src/credentials.rs (import) | A home persists across restarts and the agent refreshes it in place. Import is served on main. Sign-in, and a check that a restored login still works, are in open #144. |
| ACC-23 | Per-account Claude profile folders | drop | M6 | changed | docs/decisions/0056-agent-credentials.md; crates/core-agents/src/credentials.rs; crates/core-agents/src/launch.rs | Replacement built on main: one credential home per profile and kind, with import, show and remove served. Per-account folders exist only in open #144. |
| ACC-24 | Carry existing subscriptions into the new core | first version | M3 | partial | `agent.credentials.import` for claude and codex (crates/core-agents/kinds/claude.json, codex.json; crates/core-agents/src/credentials.rs) | Claude and Codex logins can be carried in by import. API keys are not carried: there is no import path for keys. No Electron profile import. |
| ACC-25 | App activity counters | later | M6 | not started | none | No activity counters |
| ACC-26 | Delegated sessions choose an agent by usage headroom | later | M6 | partial | `agent.delegate` with an explicit kind (crates/protocol/src/methods/agents.rs; crates/core-agents/src/sessions/delegation.rs; docs/decisions/0055-delegation-and-bring-back.md) | Delegation with a chosen kind is built. Automatic choice by usage headroom is not, since no usage data exists (ACC-09, ACC-10). |
| HIST-01 | Browse past sessions | first version | M3 | partial | massorca agent list / agent.list, crates/server/src/features/agents.rs | Lists only sessions MassOrca started (persisted, newest first); no discovered agent history, scope filter, grouping or refresh UI |
| HIST-02 | Session titles, previews and metadata | first version | M3 | partial | AgentSession fields, crates/protocol/src/methods/agents.rs | Own sessions carry title, workspace, created time, activity time and last result; no branch, model or transcript preview |
| HIST-03 | Local discovery for Claude Code and Codex | first version | M3 | not started | docs/decisions/0054-prompts-and-resume.md | No local discovery of Claude Code or Codex conversations; 0054 says importing outside sessions is not in Phase 4 |
| HIST-04 | Local discovery for the other 22 agents | later | M6 | not started | none | No discovery for other agents |
| HIST-05 | Full-text search of session history | later | M6 | not started | none | No history search. The sessions.search setting is declared (crates/server/src/features/config_keys.rs) but no index or search method exists. |
| HIST-06 | Transcript viewer | later | M6 | not started | none | No transcript viewer; terminal screen/scrollback exists but is not a transcript |
| HIST-07 | Resume a past session in a terminal | first version | M3 | done | `agent resume` (docs/cli.md); `agent.resume` in crates/server/src/commands/core_table.rs; crates/core-agents/src/sessions.rs (resume); resume args in crates/core-agents/kinds/claude.json | Resumes sessions MassOrca started: a new run in a new terminal with the agent's own resume args. Discovered sessions are not listed; the desktop has no resume control. |
| HIST-08 | Adopt or fork a past session into a structured chat | later | M6 | not started | none | No adopt or fork into structured chat |
| HIST-09 | Delete a past session | later | M6 | not started | none | No delete of agent conversation files (agent.forget only drops MassOrca's own row) |
| HIST-10 | Remote history on SSH and runtime hosts | later | M6 | partial | docs/cli.md (--host applies to every command that talks to a service); agent.list and agent.resume served | MassOrca sessions on a paired host can be listed and resumed with --host. Discovered remote history is not built; the desktop has no remote history view. |
| HIST-11 | WSL history on Windows | later | M6 | not started | none | No WSL history; no WSL handling exists on main |
| HIST-12 | Claude account history sharing | later | M6 | not started | none | No Claude account history sharing |
| HIST-13 | Cursor IDE sign-in state read | later | M6 | not started | none | No Cursor sign-in read |
| HIST-14 | OpenCode live session binding | later | M6 | not started | none | No OpenCode binding |
| HIST-15 | Projectless chat folders | later | M6 | not started | none | No projectless chat folders or Chats group in the desktop. conversations.folder is declared (crates/server/src/features/config_keys.rs) but unused. |
| HIST-16 | Automated-run filter, delegated and subagent nesting | later | M6 | partial | `agent.delegate` and `agent.tree` (crates/server/src/commands/core_table.rs); crates/core-agents/src/sessions/delegation.rs; parent_id in AgentSession; indent in crates/desktop/src/model/shell.rs | Delegation and tree are served and the desktop model indents child sessions. No headless-run filter and no Claude or OMP subagent transcripts. |
| HIST-17 | Process memory dashboard | later | M6 | not started | none | No process memory dashboard; status bar memory item is sample data |
| HIST-18 | Import sessions from other agent tools | first version | M3 | not started | docs/decisions/0054-prompts-and-resume.md | 0054: importing sessions started outside MassOrca is not in Phase 4; nothing built |
| REM-01 | Add an SSH host manually | first version | M2 | done | crates/cli/src/host/add_ssh.rs (host add-ssh); crates/cli/src/pair.rs (pair accept) | CLI only; no desktop host UI. Per 0044 a host is saved only by installing and pairing (Linux --binary); user, key and jump come from ~/.ssh/config. |
| REM-02 | Import hosts from the OpenSSH config | later | M6 | changed | docs/decisions/0044-ssh-host-bootstrap.md; crates/core-remote/src/ssh.rs | System ssh reads ~/.ssh/config, so aliases work without an import step; no host picker or sync. Replacement built on main. |
| REM-03 | Authenticate with keys, agent and password | first version | M2 | partial | crates/core-remote/src/ssh.rs (system ssh, BatchMode=yes) | Keys and SSH agent work through OpenSSH. Missing: password or passphrase prompts on demand; batch mode refuses them and 0044 defers them to an SSH library. |
| REM-04 | Verify host keys | first version | M2 | partial | crates/cli/src/host/status.rs (identity_mismatch); crates/core-remote/src/ssh.rs (known_hosts policy) | Trust is kept per host: OpenSSH known_hosts and the pinned server key. A changed key shows identity_mismatch, but the message names no fix; no desktop warning. |
| REM-05 | Jump hosts and ProxyCommand | later | M6 | partial | crates/cli/src/args.rs (host add-ssh --ssh-arg); crates/core-remote/src/ssh.rs | Bootstrap reaches bastion or ProxyCommand hosts via ~/.ssh/config or --ssh-arg. After pairing the client dials the listener directly, with no SSH tunnel, so a bastion-only host is not reached. |
| REM-06 | System OpenSSH transport | later | M6 | partial | crates/core-remote/src/ssh.rs (SystemSsh, BatchMode=yes) | System OpenSSH is the only SSH transport (0044). FIDO2 and Kerberos are not handled or tested; BatchMode disables the prompts they may need. |
| REM-07 | Connection lifecycle, reconnect and status | first version | M2 | partial | crates/cli/src/host/status.rs (connected, unreachable, refused, identity_mismatch) | Per-host state is visible. Watches reconnect with jittered backoff (crates/client/src/reconnect.rs, 0040) and report `unverifiable` / `live`; `terminal attach` resumes from its output offset. Missing: the desktop cannot target hosts. |
| REM-08 | Deploy and update the remote relay | first version | M2 | changed | docs/decisions/0044-ssh-host-bootstrap.md; crates/core-remote/src/bootstrap.rs | No relay: host add-ssh --binary uploads the Linux binary over SSH, writes the systemd unit and upgrades on re-run. Replacement built; nothing installs on connect. |
| REM-09 | Runtime ladder and plain-SSH fallback | drop | M6 | dropped | docs/decisions/0044-ssh-host-bootstrap.md | One host daemon; SSH is only how a host is installed and paired, so there is no plain-SSH degraded mode. |
| REM-10 | Native terminal dependencies on the host | drop | M6 | dropped | docs/decisions/0044-ssh-host-bootstrap.md | The host runs the same Rust binary with its own PTY and watch code; no per-OS Node addons exist to build. |
| REM-11 | Remote terminals | first version | M2 | done | crates/cli/src/target.rs (global --host); crates/cli/src/terminal.rs; crates/cli/src/agent.rs | Terminals and agents run on a paired host with --host NAME. CLI only: the desktop connects to its local service and cannot target a host. |
| REM-12 | Terminals survive disconnects | first version | M2 | done | crates/cli/src/terminal.rs (list, screen, attach); crates/server/src/features/terminal_attach.rs | terminal list --host finds terminals still running; attach and screen restore the screen from the snapshot. CLI only; no desktop remote view. |
| REM-13 | Remote file access | later | M6 | not started | none | No file browse, read, edit or watch methods in crates/protocol on main or in an open PR. |
| REM-14 | Remote git and worktrees on the host | first version | M2 | done | crates/cli/src/git/mod.rs (status, commit, fetch, pull, push); crates/cli/src/workspace.rs (create) | git commands and workspace create run on a host with --host, through its service. CLI only; the desktop cannot target a host. |
| REM-15 | Remote CLI passthrough and orchestration | later | M6 | not started | none | No CLI passthrough from host agents back to the desktop. |
| REM-16 | Manual port forwards | later | M6 | not started | none | No port forwarding. |
| REM-17 | Remote port detection and advertised URLs | later | M6 | not started | none | No remote port detection or advertised dev-server URLs. |
| REM-18 | Managed Orca server on SSH hosts | later | M6 | changed | docs/decisions/0044-ssh-host-bootstrap.md; crates/server/src/network/listen.rs | One host daemon replaces relay plus managed server, reached through the network listener (built on main). The SSH tunnel path is deferred by 0044. |
| REM-19 | Link an SSH host to a paired server | later | M6 | changed | docs/decisions/0044-ssh-host-bootstrap.md; crates/client/src/hosts.rs | An SSH host and a paired server are one host record on the client, reached through the listener (built). The verified tunnel is not built; 0044 defers it. |
| REM-20 | WSL distributions as an execution host | later | M6 | changed | docs/decisions/0015-git-on-remote-execution-hosts.md | A WSL distro runs its own service and Windows connects to it as to any host; that generic network path is on main. No WSL-specific setup or routing exists. |
| REM-21 | Ephemeral VMs from environment recipes | later | M6 | not started | none | No ephemeral VM provisioning. |
| REM-22 | Remove a host and clean its remote state | first version | M2 | done | crates/cli/src/host/records.rs (host remove); crates/core-remote/src/bootstrap.rs (uninstall) | Revokes this device unless --keep-device; --uninstall --ssh stops and removes the unit and files. Workspaces stay in the host's store; re-pairing restores access. |
| REM-23 | Linux and macOS remote hosts | first version | M2 | partial | crates/core-remote/src/bootstrap.rs (parse_uname: Linux x86_64, aarch64) | Linux hosts on x86_64 and aarch64 are built. macOS hosts are rejected by the uname check; 0044 defers macOS, so it is later, not dropped. |
| REM-24 | Windows remote hosts | later | M6 | not started | none | Windows hosts deferred by 0044; bootstrap rejects non-Linux. |
| REM-25 | Relay diagnostics and self-test | later | M6 | partial | crates/cli/src/host/status.rs; crates/cli/src/server_status.rs; crates/core-remote/src/bootstrap.rs (unit passes --log-stderr) | Refusal reasons from host status and server status; logs via journalctl. No self-test command. |
| REM-26 | Remote host answers GitHub account and Marketplace calls | later | M6 | not started | none | No GitHub account or Marketplace handling on hosts. |
| REM-27 | Read a file an agent wrote on a paired runtime | later | M6 | not started | none | No method to read agent-written files outside the worktree. |
| SRV-01 | Headless server mode (serve) | first version | M2 | done | crates/cli/src/service/foreground.rs (service run --listen); crates/cli/src/service/unit.rs; crates/server/src/network/listen.rs | Headless service with a network listener; service unit prints the systemd unit. Reached from the CLI via --host. No phone app in this repo (client crate only). |
| SRV-02 | Readiness and machine-readable output | later | M6 | partial | crates/cli/src/server_status.rs; crates/protocol/src/methods/service.rs (ServiceInfo); crates/cli/src/args.rs (global --json) | Startup prints profile and pid; ServiceInfo adds version and protocol. Endpoints, server pin and pending offers come only from server status, not at startup. |
| SRV-03 | Local owner channel for same-machine clients | first version | M2 | done | `service status` / local socket or named pipe, crates/server/src/owner.rs | Owner-only socket or pipe with peer check (0003); desktop and CLI both use it. |
| SRV-04 | Network listener and bind policy | first version | M2 | changed | docs/decisions/0045-headless-service.md; crates/server/src/network/listen.rs | Replacement built: the listener is opted in by service run --listen, not by pairing. Forms: tailscale, localhost, IP; the all-interfaces wildcard is refused without --allow-all-interfaces. |
| SRV-05 | Runtime discovery file and ownership | first version | M2 | done | `service status`, crates/server/src/service_json.rs | Per 0003 the socket name is derived from the profile (no discovery needed); service.json written; stale lock and socket reclaimed. |
| SRV-06 | Pairing offer (QR and URL) | first version | M2 | partial | crates/cli/src/pair.rs; crates/server/src/network/pairing.rs; crates/protocol/src/pairing.rs | mop1. offer is single use and expires (10 min default, 1 h max). Missing: QR code (0041 says later) and any way to retire an offer before it expires. |
| SRV-07 | Advertised address and override | first version | M2 | partial | crates/server/src/network/state.rs (advertised_addresses); crates/server/src/features/devices.rs | Offer lists bound listener addresses and the MagicDNS name. No override: pair offer takes only --admin and --ttl. |
| SRV-08 | Device registry and per-device tokens | first version | M2 | changed | docs/decisions/0041-device-identity-and-pairing.md; crates/server/src/features/devices.rs | Replacement built: per-device client certificates (mutual TLS) instead of tokens, with device list and revoke on main. |
| SRV-09 | Device scopes and grants | later | M6 | done | crates/server/src/permission.rs (Scopes STANDARD, ADMIN); crates/cli/src/args.rs (pair offer --admin) | Read and workspace by default; host-admin and device-admin only when granted at pairing with --admin. No web clients exist (SRV-20). |
| SRV-10 | Handshake, authentication and limits | first version | M2 | done | crates/server/src/network/listener.rs (128 connections, 10 s upgrade); crates/server/src/network/admission.rs; crates/server/src/network/pairing.rs | Pre-auth limits: 32 unauthenticated, 4 per address, 64 KiB hello, from the PR 85 pre-auth gate. Pairing failure limits per address and overall. |
| SRV-11 | End-to-end encryption on the network channel | first version | M2 | changed | docs/decisions/0040-network-transport.md; crates/net/src/tls.rs | Replacement built: TLS 1.3 with pinned server key and mandatory client certificates (PR 67) replaces app-level encryption. |
| SRV-12 | TLS on the network listener | later | M6 | changed | docs/decisions/0041-device-identity-and-pairing.md; docs/decisions/0042-tls-and-tailscale.md; crates/net/src/tls.rs | TLS is always on with the pinned key (0042), built on main. No optional HTTPS or WSS mode; a TLS-terminating proxy breaks device auth (0041). |
| SRV-13 | Relay path for phones without LAN reach | later | M6 | not started | none | No relay; 0042/0043 make Tailscale the way to reach a host but no record drops the relay. first-version.md: not in the first version. |
| SRV-14 | Connection mode: automatic or LAN only | later | M6 | not started | none | Depends on the relay, which does not exist. |
| SRV-15 | Multiple simultaneous clients and revocation | first version | M2 | done | crates/server/src/connection.rs (multiple clients); crates/server/src/network/live.rs; crates/server/src/features/devices.rs (device revoke) | Local and network clients stay connected together. device revoke closes only that device's connections. Phone client is not in this repo. |
| SRV-16 | Protocol version and capability negotiation | first version | M2 | done | `hello`/`welcome` negotiation, crates/server/src/handshake.rs | Version range negotiated, `rejected` names the supported range; capabilities offered per connection. Tested in crates/protocol/tests/version_negotiation.rs. |
| SRV-17 | Reconnect and resync | first version | M2 | partial | Desktop reconnect with backoff, crates/desktop/src/service/bridge.rs; `terminal.attach since` | Local link reconnects and terminals resume. Client streams reconnect with jittered backoff (crates/client/src/reconnect.rs) and `git watch` / `agent status --watch` use it. |
| SRV-18 | Client connection to a server | first version | M2 | done | crates/client/src/remote.rs (connect_host); crates/cli/src/connect.rs (--host); crates/client/src/hosts.rs | CLI drives a saved host over TLS 1.3 and WebSocket (0046). Desktop cannot target a host. |
| SRV-19 | Mobile web bundle served over RPC | drop | M6 | dropped | docs/decisions/0040-network-transport.md | Android uses the Rust client crate through UniFFI, so no web bundle is served. |
| SRV-20 | Browser web client over HTTP | later | M6 | not started | none | No browser web client; 0040 only keeps WebSocket so one could return. |
| SRV-21 | Server self-update | later | M6 | partial | crates/cli/src/host/add_ssh.rs; crates/core-remote/src/bootstrap.rs | Re-running host add-ssh --binary over SSH upgrades the binary and restarts the unit. No upgrade over the RPC link; servers without SSH are upgraded by hand (0045). |
| SRV-22 | Push registration for paired phones | later | M6 | not started | none | No push registration. |
| AUTO-01 | Scheduled automation definitions | first version | M3 | not started | crates/desktop/src/shell/canvas.rs | Automations sidebar entry opens a "This page is not built yet" placeholder; no automation methods in protocol, server or CLI |
| AUTO-02 | Destinations and workspace modes | first version | M3 | not started | none | No automation destinations or workspace modes anywhere on main or in an open PR |
| AUTO-03 | Schedule engine, missed runs and retention | first version | M3 | not started | none | No schedule engine, missed-run record or retention on main or in an open PR |
| AUTO-04 | Run now and run history | first version | M3 | not started | none | No run-now or run history for automations on main or in an open PR |
| AUTO-05 | Precheck gate | later | M6 | not started | none | No precheck gate on main or in an open PR |
| AUTO-06 | Agent launch for runs, extra arguments and session reuse | first version | M3 | partial | agent.start (massorca agent start KIND --prompt -- ARGS); agent.resume; crates/server/src/features/agents.rs | Launch with prompt and extra args, and session resume (#120), exist; no automation runs or saved prompts yet |
| AUTO-07 | Run completion, output and usage | first version | M3 | partial | agent.show / agent.watch (agent status --until done); crates/server/src/features/agents.rs | Agent session done state and result text exist; no automation run records and no token usage |
| AUTO-08 | Host ownership, fencing and orphans | first version | M3 | not started | none | No automation host ownership, fencing or orphan handling |
| AUTO-09 | Remote and SSH automation scheduling | later | M6 | not started | none | No remote or SSH automation scheduling; SSH host and network layers (#68-#70) are on main but schedule no automations |
| AUTO-10 | Headless run terminal retention | later | M6 | not started | none | No headless run terminal retention |
| AUTO-11 | External Hermes and OpenClaw cron managers | drop | M6 | not started | docs/inventory/automations.md (AUTO-11 Proposal: drop) | Only an inventory proposal to drop; no decision record or owner direction found confirming it, so not marked dropped |
| AUTO-12 | Bot team roster, editing and team access | first version | M3 | partial | crates/cli/src/team/mod.rs; crates/server/src/features/teams/bot.rs; crates/protocol/src/methods/teams.rs | Team and bot create, list, edit and remove work via CLI (`team bot list` / `team.bot_list`); access is full only; team folder in Workspaces waits for task.start; desktop roster on sample data |
| AUTO-13 | Conference room: replies, request routing and choice cards | first version | M3 | not started | crates/desktop/src/shell/canvas.rs (Conference placeholder) | Conference room canvas is a placeholder saying live agents are not built; 0072 (PR 118) puts room routing out of scope |
| AUTO-14 | Team Board: task states, columns and reassign | first version | M3 | partial | crates/server/src/features/teams/board.rs; crates/server/src/features/teams/task.rs | Board and task create, update, start, done and reopen work (CLI; `task start` runs the bot in the team's shared folder, a busy bot is refused with `bot.busy`, 0072 amendment); the Done outcome is latched into the task, so a forgotten session keeps it; stop is missing; columns per 0072; desktop Board on sample data |
| AUTO-15 | Orchestration state store and Runs | first version | M3 | partial | crates/core-store/src/migrations.rs; crates/core-store/src/tasks.rs | Team, bot, team project and task tables exist in 8 migration steps, one SQLite file; no Runs, dispatch, message, gate or federation tables; inventory's schema 61 is not on main |
| AUTO-16 | Tasks and dispatch to terminals | first version | M3 | partial | crates/cli/src/task/mod.rs; crates/server/src/features/teams/task.rs | Tasks can be created, listed and started on their bot's agent (`task start` / `task.start`); no circuit breaker |
| AUTO-17 | Worker lifecycle | first version | M3 | partial | agent.start/stop/forget/attach; agent.delegate; crates/server/src/features/agents.rs; crates/core-agents/src/sessions/delegation.rs | Writing children get their own worktree (#133); commands also reach a paired host via --host; no explicit release or retain command |
| AUTO-18 | Agent-to-agent messages | first version | M3 | not started | none | No agent-to-agent messages or group addresses |
| AUTO-19 | Decision gates and blocking questions | later | M6 | not started | none | No decision gates or blocking questions |
| AUTO-20 | Delegation from chat, team delegate and Automatic routing | first version | M3 | partial | agent.delegate (massorca agent delegate); crates/server/src/features/agents.rs; crates/core-agents/src/sessions/delegation.rs | Explicit delegation works (#133); no bot-to-teammate delegate from chat or team, and no usage-based Automatic routing (0055: not built) |
| AUTO-21 | Nested worker depth limit | first version | M3 | partial | crates/core-agents/src/sessions/delegation.rs; crates/server/src/features/config_keys.rs | Depth 1 is enforced (agent.depth_exceeded); setting agents.delegation_depth (default 3 per 0070) has no reader, so it cannot change the limit |
| AUTO-22 | Multi-server federation of runs and workers | later | M6 | not started | none | No federation of runs or workers |
| AUTO-23 | Orchestration over SSH hosts | later | M6 | partial | crates/cli/src/target.rs; crates/cli/src/agent.rs; crates/cli/src/team/mod.rs | Global --host addresses a paired host for agent, team and task commands; host add-ssh installs over SSH; no test runs orchestration through --host |
| AUTO-24 | Legacy coordinator loop | drop | M6 | dropped | docs/inventory/automations.md (AUTO-15 owner direction 2026-11-12) | Owner direction: the Run concept is first version; the legacy loop (AUTO-24) stays a drop |
| AUTO-25 | Artifact sharing commands | later | M6 | not started | none | No artifact sharing commands |
| AUTO-26 | Artifact sharing gate and account rules | later | M6 | not started | none | No artifact sharing gate or account rules |
| AUTO-27 | Artifact records, idempotency and recovery | later | M6 | not started | none | No artifact records or idempotent publish |
| AUTO-28 | Routines | first version | M3 | partial | crates/desktop/src/shell/teams/routines.rs | Routines page lists sample routines with enable switch; routine form not built; no routine service or scheduler |
| AUTO-29 | Web address (webhook) triggers | later | M6 | partial | crates/desktop/src/shell/teams/routines.rs | Routines page shows sample web addresses with hidden tokens; no webhook listener, keys or rate limit in the service |
| AUTO-30 | Calendar of routines and automations | first version | M3 | not started | crates/desktop/src/model/navigation.rs (Page::Calendar) | Calendar rail entry opens the "This page is not built yet" placeholder |
| AUTO-31 | Team files and team memory | later | M6 | partial | crates/core-store/src/team_memory.rs; crates/server/src/features/teams/memory.rs; crates/desktop/src/shell/teams/files.rs; crates/desktop/src/shell/teams/memory.rs | Team memory built: store, `team.memory_*` methods and `massorca team memory`; task start does not read it yet. Desktop Files and Memory pages render sample data only; team files and the Engram link are missing |
| AUTO-32 | Bring back child session work into the parent | first version | M3 | partial | agent bring-back (massorca agent bring-back); crates/core-agents/src/sessions/delegation.rs | Manual merge of a finished child's branch into the parent works (#133); automatic ask, auto and full policies not built (0055 defers them) |
| AUTO-33 | Bot team projects: Board, Brief and code source | first version | M3 | partial | team project add/update/rm and team board --project; crates/server/src/features/teams/team_project.rs | Projects with goal, brief and own Board exist, listed by `team project list` / `team.project_list`; code source is a link to a Workspaces project (0072 amendment), tasks on it not started yet; desktop project page on sample data |
| AUTO-34 | Move a waiting task to another bot or team | later | M6 | partial | task update --bot (crates/server/src/features/teams/task.rs) | Reassign to a bot of the same team works; no move to another team, no handover notes, no check that the task is not started, stopped or stuck |
| AUTO-35 | Task deliverables returned to the asking room | later | M6 | not started | none | No task deliverables returned to the asking room |
| AUTO-36 | File versions of team folders | later | M6 | not started | none | No file versions of team folders |
| AUTO-37 | Room answers with web research or code reading | later | M6 | not started | none | No room answers with web research or code reading |
| BRW-01 | Load and navigate web pages in the pane | first version | M4 | partial | BrowserChrome component, crates/ui/src/components/browser_chrome.rs; engine direction docs/decisions/0026-browser-pane-engine.md | Toolbar (back, forward, reload, address) exists in the gallery only; no web engine (0026 says nothing built), not used by the desktop, no persistent sign-ins |
| BRW-02 | Tabs, popups and page-opened windows | first version | M4 | partial | WorkTabKind::Browser, crates/ui/src/components/work_panel_tabs.rs | Tab strip component has a "New browser tab" kind; desktop Work panel draws no browser tabs and no popup handling; no engine (0026) |
| BRW-03 | Browser profiles (isolated cookie sessions) | later | M6 | not started | - | No browser profiles or cookie isolation; no web engine on main or in an open PR |
| BRW-04 | Browser identity (user agent mode) | drop | M6 | not started | - | Nothing built; inventory proposes drop but no decision record or owner direction on main confirms it |
| BRW-05 | Import cookies from other installed browsers | later | M6 | not started | - | No cookie import |
| BRW-06 | Read, set and clear cookies and site storage | later | M6 | not started | - | No cookie or site-storage access |
| BRW-07 | Localhost preview of dev servers | first version | M4 | not started | - | No localhost preview or per-worktree names; needs the browser pane (0026) |
| BRW-08 | Find in page | later | M6 | not started | - | BrowserChrome draws a find button, but no page search exists without an engine |
| BRW-09 | Screenshots and PDF export | later | M6 | not started | - | No page capture or PDF export |
| BRW-10 | DevTools window and debugger endpoint | later | M6 | not started | - | No DevTools or debugger proxy |
| BRW-11 | Viewport, device and environment emulation | later | M6 | not started | - | No emulation of viewport, network or geolocation |
| BRW-12 | Live screencast of a page with remote input | later | M6 | not started | - | No screencast or remote input |
| BRW-13 | Agent browser automation commands | later | M6 | not started | - | No browser automation commands in the CLI (docs/cli.md) |
| BRW-14 | Element snapshot with refs for agents | later | M6 | not started | - | No element snapshot |
| BRW-15 | Request interception, network and console capture | later | M6 | not started | - | No request interception or console capture |
| BRW-16 | File uploads and downloads | later | M6 | not started | - | No uploads or downloads |
| BRW-17 | Keyboard shortcuts, context menu and zoom in the pane | later | M6 | not started | - | No pane, so no in-page shortcuts, context menu or zoom (zoom chip exists only in the gallery component) |
| BRW-18 | Certificate prompts and passkeys | later | M6 | not started | - | No certificate prompts or passkeys |
| BRW-19 | Pane traffic through SSH or WSL hosts | later | M6 | not started | - | No pane traffic routing through SSH or WSL |
| BRW-20 | Client-hosted and headless browser pages | drop | M6 | not started | - | Nothing built; inventory proposes drop, but design D-BP-01 keeps a client-hosted page state (docs/design/screen-inventory.md), so no drop is confirmed |
| BRW-21 | Preview workspace files (documents, images, media) | later | M6 | not started | - | No file preview in the pane; Work panel tab kinds name Markdown and file but nothing renders them on main |
| BRW-22 | Element grab for agent prompts (Design Mode) | later | M6 | not started | - | BrowserChrome draws a grab button only; no element grab into prompts |
| BRW-23 | Desktop computer use: inspect apps and UI | later | M6 | not started | - | Computer use is a disabled Settings row (crates/desktop/src/model/settings_sections.rs) |
| BRW-24 | Desktop computer use: act on apps | later | M6 | not started | - | No computer-use actions |
| BRW-25 | Desktop computer use: permissions and diagnostics | later | M6 | not started | - | No permission checks or diagnostics for computer use |
| BRW-26 | Emulator device control (Android and iOS simulators) | later | M6 | not started | - | Work panel shows "Mobile emulators come later." (crates/desktop/src/shell/work_panel.rs); no emulator control or streaming |
| BRW-27 | Emulator screen streaming and input | later | M6 | not started | - | No emulator streaming or input |
| BRW-28 | Voice dictation | later | M6 | not started | - | Composer has a Dictate button and Settings a disabled Dictation row; no speech capture or recognition |
| BRW-29 | Local speech models | later | M6 | not started | - | Speech model is a disabled Settings row; no model management |
| BRW-30 | Cloud transcription with an OpenAI key | later | M6 | not started | - | No cloud transcription; the Settings Transcription key row is planned, and the vault (0071) could hold the key |
| SKL-01 | Find skills already installed in agent tools | later | M6 | not started | docs/architecture/extensions.md | No skill discovery; a `Skill` placement registry is planned for M3; ext-package validates a `skills` contribution kind (#135), nothing discovers skills |
| SKL-02 | Place skills for agent tools | later | M6 | not started | docs/architecture/extensions.md | Not built; M3 plans a `Skill` placement registry used by built-ins |
| SKL-03 | Check freshness and update installed MassOrca skills | later | M6 | not started | none | No MassOrca skills or freshness check |
| SKL-04 | MassOrca's own skills and version-matched guides | later | M6 | not started | none | No bundled skills or guides; extensions.md plans an `extension-author` skill only for Creator mode |
| SKL-05 | Share selected skills as an unlisted link | later | M6 | not started | none | No skill sharing links; 0001 distribution uses indexes, archives and dev folders, not unlisted links |
| SKL-06 | Install a shared skill bundle | later | M6 | not started | none | No bundle install |
| SKL-07 | Update and roll back a shared bundle | later | M6 | not started | none | No bundle versions or rollback |
| SKL-08 | Revoke and delete shared links and versions | later | M6 | not started | none | No link revocation |
| SKL-09 | Skill bundle package format and limits | later | M6 | partial | crates/ext-package/src/archive.rs; docs/decisions/0061-extension-package-format.md | Extension archives on main (0061, #135): SHA-256 pin, unsafe entry names refused, `skills` folders with SKILL.md; no skill bundle publish, links, install or skill limits |
| SKL-10 | Cloud storage and transfer for bundles | later | M6 | not started | none | No cloud storage or transfer |
| SKL-11 | Protect local edits during install and update | later | M6 | not started | none | No skill install path, so no edit protection |
| SKL-12 | Crash-safe install, update and removal | later | M6 | not started | docs/architecture/extensions.md | Not built on main; open #142 stages then renames and writes the lock atomically for extension installs; no crash recovery, updates or skill installs |
| SKL-13 | Remove installed skills | later | M6 | partial | crates/desktop/src/shell/marketplace/dialogs.rs; crates/desktop/src/model/marketplace/sample/mod.rs | Marketplace uninstall confirm for installed sample items, in memory only; no removal of skill copies or placements, no scope choice, no CLI command |
| SKL-14 | Delete local skill folders safely | later | M6 | not started | none | No local skill deletion |
| SKL-15 | Install and manage skills on a paired server | later | M6 | not started | crates/protocol/src/methods/devices.rs | No skills on a paired server; pairing is on main (#66, #69: pairing and device methods) but there are no skill methods |
| SKL-16 | Install and manage skills on SSH hosts | later | M6 | not started | crates/core-remote/src/lib.rs | No SSH host skills; SSH host bootstrap is on main (#68) but no skill install goes through the relay |
| SKL-17 | Skills in WSL distributions | later | M6 | not started | none | No WSL skill install |
| SKL-18 | Plugin system switch, package format and installation | later | M6 | changed | docs/decisions/0001-extension-model.md; docs/decisions/0061-extension-package-format.md | Replacement partly built: package format on main (0061, #135); install, CLI and lockfile in open #142 and #143; updates and rollback not built |
| SKL-19 | Plugin consent, enablement and audit | later | M6 | changed | docs/decisions/0001-extension-model.md; crates/ext-package/src/risk.rs | Replacement partly built: risk review on main (#136); consent fingerprint and install review in open #142; per-permission revoke not built; lockfile installer records only in #142 |
| SKL-20 | Plugin runtime and host API | later | M6 | changed | docs/decisions/0001-extension-model.md; docs/decisions/0060-extension-runtime-spike.md | Wasm replacement decided and spiked: 0060 picks QuickJS-NG in a Wasm component (spikes/ext-runtime, not product code); no plugin runtime, host API or secrets on main; M6 |
| SKL-21 | Plugin contributions: panels, commands and keybindings | later | M6 | changed | docs/decisions/0001-extension-model.md; crates/ext-package/src/manifest/contributes.rs | Replaced by declarative view documents in 14 slots plus commands and keybindings kinds; manifest validates them (#135); renderer M4 and runtime M6 not built |
| SKL-22 | Plugin marketplace sources and listings | later | M6 | changed | docs/decisions/0001-extension-model.md | Replaced by an Ed25519-signed official index plus third-party indexes; M6, not built; no index or signing on main (catalogue module only in open #142) |
| SKL-23 | Plugin kill list | later | M6 | changed | docs/decisions/0001-extension-model.md | Replaced by the revocation list carried in the signed official index; M6, not built |
| SKL-24 | Plugin language packs, VM recipes and agent profiles | later | M6 | changed | docs/architecture/extensions.md; crates/ext-package/src/manifest/contributes.rs | Replaced by `locales`, `recipes` and `agentAdapters` contribution kinds; ext-package validates them (#135) but nothing applies them; M6, not built |
| SKL-25 | Bundled launch plugins | drop | M6 | not started | docs/inventory/skills-and-plugins.md | Inventory proposes drop; no decision record or owner text, so lowered to not started |
| SKL-26 | Plugin development loop | drop | M6 | changed | docs/decisions/0001-extension-model.md | Kept in a new form: dev-folder installs with `massorca ext reload` in Creator mode (M6); `massorca ext` now returns extension.not_implemented |
| SKL-27 | Marketplace page and catalog | first version | M4 | partial | crates/desktop/src/shell/mod.rs; crates/desktop/src/model/marketplace/source.rs | Marketplace page opens from the rail (#138) with sample items (2 agents, 5 extensions, 1 skill); with a running service the source is UnavailableMarketplace and lists nothing |
| SKL-28 | Marketplace host picker | later | M6 | not started | crates/desktop/src/shell/marketplace/item.rs | Item page shows the item's host and state as a label from sample hosts; the selector does not open and switching needs the service; no per-host catalogue or installs |
| SKL-29 | Standard item pages | later | M6 | partial | crates/desktop/src/model/marketplace/item_page.rs; crates/desktop/src/shell/marketplace/item.rs | Item page parts drawn from sample items: Information, Settings, Advanced Options (installed), Danger Zone, Connection (agents); no "who can use it" part |
| SKL-30 | Install and uninstall host programs | later | M6 | not started | none | No host program installers on main; vendor installer runs for four agent CLIs are in open #142 only; Git, GitHub CLI and Node.js not covered |
| SKL-31 | GitHub accounts on the Marketplace page | later | M6 | not started | none | No gh account management |
| SKL-32 | Engram memory connection for agents | later | M6 | not started | docs/architecture/extensions.md | No Engram connection; planned as core-owned `mcpServers` and `skills` data contributions |
| SKL-33 | Engram cloud sync and memory import | later | M6 | not started | none | No Engram sync or import |
| SKL-34 | OpenMetadata connection for agents | later | M6 | not started | docs/architecture/extensions.md | No OpenMetadata connection; planned as core-owned `mcpServers` entries |
| SKL-35 | Service connections from the Marketplace | later | M6 | not started | crates/desktop/src/model/settings_sections.rs | Only disabled Connect rows in Settings Sources; no Services section or backing |
| SKL-36 | Who can use each app | later | M6 | not started | none | No per-app agent or bot access control |
| SKL-37 | Credential vault for app and service tokens | later | M6 | partial | crates/core-vault/src/vault/environment.rs; crates/server/src/features/secrets.rs; crates/cli/src/secret.rs | Encrypted write-only vault: secret.* methods and `massorca secret` CLI (0071, #132, #141); no caller injects values into launched agents (environment_for has no caller outside core-vault) |
| SKL-38 | Archify skill from a pinned release | later | M6 | not started | none | No Archify install |
| SKL-39 | MassOrca board skills and built-in commands | later | M6 | not started | docs/cli.md | No board skills list or per-host skill state; agent delegate CLI (#133) exists, but no Prototype command or board skill |
| SKL-40 | Skill copies, agent switches and team access | later | M6 | not started | none | No skill pages, agent switches or team access |
| SKL-41 | Skill commands in the CLI for any host | later | M6 | not started | none | No skill CLI commands |
| SKL-42 | Marketplace actions in the CLI | later | M6 | in review | crates/cli/src/ext.rs | Reserved `massorca ext` group still returns extension.not_implemented on main; open #143 adds extension inspect, install and remove (apps only); services, programs, accounts and access not built |
| SKL-43 | Marketplace audit | later | M6 | not started | none | No Marketplace audit |
| SKL-44 | Marketplace on the phone | later | M6 | not started | none | No phone client on main |
| CLI-01 | Put orca on PATH and inside app terminals | first version | M1 | partial | MASSORCA_BIN env; crates/core-agents/src/launch.rs | Agent runs get the bundled binary path (hooks use it); no PATH install of a stable command per OS. |
| CLI-02 | Use the CLI from WSL | later | M6 | not started | none | No WSL bridge to the Windows CLI. |
| CLI-03 | Reach the running app: local connection, status and open | first version | M1 | partial | `service status` / `service info`, autostart; crates/cli/src/connect.rs | Commands reach the local service and auto-start it; no `open` command that launches the desktop app. |
| CLI-04 | Run a headless server (serve) | first version | M2 | done | `service run --listen`, `service unit`, `pair offer`; crates/cli/src/service/foreground.rs; crates/protocol/src/pairing.rs | Headless service with listeners (PR 69/70, merged); pairing offers carry server addresses. No phone client code in crates; pairing is generic. Was in review. |
| CLI-05 | Route commands to a paired server | first version | M2 | partial | global `--host` (crates/cli/src/args.rs); `pair accept` (crates/cli/src/pair.rs); docs/decisions/0046-client-hosts.md | Named --host by 0046, not --environment. Missing: WSL distro target for Marketplace commands (CLI-31 not started). PR 70 merged. |
| CLI-06 | Manage managed servers on SSH hosts | later | M6 | partial | `host add-ssh`, `host remove --uninstall --ssh`; crates/cli/src/host/add_ssh.rs; crates/cli/src/host/records.rs | Installs, upgrades and uninstalls over SSH (PR 68/70). No rollback or recovery in crates/cli/src/host or crates/core-remote/src. Desktop path not checked. |
| CLI-07 | Target SSH hosts through the relay | later | M6 | not started | none | No relay of commands to SSH targets on main or in open PRs. |
| CLI-08 | Output, help, errors and agent discovery | first version | M1 | partial | `--json` envelope, exit codes; crates/cli/src/output.rs | JSON envelope, error codes and help exist; command descriptors are internal (crates/server/src/commands/mod.rs), no machine-readable schema exposed. |
| CLI-09 | Worktrees and file tabs | first version | M1 | partial | `workspace create/list/show/rm`, `agent list/forget`; crates/cli/src/workspace.rs | No `current`, no file or diff tabs, no browser-pane open. |
| CLI-10 | Repositories and projects | first version | M1 | partial | `project add/list/show/rm`; crates/cli/src/project.rs | Base ref only per create (`--base`); no per-repo base ref setting, no clone, no remote-host folder. |
| CLI-11 | Terminals | first version | M1 | partial | `terminal create/list/show/send/screen/attach/kill/close`; crates/cli/src/terminal.rs | No wait-on-terminal command. |
| CLI-12 | Orchestration: runs and tasks | first version | M3 | not started | none | No run or orchestration methods on main. Bot Teams task.* (0072) is a Board, not coordinator Runs. request_id retry exists on workspace.create, terminal.create, agent.start, agent.delegate. |
| CLI-13 | Orchestration: messages, questions and gates | later | M6 | not started | none | No orchestration messages, questions or gates. |
| CLI-14 | Orchestration: supervised workers and dispatch | first version | M3 | partial | `agent start/status --watch/stop/forget/prompt/delegate/tree/bring-back`; crates/cli/src/agent.rs; crates/core-agents/src/sessions/delegation.rs | Delegation, tree and bring-back built (PR 133). Bot Teams task dispatch built (`task start`, 0072 amendment). Missing: chat delegation (desktop never calls agent.delegate). |
| CLI-15 | Browser tabs | first version | M4 | not started | none | No browser tab commands. |
| CLI-16 | Browser page control | later | M6 | not started | none | No browser page control. |
| CLI-17 | Browser profiles and identity | later | M6 | not started | none | No browser profiles. |
| CLI-18 | Browser cookies, storage, capture and device settings | later | M6 | not started | none | No browser cookies, storage or capture. |
| CLI-19 | Desktop computer use | later | M6 | not started | none | No computer-use commands. |
| CLI-20 | Mobile emulators and simulators | later | M6 | not started | none | No emulator or simulator commands. |
| CLI-21 | Linear issues | later | M6 | not started | none | No Linear commands (0001 lists trackers only as a possible extension kind). |
| CLI-22 | Artifacts | later | M6 | not started | none | No artifact sharing. |
| CLI-23 | Automations | first version | M3 | not started | none | No automations. |
| CLI-24 | Agent accounts | first version | M3 | partial | agent.credentials.import/show/remove; crates/protocol/src/methods/agents.rs; crates/cli/src/agent_credentials.rs | Import, show and remove built (PR 121). Sign-in and several accounts per kind are in open PR 144 only. Usage, Codex reset credit, OpenCode and Devin: in no open PR. |
| CLI-25 | Agent status hooks | first version | M3 | changed | docs/decisions/0053-agent-status-sources.md | Hooks are installed per run, never into global agent configs; built in crates/core-agents/src/launch.rs. No enable/disable command needed. |
| CLI-26 | Claude Code agent teams bridge | later | M6 | not started | none | No Claude Code agent-teams tmux bridge. |
| CLI-27 | Agent session search | later | M6 | not started | none; sessions.search setting is registered (crates/server/src/features/config_keys.rs) with no reader found | No session search or indexing method on main. |
| CLI-28 | Skills commands | later | M6 | not started | none | No skills commands. |
| CLI-29 | Per-workspace environment recipe check | later | M6 | not started | none; ext-package parses recipe contributions (crates/ext-package/src/manifest/contributes.rs) with no CLI | No environment recipe check command on main. |
| CLI-30 | Recovery and diagnostics | later | M6 | partial | pre-migration database backups: crates/core-store/src/backup.rs (KEPT_BACKUPS = 5) | Backups are taken before each migration and pruned to five. No rollback or restore command and no memory snapshot. Was not started. |
| CLI-31 | Marketplace commands | later | M6 | not started | none on main; ext commands (install, list, show, remove, catalogue) are in open PR 143 | Connect and disconnect apps, app access, page audit and GitHub account switching are in no PR. ext is reserved on main (crates/cli/src/ext.rs). |
| CLI-32 | Bot Teams commands | first version | M3 | partial | `team` and `task` commands; crates/cli/src/team/mod.rs; crates/cli/src/task/mod.rs; docs/decisions/0072-bot-teams-core.md | Teams, bots, projects, Board, task CRUD and team memory built (PR 140 for the first four). `task start` built. Missing: rooms and messages, routines, webhooks, team files, the Engram memory link. Desktop Teams still on sample data. |
| CLI-33 | Prototypes commands | later | M6 | not started | none | No Prototypes commands. |
| CLI-34 | Jev commands | later | M6 | not started | none | No Jev commands. |
| CLI-35 | Folder workspace commands | first version | M1 | partial | `project add --folder`, `workspace create/list/rm`; crates/cli/src/workspace.rs | Folder workspaces can be added, listed and forgotten; no update or check command. |
| CLI-36 | Theme commands | first version | M4 | done | `settings set appearance.theme` (get, set, reset); crates/server/src/features/config_keys.rs; crates/cli/tests/config_commands.rs | Accepts system, dark or light through the generic settings command; no dedicated theme command. Was not started. |
| CLI-37 | Chat results | later | M6 | not started | none | No chat results command. |
| CLD-01 | Phone-to-host relay splice | later | M6 | not started | none | No relay splice. Owner default: relay not in first version (docs/inventory/first-version.md). |
| CLD-02 | Pairing invites and resumable device credentials | later | M6 | changed | docs/decisions/0041-device-identity-and-pairing.md; crates/server/src/network/pairing.rs | Replacement built: one-time expiring offer plus per-device client certificate (0041). Rotating 30-day credentials are not built. |
| CLD-03 | Host identity proof | later | M6 | not started | none | No hosted service; the host key proof exists only toward paired clients (crates/net/src/pin.rs, on main). |
| CLD-04 | Cloud sign-in and relay tokens | drop | M6 | not started | none | Proposed drop, but no decision record drops it. |
| CLD-05 | Director and cell assignment | drop | M6 | not started | none | Proposed drop, but no decision record drops it. |
| CLD-06 | Regions and region correction | drop | M6 | not started | none | Proposed drop, but no decision record drops it. |
| CLD-07 | Admission control and overload shedding | later | M6 | not started | none | No relay admission control. The listener caps (SRV-10) are a local analogue with no retry hint. |
| CLD-08 | Drain, evacuation and host migration | drop | M6 | not started | none | Proposed drop, but no decision record drops it. |
| CLD-09 | Operator API and access tiers | drop | M6 | not started | none | Proposed drop, but no decision record drops it. |
| CLD-10 | Phone push token registration | later | M6 | not started | none | No push token registration. |
| CLD-11 | Push delivery and live dismissal | later | M6 | not started | none | No push delivery. |
| CLD-12 | Push gateway deployment and recovery | drop | M6 | not started | none | Proposed drop, but no decision record drops it. |
| CLD-13 | Relay operations console and incident monitor | drop | M6 | not started | none | Proposed drop, but no decision record drops it. |
| CLD-14 | Mutation lease and fence broker | drop | M6 | not started | none | Proposed drop, but no decision record drops it. |
| CLD-15 | Infrastructure as code and gated workflows | drop | M6 | not started | none | Proposed drop, but no decision record drops it. |
| CLD-16 | Relay and push storage and aggregate observability | later | M6 | not started | none | No relay or push storage or observability endpoints. |
| CLD-17 | Feedback and crash reports as GitHub issues | later | M6 | not started | none | Desktop settings has a "Send feedback" row marked planned (crates/desktop/src/model/settings_sections.rs); no action. |
| CLD-18 | Engram memory refresh for Bot Teams | later | M6 | not started | none | No Engram integration in crates. |
| PLAT-01 | Installable builds per OS | first version | M4 | not started | - | No installer or packaging config; only .github/workflows/ci.yml |
| PLAT-02 | Homebrew cask and Linux package installs | later | M6 | not started | - | No Homebrew cask, deb or rpm, no PATH install |
| PLAT-03 | Automatic update checks and install | later | M6 | not started | - | No update checks or installer |
| PLAT-04 | Per-OS update installation rules | later | M6 | not started | - | No update installation |
| PLAT-05 | Dev channels, exact-build jumps and local builds | drop | M6 | not started | - | Nothing built; inventory proposes drop, not confirmed by a decision record |
| PLAT-06 | Update policy controls | drop | M6 | not started | - | Nothing built; inventory proposes drop, not confirmed by a decision record |
| PLAT-07 | Single instance and profile lock | first version | M1 | partial | massorca service run (exit 3 on held lock), crates/server/src/instance_lock.rs | One service per profile is done (0003); the desktop has no single-instance handling, so a second launch opens a second window instead of focusing the first |
| PLAT-08 | Startup preflight and tool detection | first version | M4 | partial | agent kinds (agent.kinds installed flag), crates/core-agents/src/resolve.rs | Agent CLI and git version detection; agent kind detection reported since #131; no preflight status, no forge CLI or shell checks; Setup checklist (model/setup.rs) has no tool checks |
| PLAT-09 | Profile storage compatibility guard | drop | M6 | partial | newer-schema refusal, crates/core-store/src/migrations.rs; pre-migration backup once per file (#122), crates/core-store/src/backup.rs | Refusal predates the 89bcad1 baseline and was missed there; backups kept (5); no rollback steps shown; inventory proposes drop, no decision on main |
| PLAT-10 | Main window lifecycle and close confirmation | first version | M4 | partial | Title bar window controls, crates/desktop/src/shell/title_bar.rs | Own title bar with minimise, maximise, close per owner direction (title bar kept); no close confirmation; service runs detached so it survives the window |
| PLAT-11 | Renderer recovery and GPU fallback | drop | M6 | not started | - | No renderer recovery or GPU fallback (proposed drop, not decided) |
| PLAT-12 | Tray and Dock attention indicators | later | M6 | not started | - | No tray or Dock badge |
| PLAT-13 | Application menu | later | M6 | not started | - | No native application menu |
| PLAT-14 | Safe quit and durable state writes | first version | M1 | partial | service.shutdown (drains, closes store), crates/server/src/features/service.rs | Service stop is graceful (0003) and state is in SQLite; fsync on vault key, server identity and agent credential writes; desktop quit has no bounded teardown |
| PLAT-15 | OS file opening and orca:// links | later | M6 | not started | - | No OS file associations or URL scheme |
| PLAT-16 | Crash dumps and Report Crash | later | M6 | not started | - | No crash dumps or Report Crash |
| PLAT-17 | Opt-in telemetry | drop | M6 | not started | - | Telemetry is a planned Settings row only; design keeps an opt-in choice (D-OB-10, docs/design/screens/shell-setup-hosts.md), so not dropped |
| PLAT-18 | Local trace log and redaction | first version | M1 | partial | massorca service run daily logs under MASSORCA_LOG, crates/cli/src/logging.rs | Service log daily, 7 kept, local only (0006); secrets kept out of logs by convention and one test (#132); desktop writes no log; frame-dump guard in crates/net/src/log_guard.rs |
| PLAT-19 | Diagnostic bundle upload | later | M6 | not started | - | No diagnostic bundle |
| PLAT-20 | Main-thread hang watchdog | drop | M6 | not started | - | No hang watchdog (proposed drop, not decided) |
| PLAT-21 | Star prompt | drop | M6 | not started | - | No star prompt (proposed drop, not decided) |
| PLAT-22 | Keep computer awake while agents run | later | M6 | not started | - | No sleep prevention; Settings row is planned; the agents.keep_awake key (crates/server/src/features/config_keys.rs) is read by nothing |
| PLAT-23 | macOS platform integration | later | M6 | not started | - | Only the macOS traffic-light position is set in crates/desktop/src/main.rs; no key repeat, privacy guidance or window workaround |
| PLAT-24 | Windows shell, process and tree handling | first version | M4 | partial | terminal create (pwsh then powershell, Job object tree kill), crates/core-terminal/src/shell.rs | PowerShell detection and process-tree termination built (0031, 0032); Git Bash detection and the Windows PATH registry merge are not (0032 defers PATH merge) |
| PLAT-25 | Linux platform integration | later | M6 | partial | vault master key: Secret Service or owner-only file, crates/core-vault/src/key/mod.rs (0071, #132) | Store is picked automatically, not by the user; no virtual display for browser panes (browser panes not built) |
| PLAT-26 | Headless serve startup and supervisor handoff | later | M6 | partial | --listen in crates/server/src/network/listen.rs; systemd unit in crates/cli/src/service/unit.rs; docs/decisions/0045-headless-service.md | #69 and #70 merged, so no open PR; missing: service updates handed to a supervisor (upgrades are a manual restart, 0045) |
| PLAT-27 | CLI on PATH and launch redirect | first version | M1 | partial | massorca binary is CLI and service (service run), crates/cli/src/service/mod.rs | One binary for CLI and service (0003); desktop is a separate massorca-desktop binary (0023); nothing puts the command on PATH |
| PLAT-28 | Main-thread responsiveness breadcrumbs | later | M6 | not started | - | No event-loop lag or unresponsive-window breadcrumbs |
| PLAT-29 | Visible product name MassOrca | first version | M4 | done | Window title MassOrca, crates/desktop/src/main.rs | Visible name is MassOrca in window and docs; note the command is massorca, not orca (0003 renames it) |
| PLAT-30 | Desktop shell: activity rail, side panel and session bar | first version | M4 | partial | Activity rail (Tasks, Calendar, Settings, theme, usage rings), crates/desktop/src/model/rail.rs | Rail and title bar built; Tasks and Calendar show a not-built placeholder; usage rings on sample data (crates/desktop/src/data.rs); Work panel terminals merged (#110); no browser tabs |
| SET-01 | Settings read, change and push | first version | M1 | partial | `config.set`, `config.reset`, `config.changed` (crates/server/src/features/config.rs; crates/protocol/src/methods/events.rs); CLI `config set` and `config reset` (crates/cli/src/args.rs) | Typed settings, reset and change events are on main. The desktop uses only workspaces.default_root (crates/desktop/src/model/settings_sections.rs) and has no config.changed subscriber. |
| SET-02 | Persistent state across restarts | first version | M1 | partial | SQLite store, crates/core-store/src/migrations.rs | Projects, workspaces, settings and agent sessions persist; desktop saves no UI state (side panel, rail mode, layout) |
| SET-03 | Schema versions and rolling backups | first version | M1 | done | automatic on store open, crates/core-store/src/migrations.rs and crates/core-store/src/backup.rs | user_version migrations, newer schema refused (store.newer_schema), VACUUM INTO backups, five kept |
| SET-04 | Sealed secrets on the device | first version | M1 | partial | docs/decisions/0071-secret-vault.md; crates/core-vault/src/key/mod.rs (DPAPI, Secret Service, file); crates/core-vault/src/cipher.rs; `secret.set` (crates/server/src/commands/secret_table.rs) | API keys are sealed in the vault. Agent account tokens stay plain files in the credential homes (0056), so "account tokens stay encrypted" is not met. |
| SET-05 | Profiles: list, create, switch | later | M6 | partial | --profile / MASSORCA_PROFILE, crates/profile/src/profile.rs | Named profiles with isolated state exist (0004); no list, create or switch command or UI |
| SET-06 | Cloud account per profile | later | M6 | not started | none | No cloud account per profile |
| SET-07 | Move projects and sessions between profiles | later | M6 | not started | none | No moving projects between profiles |
| SET-08 | Import the Electron app's profile state | first version | M3 | not started | none | No Electron profile import. The 0056 credential import is served but does not read Electron app state. |
| SET-09 | Rollback to JSON and JSON exports | later | M6 | not started | none | No JSON export or rollback |
| SET-10 | Active view remembered | later | M6 | not started | none | Desktop persists no state, so the active view is not remembered |
| SET-11 | Default keyboard shortcuts | first version | M4 | partial | crates/desktop/src/model/palette.rs and crates/desktop/src/shell/actions.rs | Command palette and 12 key bindings in actions.rs; no 93-action catalogue |
| SET-12 | Custom shortcuts per OS | later | M6 | changed | docs/decisions/0070-settings-the-desktop-needs.md; crates/server/src/features/config_keys.rs (input.keybindings); crates/server/src/setting_values.rs (refuses two actions on one shortcut) | 0070 replaces the JSON file with one validated setting, built on the service. The desktop ignores it: its 12 bindings are hard-coded in crates/desktop/src/shell/actions.rs; no Settings UI. |
| SET-13 | Interface language | later | M6 | not started | none | No interface language choice |
| SET-14 | File explorer listing | later | M6 | not started | none | No file listing method; ui FileTree component exists in the gallery only |
| SET-15 | Create, rename, copy and delete | later | M6 | not started | none | No file create, rename, copy or delete |
| SET-16 | Built-in editor: open and save | later | M6 | not started | none | No built-in editor. editor.tab_width is declared (crates/server/src/features/config_keys.rs) but nothing edits text. |
| SET-17 | File watching and live refresh | later | M6 | not started | none | No file watching for explorer or editor (core-watch serves git notifications) |
| SET-18 | Quick open by file name | later | M6 | not started | none | Palette Go to file returns an empty list (crates/desktop/src/model/shell_state.rs) |
| SET-19 | Text search in a workspace | later | M6 | not started | none | No workspace text search |
| SET-20 | Dropped files as attachments and uploads | later | M6 | not started | none | No attachments or uploads |
| SET-21 | Copy external files into a workspace | later | M6 | not started | none | No copy of external files into a workspace |
| SET-22 | Download remote files and folders | later | M6 | not started | none | No remote downloads |
| SET-23 | Open in external editor or file manager | later | M6 | not started | none | No open in external editor or reveal |
| SET-24 | Image, PDF and Markdown previews | later | M6 | not started | none | No image, PDF or Markdown previews |
| SET-25 | Notebooks with Python kernels | later | M6 | not started | none | No notebooks |
| SET-26 | Export a document to PDF | later | M6 | not started | none | No PDF export |
| SET-27 | App icon choice and Dock icon | drop | M6 | not started | none | Proposed drop in docs/inventory/README.md, no decision record on main; nothing built |
| SET-28 | Import terminal themes from Ghostty and Warp | later | M6 | not started | none | No Ghostty or Warp theme import |
| SET-29 | Secret vault with scopes, links and in-chat requests | first version | M4 | partial | docs/decisions/0071-secret-vault.md; crates/core-vault/src/names.rs (global, project and team scopes); crates/core-vault/src/vault/links.rs and requests.rs; CLI `secret` (crates/cli/src/secret.rs) | Scopes, links and requests exist and are reachable from the CLI. No value reaches an agent run (environment_for has no caller outside tests). The desktop makes no secret calls. |
| SET-30 | App light and dark themes | first version | M4 | partial | `appearance.theme` declared (crates/server/src/features/config_keys.rs); CLI `config set` (crates/cli/src/args.rs); light/dark toggle in crates/desktop/src/shell/mod.rs | Toggle works in memory and is not persisted. The desktop does not read appearance.theme, so the stored setting does not change the app yet. |
| CUS-01 | orca.yaml setup and archive scripts with per-script trust | first version | M1 | not started | docs/decisions/0009-phase1-deferrals.md | No setup or archive scripts or per-script trust; setup scripts deferred to a later phase by 0008 and 0009 |
| CUS-02 | orca.yaml extended keys | later | M6 | not started | docs/decisions/0009-phase1-deferrals.md | No orca.yaml keys; extensions.md plans recipes as a `recipes` contribution kind |
| CUS-03 | DeepSeek Harness agent integration | later | M6 | changed | docs/architecture/extensions.md; docs/decisions/0059-agents-come-from-the-marketplace.md | DeepSeek becomes a first-party `agentAdapters` extension at M6 (section 10 and 12); not built; 0059 makes agents Marketplace packages and no DeepSeek package is on main |
| CUS-04 | Per-agent command, argument and environment overrides | first version | M3 | partial | agent.kinds; crates/core-agents/src/catalog.rs; crates/server/src/features/config_keys.rs | Profile kinds with program and args; config key agents.default_kind (old name agents.default still read and mapped); Settings lists installed agents from sample data; no per-agent env, disable, binary override or Settings controls |
| CUS-05 | Declarative agent-state rules with live update and local override | later | M6 | not started | docs/decisions/0050-agent-kinds.md; crates/ext-package/src/manifest/contributes.rs | Status comes from closed Rust mechanisms named in kind JSON; declarative rules, live update and local override not built; ext-package agentRules are instruction documents, not status rules |
| CUS-06 | Plugin-contributed themes and icon themes | later | M6 | changed | docs/architecture/extensions.md | Becomes `themes` and `iconThemes` extension data contributions; theme registry M4, extensions M6; not built |
| CUS-07 | Saved terminal quick commands | later | M6 | not started | crates/desktop/src/model/settings_sections.rs | Only a disabled "Quick commands" Manage row; 0070 says the design gives no values yet; planned as user-owned commands contributions |
| CUS-08 | Stable internal extension points in the core | first version | M1 | partial | crates/server/src/registry.rs | Registry with owner and drop handle, Owner, Principal, event bus, command descriptor tables and agent kind registry exist; tool, skill, view, theme and status-rule points not built |
| PROTO-01 | Prototype requests from a chat | first version | M4 | not started | docs/architecture/extensions.md | No prototype requests; prototypes stay core pending owner confirmation (0001, extensions.md open item 1) |
| PROTO-02 | Request folder and capture contract | first version | M4 | not started | none | No visuals folder or capture contract |
| PROTO-03 | Cross-chat list and owner-host reads | later | M6 | not started | none | No cross-chat listing |
| PROTO-04 | Prototypes list and set view | first version | M4 | not started | crates/desktop/src/model/sidebar.rs | Only a Prototypes sidebar row with a count (0 from the service view); no list or set page |
| PROTO-05 | Viewer with viewports, browse and annotate | first version | M4 | not started | none | No viewer |
| PROTO-06 | Element edits, notes and note screenshots | later | M6 | not started | none | No element edits or notes |
| PROTO-07 | Iterate and send to the originating chat | later | M6 | not started | none | No send-to-chat; sessions not wired in the desktop yet |
| PROTO-08 | Main direction, stars and merge | later | M6 | not started | none | No main direction, stars or merge |
| PROTO-09 | Versions, archive and delete | later | M6 | not started | none | No versions, archive or delete |
| PROTO-10 | Project design system | later | M6 | not started | none | No project design system |
| PROTO-11 | Design system in prototype requests | later | M6 | not started | none | No design system choice in requests |
| PROTO-12 | Prototypes from child sessions, with attribution | later | M6 | not started | none | No child-session prototypes |
| PROTO-13 | orca prototypes CLI and baseline check | later | M6 | not started | none | No orca prototypes CLI |
| PROTO-14 | Design system shared between projects by link | later | M6 | not started | none | No shared design system |
| CUS-09 | Custom Marketplace items | later | M6 | not started | none | No custom Marketplace items |
| CUS-10 | Marked instruction sections in agent instruction files | later | M6 | not started | none | No instruction-file section writer |
| CUS-11 | MCP server entries written into agent configs | later | M6 | not started | docs/architecture/extensions.md; crates/ext-package/src/manifest/agent.rs | No MCP config writer; ext-package validates and risk-rates mcpServers entries (#135) but nothing writes them into agent configs |
