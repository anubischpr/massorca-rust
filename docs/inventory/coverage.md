# Inventory coverage

Status: measured 2026-11-22 on commit `89bcad1` (`origin/main`), against the 542 features of the [functional inventory](README.md). Generated content: regenerate it rather than editing rows by hand (see Method).

## Headline

| | Features | Done % | Done or in review % | Weighted % | Done incl. built replacements % |
|---|---|---|---|---|---|
| Whole inventory | 542 | 4.8% | 10.5% | 14.6% | 6.9% |
| [First version](first-version.md) | 190 | 11.4% | 22.9% | 35.4% | 15.8% |

Percentages exclude features marked `changed` or `dropped` from the denominator. **Weighted %** counts `done` as 1 and `partial` as 0.5. **Done or in review %** adds features held only in an open pull request. **Done incl. built replacements %** also counts as done the `changed` features whose replacement design is built on `main` (11 overall, 9 in the first version; detected from the row note, which says "built" and not "not built" or "in review").

Reading these numbers: the reviewers were strict. A feature whose main path works but misses one rule (an icon, a setup script, a phone surface) is `partial`, not `done`; read the note. Most of Phase 3 (server, pairing, remote hosts) is in open pull requests, and most of the Phase 4 agent methods (prompt, resume, delegation, bring-back, credentials) are declared in the protocol but not served on `main`; their code sits on local branches without an open pull request, so it counts as not started.

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
| Whole inventory | 542 | 24 | 97 | 28 | 348 | 36 | 9 | 4.8% | 10.5% | 14.6% |
| First version | 190 | 20 | 84 | 20 | 51 | 13 | 2 | 11.4% | 22.9% | 35.4% |

### Per inventory area

| Scope | Features | Done | Partial | In review | Not started | Changed | Dropped | Done % | Done or in review % | Weighted % |
|---|---|---|---|---|---|---|---|---|---|---|
| [Workspaces and worktrees](workspaces.md) (WS) | 45 | 0 | 20 | 0 | 24 | 1 | 0 | 0.0% | 0.0% | 22.7% |
| [Terminals](terminals.md) (TERM) | 28 | 6 | 9 | 2 | 7 | 4 | 0 | 25.0% | 33.3% | 43.8% |
| [Git and source control](git.md) (GIT) | 31 | 10 | 8 | 0 | 10 | 3 | 0 | 35.7% | 35.7% | 50.0% |
| [Hosting providers and issue trackers](providers-and-trackers.md) (SCM) | 32 | 0 | 0 | 0 | 31 | 1 | 0 | 0.0% | 0.0% | 0.0% |
| [Agent integrations](agents.md) (AG) | 36 | 0 | 6 | 0 | 27 | 3 | 0 | 0.0% | 0.0% | 9.1% |
| [Agent status and hooks](agent-status.md) (STAT) | 26 | 3 | 4 | 0 | 12 | 2 | 5 | 15.8% | 15.8% | 26.3% |
| [Accounts, credentials and usage](accounts-and-usage.md) (ACC) | 26 | 0 | 8 | 0 | 17 | 1 | 0 | 0.0% | 0.0% | 16.0% |
| [Session history](session-history.md) (HIST) | 18 | 0 | 3 | 0 | 15 | 0 | 0 | 0.0% | 0.0% | 8.3% |
| [SSH and remote hosts](remote-hosts.md) (REM) | 27 | 0 | 0 | 12 | 8 | 5 | 2 | 0.0% | 60.0% | 0.0% |
| [Headless server and pairing](server-and-pairing.md) (SRV) | 22 | 3 | 1 | 10 | 4 | 3 | 1 | 16.7% | 72.2% | 19.4% |
| [Automations, bot teams and orchestration](automations.md) (AUTO) | 37 | 0 | 12 | 0 | 24 | 0 | 1 | 0.0% | 0.0% | 16.7% |
| [Browser pane and devices](browser-and-devices.md) (BRW) | 30 | 0 | 2 | 0 | 28 | 0 | 0 | 0.0% | 0.0% | 3.3% |
| [Skills, plugins and marketplace](skills-and-plugins.md) (SKL) | 44 | 0 | 0 | 0 | 36 | 8 | 0 | 0.0% | 0.0% | 0.0% |
| [Command-line interface](cli.md) (CLI) | 37 | 0 | 9 | 3 | 24 | 1 | 0 | 0.0% | 8.3% | 12.5% |
| [Cloud services](cloud.md) (CLD) | 18 | 0 | 0 | 0 | 17 | 1 | 0 | 0.0% | 0.0% | 0.0% |
| [Desktop app platform](app-platform.md) (PLAT) | 30 | 1 | 8 | 1 | 20 | 0 | 0 | 3.3% | 6.7% | 16.7% |
| [Settings, persistence and files](settings-and-files.md) (SET) | 30 | 1 | 5 | 0 | 23 | 1 | 0 | 3.4% | 3.4% | 12.1% |
| [Customisation and extensibility](customisation.md) (CUS) | 11 | 0 | 2 | 0 | 7 | 2 | 0 | 0.0% | 0.0% | 11.1% |
| [Prototypes and design system](prototypes.md) (PROTO) | 14 | 0 | 0 | 0 | 14 | 0 | 0 | 0.0% | 0.0% | 0.0% |

### Per milestone

Milestones as defined in `docs/WORKING-RULES.md`. M1 to M5 hold first-version features only; everything outside the first version is M6.

| Scope | Features | Done | Partial | In review | Not started | Changed | Dropped | Done % | Done or in review % | Weighted % |
|---|---|---|---|---|---|---|---|---|---|---|
| M1 Core and CLI: workspaces, terminals, git | 66 | 14 | 43 | 2 | 3 | 4 | 0 | 22.6% | 25.8% | 57.3% |
| M2 Headless server driven by the CLI | 25 | 3 | 1 | 18 | 0 | 3 | 0 | 13.6% | 95.5% | 15.9% |
| M3 Agents, status, delegation, credentials | 74 | 2 | 32 | 0 | 32 | 6 | 2 | 3.0% | 3.0% | 27.3% |
| M4 Desktop app, first-version scope | 25 | 1 | 8 | 0 | 16 | 0 | 0 | 4.0% | 4.0% | 20.0% |
| M5 Android app | 0 | - | - | - | - | - | - | - | - | - |
| M6 Extensions and the rest of the inventory | 352 | 4 | 13 | 8 | 297 | 23 | 7 | 1.2% | 3.7% | 3.3% |

## Method

1. Worktree at the measured commit; the open pull requests listed by `gh pr list --state open` at the same time.
2. Eight read-only reviewers, one per group of inventory areas, read the code (`crates/`), the CLI reference (`docs/cli.md`), the decision records (`docs/decisions/`) and the open pull request diffs, and gave each feature one status with evidence. Evidence or it is not done: a name alone was not enough, the reviewer had to open the implementing code; a screen drawn on sample data was not counted as done.
3. A script (`docs/inventory/coverage_assemble.py`) joined the rows with the inventory's README table and `first-version.md`, checked that every one of the 542 IDs appears exactly once, and computed the totals.
4. Milestone mapping (judgement, applied by prefix with listed exceptions): first-version features of WS, TERM and GIT, the core store (SET-01 to SET-04), `orca.yaml` scripts (CUS-01), core extension points (CUS-08), the local CLI groups (CLI-01, 03, 08, 09, 10, 11, 35) and core service hygiene (PLAT-07, PLAT-14, PLAT-18, PLAT-27) are M1; SRV, REM, CLD and CLI-04, CLI-05 are M2; AG, STAT, ACC, HIST, AUTO, profile import (SET-08), per-agent overrides (CUS-04) and the agent, automation and Bot Teams CLI groups (CLI-12, 14, 23, 24, 25, 32) are M3; every other first-version feature is M4. The inventory describes core behaviour, so no feature row is Android-only and M5 has no rows: Android reaches the M1 to M4 features through the same service.
5. To regenerate: repeat step 2 on a new commit and run the assembly script on its rows; keep this method section in step with any change.

Limits: nothing was run; statuses come from reading code. The milestone mapping and the `partial` judgement involve reading the feature's intent, so two reviewers may differ on borderline rows. Work on local branches without an open pull request counts as not started.

## Features

