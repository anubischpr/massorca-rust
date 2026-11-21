# Command-line interface (`orca`)

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits), read with read-only git from worktree `unified-custom-profile-build-547ea6`. The earlier pass read worktree `massorca-server-language-820c84`.

The MassOrca command-line interface, invoked as `orca`, is a client of the running MassOrca runtime. At pv96 it exposes 346 command paths (251 at custom/integration), in the groups below plus seven new ones (CLI-31 to CLI-37). The earlier claim that the spec table and the dispatch table match one-to-one was not re-checked at pv96; the earlier count was 251 command paths in 32 handler groups (23 groups plus 9 browser sub-groups). Most commands send one request to the runtime and print the answer as text or, with `--json`, as one envelope. The CLI itself owns three things: putting itself on the user's PATH, choosing which runtime to talk to, and running a headless server (`serve`). Agents that run inside MassOrca terminals are the second main audience: they resolve the executable for their session, prefer `--json`, and load version-matched guides that the CLI serves. Two entry points sit outside the 251-command table: `--version` and `-v`, and the hidden `agent-teams-tmux` shim.

## Source directories read (repo-relative, reference repository)

- `src/cli/`: `index.ts`, `dispatch.ts`, `args.ts`, `command-spec.ts`, `handler-group-manifest.ts`, `browser-handler-groups.ts`, all of `specs/` (command tables), `handlers/` (file names, selected bodies), `runtime/` (client, launch, metadata, status, transport, websocket transport, environments, remote pairing, serve launch and update supervisor), and the host, error and vocabulary helpers (`execution-host-flag.ts`, `host-selector-alternatives.ts`, `omitted-host-scope-selectors.ts`, `ssh-bridge-command-policy.ts`, `session-caller-flags.ts`, `cli-error.ts`, `command-suggestion.ts`, `vocabulary-policy.ts`, `agent-context.ts`, `selectors.ts`).
- `src/main/cli/`: installer, install location, command installation, PATH registration, Windows user PATH registry, privileged command runner, development launcher, Linux bare-`orca` dispatcher, AppImage handling, PTY child PATH, WSL installer, WSL scripts and WSL status.
- `native/windows-cli-launcher/` (`src/main.rs`, `build.rs`, `Cargo.toml`).
- Contracts the CLI depends on, read for names, defaults and gates only: `src/shared/cli-argument-boundary.ts`, `src/shared/runtime-bootstrap.ts`, `src/shared/agent-hook-types.ts`, `src/shared/agent-skill-sharing-gate.ts`, `src/shared/artifact-sharing-gate.ts`, `src/shared/default-global-settings.ts`, `src/shared/agent-feature-install-commands.ts`, `src/shared/runtime-environment*.ts`, `src/shared/orcad-local-serve-selection.ts`, `src/shared/managed-orcad-execution-host.ts`.
- Agent-facing text: `skill-guides/orca-cli.md` (section headings and command references), `skill-guides/orchestration.md`, `skill-guides/computer-use.md`, `skill-guides/orca-emulator*.md`, `skill-guides/orca-linear.md`, `skill-guides/linear-tickets.md`, `skill-guides/orca-per-workspace-env.md`, and `skills/orca-cli/SKILL.md`.
- Counts were produced by a read-only script over `src/cli/specs/` and `src/cli/handler-group-manifest.ts`.

pv96 delta (`git diff custom/integration d3c6311a531`, read by name-status, numstat, `git show` and `git grep` on the commit): `src/cli/specs/` and `src/cli/handlers/` (new and changed files for the groups in CLI-31 to CLI-37 and the changed features), read as command paths in the spec files and as commit subjects; `src/cli/execution-host-flag.ts` (diff: the `wsl:` host parser); `src/cli/handlers/terminal-close.ts` (diff: one export); `src/cli/handlers/account.ts` (diff); `src/cli/root-help-text-primary.ts` (header); `odd/tasks/cli-write-commands.md` (in full: write-command rules S1 to S5 and the task record).

## Not read / low confidence

- Not read in detail (only spec summaries, flag counts and notes): handler bodies for the browser groups, computer, emulator, Linear, automations, project, repo, terminal, orchestration (runs, messages, workers, gates), search, worktree create and set, and file commands.
- Root-level `src/main` files: matched by name only. `src/main/wsl-managed-cli-path-restore.ts` is the WSL CLI path restore and was not read; `agent-cli-version-probe.ts` concerns third-party agent CLIs, not `orca`.
- Not read: `root-help-text-*.ts` and `help.ts`; the test files (names only); WSL status probes and repair reconciliation (headers and comments only); AppImage extraction and pruning (names and comments only); account login flows beyond the first branch.
- Not read: the native helpers under `native/computer-use-*`, `native/keyboard-layout-macos` and `native/notification-status-macos` (their behaviour is inferred from command names only); the relay under `src/relay`; the SSH bridge credential code; the runtime RPC server under `src/main/runtime`; the orchestration store; the agent-browser engine; the renderer (install and status UI copy).
- Low confidence: the exact outcome when several host selectors are combined beyond what the code comments state; behaviour on Windows and WSL (static reading only, nothing was run); the trigger conditions of the Linux bare-`orca` dispatcher; flag counts (taken from spec tables, not checked against the parser).
- Repository state: the reference repository has no `.codegraph/` index. I did not create one, because the brief keeps that repository read-only, so the reading used filesystem search and file reads.
- pv96: the handler bodies of the new groups (bot-teams, marketplace, prototypes, jev, folder-workspace, theme, chat, and the orchestration team and ask-user handlers) were read through spec summaries, file names and commit subjects only. Their owner-only and SSH-relay rules were not checked. `src/cli/ssh-bridge-command-policy.ts` did not change at pv96 (checked with the name-status list).
- pv96: `src/cli/execution-host-flag.ts` was read as a diff; the other `--host` consumers were not traced.
- pv96: the help text was checked for the MassOrca wording only.

## Overview

### Install and PATH by operating system

| OS | Packaged command | Mechanism | Development build |
|---|---|---|---|
| macOS | `/usr/local/bin/orca`, or `~/.local/bin/orca` when `/usr/local/bin` is absent (Apple Silicon) | Symlink to the launcher bundled in the app; created with an administrator prompt | `orca-dev` at `/usr/local/bin/orca-dev` |
| Linux | `~/.local/bin/orca-ide` (not `orca`, which GNOME uses for its screen reader) | Symlink to the bundled launcher; AppImage builds link a stable launcher from an extracted cache | `orca-dev` at `~/.local/bin/orca-dev` |
| Windows | `orca.exe` (native launcher) in the app's `resources\bin` folder | That folder is added to the user PATH; a new shell may be needed | `orca-dev.cmd` in `%LOCALAPPDATA%\Programs\Orca Dev\bin`, plus PATH entry |
| WSL | Managed `orca` command inside each distribution | Forwards each call through PowerShell interop to the Windows CLI | Not applicable |
| Inside app terminals | Works without any global install | The app prepends its own launcher folder to PATH for terminals and child processes | Dev bin folder under the app data directory |