| ID | Feature | Proposed | Milestone | Status | Evidence | Note |
|---|---|---|---|---|---|---|
| WS-01 | Add an existing git repository as a project | first version | M1 | partial | `massorca project add` / `project.add`, crates/core-workspace/src/projects.rs | Git repo registered at its toplevel, duplicate path and name refused (project.exists), name settable; desktop Add a project dialog calls the service. No project icon. |
| WS-02 | Add a folder as a plain-folder project | first version | M1 | partial | `project add --folder` / `project.add` kind folder, crates/core-workspace/src/projects.rs | Folder projects register (desktop auto-detects kind). No No Git marker on desktop, no warning before adding a non-git folder, no phone. |
| WS-03 | Create, clone or initialise a project | later | M6 | not started | none | No create-repo, clone or git init on main or in an open PR. |
| WS-04 | Per-project base branch and workspace location | first version | M1 | partial | `project add --workspaces-root` / `project.add` workspaces_root, crates/protocol/src/methods/projects.rs | Per-project workspace folder exists. No per-project base branch setting; base is per create (`--base`, default HEAD). |
| WS-05 | Create a git worktree workspace | first version | M1 | partial | `workspace create` / `workspace.create`, crates/core-workspace/src/create.rs | Worktree on a new branch from a base, registered, idempotent request id; desktop New workspace calls it. No setup result (setup scripts deferred, ADR 0008). |
| WS-06 | Workspace and branch naming | first version | M1 | partial | name rules in crates/core-workspace/src/names.rs | Names are validated, not sanitised; no branch prefix, generated names, collision suffixes or accent folding (deferred, decisions 0008/0009). |
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
| WS-21 | Workspace lineage (parent and child) | first version | M1 | partial | `agent.tree` constant + agent_sessions.parent_id, crates/protocol/src/methods/agents.rs | Session parent column and protocol types exist (decision 0055), but no server handler, never set, no workspace lineage or manual link. |
| WS-22 | Links from workspaces to issues, PRs and MRs | later | M6 | not started | none | No issue/PR/MR links; providers are a later phase (decision 0010). |
| WS-23 | Base-branch drift and local base refresh | later | M6 | not started | none | No base drift display or local base fast-forward. |
| WS-24 | Worktrees created outside MassOrca: visibility and import | later | M6 | partial | `workspace list` unregistered entries, crates/core-workspace/src/unregistered.rs | Worktrees git registers under a project's workspaces root are reported as unregistered. No import, no show or hide per project, source or host. |
| WS-25 | Disk usage and stale-workspace cleanup | later | M6 | not started | none | No disk usage or stale-workspace cleanup. |
| WS-26 | Prepared checkouts for faster creates | later | M6 | not started | none | No prepared checkouts. |
| WS-27 | Ephemeral VM workspaces from environment recipes | later | M6 | not started | none | No VM-backed workspaces. |
| WS-28 | Windows runtime per project (Windows host or WSL) | later | M6 | changed | docs/decisions/0015-git-on-remote-execution-hosts.md | WSL is served by a service running inside the distribution, not per-project routing. Replacement depends on remote transport, in review (PRs 66-70). |
| WS-29 | Workspace records: persistence and live catalog | first version | M1 | partial | `workspace.list` + workspace.created/removed events, crates/server/src/features/workspaces.rs | Persisted in the store, live across clients by events (desktop store), missing dirs reported, git as truth. No manual row names or session references. |
| WS-30 | Profiles: switcher and moving projects between profiles | drop | M6 | partial | `--profile` / MASSORCA_PROFILE, crates/cli/src/args.rs | Profiles exist per CLI option (decision 0004). No switcher and no moving a project between profiles. Inventory ranks it drop, but no decision record. |
| WS-31 | No Git warning when adding a folder | first version | M1 | not started | crates/desktop/src/model/project_add.rs | Desktop sends project.add with no kind; a non-git folder is registered silently, no warning or Initialize Git choice. |
| WS-32 | No Git marker and Initialize Git on folder projects | later | M6 | not started | none | No No Git pill or Initialize Git action (only Needs Git reasons on disabled git menu rows). |
| WS-33 | One-line worktree sidebar (default layout) | first version | M1 | partial | sidebar tree, crates/desktop/src/model/sidebar.rs | Project, host and workspace rows come from the service; session rows only from sample data (service_view leaves sessions empty). No Show N more folding. |
| WS-34 | Session row actions and hover card | later | M6 | not started | none | No session row actions or hover card; session rows are sample data. |
| WS-35 | Child sessions listed under their parent across worktrees | first version | M1 | partial | child session depth, crates/desktop/src/model/sidebar.rs | Children indent under their parent only on sample data; no real sessions or cross-worktree listing. |
| WS-36 | Checklist card in agent chats | later | M6 | not started | none | No checklist card in agent chats. |
| WS-37 | Checklist tab in the floating workspace | later | M6 | not started | none | No Checklist tab in the floating workspace. |
| WS-38 | Open an existing session in the floating workspace | later | M6 | not started | none | No floating workspace session tabs. |
| WS-39 | Host selector in the floating workspace | later | M6 | not started | none | No host selector in a floating workspace; host rows in the sidebar are local or sample only. |
| WS-40 | Start a session in an existing branch or worktree from the create page | first version | M1 | partial | start page from service records, crates/desktop/src/model/start_page.rs | Page shows host, project, branch from real records, but sessions are off (Agent sessions come later). PR 110 enables Open terminal; chat start still missing. |
| WS-41 | Prototypes in the Workspaces side panel | first version | M1 | partial | Prototypes nav row, crates/desktop/src/model/sidebar.rs | Sidebar has a Prototypes entry, but the service count is always 0 and no prototypes are listed. |
| WS-42 | Show bot teams filter in Workspaces | later | M6 | not started | none | No Show bot teams filter in Workspaces. |
| WS-43 | CLI: list and close a worktree's agent sessions | later | M6 | partial | `agent list --workspace` and `agent stop` / `agent forget`, crates/cli/src/agent.rs | A workspace's agent sessions can be listed and stopped by id under agent commands. No worktree sessions command, no chats. |
| WS-44 | CLI: folder workspace commands | first version | M1 | partial | `workspace create/list/show/rm` on folder projects, crates/cli/src/workspace.rs | List, add and stop tracking work. No groups, rename, move, archive, pin, comment or check. |
| WS-45 | Sidebar card layout choice | later | M6 | not started | none | No sidebar card layout choice (Density is a planned, inert settings row). |
| TERM-01 | Create a terminal in a workspace | first version | M1 | done | `massorca terminal create <WS> [--shell] [--cwd]` -> terminal.create; crates/core-terminal/src/manager.rs | Starts in workspace root or --cwd with default or chosen shell. Desktop side-panel entry point is in PR 110. |
| TERM-02 | Tabs and pane binding | first version | M1 | in review | PR 110 | Main has only the desktop bridge and store (crates/desktop/src/service/terminals.rs); PR 110 adds Work panel tabs that reattach. Cross-workspace reference sessions not seen. |
| TERM-03 | Split panes | later | M6 | not started | none | No split panes in desktop or ui crates on main or in PR 110. |
| TERM-04 | Terminal host and reconnection after UI restart | first version | M1 | done | terminal.attach; crates/core-terminal/src/manager.rs | Service owns terminals (0033), so closing a client or the desktop leaves them running; `terminal attach` reconnects. |
| TERM-05 | Attach, detach and multiple clients | first version | M1 | changed | docs/decisions/0033-terminal-session-model.md | Built: many clients attach at once, nobody is evicted; the old "new viewer takes over" rule is dropped. |
| TERM-06 | Output streaming and flow control | first version | M1 | changed | docs/decisions/0034-terminal-scrollback-and-snapshot.md | Built: bounded 2 MiB queues per attachment and a fresh snapshot for laggards instead of pausing the shell. |
| TERM-07 | Scrollback and screen restore on reattach | first version | M1 | done | terminal.attach (snapshot/resume); crates/core-terminal/src/screen.rs | Snapshot restores screen, cursor, colours, modes and 1000 rows; resume from 1 MiB ring by offset. |
| TERM-08 | Terminal history on disk | later | M6 | not started | docs/decisions/0034-terminal-scrollback-and-snapshot.md | 0034 says no disk history in Phase 2; deferred, not dropped. |
| TERM-09 | Cold restore after a lost host or crash | later | M6 | not started | docs/decisions/0034-terminal-scrollback-and-snapshot.md | No cold restore; service stop ends terminals and nothing is restored (0033). |
| TERM-10 | Shell detection and selection | first version | M1 | done | terminal.create shell / `--shell`; crates/core-terminal/src/shell.rs | Default pwsh then powershell on Windows, $SHELL then zsh/bash/sh on Unix; explicit --shell. No terminal.default_shell setting yet. |
| TERM-11 | Shell integration: readiness, working directory, titles | first version | M1 | partial | Terminal.title from OSC 0/2; crates/core-terminal/src/manager.rs | Titles tracked; no readiness markers, startup-command barrier or working-directory tracking (0032 defers them). |
| TERM-12 | Busy state and close confirmation | later | M6 | not started | none | No busy detection or close confirmation on main or in PR 110. |
| TERM-13 | Closing and killing terminals | first version | M1 | done | terminal.kill / terminal.close, agent.stop; crates/core-terminal/src/process_tree.rs | Job object on Windows, session kill on Unix; graceful then forced after 5 s (0031, 0033). |
| TERM-14 | Input, paste, resize and terminal modes | first version | M1 | done | terminal.input / terminal.resize, `terminal attach`; crates/cli/src/terminal_attach.rs | Raw input, resize and modes (bracketed paste, mouse, alt screen) kept in snapshot. Desktop pane is in PR 110. |
| TERM-15 | Terminal query replies and colour answers | first version | M1 | partial | cursor query answering; crates/core-terminal/src/manager.rs | Service answers ESC[6n while no client is attached; no colour (OSC 10/11) answers. PR 113 and 116 refine cursor queries per client. |
| TERM-16 | Terminal environment | first version | M1 | partial | build_environment; crates/core-terminal/src/shell.rs | TERM, COLORTERM, TERM_PROGRAM, MASSORCA_* ids and caller env set; no Marketplace variables. |
| TERM-17 | Windows terminals: ConPTY, PowerShell, PATH | first version | M1 | partial | ConPTY pty; crates/core-terminal/src/pty.rs | ConPTY and PowerShell default built; registry PATH merge deferred (0032), inherited PATH only. |
| TERM-18 | macOS and Linux platform handling | first version | M1 | partial | Unix session process tree; crates/core-terminal/src/process_tree.rs | Unix pty and process groups built; no evidence of macOS privacy-prompt or Linux long-path handling. |
| TERM-19 | Host updates and crash recovery | first version | M1 | changed | docs/decisions/0033-terminal-session-model.md | No separate terminal host: UI restarts keep terminals, but a service restart or update ends them (built as decided). |
| TERM-20 | Host log | later | M6 | changed | docs/decisions/0033-terminal-session-model.md | No host process; the service log (0006, logs/service.<date>.log) covers it. Built. |
| TERM-21 | Built-in terminal themes | first version | M1 | partial | TerminalColors tokens; crates/ui/src/components/terminal_surface/colors.rs | Terminal palette comes from app design tokens; no dark/light terminal theme picker. Pane not wired on main (PR 110). |
| TERM-22 | Theme import from Ghostty and Warp | later | M6 | not started | none | Desktop settings only show an "Import theme" row label (crates/desktop/src/model/settings_sections.rs); no importer. |
| TERM-23 | Per-workspace shell command history | later | M6 | not started | none | No per-workspace shell history. |
| TERM-24 | Credential guard and agent shell hooks | first version | M1 | partial | `agent hook`; crates/core-agents/src/launch.rs | Agent status hooks installed per run; git credential guard only on service git calls (crates/core-git/src/git.rs), not in agent terminal env. |
| TERM-25 | Terminal preview in the dashboard | later | M6 | not started | none | No dashboard terminal preview. |
| TERM-26 | Headless-server terminals | first version | M1 | in review | PR 69, PR 70 | Main keeps terminals in a long-lived local service; network listener and `service run --listen` are in PR 69/70. Bot-team routine keep-alive not seen. |
| TERM-27 | Closing a workspace or worktree closes its terminals | first version | M1 | partial | workspace.remove; crates/server/src/features/workspaces.rs | Workspace removal does not kill its terminals (0033 says wired later); no terminal history to erase. |
| TERM-28 | WSL terminals (behaviour note) | later | M6 | partial | `terminal create --shell wsl.exe`; crates/core-terminal/src/shell.rs | 0032 makes WSL an explicit shell choice only; no distro picker or WSL integration. |
| GIT-01 | Git availability and version baseline | first version | M1 | partial | git discovery + `git.unavailable`, crates/core-git/src/git.rs | Service checks git runs and maps a missing binary to git.unavailable; no present/missing/unknown report per host, no Marketplace install. |
| GIT-02 | Non-interactive git runner and credential policy | first version | M1 | done | GIT_TERMINAL_PROMPT=0, askpass off, ssh BatchMode, timeouts; crates/core-git/src/git.rs | Non-interactive runner with process-tree timeout and classified stderr (decision 0013). |
| GIT-03 | Repository detection and registration | first version | M1 | partial | `project add` / `project.add`, crates/core-workspace/src/projects.rs | Tells repo from folder via rev-parse --show-toplevel. A linked worktree resolves to its own toplevel, not to its main repository. |
| GIT-04 | Clone and initialize repositories | later | M6 | not started | none | No clone or init. |
| GIT-05 | Status with line counts and listing caps | first version | M1 | done | `massorca git status` / `git.status`, crates/server/src/features/git/status.rs | Staged, unstaged, untracked, conflicted with line counts and listing caps (decision 0011). Desktop panel in review (PR 115). |
| GIT-06 | File diffs for unstaged, staged and HEAD comparisons | first version | M1 | partial | `massorca git diff` + `git patch` / `git.diff_file`, crates/server/src/features/git/diff.rs | Worktree, staged, HEAD and range diffs with binary sizes. No image previews; submodule pointer diffs not confirmed. Desktop DiffView in PR 115. |
| GIT-07 | Stage and unstage files | first version | M1 | done | `massorca git stage/unstage [--all]` / `git.stage`, `git.unstage`, crates/server/src/features/git/index.rs | One file, a selection or everything. Desktop in PR 115. |
| GIT-08 | Discard changes | first version | M1 | done | `massorca git discard` / `git.discard`, crates/server/src/features/git/index.rs | Untracked delete and staged drop need explicit flags; paths outside the worktree refused (git.path_outside_worktree). |
| GIT-09 | Commit with a typed message | first version | M1 | done | `massorca git commit` / `git.commit`, crates/server/src/features/git/commit.rs | Typed message, hooks always run (decision 0010). Desktop commit box in PR 115. |
| GIT-10 | AI-generated commit messages | later | M6 | not started | none | No AI-generated commit messages. |
| GIT-11 | Branch list and checkout | later | M6 | done | `massorca git branches` / `git switch` (`git.branches`, `git.branch_switch`), crates/server/src/features/git/branch.rs | Local branch list and checkout, refusing overwrites. |
| GIT-12 | Rename branch and folder after first agent work | later | M6 | partial | `massorca git branch-rename` / `git.branch_rename`, crates/server/src/features/git/branch.rs | Manual branch rename only; no folder move and no trigger on first agent work. |
| GIT-13 | Create a worktree from a base branch | first version | M1 | partial | `workspace create --base` / `workspace.create`, crates/core-workspace/src/create.rs | New branch from a chosen base works. Base refresh and base-ref fallback order deferred (ADR 0008). |
| GIT-14 | Worktree listing and scanning | first version | M1 | partial | worktree porcelain parser, crates/core-git/src/worktree_porcelain.rs | Parses locked and prunable internally; users only see rows plus unregistered worktrees under the root, without locked or prunable flags. |
| GIT-15 | Worktree removal with safety checks | first version | M1 | done | `workspace rm [--force]` / `workspace.remove`, crates/core-workspace/src/remove.rs | Dirty refused unless forced, ignored files and orphans need their own waiver, unmerged branches kept (branch -d). |
| GIT-16 | Preserved branches and force-delete | later | M6 | partial | `workspace rm` kept branch + `git branch-delete --force`, crates/server/src/features/git/branch.rs | Branch kept on removal and force delete reports its commit, but no saved-commit check before force delete. |
| GIT-17 | Pre-warmed worktree checkouts | drop | M6 | not started | none | Not built. Inventory ranks it drop, but no decision record removes it. |
| GIT-18 | Sparse checkout and shared paths for new worktrees | later | M6 | not started | none | No sparse checkout or shared paths. |
| GIT-19 | Push, pull, fetch and upstream ahead or behind | first version | M1 | done | `massorca git push/pull/fetch` / `git.push`, `git.pull`, `git.fetch`, crates/server/src/features/git/remote.rs | Push to upstream (creates it), pull ff-only/merge/rebase, fetch with prune, ahead and behind in status. Desktop omits them (PR 115). |
| GIT-20 | Rebase worktree onto base branch | later | M6 | not started | none | No rebase onto base; pull --mode rebase rebases only onto the upstream. |
| GIT-21 | Merge and rebase conflict state and abort | later | M6 | done | `git.status` operation + `massorca git abort` / `git.abort`, crates/server/src/features/git/remote.rs | Merge, rebase, cherry-pick and revert in progress shown; abort supported. |
| GIT-22 | History, branch and commit comparison | later | M6 | done | `massorca git log` + `git diff --from REV` / `git.log`, `git.diff` range, crates/server/src/features/git/log.rs | Paged history and branch-against-base comparison. |
| GIT-23 | Submodule status and pointer diffs | later | M6 | partial | `git.status` entry submodule flag, crates/protocol/src/methods/git/status.rs | Status marks submodule entries; no dedicated submodule status or pointer diff view. |
| GIT-24 | Ignore helpers and huge-folder suggestions | later | M6 | not started | none | No ignore helpers. |
| GIT-25 | Idle ref packing and repository maintenance | later | M6 | not started | none | No idle ref packing. |
| GIT-26 | Fork sync and fork remote setup | later | M6 | not started | docs/decisions/0010-git-operations-surface.md | Fork and provider behaviour is a later phase outside git/1. |
| GIT-27 | Remote file and commit web links | later | M6 | not started | docs/decisions/0010-git-operations-surface.md | Web links are provider behaviour, a later phase. |
| GIT-28 | Git on SSH hosts through the relay | later | M6 | changed | docs/decisions/0015-git-on-remote-execution-hosts.md | SSH relay replaced by running the service on the SSH host with the same protocol; that transport is in review (PRs 66-70). |
| GIT-29 | Git inside WSL distributions | later | M6 | changed | docs/decisions/0015-git-on-remote-execution-hosts.md | Per-command WSL routing replaced by a service inside the distribution; needs the remote transport in review (PRs 66-70). |
| GIT-30 | Bring a delegated child's commits back into the parent branch | first version | M1 | changed | docs/decisions/0055-delegation-and-bring-back.md | Replaced by explicit `agent bring-back` (merge --no-ff, abort on conflict, no ask/auto/full policy). Not built: only `agent.bring_back` protocol types, no server or CLI. |
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
| SCM-22 | Unified task drawer and source picker | first version | M4 | not started | crates/desktop/src/shell/canvas.rs | Tasks rail entry and nav row exist but open a "This page is not built yet" placeholder; no task sources in the service |
| SCM-23 | Agent access to Linear (CLI and RPC) | later | M6 | not started | none | No orca linear command or RPC |
| SCM-24 | Create workspace from an issue or PR | later | M6 | not started | none | Workspace create takes name and branch by hand; no issue or PR prefill |
| SCM-25 | Link and relink a workspace to a task | later | M6 | not started | none | No workspace-to-task link field |
| SCM-26 | Issue command template | later | M6 | not started | docs/decisions/0009-phase1-deferrals.md | No issue command template; workspace extras deferred by 0008 and 0009 |
| SCM-27 | GitHub Projects board (Tasks) | later | M6 | changed | docs/decisions/0001-extension-model.md | Owner direction in 0001: GitHub Projects becomes a first-party extension (board view plus itemSources) at M6; not built |
| SCM-28 | Self-hosted instance support | later | M6 | not started | none | No provider layer, so no self-hosted instances |
| SCM-29 | Remote and WSL execution of provider calls | later | M6 | not started | none | No provider calls to run remotely; SSH and WSL relays are absent on main |
| SCM-30 | Star Orca on GitHub (upstream promotion) | drop | M6 | not started | docs/inventory/providers-and-trackers.md | Inventory proposes drop; no decision record or owner text confirms it, so lowered to not started |
| SCM-31 | Host-wide GitHub logins: list, switch active, remove | later | M6 | not started | none | No gh login listing, switching or removal |
| SCM-32 | Feedback reports published as GitHub issues | drop | M6 | not started | docs/inventory/open-questions.md | Inventory proposes drop but says owner to confirm; no decision record, so lowered to not started |
| AG-01 | Supported agent catalog, defaults and enable/disable | first version | M3 | changed | docs/decisions/0050-agent-kinds.md; `agent kinds`, crates/core-agents/src/catalog.rs | Replaced 45 ids by kinds as data: 4 built-ins plus profile kind files; built. No default kind, auto-pick order or enable/hide switch |
| AG-02 | Install detection, refresh and installation help | first version | M3 | partial | `agent kinds` (installed, program_path), crates/core-agents/src/resolve.rs | Local PATH detection, re-read on every call, is built. No WSL or SSH host detection, no install-docs links |
| AG-03 | CLI version probing and capability gates | later | M6 | not started | docs/decisions/0050-agent-kinds.md | 0050 says no version probe in Phase 4; no capability gates (no structured chats yet) |
| AG-04 | Terminal launch command construction | first version | M3 | changed | docs/decisions/0051-agent-sessions.md; `agent start`, crates/core-agents/src/launch.rs | Agent spawned directly as the terminal program (no shell, so no per-shell quoting); args and env from kind plus caller; built. CLI passes no env |
| AG-05 | Prompt delivery to terminal agents | first version | M3 | partial | `agent start --prompt` (argv/flag), crates/core-agents/src/launch.rs | Launch-time argv/flag built. Paste-after-ready (agent.prompt) only in protocol, no handler; on unmerged branch phase4/agent-prompt, no PR |
| AG-06 | Draft (unsent) prompts | later | M6 | not started | - | No draft prompt seeding anywhere |
| AG-07 | Image and file attachments for terminal agents | later | M6 | not started | - | No attachment or @file handling for terminal agents |
| AG-08 | Launch intent API (agent.launch) and launch receipts | first version | M3 | partial | `agent.start`, crates/server/src/features/agents.rs | Start in an existing workspace works with request_id retry. No new-workspace placement in the call; prompt receipt type exists but agent.prompt is not served |
| AG-09 | Creating a workspace with a startup agent | first version | M3 | not started | - | workspace create has no startup agent; only two steps (workspace create, then agent start) |
| AG-10 | Permission posture (Yolo / Manual) and access presets | first version | M3 | changed | docs/decisions/0050-agent-kinds.md | No permission-bypass switch; caller passes bypass flags explicitly via `agent start -- <args>` (built). No presets |
| AG-11 | Per-agent Command, Arguments and Environment settings | first version | M3 | partial | `<profile>/agents/kinds/*.json`, crates/core-agents/src/catalog.rs | User kinds can declare program/args; per-start args/env on agent.start. No standing override of a built-in (0050 defers to settings registry); CLI sends no env |
| AG-12 | Workspace trust pre-writes | first version | M3 | not started | - | No trust-prompt pre-writes in core-agents |
| AG-13 | Resume, continue and fork of terminal sessions | first version | M3 | partial | `agent start` assigns Claude --session-id, crates/core-agents/src/launch.rs | agent_ref stored for resume, but agent.resume has no handler or CLI on main (unmerged branch phase4/agent-prompt, no PR). No fork; credential homes import not served |
| AG-14 | Model, effort and mode selection for launches | first version | M3 | not started | - | No model/effort/mode selection; only raw caller args after `--` |
| AG-15 | Structured (native) chat platform | first version | M3 | not started | - | No structured chat; activity source `output` reserved (0052) |
| AG-16 | Chat capabilities (queue, steer, stop, rewind, compact, goals, approvals, questions, subagents, context usage) | later | M6 | not started | - | No chat controls |
| AG-17 | Claude structured adapter | first version | M3 | not started | - | Claude runs only as a terminal agent; no structured adapter |
| AG-18 | Codex structured adapter | later | M6 | not started | - | Codex runs only as a terminal agent |
| AG-19 | Pi RPC adapter and Gentle Shell | later | M6 | not started | - | Gentle Shell is a terminal kind with no status; no RPC adapter |
| AG-20 | Antigravity stream-json adapter | later | M6 | not started | - | Antigravity is a terminal kind with no status; no stream-json adapter |
| AG-21 | ACP client (Grok, OpenCode, OMP) | later | M6 | not started | - | No ACP client |
| AG-22 | Chat attachments and image prompts | later | M6 | not started | - | No chat attachments |
| AG-23 | Model catalog service for chats | first version | M3 | not started | - | No model catalog |
| AG-24 | Terminal-backed chat view (transcript chat) | later | M6 | not started | - | No transcript chat view |
| AG-25 | Provider process supervision | later | M6 | partial | docs/decisions/0051-agent-sessions.md; crates/core-agents/src/sessions.rs | Agents run in service terminals: tree kill and end with the service (0031/0033). No headless provider children exist yet |
| AG-26 | Delegated (child) sessions | first version | M3 | not started | protocol `agent.delegate` constant only, crates/protocol/src/methods/agents.rs | No handler or CLI on main; implementation on unmerged branch phase4/agent-delegation, no open PR |
| AG-27 | AI text generation through agent CLIs | later | M6 | not started | - | No AI text generation through agent CLIs |
| AG-28 | Claude Agent Teams mode | later | M6 | not started | - | No Claude Agent Teams mode (PR 118 is a docs-only Bot Teams decision) |
| AG-29 | Agent process recognition in panes | later | M6 | not started | - | Sessions own their terminal, so MassOrca-launched agents are known; no recognition of agents in other panes |
| AG-30 | Per-agent startup quirks | later | M6 | not started | - | No per-agent startup workarounds |
| AG-31 | Bring back a child session's git work into the parent worktree | later | M6 | not started | protocol `agent.bring_back` constant only, crates/protocol/src/methods/agents.rs | No handler/CLI on main; branches phase4/agent-delegation and phase4-merge unmerged, no PR. 0055 makes it manual only in Phase 4 |
| AG-32 | Shared checklist card in agent chats | later | M6 | not started | - | No checklist card |
| AG-33 | Jev decision for delegated task tier | later | M6 | not started | - | No delegated tier decision service |
| AG-34 | Ask the person a question from a chat agent | later | M6 | not started | - | No question card |
| AG-35 | Results card: environment URLs and opened files | later | M6 | not started | - | No Results card |
| AG-36 | Design-system folder access for chat agents in linked worktrees | later | M6 | not started | - | No design-system folder grant |
| STAT-01 | Agent status states and the status row | first version | M3 | partial | `agent status` / `agent.show`, crates/cli/src/agent.rs | Four activity states plus execution shown in CLI from the real service. Desktop status row uses sample data (crates/desktop/src/data.rs); no child row naming parent |
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
| STAT-16 | Remote host status over SSH | first version | M3 | partial | crates/core-agents/src/sessions.rs | Store lives on the executing host; remote connection is in PRs 69/70 but PR 70 does not wire `agent` commands to --host, desktop has no agent data |
| STAT-17 | Status during disconnect and reconnect | first version | M3 | partial | execution state `unverifiable`, crates/protocol/src/methods/agents.rs | Wire vocabulary exists; no client keeps last state as unverifiable and replays on reconnect |
| STAT-18 | WSL agent relay | later | M6 | not started | - | No WSL relay |
| STAT-19 | Attention and unread markers | first version | M3 | not started | - | No attention or unread markers |
| STAT-20 | Desktop notifications | later | M6 | not started | - | No desktop notifications |
| STAT-21 | Notification sounds | later | M6 | not started | - | No notification sounds |
| STAT-22 | Phone alerts and desktop-away gating | later | M6 | not started | - | No phone alerts |
| STAT-23 | Keep-awake while agents work | later | M6 | not started | - | No keep-awake |
| STAT-24 | Rename workspace from first agent work | later | M6 | not started | - | No rename from first agent work |
| STAT-25 | Repository setup and archive hooks (orca.yaml) | later | M6 | not started | docs/decisions/0009-phase1-deferrals.md | Setup scripts deferred to a later phase; nothing built |
| STAT-26 | Delegated parent link on status rows | first version | M3 | not started | AgentSession.parent_id field, crates/protocol/src/methods/agents.rs | Field exists but delegation is not served on main (unmerged branch phase4/agent-delegation, no PR) |
| ACC-01 | Claude Code sign-in and import | first version | M3 | partial | agent.credentials.import declared in crates/protocol/src/methods/agents.rs; launch uses <profile>/agents/homes/claude via crates/core-agents/src/launch.rs | Decision 0056: import of a Claude home. No service handler and no CLI on main (server registers no credentials method); no sign-in flow |
| ACC-02 | Codex sign-in and import | first version | M3 | partial | agent.credentials.import declared in crates/protocol/src/methods/agents.rs; CODEX_HOME set at launch in crates/core-agents/src/launch.rs | Same as ACC-01 for Codex (auth.json, config.toml). Import not served, no CLI, no sign-in |
| ACC-03 | Account registry and duplicate detection | first version | M3 | not started | none | No account registry, per-runtime (host or WSL) accounts or duplicate detection; 0056 keeps one credential home per profile and kind |
| ACC-04 | Claude Code account switching | first version | M3 | not started | none | No account switching; 0056 gives one Claude home per profile, chosen by --profile, not per account |
| ACC-05 | Codex account switching | first version | M3 | not started | none | No Codex account switching; one CODEX_HOME per profile (0056) |
| ACC-06 | Re-authentication and account removal | first version | M3 | not started | agent.credentials.remove declared only, crates/protocol/src/methods/agents.rs | Remove is protocol constant only, no handler or CLI; no re-authentication |
| ACC-07 | Claude token refresh and refresh ownership | first version | M3 | not started | docs/decisions/0056-agent-credentials.md | 0056: MassOrca does not check or refresh a login in Phase 4; the agent refreshes its own tokens |
| ACC-08 | Live-session protection during account changes | first version | M3 | not started | none | No live-session protection during account changes; account changes do not exist yet |
| ACC-09 | Claude usage and limits | first version | M3 | partial | UsageModel rings, crates/desktop/src/data.rs | Rail rings drawn from sample data only (data.rs says sample data now); no Claude usage reading in service or CLI |
| ACC-10 | Codex usage and limits | first version | M3 | partial | UsageModel rings, crates/desktop/src/data.rs | Same as ACC-09: sample rings only, no Codex usage source |
| ACC-11 | Codex reset credits | later | M6 | not started | none | No Codex reset credits anywhere |
| ACC-12 | Read-only sign-in detection for Cursor, Grok, Gemini and Kimi | later | M6 | not started | none | No Cursor, Grok, Gemini or Kimi sign-in detection |
| ACC-13 | Stored API keys and cookies for key-based providers | later | M6 | not started | none | No stored API keys or cookies for key-based providers; the vault (0071) is not built |
| ACC-14 | Managed OpenCode and Devin accounts | later | M6 | not started | none | No OpenCode or Devin accounts |
| ACC-15 | Usage refresh cadence and caching | first version | M3 | not started | none | No usage source, so no refresh cadence or caching |
| ACC-16 | Usage and limits dashboard | later | M6 | partial | rail usage rings and popover states, crates/desktop/src/model/shell.rs | Rings and popover text exist on sample data; no switcher menus, no usage page, no real data |
| ACC-17 | Limit warnings | drop | M6 | not started | none | Proposed drop in docs/inventory/README.md, but no decision record on main; nothing built |
| ACC-18 | Automatic account rotation on limits | drop | M6 | not started | none | Proposed drop in docs/inventory/README.md, no decision record; nothing built |
| ACC-19 | Local token-usage analytics and cost estimates | later | M6 | not started | none | No local token or cost analytics |
| ACC-20 | Usage attribution to workspaces and sessions | later | M6 | not started | none | No usage attribution |
| ACC-21 | Credential at-rest protection | first version | M3 | partial | profile dir 0700 per 0004, crates/profile/src/profile.rs | Profile dir owner-only on Unix; 0056 file modes (0600 homes) belong to import, which is not served; no encryption of secrets (0071 unbuilt) |
| ACC-22 | Keep refreshed sign-ins across app restart | first version | M3 | partial | profile credential home at launch, crates/core-agents/src/paths.rs | A home under the profile persists across restarts and the agent refreshes in place; getting a login there (import) is not served |
| ACC-23 | Per-account Claude profile folders | drop | M6 | changed | docs/decisions/0056-agent-credentials.md | Replaced by one credential home per profile and kind (<profile>/agents/homes/<kind>); launch side built in crates/core-agents/src/launch.rs, import not served |
| ACC-24 | Carry existing subscriptions into the new core | first version | M3 | partial | agent.credentials.import declared, crates/protocol/src/methods/agents.rs | 0056 import path designed; launch uses an existing home, but import has no handler or CLI; API keys need the unbuilt vault |
| ACC-25 | App activity counters | later | M6 | not started | none | No activity counters |
| ACC-26 | Delegated sessions choose an agent by usage headroom | later | M6 | not started | agent.delegate declared only, crates/protocol/src/methods/agents.rs | Delegation not served on main and no usage headroom data |
| HIST-01 | Browse past sessions | first version | M3 | partial | massorca agent list / agent.list, crates/server/src/features/agents.rs | Lists only sessions MassOrca started (persisted, newest first); no discovered agent history, scope filter, grouping or refresh UI |
| HIST-02 | Session titles, previews and metadata | first version | M3 | partial | AgentSession fields, crates/protocol/src/methods/agents.rs | Own sessions carry title, workspace, created time and last result; no branch, model, last activity or transcript preview |
| HIST-03 | Local discovery for Claude Code and Codex | first version | M3 | not started | docs/decisions/0054-prompts-and-resume.md | No local discovery of Claude Code or Codex conversations; 0054 says importing outside sessions is not in Phase 4 |
| HIST-04 | Local discovery for the other 22 agents | later | M6 | not started | none | No discovery for other agents |
| HIST-05 | Full-text search of session history | later | M6 | not started | none | No history search; sessions.search setting is only planned in 0070 |
| HIST-06 | Transcript viewer | later | M6 | not started | none | No transcript viewer; terminal screen/scrollback exists but is not a transcript |
| HIST-07 | Resume a past session in a terminal | first version | M3 | partial | agent.resume declared in crates/protocol/src/methods/agents.rs; design in docs/decisions/0054-prompts-and-resume.md | agent_ref is stored, but agent.resume has no server handler and no CLI on main; only MassOrca-started sessions |
| HIST-08 | Adopt or fork a past session into a structured chat | later | M6 | not started | none | No adopt or fork into structured chat |
| HIST-09 | Delete a past session | later | M6 | not started | none | No delete of agent conversation files (agent.forget only drops MassOrca's own row) |
| HIST-10 | Remote history on SSH and runtime hosts | later | M6 | not started | none | No remote history |
| HIST-11 | WSL history on Windows | later | M6 | not started | none | No WSL history |
| HIST-12 | Claude account history sharing | later | M6 | not started | none | No Claude account history sharing |
| HIST-13 | Cursor IDE sign-in state read | later | M6 | not started | none | No Cursor sign-in read |
| HIST-14 | OpenCode live session binding | later | M6 | not started | none | No OpenCode binding |
| HIST-15 | Projectless chat folders | later | M6 | not started | none | No projectless chat folders; conversations.folder only planned in 0070 |
| HIST-16 | Automated-run filter, delegated and subagent nesting | later | M6 | not started | parent_id in AgentSession, crates/protocol/src/methods/agents.rs | Delegate and tree methods declared but not served; no automated-run filter or subagent transcripts |
| HIST-17 | Process memory dashboard | later | M6 | not started | none | No process memory dashboard; status bar memory item is sample data |
| HIST-18 | Import sessions from other agent tools | first version | M3 | not started | docs/decisions/0054-prompts-and-resume.md | 0054: importing sessions started outside MassOrca is not in Phase 4 |
| REM-01 | Add an SSH host manually | first version | M2 | in review | PR 70 `host add-ssh <ssh-target>` (crates/cli/src/host/add_ssh.rs), PR 68 crates/core-remote/src/bootstrap.rs | Per 0044 SSH only installs and pairs; host is then used via `--host`. Address, user, key and jump come from ~/.ssh/config. No desktop UI (settings row says "come later"). |
| REM-02 | Import hosts from the OpenSSH config | later | M6 | changed | docs/decisions/0044-ssh-host-bootstrap.md | System ssh reads ~/.ssh/config directly, so aliases work without an import step; no host picker or sync. Replacement in review (PR 68, 70). |
| REM-03 | Authenticate with keys, agent and password | first version | M2 | in review | PR 68 crates/core-remote/src/ssh.rs (system ssh in batch mode) | Keys and agent via OpenSSH. Password or passphrase prompts not supported (batch mode); 0044 defers them to an in-process SSH library. |
| REM-04 | Verify host keys | first version | M2 | in review | PR 68 crates/core-remote/src/ssh.rs; PR 69/70 `host.identity_mismatch` | SSH host keys are OpenSSH known_hosts policy (0044); the MassOrca server is pinned and a changed key reports identity_mismatch. No desktop warning UI. |
| REM-05 | Jump hosts and ProxyCommand | later | M6 | in review | PR 70 `host add-ssh --ssh-arg` (crates/cli/src/host/add_ssh.rs) | Jump hosts and ProxyCommand work through ~/.ssh/config or `--ssh-arg`, because the system ssh is used (0044). |
| REM-06 | System OpenSSH transport | later | M6 | in review | PR 68 crates/core-remote/src/ssh.rs, crates/core-remote/src/process.rs | System OpenSSH is the only SSH transport (0044); no FIDO2 or Kerberos specific handling, inherited from ssh. |
| REM-07 | Connection lifecycle, reconnect and status | first version | M2 | in review | PR 70 `host status [<NAME>]` (crates/cli/src/host/status.rs) | Reports connected, unreachable, refused, identity_mismatch. Automatic reconnect for network clients (0040 backoff) not in any PR; no desktop host state. |
| REM-08 | Deploy and update the remote relay | first version | M2 | changed | docs/decisions/0044-ssh-host-bootstrap.md; PR 68 crates/core-remote/src/bootstrap.rs | No relay: `host add-ssh` uploads the same massorca binary, writes a systemd user unit and re-running it upgrades. In review (PR 68, 70); binary is supplied with `--binary`. |
| REM-09 | Runtime ladder and plain-SSH fallback | drop | M6 | dropped | docs/decisions/0044-ssh-host-bootstrap.md | One host daemon; SSH is only how a host is installed and paired, so there is no plain-SSH degraded mode. |
| REM-10 | Native terminal dependencies on the host | drop | M6 | dropped | docs/decisions/0044-ssh-host-bootstrap.md | The host runs the same Rust binary with its own PTY and watch code; no per-OS Node addons exist to build. |
| REM-11 | Remote terminals | first version | M2 | in review | PR 70 global `--host <NAME>` with `terminal` and `agent` commands (crates/cli/src/target.rs); PR 69 crates/client/src/remote.rs | Terminals and agents run in the host's service; CLI only. Desktop cannot target a host yet. |
| REM-12 | Terminals survive disconnects | first version | M2 | in review | `terminal.attach` with `since` on main (crates/server/src/features/terminal_attach.rs); remote path PR 69, 70 | Terminals live in the host service and resume from an output offset (0035); reaching them remotely needs PR 69/70. No desktop remote view. |
| REM-13 | Remote file access | later | M6 | not started | none | No file browse, read, edit or watch methods in crates/protocol on main or in an open PR. |
| REM-14 | Remote git and worktrees on the host | first version | M2 | in review | PR 70 `--host <NAME> git ...` and `workspace` commands; git methods on main (crates/server/src/features/git/mod.rs) | Per 0015 remote git is the same git/1 over the network; network path is PR 69/70. CLI only. |
| REM-15 | Remote CLI passthrough and orchestration | later | M6 | not started | none | No CLI passthrough from host agents back to the desktop. |
| REM-16 | Manual port forwards | later | M6 | not started | none | No port forwarding. |
| REM-17 | Remote port detection and advertised URLs | later | M6 | not started | none | No remote port detection or advertised dev-server URLs. |
| REM-18 | Managed Orca server on SSH hosts | later | M6 | changed | docs/decisions/0044-ssh-host-bootstrap.md | One daemon replaces relay plus managed server; reached through the network listener, not a tunnel. In review (PR 68, 69, 70); SSH tunnel path deferred. |
| REM-19 | Link an SSH host to a paired server | later | M6 | changed | docs/decisions/0044-ssh-host-bootstrap.md | An SSH host and a paired server are the same host on the client; phones reach it via the listener. In review (PR 69, 70). |
| REM-20 | WSL distributions as an execution host | later | M6 | changed | docs/decisions/0015-git-on-remote-execution-hosts.md | A WSL distro runs its own service and Windows connects to it as to any host. Network path in review (PR 69, 70); no WSL-specific setup. |
| REM-21 | Ephemeral VMs from environment recipes | later | M6 | not started | none | No ephemeral VM provisioning. |
| REM-22 | Remove a host and clean its remote state | first version | M2 | in review | PR 70 `host remove <NAME> [--uninstall --ssh <TARGET>]` (crates/cli/src/host/mod.rs) | Revokes this device and forgets the host; workspaces stay in the host's store unless uninstalled. Re-adding restores access by pairing again. |
| REM-23 | Linux and macOS remote hosts | first version | M2 | in review | PR 68 crates/core-remote/src/bootstrap.rs | Linux x86_64 and aarch64 only; macOS hosts are rejected (0044 defers macOS). |
| REM-24 | Windows remote hosts | later | M6 | not started | none | Windows hosts deferred by 0044; bootstrap rejects non-Linux. |
| REM-25 | Relay diagnostics and self-test | later | M6 | in review | PR 70 `host status`, `server status` (crates/cli/src/server_status.rs) | Status and refusal reasons plus journalctl logs via `--log-stderr` unit; no dedicated self-test command. |
| REM-26 | Remote host answers GitHub account and Marketplace calls | later | M6 | not started | none | No GitHub account or Marketplace handling on hosts. |
| REM-27 | Read a file an agent wrote on a paired runtime | later | M6 | not started | none | No method to read agent-written files outside the worktree. |
| SRV-01 | Headless server mode (serve) | first version | M2 | in review | PR 69/70 `service run --listen <ADDR>` (crates/server/src/network/listen.rs); `service unit` | Main has headless `service run` but local socket only. Network listener and systemd unit in PR 69, 70. |
| SRV-02 | Readiness and machine-readable output | later | M6 | in review | `service run --json` start document on main (crates/cli/src/service/control.rs); PR 70 `server status` | Main prints pid, version, protocol. Endpoint, pin and pending pairing status come with `server status` in PR 70. |
| SRV-03 | Local owner channel for same-machine clients | first version | M2 | done | `service status` / local socket or named pipe, crates/server/src/owner.rs | Owner-only socket or pipe with peer check (0003); desktop and CLI both use it. |
| SRV-04 | Network listener and bind policy | first version | M2 | in review | PR 69 crates/server/src/network/listen.rs; PR 70 `--listen`, `--allow-all-interfaces` | Default tailnet binding; 0.0.0.0 refused without flag (0045). No pairing opt-in toggle; the listener is opted in by flag. |
| SRV-05 | Runtime discovery file and ownership | first version | M2 | done | `service status`, crates/server/src/service_json.rs | Per 0003 the socket name is derived from the profile (no discovery needed); service.json written; stale lock and socket reclaimed. |
| SRV-06 | Pairing offer (QR and URL) | first version | M2 | in review | PR 70 `pair offer` / `pair accept` (crates/cli/src/pair.rs); PR 66 crates/protocol/src/pairing.rs | `mop1.` offer string, single use, expiring. No QR code yet (0041 "later a QR code"); no explicit regenerate. |
| SRV-07 | Advertised address and override | first version | M2 | in review | PR 69 `advertised_addresses` (crates/server/src/network/state.rs) | Offer lists bound addresses plus MagicDNS name. No user override of the advertised address in any PR. |
| SRV-08 | Device registry and per-device tokens | first version | M2 | changed | docs/decisions/0041-device-identity-and-pairing.md | Per-device client certificates (mTLS) instead of tokens; devices table and revoke in review (PR 66, 69, 70). |
| SRV-09 | Device scopes and grants | later | M6 | in review | PR 69 crates/server/src/permission.rs; PR 70 `pair offer --admin` | Scopes read, workspace, host-admin, device-admin chosen at pairing, never widened (0041). |
| SRV-10 | Handshake, authentication and limits | first version | M2 | in review | PR 69 crates/server/src/network/listener.rs, crates/server/src/network/auth.rs; PR 85 | 128-connection cap, 10 s handshake, pairing rate limits, pre-auth gate (PR 85). |
| SRV-11 | End-to-end encryption on the network channel | first version | M2 | changed | docs/decisions/0040-network-transport.md | TLS 1.3 with pinned server and client certificates replaces app-level encryption. In review (PR 67 crates/net/src/tls.rs, PR 69). |
| SRV-12 | TLS on the network listener | later | M6 | changed | docs/decisions/0041-device-identity-and-pairing.md | TLS is always on with a self-signed pinned key; a TLS-terminating reverse proxy is unsupported (0041, 0043). In review (PR 67). |
| SRV-13 | Relay path for phones without LAN reach | later | M6 | not started | none | No relay; 0042/0043 make Tailscale the way to reach a host but no record drops the relay. first-version.md: not in the first version. |
| SRV-14 | Connection mode: automatic or LAN only | later | M6 | not started | none | Depends on the relay, which does not exist. |
| SRV-15 | Multiple simultaneous clients and revocation | first version | M2 | in review | Multiple local clients on main (crates/server/src/connection.rs); PR 69 live revoke (crates/server/src/network/live.rs) | Main serves desktop and CLI at once locally. Revoking one device and cutting only its connections is PR 69. |
| SRV-16 | Protocol version and capability negotiation | first version | M2 | done | `hello`/`welcome` negotiation, crates/server/src/handshake.rs | Version range negotiated, `rejected` names the supported range; capabilities offered per connection. Tested in crates/protocol/tests/version_negotiation.rs. |
| SRV-17 | Reconnect and resync | first version | M2 | partial | Desktop reconnect with backoff, crates/desktop/src/service/bridge.rs; `terminal.attach since` | Local link reconnects and terminals resume. Network client reconnect with backoff (0040) is not on main or in a PR. |
| SRV-18 | Client connection to a server | first version | M2 | in review | PR 69 crates/client/src/remote.rs, crates/client/src/hosts.rs; PR 70 `--host` | CLI drives a remote server over mTLS WebSocket with a saved host list. Desktop cannot target a host. |
| SRV-19 | Mobile web bundle served over RPC | drop | M6 | dropped | docs/decisions/0040-network-transport.md | Android uses the Rust client crate through UniFFI, so no web bundle is served. |
| SRV-20 | Browser web client over HTTP | later | M6 | not started | none | No browser web client; 0040 only keeps WebSocket so one could return. |
| SRV-21 | Server self-update | later | M6 | in review | PR 68/70 re-running `host add-ssh` upgrades and restarts the unit | Upgrade is over SSH from the client (0044, 0045), not over the RPC link; no self-update method. |
| SRV-22 | Push registration for paired phones | later | M6 | not started | none | No push registration. |
| AUTO-01 | Scheduled automation definitions | first version | M3 | not started | crates/desktop/src/shell/canvas.rs | Automations sidebar entry opens a "This page is not built yet" placeholder; no automation methods in protocol, server or CLI |
| AUTO-02 | Destinations and workspace modes | first version | M3 | not started | none | No automation destinations or workspace modes anywhere on main or in an open PR |
| AUTO-03 | Schedule engine, missed runs and retention | first version | M3 | not started | none | No schedule engine, missed-run record or retention on main or in an open PR |
| AUTO-04 | Run now and run history | first version | M3 | not started | none | No run-now or run history for automations on main or in an open PR |
| AUTO-05 | Precheck gate | later | M6 | not started | none | No precheck gate on main or in an open PR |
| AUTO-06 | Agent launch for runs, extra arguments and session reuse | first version | M3 | partial | agent.start (massorca agent start KIND --prompt -- ARGS); crates/server/src/features/agents.rs | Generic agent launch with prompt and extra args exists; no automation runs or saved prompts; session reuse (agent.resume) is a protocol constant only |
| AUTO-07 | Run completion, output and usage | first version | M3 | partial | agent.show / agent.watch (agent status --until done); crates/server/src/features/agents.rs | Agent session done state and result text exist; no automation run records and no token usage |
| AUTO-08 | Host ownership, fencing and orphans | first version | M3 | not started | none | No automation host ownership, fencing or orphan handling |
| AUTO-09 | Remote and SSH automation scheduling | later | M6 | not started | none | No remote or SSH automation scheduling; network/SSH layers are only in open PRs 67-70 and do not add automations |
| AUTO-10 | Headless run terminal retention | later | M6 | not started | none | No headless run terminal retention |
| AUTO-11 | External Hermes and OpenClaw cron managers | drop | M6 | not started | docs/inventory/automations.md (AUTO-11 Proposal: drop) | Only an inventory proposal to drop; no decision record or owner direction found confirming it, so not marked dropped |
| AUTO-12 | Bot team roster, editing and team access | first version | M3 | partial | crates/desktop/src/model/teams/state.rs (create_team, add_bot) | Desktop roster UI runs on SampleTeamsSource; with a real client it gets UnavailableTeams; no team service; PR 118 is decision 0072 (design only, no code) |
| AUTO-13 | Conference room: replies, request routing and choice cards | first version | M3 | not started | crates/desktop/src/shell/canvas.rs (Conference placeholder) | Conference room canvas is a placeholder saying live agents are not built; 0072 (PR 118) puts room routing out of scope |
| AUTO-14 | Team Board: task states, columns and reassign | first version | M3 | partial | crates/desktop/src/model/teams/board.rs | Board UI on sample data with columns Open/Running/Waiting for you/Done (not inventory columns); no task service; PR 118 designs task.* methods only |
| AUTO-15 | Orchestration state store and Runs | first version | M3 | not started | crates/core-store/src/migrations.rs | Store has projects, workspaces, settings and agent_sessions only; no Runs, tasks or team tables |
| AUTO-16 | Tasks and dispatch to terminals | first version | M3 | not started | none | No orchestration tasks, dispatch or circuit breaker |
| AUTO-17 | Worker lifecycle | first version | M3 | partial | agent.start/stop/forget/attach; crates/server/src/features/agents.rs | Agents start, read, stop and forget in a workspace; no worker placement into new worktrees (agent.delegate unimplemented), release or retain |
| AUTO-18 | Agent-to-agent messages | first version | M3 | not started | none | No agent-to-agent messages or group addresses |
| AUTO-19 | Decision gates and blocking questions | later | M6 | not started | none | No decision gates or blocking questions |
| AUTO-20 | Delegation from chat, team delegate and Automatic routing | first version | M3 | partial | agent.delegate in crates/protocol/src/methods/agents.rs; docs/decisions/0055-delegation-and-bring-back.md | 0055 replaces the card with explicit agent.delegate; only protocol constants and params exist, no server handler or CLI; Automatic routing not built per 0055 |
| AUTO-21 | Nested worker depth limit | first version | M3 | partial | agent.depth_exceeded in crates/protocol/src/codes.rs; docs/decisions/0055-delegation-and-bring-back.md | 0055 fixes depth 1; error code only, no enforcement since delegate is unimplemented; desktop setting row is a planned control showing 3 |
| AUTO-22 | Multi-server federation of runs and workers | later | M6 | not started | none | No federation of runs or workers |
| AUTO-23 | Orchestration over SSH hosts | later | M6 | not started | none | No orchestration over SSH hosts |
| AUTO-24 | Legacy coordinator loop | drop | M6 | dropped | docs/inventory/automations.md (AUTO-15 owner direction 2026-11-12) | Owner direction: the Run concept is first version; the legacy loop (AUTO-24) stays a drop |
| AUTO-25 | Artifact sharing commands | later | M6 | not started | none | No artifact sharing commands |
| AUTO-26 | Artifact sharing gate and account rules | later | M6 | not started | none | No artifact sharing gate or account rules |
| AUTO-27 | Artifact records, idempotency and recovery | later | M6 | not started | none | No artifact records or idempotent publish |
| AUTO-28 | Routines | first version | M3 | partial | crates/desktop/src/shell/teams/routines.rs | Routines page lists sample routines with enable switch; routine form not built; no routine service or scheduler |
| AUTO-29 | Web address (webhook) triggers | later | M6 | partial | crates/desktop/src/shell/teams/routines.rs | Routines page shows sample web addresses with hidden tokens; no webhook listener, keys or rate limit in the service |
| AUTO-30 | Calendar of routines and automations | first version | M3 | not started | crates/desktop/src/model/navigation.rs (Page::Calendar) | Calendar rail entry opens the "This page is not built yet" placeholder |
| AUTO-31 | Team files and team memory | later | M6 | partial | crates/desktop/src/shell/teams/files.rs; crates/desktop/src/shell/teams/memory.rs | Files and memory pages render sample data only; no team files, memory or Engram link in the service |
| AUTO-32 | Bring back child session work into the parent | first version | M3 | partial | agent.bring_back in crates/protocol/src/methods/agents.rs; docs/decisions/0055-delegation-and-bring-back.md | Protocol params and agent_sessions.brought_back_at column exist; no server handler or CLI; 0055 defers automatic ask/auto/full policies |
| AUTO-33 | Bot team projects: Board, Brief and code source | first version | M3 | partial | crates/desktop/src/shell/teams/project.rs | Project page with Brief and own Board on sample data; no project service; code source out of scope in 0072 (PR 118, design only) |
| AUTO-34 | Move a waiting task to another bot or team | later | M6 | not started | none | Board only moves tasks between columns in sample data; no move to another bot or team |
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
| BRW-20 | Client-hosted and headless browser pages | drop | M6 | not started | - | Nothing built; inventory proposes drop but no decision record confirms it |
| BRW-21 | Preview workspace files (documents, images, media) | later | M6 | not started | - | No file preview in the pane; Work panel tab kinds name Markdown and file but nothing renders them on main |
| BRW-22 | Element grab for agent prompts (Design Mode) | later | M6 | not started | - | BrowserChrome draws a grab button only; no element grab into prompts |
| BRW-23 | Desktop computer use: inspect apps and UI | later | M6 | not started | - | Computer use is a disabled Settings row (crates/desktop/src/model/settings_sections.rs) |
| BRW-24 | Desktop computer use: act on apps | later | M6 | not started | - | No computer-use actions |
| BRW-25 | Desktop computer use: permissions and diagnostics | later | M6 | not started | - | No permission checks or diagnostics for computer use |
| BRW-26 | Emulator device control (Android and iOS simulators) | later | M6 | not started | - | WorkTabKind::MobileEmulator exists in the ui component only; no emulator control |
| BRW-27 | Emulator screen streaming and input | later | M6 | not started | - | No emulator streaming or input |
| BRW-28 | Voice dictation | later | M6 | not started | - | Composer has a Dictate button and Settings a disabled Dictation row; no speech capture or recognition |
| BRW-29 | Local speech models | later | M6 | not started | - | Speech model is a disabled Settings row; no model management |
| BRW-30 | Cloud transcription with an OpenAI key | later | M6 | not started | - | No cloud transcription |
| SKL-01 | Find skills already installed in agent tools | later | M6 | not started | docs/architecture/extensions.md | No skill discovery; a `Skill` placement registry is planned for M3 and skills as a `skills` contribution kind |
| SKL-02 | Place skills for agent tools | later | M6 | not started | docs/architecture/extensions.md | Not built; M3 plans a `Skill` placement registry used by built-ins |
| SKL-03 | Check freshness and update installed MassOrca skills | later | M6 | not started | none | No MassOrca skills or freshness check |
| SKL-04 | MassOrca's own skills and version-matched guides | later | M6 | not started | none | No bundled skills or guides; extensions.md plans an `extension-author` skill only for Creator mode |
| SKL-05 | Share selected skills as an unlisted link | later | M6 | not started | none | No skill sharing links; 0001 distribution uses indexes, archives and dev folders, not unlisted links |
| SKL-06 | Install a shared skill bundle | later | M6 | not started | none | No bundle install |
| SKL-07 | Update and roll back a shared bundle | later | M6 | not started | none | No bundle versions or rollback |
| SKL-08 | Revoke and delete shared links and versions | later | M6 | not started | none | No link revocation |
| SKL-09 | Skill bundle package format and limits | later | M6 | not started | docs/architecture/extensions.md | Not built; skills would ship inside `.mxt` extension packages addressed by SHA-256 (section 2), M6 |
| SKL-10 | Cloud storage and transfer for bundles | later | M6 | not started | none | No cloud storage or transfer |
| SKL-11 | Protect local edits during install and update | later | M6 | not started | none | No skill install path, so no edit protection |
| SKL-12 | Crash-safe install, update and removal | later | M6 | not started | docs/architecture/extensions.md | Not built; extension lifecycle plans "Nothing was changed" on failed install, M6 |
| SKL-13 | Remove installed skills | later | M6 | not started | none | No skill removal |
| SKL-14 | Delete local skill folders safely | later | M6 | not started | none | No local skill deletion |
| SKL-15 | Install and manage skills on a paired server | later | M6 | not started | none | No skills on a paired server; pairing itself only in open PRs 66 and 69 |
| SKL-16 | Install and manage skills on SSH hosts | later | M6 | not started | none | No SSH host skills |
| SKL-17 | Skills in WSL distributions | later | M6 | not started | none | No WSL skill install |
| SKL-18 | Plugin system switch, package format and installation | later | M6 | changed | docs/decisions/0001-extension-model.md | Replaced by extension packages (extension.json, .mxt by SHA-256, lockfile, rollback) from index, archive or dev folder; M6, not built |
| SKL-19 | Plugin consent, enablement and audit | later | M6 | changed | docs/decisions/0001-extension-model.md | Replaced by core-computed risk review, consent fingerprint and per-permission revoke; M6, not built |
| SKL-20 | Plugin runtime and host API | later | M6 | changed | docs/decisions/0001-extension-model.md | Node worker replaced by Wasm components on Wasmtime with WIT host imports and host-injected secrets; M6, not built |
| SKL-21 | Plugin contributions: panels, commands and keybindings | later | M6 | changed | docs/decisions/0001-extension-model.md | Replaced by declarative view documents in 14 slots plus commands and keybindings kinds; renderer M4, runtime M6, not built |
| SKL-22 | Plugin marketplace sources and listings | later | M6 | changed | docs/decisions/0001-extension-model.md | Replaced by an Ed25519-signed official index pinning version and hash, plus third-party indexes; M6, not built |
| SKL-23 | Plugin kill list | later | M6 | changed | docs/decisions/0001-extension-model.md | Replaced by the revocation list carried in the signed official index; M6, not built |
| SKL-24 | Plugin language packs, VM recipes and agent profiles | later | M6 | changed | docs/architecture/extensions.md | Replaced by `locales`, `recipes` and `agentAdapters` contribution kinds; M6, not built |
| SKL-25 | Bundled launch plugins | drop | M6 | not started | docs/inventory/skills-and-plugins.md | Inventory proposes drop; no decision record or owner text, so lowered to not started |
| SKL-26 | Plugin development loop | drop | M6 | changed | docs/decisions/0001-extension-model.md | Kept in a new form: dev-folder installs with `massorca ext reload` in Creator mode (M6); `massorca ext` now returns extension.not_implemented |
| SKL-27 | Marketplace page and catalog | first version | M4 | not started | crates/desktop/src/shell/canvas.rs | Marketplace rail entry and palette command open a "This page is not built yet" placeholder; no catalog in the service |
| SKL-28 | Marketplace host picker | later | M6 | not started | none | No Marketplace host picker |
| SKL-29 | Standard item pages | later | M6 | not started | none | No item pages |
| SKL-30 | Install and uninstall host programs | later | M6 | not started | none | No host program installers |
| SKL-31 | GitHub accounts on the Marketplace page | later | M6 | not started | none | No gh account management |
| SKL-32 | Engram memory connection for agents | later | M6 | not started | docs/architecture/extensions.md | No Engram connection; planned as core-owned `mcpServers` and `skills` data contributions |
| SKL-33 | Engram cloud sync and memory import | later | M6 | not started | none | No Engram sync or import |
| SKL-34 | OpenMetadata connection for agents | later | M6 | not started | docs/architecture/extensions.md | No OpenMetadata connection; planned as core-owned `mcpServers` entries |
| SKL-35 | Service connections from the Marketplace | later | M6 | not started | crates/desktop/src/model/settings_sections.rs | Only disabled Connect rows in Settings Sources; no Services section or backing |
| SKL-36 | Who can use each app | later | M6 | not started | none | No per-app agent or bot access control |
| SKL-37 | Credential vault for app and service tokens | later | M6 | not started | docs/decisions/0071-secret-vault.md | Vault design accepted (write-only, injected into agents at launch) but no secret.set or store on main; Settings vault row is planned only |
| SKL-38 | Archify skill from a pinned release | later | M6 | not started | none | No Archify install |
| SKL-39 | MassOrca board skills and built-in commands | later | M6 | not started | none | No board skills or Prototype or Orchestration delegate commands |
| SKL-40 | Skill copies, agent switches and team access | later | M6 | not started | none | No skill pages, agent switches or team access |
| SKL-41 | Skill commands in the CLI for any host | later | M6 | not started | none | No skill CLI commands |
| SKL-42 | Marketplace actions in the CLI | later | M6 | not started | crates/cli/src/ext.rs | No orca marketplace commands; the reserved `massorca ext` group fails with extension.not_implemented until M6 |
| SKL-43 | Marketplace audit | later | M6 | not started | none | No Marketplace audit |
| SKL-44 | Marketplace on the phone | later | M6 | not started | none | No phone client on main |
| CLI-01 | Put orca on PATH and inside app terminals | first version | M1 | partial | MASSORCA_BIN env; crates/core-agents/src/launch.rs | Agent runs get the bundled binary path (hooks use it); no PATH install of a stable command per OS. |
| CLI-02 | Use the CLI from WSL | later | M6 | not started | none | No WSL bridge to the Windows CLI. |
| CLI-03 | Reach the running app: local connection, status and open | first version | M1 | partial | `service status` / `service info`, autostart; crates/cli/src/connect.rs | Commands reach the local service and auto-start it; no `open` command that launches the desktop app. |
| CLI-04 | Run a headless server (serve) | first version | M2 | in review | PR 70, PR 69 | `service run --listen`, `service unit` and pairing offers are in open PRs; main has no network listener. |
| CLI-05 | Route commands to a paired server | first version | M2 | in review | PR 70 | Global `--host` and `pair accept` in PR 70; no WSL distro target. |
| CLI-06 | Manage managed servers on SSH hosts | later | M6 | in review | PR 68, PR 70 | `host add-ssh` installs the server via system ssh; update, rollback and recovery not seen. |
| CLI-07 | Target SSH hosts through the relay | later | M6 | not started | none | No relay of commands to SSH targets on main or in open PRs. |
| CLI-08 | Output, help, errors and agent discovery | first version | M1 | partial | `--json` envelope, exit codes; crates/cli/src/output.rs | JSON envelope, error codes and help exist; command descriptors are internal (crates/server/src/commands/mod.rs), no machine-readable schema exposed. |
| CLI-09 | Worktrees and file tabs | first version | M1 | partial | `workspace create/list/show/rm`, `agent list/forget`; crates/cli/src/workspace.rs | No `current`, no file or diff tabs, no browser-pane open. |
| CLI-10 | Repositories and projects | first version | M1 | partial | `project add/list/show/rm`; crates/cli/src/project.rs | Base ref only per create (`--base`); no per-repo base ref setting, no clone, no remote-host folder. |
| CLI-11 | Terminals | first version | M1 | partial | `terminal create/list/show/send/screen/attach/kill/close`; crates/cli/src/terminal.rs | No wait-on-terminal command. |
| CLI-12 | Orchestration: runs and tasks | first version | M3 | not started | none | No orchestration runs or tasks. |
| CLI-13 | Orchestration: messages, questions and gates | later | M6 | not started | none | No orchestration messages, questions or gates. |
| CLI-14 | Orchestration: supervised workers and dispatch | first version | M3 | partial | `agent start/status --watch/stop/forget`; crates/cli/src/agent.rs | Workers can be started, watched and stopped; agent.delegate exists in protocol but has no CLI; no task dispatch. |
| CLI-15 | Browser tabs | first version | M4 | not started | none | No browser tab commands. |
| CLI-16 | Browser page control | later | M6 | not started | none | No browser page control. |
| CLI-17 | Browser profiles and identity | later | M6 | not started | none | No browser profiles. |
| CLI-18 | Browser cookies, storage, capture and device settings | later | M6 | not started | none | No browser cookies, storage or capture. |
| CLI-19 | Desktop computer use | later | M6 | not started | none | No computer-use commands. |
| CLI-20 | Mobile emulators and simulators | later | M6 | not started | none | No emulator or simulator commands. |
| CLI-21 | Linear issues | later | M6 | not started | none | No Linear commands (0001 lists trackers only as a possible extension kind). |
| CLI-22 | Artifacts | later | M6 | not started | none | No artifact sharing. |
| CLI-23 | Automations | first version | M3 | not started | none | No automations. |
| CLI-24 | Agent accounts | first version | M3 | partial | agent.credentials.import/show/remove; crates/protocol/src/methods/agents.rs | Service methods exist but no CLI subcommand; no usage, Codex reset credit or OpenCode/Devin profiles. |
| CLI-25 | Agent status hooks | first version | M3 | changed | docs/decisions/0053-agent-status-sources.md | Hooks are installed per run, never into global agent configs; built in crates/core-agents/src/launch.rs. No enable/disable command needed. |
| CLI-26 | Claude Code agent teams bridge | later | M6 | not started | none | No Claude Code agent-teams tmux bridge. |
| CLI-27 | Agent session search | later | M6 | not started | none | No session search. |
| CLI-28 | Skills commands | later | M6 | not started | none | No skills commands. |
| CLI-29 | Per-workspace environment recipe check | later | M6 | not started | none | No environment recipe check. |
| CLI-30 | Recovery and diagnostics | later | M6 | not started | none | No profile rollback or memory snapshot. |
| CLI-31 | Marketplace commands | later | M6 | not started | none | No Marketplace commands. |
| CLI-32 | Bot Teams commands | first version | M3 | not started | none | No Bot Teams CLI; desktop Teams uses sample data; PR 118 is a docs decision only. |
| CLI-33 | Prototypes commands | later | M6 | not started | none | No Prototypes commands. |
| CLI-34 | Jev commands | later | M6 | not started | none | No Jev commands. |
| CLI-35 | Folder workspace commands | first version | M1 | partial | `project add --folder`, `workspace create/list/rm`; crates/cli/src/workspace.rs | Folder workspaces can be added, listed and forgotten; no update or check command. |
| CLI-36 | Theme commands | first version | M4 | not started | none | `config get/set` exist but the only registered key is workspaces.default_root; no theme setting. |
| CLI-37 | Chat results | later | M6 | not started | none | No chat results command. |
| CLD-01 | Phone-to-host relay splice | later | M6 | not started | none | No relay splice. Owner default: relay not in first version (docs/inventory/first-version.md). |
| CLD-02 | Pairing invites and resumable device credentials | later | M6 | changed | docs/decisions/0041-device-identity-and-pairing.md | One-time offer plus per-device certificate with no expiry replaces invites and rotating 30-day credentials. In review (PR 66, 69, 70). |
| CLD-03 | Host identity proof | later | M6 | not started | none | No hosted service; the host key proof exists only toward paired clients (PR 67 pins). |
| CLD-04 | Cloud sign-in and relay tokens | drop | M6 | not started | none | Proposed drop, but no decision record drops it. |
| CLD-05 | Director and cell assignment | drop | M6 | not started | none | Proposed drop, but no decision record drops it. |
| CLD-06 | Regions and region correction | drop | M6 | not started | none | Proposed drop, but no decision record drops it. |
| CLD-07 | Admission control and overload shedding | later | M6 | not started | none | No relay admission control. |
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
| PLAT-08 | Startup preflight and tool detection | first version | M4 | partial | agent kinds (agent.kinds installed flag), crates/core-agents/src/resolve.rs | Agent CLI and git version detection exist; no single preflight status, no forge CLI or shell checks, no login-shell PATH probe; desktop Setup shows no tool checks |
| PLAT-09 | Profile storage compatibility guard | drop | M6 | not started | - | No downgrade guard; inventory proposes drop, not confirmed by a decision record |
| PLAT-10 | Main window lifecycle and close confirmation | first version | M4 | partial | Title bar window controls, crates/desktop/src/shell/title_bar.rs | Own title bar with minimise, maximise, close per owner direction (title bar kept); no close confirmation; service runs detached so it survives the window |
| PLAT-11 | Renderer recovery and GPU fallback | drop | M6 | not started | - | No renderer recovery or GPU fallback (proposed drop, not decided) |
| PLAT-12 | Tray and Dock attention indicators | later | M6 | not started | - | No tray or Dock badge |
| PLAT-13 | Application menu | later | M6 | not started | - | No native application menu |
| PLAT-14 | Safe quit and durable state writes | first version | M1 | partial | service.shutdown (drains, closes store), crates/server/src/features/service.rs | Service stop is graceful (0003) and state is in SQLite; desktop quit has no bounded teardown and there are no atomic temp-fsync-rename writes for other files |
| PLAT-15 | OS file opening and orca:// links | later | M6 | not started | - | No OS file associations or URL scheme |
| PLAT-16 | Crash dumps and Report Crash | later | M6 | not started | - | No crash dumps or Report Crash |
| PLAT-17 | Opt-in telemetry | drop | M6 | not started | - | Telemetry is a disabled Settings row only (proposed drop, not decided) |
| PLAT-18 | Local trace log and redaction | first version | M1 | partial | massorca service run daily logs under MASSORCA_LOG, crates/cli/src/logging.rs | Service log rotated daily, 7 kept, local only (0006); redaction is by convention (no Debug of secrets); desktop writes no log; frame-dump guard in PR 85 |
| PLAT-19 | Diagnostic bundle upload | later | M6 | not started | - | No diagnostic bundle |
| PLAT-20 | Main-thread hang watchdog | drop | M6 | not started | - | No hang watchdog (proposed drop, not decided) |
| PLAT-21 | Star prompt | drop | M6 | not started | - | No star prompt (proposed drop, not decided) |
| PLAT-22 | Keep computer awake while agents run | later | M6 | not started | - | No sleep prevention; Settings row disabled |
| PLAT-23 | macOS platform integration | later | M6 | not started | - | Only the macOS traffic-light position is set in crates/desktop/src/main.rs; no key repeat, privacy guidance or window workaround |
| PLAT-24 | Windows shell, process and tree handling | first version | M4 | partial | terminal create (pwsh then powershell, Job object tree kill), crates/core-terminal/src/shell.rs | PowerShell detection and process-tree termination built (0031, 0032); Git Bash detection and the Windows PATH registry merge are not (0032 defers PATH merge) |
| PLAT-25 | Linux platform integration | later | M6 | not started | - | Linux only gets app_id and client decorations; no credential store choice or virtual display |
| PLAT-26 | Headless serve startup and supervisor handoff | later | M6 | in review | PR 69 (network listener), PR 70 (service unit) | main has massorca service run (local socket only); --listen and the systemd unit are in open PRs 69 and 70; supervisor update handoff not started |
| PLAT-27 | CLI on PATH and launch redirect | first version | M1 | partial | massorca binary is CLI and service (service run), crates/cli/src/service/mod.rs | One binary for CLI and service (0003); desktop is a separate massorca-desktop binary (0023); nothing puts the command on PATH |
| PLAT-28 | Main-thread responsiveness breadcrumbs | later | M6 | not started | - | No event-loop lag or unresponsive-window breadcrumbs |
| PLAT-29 | Visible product name MassOrca | first version | M4 | done | Window title MassOrca, crates/desktop/src/main.rs | Visible name is MassOrca in window and docs; note the command is massorca, not orca (0003 renames it) |
| PLAT-30 | Desktop shell: activity rail, side panel and session bar | first version | M4 | partial | Activity rail (Tasks, Calendar, Settings, theme, usage rings), crates/desktop/src/model/rail.rs | Rail and title bar built (owner kept the title bar); Tasks and Calendar are placeholders; usage rings on sample data; Work panel terminals in PR 110, no browser tabs |
| SET-01 | Settings read, change and push | first version | M1 | partial | massorca config get/set/list, config.set in crates/server/src/features/config.rs | Works for one key (workspaces.default_root), desktop folder row reads it; no config.changed push, no config.reset, 0070 keys and settings CLI name not built |
| SET-02 | Persistent state across restarts | first version | M1 | partial | SQLite store, crates/core-store/src/migrations.rs | Projects, workspaces, settings and agent sessions persist; desktop saves no UI state (side panel, rail mode, layout) |
| SET-03 | Schema versions and rolling backups | first version | M1 | done | automatic on store open, crates/core-store/src/migrations.rs and crates/core-store/src/backup.rs | user_version migrations, newer schema refused (store.newer_schema), VACUUM INTO backups, five kept |
| SET-04 | Sealed secrets on the device | first version | M1 | not started | docs/decisions/0071-secret-vault.md | Vault accepted (0071) but no secrets table, cipher or secret.* methods on main or in an open PR |
| SET-05 | Profiles: list, create, switch | later | M6 | partial | --profile / MASSORCA_PROFILE, crates/profile/src/profile.rs | Named profiles with isolated state exist (0004); no list, create or switch command or UI |
| SET-06 | Cloud account per profile | later | M6 | not started | none | No cloud account per profile |
| SET-07 | Move projects and sessions between profiles | later | M6 | not started | none | No moving projects between profiles |
| SET-08 | Import the Electron app's profile state | first version | M3 | not started | none | No Electron profile import; nearest design is 0056 credential import, which is not served |
| SET-09 | Rollback to JSON and JSON exports | later | M6 | not started | none | No JSON export or rollback |
| SET-10 | Active view remembered | later | M6 | not started | none | Desktop persists no state, so the active view is not remembered |
| SET-11 | Default keyboard shortcuts | first version | M4 | partial | palette commands and key bindings, crates/desktop/src/model/palette.rs and crates/desktop/src/shell/actions.rs | 9 palette commands and 12 bindings, few with shown shortcuts; no 93-action catalogue |
| SET-12 | Custom shortcuts per OS | later | M6 | changed | docs/decisions/0070-settings-the-desktop-needs.md | 0070 replaces the JSON file with one validated input.keybindings setting; not built (only workspaces.default_root declared) |
| SET-13 | Interface language | later | M6 | not started | none | No interface language choice |
| SET-14 | File explorer listing | later | M6 | not started | none | No file listing method; ui FileTree component exists in the gallery only |
| SET-15 | Create, rename, copy and delete | later | M6 | not started | none | No file create, rename, copy or delete |
| SET-16 | Built-in editor: open and save | later | M6 | not started | none | No built-in editor |
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
| SET-27 | App icon choice and Dock icon | drop | M6 | not started | none | Proposed drop in docs/inventory/README.md, no decision record; nothing built |
| SET-28 | Import terminal themes from Ghostty and Warp | later | M6 | not started | none | No Ghostty or Warp theme import |
| SET-29 | Secret vault with scopes, links and in-chat requests | first version | M4 | not started | docs/decisions/0071-secret-vault.md | 0071 accepted, not built; only the ui SecretRequestCard component exists in the gallery |
| SET-30 | App light and dark themes | first version | M4 | partial | Toggle theme in rail and palette, crates/desktop/src/shell/mod.rs | Light and dark toggle works in memory; starts dark, not persisted; appearance.theme setting (0070) and CLI not built |
| CUS-01 | orca.yaml setup and archive scripts with per-script trust | first version | M1 | not started | docs/decisions/0009-phase1-deferrals.md | No setup or archive scripts or per-script trust; setup scripts deferred to a later phase by 0008 and 0009 |
| CUS-02 | orca.yaml extended keys | later | M6 | not started | docs/decisions/0009-phase1-deferrals.md | No orca.yaml keys; extensions.md plans recipes as a `recipes` contribution kind |
| CUS-03 | DeepSeek Harness agent integration | later | M6 | changed | docs/architecture/extensions.md | DeepSeek Harness becomes a first-party `agentAdapters` extension at M6 (section 10 and 12); not built, though a user kind file could declare it today |
| CUS-04 | Per-agent command, argument and environment overrides | first version | M3 | partial | agent.kinds, crates/core-agents/src/catalog.rs | A profile can declare extra kinds in agents/kinds/*.json (program, args); built-ins cannot be overridden (0050), no per-agent env, disable, default agent or Settings UI |
| CUS-05 | Declarative agent-state rules with live update and local override | later | M6 | not started | docs/decisions/0050-agent-kinds.md | Status comes from a closed set of Rust mechanisms named in kind JSON; declarative rules with live update and local override not built (`agentRules` kind planned) |
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
| CUS-11 | MCP server entries written into agent configs | later | M6 | not started | docs/architecture/extensions.md | No MCP config writer; planned as core-owned `mcpServers` contributions |