### How the CLI finds and reaches MassOrca

Resolution order, as the code applies it:

1. Local-only groups ignore every remote selector: `account`, `artifacts`, `environment`, `host list`, `serve`, `agent` (hooks), `vm`, `agent-context` and `profile`.
2. An explicit `--host` on the few commands that accept it (`worktree create`, the `project setup` commands, `automations create` and `edit`) picks the target: `local`, `ssh:<target>` (resolved by the running app), or `runtime:<environment>` (resolved against saved servers by id or name).
3. Otherwise `--environment` (or `ORCA_ENVIRONMENT`) selects a saved paired server. `--pairing-code` (or `ORCA_PAIRING_CODE`, or `ORCA_REMOTE_PAIRING`) connects once without saving. Both together are refused. An explicit `--host runtime:` outranks the ambient variables, but a disagreeing `ORCA_ENVIRONMENT` is refused rather than silently retargeting a mutation.
4. With no remote selector, the CLI reads the runtime metadata file (`orca-runtime.json`) from the app data folder and talks to the local socket or named pipe with the token in that file. `ORCA_USER_DATA_PATH` overrides the folder for development and parallel instances.
5. Remote calls use a paired WebSocket session. When the CLI runs through the SSH relay, the owner-only commands are refused, and so are `--environment`, `--pairing-code` and `--host runtime:` (see CLI-07).

### Command groups at a glance

| Group | Commands | Main audience | Feature |
|---|---|---|---|
| core: `open`, `status`, `serve` | 3 | Agents (`open`, `status`); owner (`serve`) | CLI-03, CLI-04 |
| core: `claude-teams` | 1 | Agents (Claude Code) | CLI-26 |
| agent-context | 1 | Agents | CLI-08 |
| environment and host | 6 | Owner | CLI-05 |
| managed server (`environment status`, `update`, `rollback`, `recover`, `stop`, `cancel-stop`, `forget`) | 7 | Owner | CLI-06 |
| account | 6 | Owner | CLI-24 |
| worktree | 9 | Agents and owner | CLI-09 |
| file | 3 | Agents | CLI-09 |
| repo and project | 13 | Owner, agents (read) | CLI-10 |
| terminal | 11 | Agents | CLI-11 |
| orchestration | 37 | Agents (coordinators and workers) | CLI-12, CLI-13, CLI-14, CLI-32 (team commands) |
| browser, 9 sub-groups | 80 | Agents | CLI-15 to CLI-18 |
| computer | 14 | Agents | CLI-19 |
| emulator | 16 | Agents and owner | CLI-20 |
| linear | 27 | Agents and owner | CLI-21 |
| artifacts | 5 | Agents and owner | CLI-22 |
| automations | 7 | Owner | CLI-23 |
| agent-hooks | 4 | Owner (app invokes `prepare-codex`) | CLI-25 |
| search | 1 | Agents and owner | CLI-27 |
| skills and skill-sharing | 7 | Agents (`get`, `list`, `picker`); owner (install, update, share) | CLI-28 |
| vm | 1 | Agents and owner | CLI-29 |
| profile-state and diagnostics | 3 | Owner | CLI-30 |
| bot-teams (team, bot, room, task, routine, memory, webhook and file commands) | 40 | Owner and bots | CLI-32 |
| marketplace (with `github accounts`) | 17 | Owner | CLI-31 |
| prototypes | 14 | Agents and owner | CLI-33 |
| jev | 5 | Owner | CLI-34 |
| folder-workspace | 5 | Agents and owner | CLI-35 |
| theme | 2 | Both | CLI-36 |
| chat (results) | 1 | Both | CLI-37 |

Total: 346 command paths at pv96 (251 at custom/integration). Other facts from the specs: 33 commands are flagged destructive at pv96 (15 at custom/integration); `terminal stop` is hidden and deprecated; `coordinator-start` and `coordinator-stop` are retired and only print guidance; 11 command prefixes are owner-only and refused over the SSH relay (account, agent hooks, claude-teams, environment, host list, profile, serve, skills install, skills update, skills share, vm). The SSH-relay rules for the groups added at pv96 were not checked.

Meant for agents: `agent-context`, `skills get` and `skills list`, `open` and `status` (the guide tells agents to start the app with `open --json` when it is not running), worktree and file commands, terminal commands, orchestration, the browser, computer, emulator, Linear and search commands, `artifacts share` and `update`, and `vm recipe doctor`. Owner-only: account, environment and host, managed servers, `serve` as an operator command, profile state, diagnostics, automations, and the install, update and share actions of skills.

## Features

### CLI-01 Put `orca` on PATH and inside app terminals

- **Audience:** owner (installs from the desktop app); agents depend on the result.
- **What happens:** The packaged desktop app offers a registration action and shows its state (installed, not installed, stale, conflict or unsupported) and whether the command's folder is on PATH. On macOS it links `/usr/local/bin/orca` (or `~/.local/bin/orca`) to the launcher bundled in the app, using an administrator prompt. On Linux it links `~/.local/bin/orca-ide`. On Windows it adds the bundled `resources\bin` folder, with the native `orca.exe` launcher, to the user PATH.
- **Rules and edge cases:** A command that MassOrca does not own is never replaced; the install refuses and asks the user to remove it first. Only a managed legacy Linux `orca` link is reclaimed. Registration and removal are serialized. If Windows policy blocks PATH writes, the app shows the folder to add by hand. Linux AppImage builds extract the payload into a cache and link a stable launcher, pruning older extracted copies. Development builds register `orca-dev` only, so they never claim the production name.
- **Inside app terminals:** For every terminal and child process, the app prepends its own launcher folder to PATH (bundled folder on macOS and Windows, a bare-`orca` shim folder on Linux, the dev bin folder in dev builds). Agents launched by MassOrca therefore reach the bundled build even without a global install. The app also exports the resolved launcher path and the profile path to those processes.
- **Core and data:** No runtime call. Persists a symlink or wrapper file, a Windows user PATH entry, and launcher folders under the app data directory for dev builds and shims.
- **Depends on:** desktop app packaging and updates (app-platform domain); CLI-02 for WSL.

### CLI-02 Use the CLI from WSL

- **Audience:** both; agents running in WSL panes need it to reach the same app.
- **What happens:** When a WSL distribution is present, the desktop app can install a managed `orca` command inside it. Each call is forwarded through PowerShell interop to the Windows MassOrca CLI, so the distribution talks to the same app and profile. Status uses the same install states as the host command (installed, not installed, stale, conflict, unsupported).
- **Rules and edge cases:** Requires Windows interop (`powershell.exe` reachable from the distribution); without it, status says so. A user-owned `orca` in the distribution is never replaced, and the legacy managed name is recognized and migrated. Repair recreates the launcher and bridge when they drift. The bridge can be pinned to one app instance. Inside WSL sessions the app exports `ORCA_CLI_COMMAND` with the managed command.
- **Core and data:** Files in the distribution: a launcher under `~/.local/bin` and a PowerShell bridge under `~/.local/share/orca`. Registration state is tracked on the Windows side.
- **Depends on:** CLI-01; WSL environment support (name only, another domain).

### CLI-03 Reach the running app: local connection, `status` and `open`

- **Audience:** both.
- **What happens:** Local commands read `orca-runtime.json` from the app's profile folder: `~/Library/Application Support/orca` on macOS, `%APPDATA%\orca` on Windows, `$XDG_CONFIG_HOME/orca` or `~/.config/orca` on Linux. They then send one request over the app's local socket (macOS, Linux) or named pipe (Windows), authenticated with the token in that file.
- **`status`** reports whether the app runs (process id), runtime readiness (ready, graph not ready, starting), connection state, runtime id, app version, capabilities and degradations. Missing metadata means "not running"; metadata whose process has died means "stale".
- **`open`** starts the app and waits up to 15 seconds for a desktop window. It uses the platform's launcher (`open` on macOS, the app executable elsewhere); an override command can be set. If the app is running headless because its persistent terminal provider is unavailable, `open` refuses, asks the user to quit and restart normally, and does not force a second instance.
- **Rules and edge cases:** `runtime_unavailable` tells the user to start the app. A sandbox that blocks the socket returns an access-denied error, which is not evidence that the app is dead. Timeouts: 60 seconds by default, 1 second for status probes, and long waits add a 10-second grace. Orchestration mutations first check that the runtime advertises the orchestration contract; an older runtime returns `orchestration_migration_required` and no effect is applied.
- **Core and data:** Reads metadata only; persists nothing.
- **Depends on:** desktop app runtime (app-platform domain); CLI-08 for error envelopes.

### CLI-04 Run a headless server (`serve`)

- **Audience:** owner (operator); remote clients depend on it.
- **What happens:** Runs the MassOrca runtime in the foreground without a window, then prints the bound endpoint, the address advertised to clients, and the pairing status. Ctrl+C stops it. Options: port; advertised address (for a LAN, VPN, SSH forward or reverse proxy); no pairing; a mobile-scoped pairing link or QR; a desktop-control grant; and a project root with recipe output, which prints a VM recipe result as JSON and keeps the server running. When a web client bundle exists, it also prints a browser URL with the pairing data embedded.
- **Rules and edge cases:** A paired client cannot drive this machine's desktop (CLI-19) unless the pairing offer was created with the desktop-control grant. The headless runtime ("orcad") is the default; `ORCA_SERVE_RUNTIME=electron` keeps the older Electron-based serve. The serve command uses the profile the caller selected. On macOS, an in-place app update that happens while serving is handled by a supervisor that resumes an interrupted handoff.
- **Core and data:** Starts the runtime on the selected profile; persists that profile's data and pairing records.
- **Depends on:** CLI-05 (clients pair with it); CLI-06 (managed servers run this same runtime over SSH); server-and-pairing domain.

### CLI-05 Route commands to a paired MassOrca server

- Source: pv96 (unmerged). `--host wsl:<distro>` is accepted by the Marketplace GitHub-account and skill-host commands, and a paired server can be audited from this computer's app with `--host runtime:<id>`.

- **Audience:** owner sets it up; agents and scripts use the flags when told a target.
- **What happens:** `environment add --name <name> --pairing-code <code>` saves a paired server from a pairing offer (an `orca://pair` link or the bare payload) in the local environment store, `orca-environments.json`, in the profile folder. Later commands accept `--environment <name or id>` to run against it. `environment list`, `show` and `rm` manage saved servers; `rm` removes only the local record, and the server keeps running (stopping it is CLI-06). `host name` sets the name this runtime reports to clients. `host list` lists every machine this host can target and how to name it: saved servers from the local store and SSH targets from the running app.
- **Rules and edge cases:** `--environment` with `--pairing-code` is refused. `--environment` accepts a name or an id; `runtime:` ids match by id only. Ambiguous names are refused, and an unknown name fails with close matches and next steps instead of an empty result. A successful remote call records the server's last-used time. Before mutations, the CLI checks that the remote runtime supports the orchestration contract. The connection is a paired WebSocket session.
- **Core and data:** Writes `orca-environments.json` (saved records, pairing offer data, last-used time). The local metadata file is read-only.
- **Depends on:** server-and-pairing domain; remote-hosts domain; CLI-07 for SSH targets.

### CLI-06 Manage managed servers on SSH hosts

- **Audience:** owner, desktop only.
- **What happens:** For an SSH host, the desktop app can install and run the headless runtime as a managed server. `environment status` shows the active version, running terminals and pending work. `environment update` moves the server to this app's build; it is deferred if it would restart running terminals, unless `--force` is given. `environment rollback` returns to the previous version. `environment recover` finishes or undoes an interrupted update, rollback or stop; restoring the pre-launch profile snapshot needs `--accept-changed-state --yes`. `environment stop` stops the server on the host and unlinks it from this machine; it needs `--yes` and is refused if the host cannot prove the process exited. `environment cancel-stop` withdraws a stop that the server has not acted on yet. `environment forget` unlinks a server whose host is gone, without contacting the host and without stopping anything there, so the process may still run.
- **Rules and edge cases:** Runs only on the desktop app; a headless runtime refuses it. Owner-only over the SSH relay. An active server must be switched away from before it can be stopped.
- **Core and data:** Managed-server links are kept by the desktop registry; the host keeps its own process state.
- **Depends on:** CLI-07 (SSH targets); CLI-04 (same runtime); remote-hosts domain.

### CLI-07 Target SSH hosts through the relay (`--host ssh:`)

- **Audience:** both; agents in SSH terminals are the relay's users.
- **What happens:** On the commands that take `--host`, `--host ssh:<target>` asks the connected app to act on an SSH target it owns. `--host local` targets this machine. An unknown target fails with an error; it never silently matches nothing.
- **Rules and edge cases:** SSH targets are created and connected only in the desktop UI, because the desktop owns SSH connections. When the CLI is invoked through the SSH relay bridge, owner-only commands are refused with a message to run them on the MassOrca host, and `--environment`, `--pairing-code` and `--host runtime:` are refused because they would route through the host's own paired servers. The relayed call presents a scoped credential, not the owner token. `ORCA_CLI_CWD` carries the remote working directory so that `--worktree active` resolves there. The cwd-based selectors `active` and `current` work only against the local runtime; remote calls need explicit selectors.
- **Core and data:** None local; SSH target records live in the connected app.
- **Depends on:** remote-hosts domain; CLI-05; CLI-08 for selectors.

### CLI-08 Output, help, errors and agent discovery

- Source: pv96 (unmerged). Help and root text say MassOrca while the command stays `orca`, and the root help lists the new groups.

- **Audience:** agents (primary); owner.
- **What happens:** `--json` makes a command print one envelope. Success carries `ok: true`, the result and the runtime id. Failure carries `ok: false` with an error code, a message and data. Without `--json` the output is human text. `--help`, `help <command>`, `--version` and `-v` work. Unknown commands and flags get a short list of close matches; destructive commands are never suggested for a benign typo. Syntax and flag errors are reported before any connection is attempted. `orca agent-context --json` prints the machine-readable command schema (schema version 1: path, aliases, summary, usage, flags, positional arguments, examples, notes), without hidden commands. The human form prints only a count and points to `--json`.
- **Rules and edge cases:** Vocabulary policy: delete verbs are `rm` and single-item reads are `show`; older names remain as aliases (for example `worktree remove`, `account remove`, `terminal focus`, `skills show`). Worktree selectors are `active` and `current` (cwd-based, local only) or explicit forms (`path:`, `id:`, `name:`, `branch:`, `identity:`). A `selector_not_found` error includes recovery hints. Identity flags (`--from`, `--terminal`) name the caller or a target, depending on the command; inside an agent session, a caller flag that names another identity is refused before dispatch. The global flags (`--json`, `--help`, `--environment`, `--pairing-code`) are shared by about 240 of the 251 specs.
- **Core and data:** None; the envelope shape is shared with the runtime.
- **Depends on:** every command feature.

### CLI-09 Worktrees and file tabs

- Source: pv96 (unmerged). `worktree sessions` lists the chats and terminal agents under a worktree, `worktree session-close` closes one by the id it printed, and `file open --preview` renders an HTML file in the browser pane.

- **Audience:** agents (primary); owner.
- **Commands (12):** `worktree list`, `show`, `current`, `create`, `set`, `ps`, `rm`, `sessions`, `session-close`; `file open`, `file diff`, `file open-changed`.
- **What happens:** `worktree create` makes a MassOrca-managed git worktree for a repository or project, with optional agent, prompt, base ref, setup mode (run, skip or inherit), a linked tracker issue or merge request, and a host (`--host` or `--project-host-setup`). `list`, `show` and `current` (the worktree that contains the working directory) read them. `ps` gives a compact orchestration summary across worktrees. `set` edits metadata, links, comments and unread state. `rm` removes the worktree from MassOrca and from git, and is flagged destructive. The file commands open a file, a diff, or all git-changed files as tabs in a worktree without moving the user's view unless `--focus` is given.
- **Rules and edge cases:** Plain-folder workspaces are created through project setup (CLI-10), not through `worktree create`. Remote calls need explicit selectors. Visibility rules for non-MassOrca worktrees still apply (CLI-10).
- **Core and data:** Worktree metadata is owned by the runtime; the git worktree itself lives in the repository.
- **Depends on:** CLI-10 (repositories and projects); CLI-11 (terminals); CLI-14 (`ps`); git domain (name only).

### CLI-10 Repositories and projects

- **Audience:** owner (setup); agents read them and create worktrees from them.
- **Commands (13):** `repo list`, `add`, `show`, `set`, `set-base-ref`, `search-refs`; `project list`, `setups`, `setup-existing-folder`, `setup-clone`, `setup-create`, `setup-update`, `setup-delete`.
- **What happens:** `repo add <path>` registers a local git repository by path. `repo set` chooses whether non-MassOrca worktrees appear for it. `repo set-base-ref` sets the default base for future worktrees. `repo search-refs` searches branches and tags. A project is a durable grouping. A project setup says where the project is available on a host, at a concrete filesystem path. Setups are made by importing an existing folder, cloning a repository URL, or saving independent metadata (`setup-create`); `setup-update` edits one; `setup-delete` removes one and is flagged destructive.
- **Rules and edge cases:** On a remote runtime the setup path must be absolute on that machine. `--host runtime:<id>` must name a known server; names are not accepted there. SSH target setup happens in the desktop UI. Folder-based workspaces come from existing-folder setups.
- **Core and data:** Repository, project and setup records are owned by the runtime.
- **Depends on:** CLI-05 and CLI-07 (hosts); CLI-09 (worktrees); git domain and remote-hosts domain (names only).

### CLI-11 Terminals

- **Audience:** agents (primary); owner.
- **Commands (11):** `terminal list`, `show`, `read`, `send`, `wait`, `create`, `switch`, `rename`, `split`, `close`; `stop` is hidden and deprecated.
- **What happens:** `terminal create` starts a terminal session in the current worktree. `list` and `show` list live, MassOrca-managed terminals, and `show` adds metadata and a preview. `read` returns bounded output. `send` types input into a live terminal, addressed by handle. `wait --for exit|tui-idle` blocks until the terminal exits or its interface goes idle, up to a timeout. `switch`, `rename` and `split` change the UI tabs and panes. `close` closes one terminal, its whole tab, or every terminal in a workspace; it is flagged destructive.
- **Rules and edge cases:** Terminal sessions are owned by the runtime, not by the calling process. Waits add a client grace over the requested timeout. Use `close` instead of the deprecated `stop`.
- **Core and data:** Live sessions and their scrollback are held by the runtime (terminals domain).
- **Depends on:** CLI-09 (worktrees); CLI-14 (worker terminals); terminals domain.

### CLI-12 Orchestration: runs and tasks

- **Audience:** agents (coordinators); owner (inspection).
- **Commands (12):** `orchestration run-create`, `run-use`, `run-current`, `run-list`, `run-show`, `task-create`, `task-list`, `task-update`, `request-show`, `reset`; `coordinator-start` and `coordinator-stop` are retired.
- **What happens:** A Run is a lightweight container for coordinated work. `run-create` creates one and binds the calling coordinator terminal to it; `run-use` binds to an existing Run; `run-current` shows the bound Run; `run-list` and `run-show` read Runs. Tasks belong to a Run: `task-create`, `task-list`, and `task-update` (status). `request-show` tells a caller whether an earlier mutation, identified by its request id, already took effect, which is how a lost response is resolved. `reset` clears one explicitly named orchestration state scope. The two retired commands only tell the caller to load the current orchestration skill.
- **Rules and edge cases:** Durable mutations carry a request id. When one fails, the CLI attaches a recovery command that uses `request-show`. Orchestration mutations check the runtime's contract first (CLI-03).
- **Core and data:** Run and task records are owned by the runtime's orchestration store.
- **Depends on:** CLI-11 (terminals); CLI-13 and CLI-14 (messages, workers); agents domain.
- **Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`):** orchestration is in the first version; agents drive it through these commands, and the CLI is the core test harness. The two retired coordinator commands are still not carried over. Proposal: first version (was later).

### CLI-13 Orchestration: messages, questions and decision gates

- Source: pv96 (unmerged). `orchestration ask-user` lets an agent in any chat ask the person a question through the question card (the wait behaviour was not read); the bot-team `team-*` commands are in CLI-32.

- **Audience:** agents (primary); owner resolves gates.
- **Commands (8):** `orchestration send`, `check`, `reply`, `inbox`, `ask`, `gate-create`, `gate-resolve`, `gate-list`.
- **What happens:** `send` delivers an inter-agent message. `check` reads the calling agent's messages and can wait for new ones. `reply` answers a message. `inbox` lists messages across recipients or for one recipient, which is the coordinator's view. `ask` sends a question to the coordinator and blocks until it is answered. A decision gate blocks a task until someone resolves it: `gate-create` attaches one to a task, `gate-resolve` records the decision, and `gate-list` lists them.
- **Rules and edge cases:** The sender is named with `--from`, a caller identity; inside an agent session a different caller is refused (CLI-08). Waits use timeouts with client grace.
- **Core and data:** Message and gate records are kept in the orchestration store.
- **Depends on:** CLI-12; CLI-14; CLI-11.

### CLI-14 Orchestration: supervised workers and dispatch

- Source: pv96 (unmerged). `delegate` started from a bot task chat does not show the agent-choice card; the bot-team commands are in CLI-32.

- **Audience:** agents (coordinators); owner (stop and release).
- **Commands (11):** `orchestration worker-start`, `worker-show`, `worker-read`, `worker-list`, `worker-stop`, `worker-abandon`, `worker-release`, `worker-retain`, `dispatch`, `dispatch-show`, `delegate`.
- **What happens:** `worker-start` starts one supervised worker: an agent in its own terminal, on the Run's home or on a connected server, with prompt, agent, worktree and timeout options (24 flags). `worker-read` reads bounded output. `worker-show` and `worker-list` show state and resource accounting. `worker-stop` fences the worker's dispatch and stops its terminal. `worker-abandon` fences it without claiming that the process stopped, for workers that cannot be proven stopped. `worker-release` frees the terminal of a settled worker. `worker-retain` keeps a terminal alive for debugging. `dispatch` assigns a task to a terminal, and `dispatch-show` shows the context. `delegate` starts a child session of the current chat on an agent the user chooses, from a one-line summary and a full prompt.
- **Rules and edge cases:** The worker start timeout is bounded by the maximum timer, and the client waits longer than the server's readiness budget. Abandon exists precisely because a stop cannot always be proven.
- **Core and data:** Dispatch and worker records are owned by the runtime; agent processes run in runtime terminals.
- **Depends on:** CLI-11; CLI-09; CLI-12; CLI-05 (workers on paired servers); agents domain.
- Owner direction (consistency pass, 2026-11-12): Proposal: first version. Owner direction (2026-11-12): orchestration is first version, and coordinators launch and supervise child workers through these commands; the CLI is also the test harness.

### CLI-15 Browser tabs

- **Audience:** both.
- **Commands (7):** `open-url`, `tab list`, `tab show`, `tab current`, `tab create`, `tab switch`, `tab close`.
- **What happens:** These manage the tabs of the built-in browser pane, which is scoped to a worktree. `tab create` opens a tab in the current worktree. `list`, `show` (by page id) and `current` read tabs. `switch` activates one and `close` closes one. `open-url` opens a URL on the paired client that hosts the calling terminal, so a remote terminal's link opens on the device that shows it.
- **Rules and edge cases:** The built-in pane is neither Chrome nor Safari, and it is not the app's own interface. Page content is untrusted data; the agent guide forbids running page text as commands.
- **Core and data:** Tab state is held by the runtime per worktree.
- **Depends on:** browser-and-devices domain; CLI-16.

### CLI-16 Browser page control

- **Audience:** agents.
- **Commands (37):** navigation: `goto`, `back`, `forward`, `reload`, `scroll`, `wait`, `eval`, `pdf`, `screenshot`, `full-screenshot`, `snapshot`; interaction: `click`, `dblclick`, `fill`, `type`, `select`, `check`, `uncheck`, `focus`, `clear`, `select-all`, `keypress`, `hover`, `drag`, `upload`, `scrollintoview`, `get`, `is`, `inserttext`, `mouse move`, `mouse down`, `mouse up`, `mouse wheel`, `find`, `download`, `highlight`; plus `exec`, a raw passthrough to the browser automation engine.
- **What happens:** `snapshot` captures the accessibility tree of the active tab with element references. Later actions use those references (click, fill, select, check, upload, drag). `get` and `is` read element properties and states. `wait` waits for an element, text, a URL, a load state or a JavaScript condition. Navigation commands move through history. Screenshots (viewport or full page) and PDF export capture the page. `eval` runs JavaScript in the page; `find` locates an element by a semantic locator and acts on it.
- **Rules and edge cases:** References belong to a snapshot and are meant to be taken again after the page changes. `eval` and `exec` run in the page context; the guide tells agents not to run page-provided text. Waits have timeouts.
- **Core and data:** Engine state per tab; not read.
- **Depends on:** CLI-15; browser-and-devices domain (the engine is a Phase 0 research item in the plan).

### CLI-17 Browser profiles and identity

- **Audience:** both.
- **Commands (9):** `tab profile list`, `create`, `delete` (destructive), `set`, `show`, `use-default`, `clone`; `browser identity get`, `set`.
- **What happens:** A browser session profile keeps its own cookies and storage. A tab can be switched to another profile, cloned into one, or returned to the default profile; `tab profile show` reports the bound profile. `browser identity set` chooses the browser identity used for every page on this host.
- **Rules and edge cases:** Deleting a profile removes its state. Identity applies to the whole host, not to one tab.
- **Core and data:** Profile records are kept by the browser pane's store.
- **Depends on:** browser-and-devices domain; accounts-and-usage domain for signed-in sessions (name only).

### CLI-18 Browser cookies, storage, capture and device settings

- **Audience:** agents; owner for clipboard and dialogs.
- **Commands (27):** `cookie get`, `set`, `delete` (destructive); `capture start`, `stop`, `console`, `network`; `intercept enable`, `disable`, `list`; `viewport`, `geolocation`, `set device`, `set offline`, `set headers`, `set credentials`, `set media`; `clipboard read`, `write`; `dialog accept`, `dismiss`; `storage local get`, `set`, `clear` (destructive); `storage session get`, `set`, `clear` (destructive).
- **What happens:** Cookies of the active tab can be read, written and deleted. Console and network events can be recorded and read back. Interception pauses matching requests so an agent can inspect them and list the paused ones. Viewport, device, offline mode, extra headers, HTTP credentials, colour scheme and reduced motion can be set. Geolocation can be overridden. The clipboard and browser dialogs can be driven. `localStorage` and `sessionStorage` can be read, written and cleared.
- **Rules and edge cases:** Intercepted requests stay paused until acted on. Clear commands delete data and are flagged destructive.
- **Core and data:** Per-tab settings and captured events are held by the runtime.
- **Depends on:** CLI-16; browser-and-devices domain.

### CLI-19 Desktop computer use

- **Audience:** agents (computer-use guidance).
- **Commands (14):** `computer capabilities`, `list-apps`, `permissions`, `list-windows`, `get-app-state`, `click`, `scroll`, `drag`, `perform-secondary-action`, `type-text`, `press-key`, `hotkey`, `paste-text`, `set-value`.
- **What happens:** These drive native desktop apps through each operating system's accessibility layer. The agent can list running apps and their windows, capture a compact accessibility snapshot of an app, click, scroll or drag an element or a window coordinate (with modifiers), type literal text, press keys or hotkeys, paste exact text, set the value of a settable element, and run an advertised secondary action. `permissions` opens the operating-system permission setup.
- **Rules and edge cases:** The app needs the operating-system permissions. The guidance sends web content to the built-in pane and reserves computer use for OS windows and external apps. A paired client cannot drive this machine's desktop unless the server was started with the desktop-control grant (CLI-04).
- **Core and data:** Platform helper processes (macOS, Windows and Linux helpers in the native folder; not read).
- **Depends on:** CLI-04; desktop permissions (app-platform domain).

### CLI-20 Mobile emulators and simulators

- **Audience:** agents (emulator guidance) and owner.
- **Commands (16):** `emulator list`, `devices`, `attach`, `tap`, `type`, `gesture`, `button`, `rotate`, `exec`, `kill`, `shutdown`, `install`, `launch`, `permissions`, `ax`, `logcat`.
- **What happens:** Lists running and available Android emulators and iOS simulators, including raw simulator helpers. `attach` starts a helper for a device and makes that device active for a worktree. Taps use normalized coordinates (0 to 1). Text is typed as US ASCII. The agent can send multi-point gestures, press hardware buttons, rotate the device, install an APK, launch an Android app by package, grant or revoke Android runtime permissions, dump the accessibility tree, and capture a one-shot logcat. `exec` passes a raw command to the helper. `kill` stops a helper; `shutdown` also shuts the simulator down.
- **Rules and edge cases:** Android goes through `adb`; iOS goes through a helper that serves simulator frames and accessibility data. Text input is limited to US ASCII. Host toolchains (Android SDK, Xcode) are needed; this was not verified.
- **Core and data:** Helper process state per worktree.
- **Depends on:** CLI-09 (worktree binding); host toolchains (not verified).

### CLI-21 Linear issues

- **Audience:** agents (Linear guidance) and owner.
- **Commands (27):** `linear save-issue`, `create`, `list-issues`, `list`, `issue`, `search`, `attach`, `comment add`, `status set`, `assignee set`, `assignee clear`, `priority set`, `priority clear`, `estimate set`, `estimate clear`, `due-date set`, `due-date clear`, `label add`, `label remove`, `label set`, `relation add`, `relation remove`, `team list`, `team members`, `team states`, `team labels`, `project list`.
- **What happens:** Reads and changes issues in connected Linear workspaces. The agent can list issues with MCP-compatible filters, read an issue's full context, search, create or update issues, set status, assignee, priority, estimate and due date, manage labels and relations, add comments, and attach links. Teams, members, workflow states, labels and projects can be listed.
- **Rules and edge cases:** Requires a connected Linear workspace. Writes change an external system, so the result of a failed write may need checking in Linear itself.
- **Core and data:** Linear holds the issues; the runtime holds the connection.
- **Depends on:** the task-tracker integration (providers-and-trackers domain, name only).

### CLI-22 Artifacts

- **Audience:** both.
- **Commands (5):** `artifacts share`, `update`, `unshare` (destructive), `list`, `delete` (destructive).
- **What happens:** `share` publishes an HTML or Markdown file to the signed-in MassOrca account as an artifact with a link. `update` replaces a previously shared file. `unshare` and `delete` remove an artifact. `list` shows the artifacts owned by the signed-in account.
- **Rules and edge cases:** Sharing is gated by the artifact-sharing setting, which defaults to off. The CLI reads the file only within a size limit. Cloud calls use the signed-in desktop account.
- **Core and data:** Artifact records are cloud-side; the CLI reads the local file only.
- **Depends on:** cloud domain; accounts-and-usage domain (sign-in).

### CLI-23 Automations

- **Audience:** owner (primary); agents may use them for scheduled runs (not verified).
- **Commands (7):** `automations list`, `show`, `create`, `edit`, `remove` (destructive), `run`, `runs`.
- **What happens:** An automation runs a prompt with an agent on a schedule. Triggers: hourly, daily, weekdays, weekly, a 5-field cron expression, or an RRULE. Each run creates a new worktree unless `--workspace` names an existing one. A precheck command can gate a run. `run` starts one now. `runs` lists the run history. `remove` deletes the automation and its history.
- **Rules and edge cases:** The repository defaults to the enclosing worktree. `--project` with `--host` or `--project-host-setup` picks the host setup. `--host runtime:` takes a server id. Task-source context is attached as JSON and cleared on edit with `null`.
- **Core and data:** Automation records and run history are kept in the runtime.
- **Depends on:** automations domain; CLI-05, CLI-07, CLI-09, CLI-10.
- Owner direction (consistency pass, 2026-11-12): Proposal: first version. Owner direction (2026-11-12): automations and routines share one schedule engine in the first version; every core feature needs a CLI entry point.

### CLI-24 Agent accounts

- Source: pv96 (unmerged). `account usage` shows session and weekly usage per agent provider on this host, and `account codex-reset-credit` spends one earned Codex reset credit on the selected account.

- **Audience:** owner.
- **Commands (6):** `account add`, `account list`, `account select`, `account rm`, `account usage`, `account codex-reset-credit`.
- **What happens:** `account add --agent claude|codex|opencode|devin` runs the agent's own login in the current terminal (`claude login` and `codex login` for Claude and Codex) and then registers the account with the local runtime. `list` shows managed accounts. `select` picks the OpenCode or Devin profile used for new launches. `rm` removes a managed OpenCode or Devin profile and its private data; it is flagged destructive.
- **Rules and edge cases:** Owner-only: refused over the SSH relay. Before the login starts, the CLI checks the runtime for version skew, so a failed OAuth round trip is not wasted. Some login flows use a loopback callback that a remote browser cannot reach; the code treats that case separately (not read).
- **Core and data:** Account registrations and private profile data are kept on this host.
- **Depends on:** accounts-and-usage domain; agents domain.

### CLI-25 Agent status hooks

- **Audience:** owner enables them; the app and the agents depend on the result.
- **Commands (4):** `agent hooks on`, `off`, `status`, `prepare-codex`.
- **What happens:** `on` installs MassOrca-managed status hooks into the configuration of up to 22 agent CLIs (among them Claude, Codex, Gemini, Cursor, Copilot, Grok, Hermes, Devin, Kimi and Antigravity). The hooks report session events to the app, which shows agent status. `off` disables them and removes the local hook entries. `status` reports per agent: installed, not installed, partial, error or skipped, with the configuration path. `prepare-codex` prepares a WSL pane's managed Codex home before its shell starts; the app calls it.
- **Rules and edge cases:** Owner-only over SSH. Hook scripts carry a request-shape version, so a stale script from an older build is reported as such.
- **Core and data:** Edits the agents' own configuration files and writes a managed hook script.
- **Depends on:** agent-status domain; agents domain.

### CLI-26 Claude Code agent teams bridge

- **Audience:** agents (Claude Code).
- **Commands:** `claude-teams` (listed, 1); `agent-teams-tmux` (hidden shim).
- **What happens:** `orca claude-teams` starts Claude Code's Agent Teams feature in the current MassOrca terminal. Everything after the command belongs to Claude Code and is passed through unchanged. Claude Code's tmux calls are answered by the hidden `agent-teams-tmux` shim, which forwards each call to the runtime with the team id and token from the environment, and returns stdout, stderr and the exit code.
- **Rules and edge cases:** Owner-only over SSH for `claude-teams`. The initial managed terminal types the literal `orca claude-teams`, so headless Linux installs get a bare-`orca` dispatcher, written only when no user file occupies that name.
- **Core and data:** Team state is held by the runtime (agents domain).
- **Depends on:** agents domain; CLI-11.

### CLI-27 Agent session search

- **Audience:** both.
- **Command (1):** `search` (11 flags).
- **What happens:** Full-text search across agent sessions indexed on one host: this machine by default, or a paired server selected with `--environment` or `--pairing-code`. There is no search across all machines. Filters: `--agent` (repeatable), `--path` (repeatable, a literal path on the host), and `--scope conversation` (user and assistant turns) or `all` (the default, which also covers commands and tool output). `--fresh` waits up to 5 seconds for the index to catch up. Multi-word queries must be quoted or passed with `--query`.
- **Rules and edge cases:** In an SSH terminal, the forwarded CLI searches the controlling app by default.
- **Core and data:** The index is maintained by the host (the AI-vault index); the CLI only reads it.
- **Depends on:** session-history domain.

### CLI-28 Skills commands

- Source: pv96 (unmerged). `skills picker` lists what a chat composer's `/` menu offers on this host, `skills install` takes catalog ids and can switch agents without a package file, and archify installs from its pinned release.

- **Audience:** agents (`skills get` is their entry point); owner (install, update, share).
- **Commands (6):** `skills list`, `skills get`, `skills install`, `skills update`, `skills installed`, `skills share`.
- **What happens:** `skills list` lists the version-matched guides bundled into this CLI build. `skills get <topic>` prints one as Markdown (`--reference`, `--references` and `--full` select parts). `skills install` and `skills update` manage named MassOrca skills by running the community `skills` npm package through `npx` against the MassOrca skills repository; `--dry-run` prints the command instead of running it. `skills installed` lists installed selectors. `skills share` publishes selected installed skills behind one unlisted link.
- **Rules and edge cases:** Install needs an explicit agent target or a detected agent. A run without targets is refused, because the community tool would then install into every agent it knows. `skills share` requires the agent skill-sharing setting, which defaults to off. An unknown guide topic tells the user to update MassOrca, and agents are told not to guess commands. Install and update are owner-only over SSH.
- **Core and data:** Installed files go to the agents' skill folders; guides are embedded in the binary.
- **Depends on:** skills-and-plugins domain (installation, sharing and guides).

### CLI-29 Per-workspace environment recipe check

- **Audience:** both.
- **Command (1):** `vm recipe doctor` (4 flags).
- **What happens:** Validates a per-workspace environment recipe (the `environmentRecipes` entry in the project's `orca.yaml`) without provisioning anything, unless the provision flag is given, and reports what must be fixed.
- **Rules and edge cases:** Provisioning creates paid resources, and the agent guide requires explicit approval before each paid step. Owner-only over SSH.
- **Core and data:** Reads the repository recipe; persists nothing.
- **Depends on:** per-workspace environments domain (name only); skills-and-plugins domain for plugin-contributed recipes.

### CLI-30 Recovery and diagnostics

- **Audience:** owner (support).
- **Commands (3):** `profile state exports`, `profile state rollback` (destructive), `diagnostics memory`.
- **What happens:** `profile state exports` lists retained SQLite backups and JSON exports of the profile state. `profile state rollback` restores one of them, or keeps the current JSON or SQLite profile. `diagnostics memory` collects a memory snapshot of the app and its managed terminals.
- **Rules and edge cases:** Rollback needs an explicit choice of backup. The profile group is owner-only over SSH.
- **Core and data:** Backups and exports are kept by the app's profile store.
- **Depends on:** settings-and-files domain (profile state).

### CLI-31 Marketplace commands

- Source: pv96 (unmerged). New in pv96: a Marketplace command group that connects apps and services, installs agent tools and skills, sets app access, audits pages and switches GitHub accounts.

- **Audience:** owner (connects, installs, audits); agents may read the lists.
- **Commands (17):** `marketplace list`, `connect`, `disconnect`, `access`, `audit`; `marketplace program status`, `install`, `uninstall`, `reinstall`; `marketplace services list`, `connect`, `disconnect`; `marketplace skills list`, `show`, `install`, `agents`, `uninstall`; `marketplace github accounts list`, `switch`, `rm`.
- **What happens:** `list` shows Marketplace apps with each agent's integration status, and `connect` or `disconnect` an app, which writes or removes the entries MassOrca keeps in agents' configurations. `access` shows or sets which agents may use an app on a host, and `audit` shows which Marketplace pages open or are blocked on a host, and why. `program` shows, installs, removes or reinstalls an agent CLI or tool on a host. `services` lists the GitLab, Bitbucket, Azure DevOps, Gitea, Linear and Jira status, and connects or disconnects a service with a token. `skills` lists and shows Marketplace skills on a host, lists the agents that read them, and installs or removes them. `github accounts` lists, switches and signs out the GitHub accounts that `gh` holds on a host.
- **Rules and edge cases:** Tokens come from stdin or an environment variable (the write-command rule in `odd/tasks/cli-write-commands.md`). `--host` takes `local`, `runtime:<id>`, `ssh:<id>` or, for the GitHub-account and skill commands, `wsl:<distro>`. Long audits wait longer than the host relay's 90 s limit (commit history).
- **Core and data:** Marketplace state and the sealed token store live on the host that runs the command (SET-04, SET-29).
- **Depends on:** SKL-02 and SKL-04 (names), SET-29 (names), CLI-05, CLI-07.

### CLI-32 Bot Teams commands

- Source: pv96 (unmerged). New in pv96: a Bot Teams command group, plus the orchestration `team-*` commands, for rooms, messages, tasks, routines, teams, memory, webhooks and files.

- **Audience:** owner; bots in a team (for `delegate`, `send` and `answer`).
- **Commands (40):** `bot-teams list`, `room`, `routines`, `messages`, `tasks`, `task show`, `send`, `answer`, `delegate`; `bot-teams team create|update|rm`; `bot add|update|rm`; `memory list|rm`; `webhook list|create|rotate-secret`; `file versions|restore-version`; `task create|start|reassign|set-project`; `project create|update|archive|restore|rm|brief|code-source|workspace`; `routine create|update|rm|pause|resume|runs`. The orchestration commands `team-delegate`, `team-remember`, `team-post`, `team-tasks` and `team-task-create` belong to the same feature.
- **What happens:** Reads and writes the Bot teams of this host: rooms and their messages (with reply quotes and attachments), tasks and their status, answers to a room's question card, hand-offs to a named bot (with `--wait` to read its result), teams and their bots, the memory the host keeps for teams, webhooks, team files with their versions, projects and their briefs, and scheduled routines with their runs.
- **Rules and edge cases:** Tokens and secrets come from stdin or an environment variable (the write-command rule). The rules set no new stream opcodes and only optional wire fields (`odd/tasks/cli-write-commands.md`, S5). Denied bots do not open terminals or agent launches, per the commit history (the denial rule itself was not read).
- **Core and data:** Team, bot, room, task, routine and memory records live on the host (AUTO-12, AUTO-14).
- **Depends on:** AUTO-12, AUTO-14, AUTO-18, AUTO-19, AUTO-20, CLI-12, CLI-13, CLI-14.
- Owner direction (consistency pass, 2026-11-12): Proposal: first version. Owner direction (2026-11-12): Bot Teams is a core mode in the first version (AUTO-12); its CLI is the test harness.

### CLI-33 Prototypes commands

- Source: pv96 (unmerged). New in pv96: a Prototypes command group for prototype requests and design systems.

- **Audience:** owner and agents working in a chat.
- **Commands (14):** `prototypes list`, `create`, `archive`, `unarchive`, `rm`, `restore-version`, `write-attachment`, `read-file`, `baseline-check`; `prototypes design-system read`, `check`, `versions`, `version`, `restore`.
- **What happens:** Lists prototype sets and design systems, creates a prototype request folder for a chat as the Prototypes page does, archives and restores requests, and copies an earlier version of one direction into its next version. It saves a note screenshot (PNG) into a request, prints one file's HTML, checks that a prototype copies the stylesheets of the page it reproduces, and reads, checks, lists and restores design system versions.
- **Rules and edge cases:** A request folder can be deleted only when its chat or worktree is gone. `design-system check` reports whether a design system is valid (commit history).
- **Core and data:** Request folders and design system versions live with the chat's project on the host.
- **Depends on:** AG-15 (chats), workspaces (names), the design pass in `docs/design/` (not restated).

### CLI-34 Jev commands

- Source: pv96 (unmerged). New in pv96: a Jev command group for the opt-in decision switch and its API key.

- **Audience:** owner.
- **Commands (5):** `jev status`, `enable`, `disable`, `set-key`, `clear-key`.
- **What happens:** `status` shows whether Jev decisions are enabled and whether a key is stored. `enable` and `disable` change the switch for this host. `set-key` stores the Jev API key, and `clear-key` removes it and turns Jev decisions off. Jev makes some of MassOrca's decisions (the commit history names the task tier behind delegation); when Jev fails, the deterministic answer applies.
- **Rules and edge cases:** Off by default. Decisions leave the host only for someone who opted in and saved a key (code comment in the defaults). The key comes from stdin or an environment variable (the write-command rule).
- **Core and data:** The switch is a setting (SET-01); the key is kept on the host.
- **Depends on:** SET-01 (setting), AG-26 (delegation tiers, names).

### CLI-35 Folder workspace commands

- Source: pv96 (unmerged). New in pv96: a folder-workspace command group for plain-folder workspaces.

- **Audience:** both.
- **Commands (5):** `folder-workspace list`, `create`, `update`, `rm`, `path-status`.
- **What happens:** Lists the folder workspaces on this host, adds one to a project group, and renames, moves, archives, pins or comments on one (`update`). `rm` stops tracking a folder workspace, and `path-status` checks that a folder workspace, project group or path still exists as a folder.
- **Rules and edge cases:** `rm` stops tracking the workspace; its wording does not say the folder is deleted, so the folder is treated as left in place. Plain folders never get git worktree commands (CLI-09).
- **Core and data:** Folder workspace records live in the runtime.
- **Depends on:** WS-09 (plain-folder workspaces), WS-11 (project groups), CLI-10.

### CLI-36 Theme commands

- Source: pv96 (unmerged). New in pv96: a pair of commands that read and set the app theme.

- **Audience:** both.
- **Commands (2):** `theme get`, `theme set`.
- **What happens:** `theme get` shows the MassOrca app theme (light, dark or system). `theme set` changes it to the same value that Settings > Appearance and the rail toggle change.
- **Rules and edge cases:** One saved value serves every writer (SET-30). The commands use the standard `--json` envelope (CLI-08).
- **Core and data:** The theme is a global setting (SET-01).
- **Depends on:** SET-30, SET-01.

### CLI-37 Chat results

- Source: pv96 (unmerged). New in pv96: a read-only `chat results` listing of a native chat's Results card.

- **Audience:** both.
- **Commands (1):** `chat results`.
- **What happens:** Lists a native chat's Results: its environment URLs and the files it changed, including files opened with `--preview` (CLI-09).
- **Rules and edge cases:** Read-only. The Results card and its data belong to the chat domain.
- **Core and data:** Chat records are kept by the runtime (AG-15).
- **Depends on:** AG-15, AG-16 (names), CLI-09.
