# Proposed first version

Status: proposal for the owner to validate, 2026-11-12 (written overnight, without questions to the owner).

This is the short list to read first. It lists only the features proposed for the first version: **190 of 542**, one line each, grouped by domain. Everything else is "later" or "drop" in the [README summary table](README.md#summary-table), with a reason per row. The inventory describes **pv96** (commit `d3c6311a531`, `custom/integration` plus 852 unmerged commits).

How to read a line: **ID** feature name: what the user gets. `size` (S = days, M = 1-2 weeks, L = several weeks, XL = months). *(owner direction)* means the owner's design decisions put it here; *(pv96)* means the pv96 delta added or changed it. The full description is in the domain document named in each heading.

## The first version in one paragraph

A Rust core that runs as a service on Windows, macOS, Linux and a headless VPS, driven by the GPUI desktop, the Android app and the CLI. Projects are git repositories or plain folders; workspaces are worktrees or folders. Terminals survive client restarts and disconnects. Git covers status, diff, stage, commit, push and pull, and worktrees. Claude Code and Codex run in terminals with resume, status hooks and account switching, and the owner's subscriptions and past sessions are imported. On top of that, the owner's decisions add: a structured chat with a model picker, delegation to child agents (one engine for both modes), Bot Teams with a Conference room, Board and Routines, one schedule engine with a Calendar, a secret vault, usage rings, a Tasks page, light and dark themes, the Marketplace catalog page and Prototypes. Remote work runs on Linux and macOS SSH hosts; phones pair over the local network or SSH, not through a relay.

## Size of the proposal

| | S | M | L | XL | Total |
|---|---|---|---|---|---|
| First version | 49 | 96 | 42 | 3 | 190 |

The three XL items are the structured chat platform (AG-15), the Claude chat adapter (AG-17) and the Conference room (AUTO-13). All three come from the owner's decisions. This is still a large first version. The first pass on `custom/integration` proposed 137; the owner's design decisions added 31, the pv96 delta 13, and the overnight design decisions and consistency pass 9 more. Trimming candidates are the first item in [open-questions.md](open-questions.md).

## Features by domain

### Workspaces and worktrees (17 of 45, [workspaces.md](workspaces.md))

- **WS-01** Add an existing git repository as a project: Register a local git repository as a project, with duplicate detection, a name and an icon. `S`
- **WS-02** Add a folder as a plain-folder project: Register a non-git folder as a project whose folder is its single workspace; a No Git marker shows on desktop and phone, and adding a non-git folder warns first. `S` *(pv96)*
- **WS-04** Per-project base branch and workspace location: Choose the branch new workspaces start from and the folder they are created in, per project. `S`
- **WS-05** Create a git worktree workspace: Create a worktree with a new branch from a base, register it, and get back its setup result. `M`
- **WS-06** Workspace and branch naming: Get sanitised names, a configurable branch prefix, generated names and collision suffixes; accented letters fold to plain letters, and a delegated child is named from its chat. `S` *(pv96)*
- **WS-09** Plain-folder workspaces: Create a workspace bound to an existing directory, with terminals and agents but no git, and an optional one-line row name. `S` *(pv96)*
- **WS-12** Workspace placement on disk: Decide where new worktrees go: a global root, a per-project base, and a nested or flat layout. `M`
- **WS-16** Remove a workspace: Stop terminals and agents, delete the worktree and its branch when safe, with an explicit waiver for each refusal; removing a parent also removes its delegated children's worktrees. `M` *(pv96)*
- **WS-17** Remove a project from MassOrca: Drop a project and its workspace records from the list, without touching files on disk. `S`
- **WS-21** Workspace lineage (parent and child): Record which workspace or session created another, from orchestration, the CLI or a manual link, and show children under their parent across worktrees. `M` *(owner direction)* *(pv96)*
- **WS-29** Workspace records: persistence and live catalog: Keep the workspace list across restarts and in step across clients, with Git as the source of truth; manual row names and session references follow the workspace. `M` *(pv96)*
- **WS-31** No Git warning when adding a folder: Adding a folder that is not a git repository shows what it loses, with Continue without Git, Initialize Git here (when supported) or Cancel. `S` *(pv96)*
- **WS-33** One-line worktree sidebar (default layout): Each branch and each of its sessions takes one line with status; five sessions show, the rest fold behind Show N more, and needs-human or failed sessions never fold. `M` *(pv96)*
- **WS-35** Child sessions listed under their parent across worktrees: A session that runs in another worktree is listed once under its parent with a branch icon; its own branch row stays listed. `M` *(owner direction)* *(pv96)*
- **WS-40** Start a session in an existing branch or worktree from the create page: The create page starts a new session in an existing branch or worktree, with host, project and branch above the composer and a Chat or Terminal switch; a workspace with no session shows this page. `M` *(pv96)*
- **WS-41** Prototypes in the Workspaces side panel: The Workspaces side panel lists prototypes under a Prototipos entry. `S` *(owner direction)* *(pv96)*
- **WS-44** CLI: folder workspace commands: `orca folder-workspace` lists, adds to a project group, renames, moves, archives, pins, comments on, stops tracking and checks folder workspaces. `S` *(pv96)*

### Terminals (19 of 28, [terminals.md](terminals.md))

- **TERM-01** Create a terminal in a workspace: Open a shell in a folder or worktree from the side panel, starting in the right directory with the chosen shell. `M` *(pv96)*
- **TERM-02** Tabs and pane binding: Terminal tabs sit in the workspace's side panel, reattach to the same terminal after a restart, and can show another workspace's session by reference. `S` *(pv96)*
- **TERM-04** Terminal host and reconnection after UI restart: Quit or restart the app and find running terminals still alive. `L`
- **TERM-05** Attach, detach and multiple clients: Reconnect a pane to its live terminal; a new viewer takes over the terminal. `M`
- **TERM-06** Output streaming and flow control: Heavy output stays smooth and background terminals stay bounded in memory. `M`
- **TERM-07** Scrollback and screen restore on reattach: A reopened terminal shows the same screen, cursor, colours and scrollback. `M`
- **TERM-10** Shell detection and selection: Terminals open the user's shell: PowerShell, pwsh, zsh, bash or fish. `S`
- **TERM-11** Shell integration: readiness, working directory, titles: Startup commands wait for shell readiness; the tab follows the working directory and title. `M`
- **TERM-13** Closing and killing terminals: Closing a terminal stops its shell and child processes, including agent process trees. `M`
- **TERM-14** Input, paste, resize and terminal modes: Typing, multi-line paste, resizing and full-screen programs with the mouse work. `M`
- **TERM-15** Terminal query replies and colour answers: Full-screen agent UIs get colour and capability answers even when the terminal is hidden. `S`
- **TERM-16** Terminal environment: Programs see a colour-capable terminal with the workspace's environment, plus Marketplace variables on local hosts. `S` *(pv96)*
- **TERM-17** Windows terminals: ConPTY, PowerShell, PATH: PowerShell terminals on Windows with the user's PATH and profile experience. `L`
- **TERM-18** macOS and Linux platform handling: Terminals work with macOS privacy prompts, Linux long paths and correct process groups. `M`
- **TERM-19** Host updates and crash recovery: Running terminals survive an app update; a host crash ends its terminals but never loops. `L`
- **TERM-21** Built-in terminal themes: Pick a dark and a light terminal theme, optionally following the system. `S`
- **TERM-24** Credential guard and agent shell hooks: Unattended agent terminals never block on git passwords; agent hooks work in the shell. `M`
- **TERM-26** Headless-server terminals: Clients run terminals on a server that keeps them across restarts; the server stays up while bot-team routines are scheduled. `L` *(pv96)*
- **TERM-27** Closing a workspace or worktree closes its terminals: Removing a workspace stops its terminals and erases its terminal history. `M`

### Git and source control (13 of 31, [git.md](git.md))

- **GIT-01** Git availability and version baseline: Detects whether git exists on each host and reports it as present, missing or unknown; the Marketplace can install git on this computer in one click. `S` *(pv96)*
- **GIT-02** Non-interactive git runner and credential policy: Git never stops at a prompt; failures return fast with readable text. `M`
- **GIT-03** Repository detection and registration: Adding a folder tells a git repo from a plain folder and resolves worktrees to their repo. `S`
- **GIT-05** Status with line counts and listing caps: Changed files grouped as staged, unstaged and untracked, with line counts. `M`
- **GIT-06** File diffs for unstaged, staged and HEAD comparisons: Open the diff of any changed file, with image previews and submodule pointer diffs. `M`
- **GIT-07** Stage and unstage files: Stage or unstage one file, a selection, or everything. `S`
- **GIT-08** Discard changes: Throw away a file's changes or delete an untracked file, refusing paths outside the worktree. `S`
- **GIT-09** Commit with a typed message: Commit the staged set with a message the user writes; repository hooks run. `S`
- **GIT-13** Create a worktree from a base branch: Create a workspace on a new branch cut from a chosen base, with base refresh and fallback. `M`
- **GIT-14** Worktree listing and scanning: See all linked worktrees of a repo, with locked and prunable flags. `S`
- **GIT-15** Worktree removal with safety checks: Remove a workspace; dirty work is refused unless forced; unsafe branches are kept. `L`
- **GIT-19** Push, pull, fetch and upstream ahead or behind: Push to the tracked remote branch, pull, fetch with prune, and see ahead and behind counts. `M`
- **GIT-30** Bring a delegated child's commits back into the parent branch: Merges a finished child's branch into the parent's branch after predicting conflicts, under an ask, auto or full policy from the parent's access. `M` *(pv96)*

### Hosting providers and issue trackers (6 of 32, [providers-and-trackers.md](providers-and-trackers.md))

- **SCM-01** GitHub connection and diagnostics: Sign in to GitHub from the app through gh's device code, or use the existing gh login, and see why auth fails. `M` *(pv96)*
- **SCM-03** Hosting provider detection and repository identity: Repos are matched to their host and owner from git remotes, fork-aware. `M`
- **SCM-07** Branch review lookup and sidebar status: Each worktree shows its PR or MR state, a failing-checks mark and a browser link; the one-line row shows PR and CI as one tinted icon. `L` *(pv96)*
- **SCM-08** Status refresh pacing and rate-limit handling: PR status stays current within rate limits and shows the last known state on failure. `M`
- **SCM-09** Create pull or merge request from a branch: After push, open a PR or MR with base, title, body and draft state. `L`
- **SCM-22** Unified task drawer and source picker: The Tasks page lists tasks waiting for approval and opens a create screen for a task or session; one list across enabled sources, with the WorkSpaces or Bot Team switch still gated. `M` *(owner direction)* *(pv96)*

### Agent integrations (15 of 36, [agents.md](agents.md))

- **AG-01** Supported agent catalog, defaults and enable/disable: Pick, default and hide from 45 known agent CLIs with stable ids and an auto-pick order. `M`
- **AG-02** Install detection, refresh and installation help: See which agent CLIs are installed on local, WSL or SSH hosts, refresh after installing, link to install docs. `M`
- **AG-04** Terminal launch command construction: Start any agent in a workspace terminal with correct per-shell quoting, overrides and env. `M`
- **AG-05** Prompt delivery to terminal agents: The first task text reaches the agent by argv, flag, startup query or paste-after-ready with Enter. `L`
- **AG-08** Launch intent API (agent.launch) and launch receipts: One call launches an agent in an existing or new workspace and reports surface, prompt delivery and placement. `M`
- **AG-09** Creating a workspace with a startup agent: New worktree or folder workspace starts with the chosen agent and prompt. `S`
- **AG-10** Permission posture (Yolo / Manual) and access presets: One switch turns agent approvals on or off using each agent's bypass flag; presets for chats. `S`
- **AG-11** Per-agent Command, Arguments and Environment settings: Override an agent's command, add standing arguments and environment variables, including Marketplace app variables for chat and relay-launched agents. `S` *(pv96)*
- **AG-12** Workspace trust pre-writes: Agents started by MassOrca skip the trust-this-folder prompt. `M`
- **AG-13** Resume, continue and fork of terminal sessions: Reopen an agent conversation with the right per-agent resume command; fork Claude/Codex; keep logins across restarts. `M`
- **AG-14** Model, effort and mode selection for launches: Pick model, effort, fast mode at launch or mid-session via flags and slash commands, with live model lists. `L` *(owner direction)*
- **AG-15** Structured (native) chat platform: Chat tabs that keep running in the core, with a summary Results card, a shared checklist and question cards. `XL` *(owner direction)* *(pv96)*
- **AG-17** Claude structured adapter: Claude Code runs as a structured chat, with the checklist format and a design-system folder grant in linked worktrees. `XL` *(pv96)*
- **AG-23** Model catalog service for chats: The chat model picker shows one featured set per agent and folds the rest under "More models". `L` *(owner direction)* *(pv96)*
- **AG-26** Delegated (child) sessions: A chat hands a task to a child session, by default in its own worktree; the current worktree only for read-only work, a plain folder, or a manual pick on the card, and then the parent waits (child placement rules, owner 2026-11-12). `L` *(owner direction)* *(pv96)*

### Agent status and hooks (12 of 26, [agent-status.md](agent-status.md))

- **STAT-01** Agent status states and the status row: Every agent pane and structured chat shows one of four states, and a delegated child row names its parent. `M` *(pv96)*
- **STAT-02** Hook listener and provider event mapping: Status updates live from each hook-capable agent CLI without polling. `L`
- **STAT-04** Installing managed hooks into agent configs: Orca wires its status hooks into each chosen agent's own config, at startup or on toggle. `L`
- **STAT-05** Removing managed hooks and per-agent switches: Turning status hooks off removes only Orca's entries and keeps the user's own hooks. `M`
- **STAT-06** Installer safety: atomic writes, backups, ownership and locks: Agent config files are written atomically, backed up once, and kept safe across several instances. `M`
- **STAT-13** Status store on the execution host and persistence: Status lives on the machine that runs the agent and survives a UI restart. `M`
- **STAT-14** Pane identity, closure and reuse rules: A closed pane shows no agent, and a reused pane shows only its new agent. `M`
- **STAT-15** Stale rows and dead-process cleanup: Rows older than 30 minutes stop counting as fresh, and blocked or waiting rows stay in the approvals list until their pane reports again. `M` *(pv96)*
- **STAT-16** Remote host status over SSH: Agents on a remote host show their status in the desktop and CLI. `M`
- **STAT-17** Status during disconnect and reconnect: A dropped link keeps the last state marked as unverified, then replays it on reconnect. `S`
- **STAT-19** Attention and unread markers: A pane that needs input or has a finished turn is marked until the user looks at it. `M`
- **STAT-26** Delegated parent link on status rows: Paired clients show a delegated child under the parent chat that delegated it. `S` *(owner direction)* *(pv96)*

### Accounts, credentials and usage (14 of 26, [accounts-and-usage.md](accounts-and-usage.md))

- **ACC-01** Claude Code sign-in and import: Add a Claude account by signing in, or import an existing Claude config folder. `M`
- **ACC-02** Codex sign-in and import: Add a Codex account by signing in, or import an existing Codex home. `M`
- **ACC-03** Account registry and duplicate detection: See accounts per runtime (host or WSL distro) and avoid adding the same login twice. `S`
- **ACC-04** Claude Code account switching: Choose which Claude account new sessions use, per host or WSL distro. `L`
- **ACC-05** Codex account switching: Choose which Codex account new panes use, per host or WSL distro. `M`
- **ACC-06** Re-authentication and account removal: Sign an expired account in again, or remove an account safely. `S`
- **ACC-07** Claude token refresh and refresh ownership: Claude accounts keep working without signing in again every day. `M`
- **ACC-08** Live-session protection during account changes: Switching accounts does not break running Claude terminals or lose refreshed tokens. `M`
- **ACC-09** Claude usage and limits: See the Claude 5-hour and weekly limits per account, as rings on the activity rail. `M` *(owner direction)* *(pv96)*
- **ACC-10** Codex usage and limits: See the Codex 5-hour and weekly limits per account, as rings on the activity rail. `M` *(owner direction)* *(pv96)*
- **ACC-15** Usage refresh cadence and caching: Limits refresh on a timer and on demand, and stale numbers are dropped. `S`
- **ACC-21** Credential at-rest protection: Credential files are owner-only, OS-protected or encrypted, and never shown in plain text in settings; the Windows desktop may publish a file before its ACL lands. `M` *(pv96)*
- **ACC-22** Keep refreshed sign-ins across app restart: Sign-ins survive an app restart without losing refreshed tokens. `S`
- **ACC-24** Carry existing subscriptions into the new core: Existing Claude and Codex accounts (and API keys) work in the new app. `M`

### Session history (5 of 18, [session-history.md](session-history.md))

- **HIST-01** Browse past sessions: See past conversations from all discovered agents, with scope filter, grouping, sorting and refresh. `M` *(pv96)*
- **HIST-02** Session titles, previews and metadata: Each row shows title, working directory, branch, model, last activity and a short preview. `S`
- **HIST-03** Local discovery for Claude Code and Codex: Claude Code and Codex conversations on this machine appear without any setup. `M`
- **HIST-07** Resume a past session in a terminal: Open a session's directory and continue it with the agent's own resume command. `M`
- **HIST-18** Import sessions from other agent tools: Continue work started in another agent tool inside MassOrca. `L`

### SSH and remote hosts (10 of 27, [remote-hosts.md](remote-hosts.md))

- **REM-01** Add an SSH host manually: Save a remote machine by hand (address, user, key or agent, jump or proxy) and connect to it from any workspace. `S`
- **REM-03** Authenticate with keys, agent and password: Connect with the usual SSH keys, the SSH agent, or a password or passphrase asked on demand. `M`
- **REM-04** Verify host keys: Be warned when a host key changes, with the fix named; trust is remembered per host. `M`
- **REM-07** Connection lifecycle, reconnect and status: See a host's connection state and have it reconnect automatically after drops, sleep or wake. `M`
- **REM-08** Deploy and update the remote relay: Connect to a host and have the matching remote daemon installed or updated without manual steps. `L` *(pv96)*
- **REM-11** Remote terminals: Open shells and run agents on the host from any workspace. `M`
- **REM-12** Terminals survive disconnects: Find remote terminals still running after a network drop, with their screen restored. `M`
- **REM-14** Remote git and worktrees on the host: Run git status, diff, commit, push and pull on the host, and create worktrees there. `L`
- **REM-22** Remove a host and clean its remote state: Delete a host without losing workspaces that point to it, re-adding it later restores them. `S`
- **REM-23** Linux and macOS remote hosts: Use Linux or macOS machines as remote hosts. `M`

### Headless server and pairing (13 of 22, [server-and-pairing.md](server-and-pairing.md))

- **SRV-01** Headless server mode (serve): Run the core without a window on a VPS or a machine with no screen, reachable by phone and CLI. `M`
- **SRV-03** Local owner channel for same-machine clients: Desktop UI and local CLI talk to the core over a local socket or named pipe. `M`
- **SRV-04** Network listener and bind policy: Phones and remote CLIs reach the server over LAN or Tailscale; stays loopback until pairing opts in. `M`
- **SRV-05** Runtime discovery file and ownership: The local CLI finds the running core without configuration; stale ownership is reclaimed. `S`
- **SRV-06** Pairing offer (QR and URL): Pair a phone or CLI by scanning a QR code or pasting a code; regenerate retires a leaked code. `S` *(pv96)*
- **SRV-07** Advertised address and override: The phone dials a reachable address; the core suggests one and the user can override it. `S`
- **SRV-08** Device registry and per-device tokens: Each paired device has its own credential that can be revoked alone; revoke cuts only that device. `M`
- **SRV-10** Handshake, authentication and limits: Only paired devices get in; timeouts and connection caps bound resource use. `L`
- **SRV-11** End-to-end encryption on the network channel: Tokens and terminal output stay unreadable on shared networks, without TLS. `L`
- **SRV-15** Multiple simultaneous clients and revocation: Desktop, phone and CLI stay connected to one core at once; revoking one device disconnects only it. `M`
- **SRV-16** Protocol version and capability negotiation: Old and new clients keep working; new features turn on only where both sides support them; "update required" messages. `M` *(pv96)*
- **SRV-17** Reconnect and resync: After network loss or backgrounding, the client reconnects and open screens catch up without frozen state. `L`
- **SRV-18** Client connection to a server: CLI and desktop drive a remote server over one authenticated, encrypted link, with saved environments. `L`

### Automations, bot teams and orchestration (20 of 37, [automations.md](automations.md))

- **AUTO-01** Scheduled automation definitions: Recurring agent runs on a schedule in a chosen project, managed from the app or the CLI. `L` *(owner direction)*
- **AUTO-02** Destinations and workspace modes: Runs go to one existing workspace or to a fresh worktree per run, with clear refusal reasons. `M`
- **AUTO-03** Schedule engine, missed runs and retention: Due runs start once without backlogs; missed runs are recorded; desktop schedules pause when the app is closed. `M` *(owner direction)*
- **AUTO-04** Run now and run history: Start an automation immediately and review its past runs. `S` *(owner direction)*
- **AUTO-06** Agent launch for runs, extra arguments and session reuse: Each run starts the chosen agent with a saved prompt; optional extra arguments and reuse of the previous session. `M`
- **AUTO-07** Run completion, output and usage: A run shows when its agent finishes, its output, and its token usage. `L`
- **AUTO-08** Host ownership, fencing and orphans: Each automation belongs to one host; changed or removed hosts are flagged and never run blindly. `L`
- **AUTO-12** Bot team roster, editing and team access: Create and edit teams and bots, set each team's access mode (full access by default), and show a team's folder in Workspaces when its Workspaces option is on. `L` *(owner direction)* *(pv96)*
- **AUTO-13** Conference room: replies, request routing and choice cards: Bots reply in the room, one query routes each request to a bot, a team or a conversation, unclear requests get a choice card, and @ mentions reach bots, teams, projects and files. `XL` *(owner direction)* *(pv96)*
- **AUTO-14** Team Board: task states, columns and reassign: Tasks show Not started, Working, Waiting for you, Stopped, Stuck or Done in Todo, In Progress, In Review and Done columns; a task can be started, stopped or reassigned. `L` *(owner direction)* *(pv96)*
- **AUTO-15** Orchestration state store and Runs: The runtime database keeps Runs, tasks and bot team state at schema 61 in one file. `M` *(owner direction)* *(pv96)*
- **AUTO-16** Tasks and dispatch to terminals: Agents create tasks and send them to a terminal, with a circuit breaker after repeated failures. `L` *(owner direction)*
- **AUTO-17** Worker lifecycle: Start, read, stop, release and retain worker agents in worktrees, locally or on a server. `L` *(owner direction)*
- **AUTO-18** Agent-to-agent messages: Agents send, check and reply to typed messages, with group addresses. `L`
- **AUTO-20** Delegation from chat, team delegate and Automatic routing: A bot hands work to a teammate by name with no card; Automatic routing ranks agents by the tighter of their 5-hour and weekly usage and can take the tier from an optional Jev advisor. `L` *(owner direction)* *(pv96)*
- **AUTO-21** Nested worker depth limit: Limits how deep workers may dispatch further workers (default 1). `S` *(owner direction)*
- **AUTO-28** Routines: Each team bot runs routines daily, weekly, monthly or once at a time on the creator's clock, with pause, run history and one task per run. `L` *(owner direction)* *(pv96)*
- **AUTO-30** Calendar of routines and automations: One Calendar page shows automation runs and routine runs, with done runs hidden by default. `M` *(owner direction)* *(pv96)*
- **AUTO-32** Bring back child session work into the parent: A finished delegated child's commits can be merged into the parent worktree by hand, or automatically under ask, auto and full policies. `L` *(owner direction)* *(pv96)*
- **AUTO-33** Bot team projects: Board, Brief and code source: A team keeps projects with their own Board and Brief, moves tasks into them, and records whether the code is an existing repository or a new folder with git. `M` *(owner direction)* *(pv96)*

### Browser pane and devices (3 of 30, [browser-and-devices.md](browser-and-devices.md))

- **BRW-01** Load and navigate web pages in the pane: Browse web pages in a pane tab with back, forward and reload, and sign-ins that survive restarts. `M`
- **BRW-02** Tabs, popups and page-opened windows: Open, switch and close browser tabs in the workspace's side panel; links and popups open as tabs in the pane. `M` *(pv96)*
- **BRW-07** Localhost preview of dev servers: Open a dev server running in a worktree in the pane under a stable per-worktree name. `M`

### Skills, plugins and marketplace (1 of 44, [skills-and-plugins.md](skills-and-plugins.md))

- **SKL-27** Marketplace page and catalog: One page for tools, agents, skills and services, opened from the activity rail, with a sidebar list and a Settings pane. `L` *(owner direction)* *(pv96)*

### Command-line interface (17 of 37, [cli.md](cli.md))

- **CLI-01** Put orca on PATH and inside app terminals: One stable orca command per OS, and agent terminals reach the bundled CLI without a global install. `M`
- **CLI-03** Reach the running app: local connection, status and open: Every command talks to the local app; status shows its state and open starts it. `S`
- **CLI-04** Run a headless server (serve): A server runs without a window and advertises an endpoint for pairing phones and remote CLIs. `M`
- **CLI-05** Route commands to a paired server: Save a paired server once and target it with --environment, or connect once with a pairing code; Marketplace commands also accept a WSL distro. `M` *(pv96)*
- **CLI-08** Output, help, errors and agent discovery: Every command has a JSON envelope, clear errors and a machine-readable command schema for agents; help says MassOrca. `M` *(pv96)*
- **CLI-09** Worktrees and file tabs: Create, list, show, current and remove managed worktrees; list and close a worktree's agent sessions; open files and diffs as tabs, or an HTML file in the browser pane. `M` *(pv96)*
- **CLI-10** Repositories and projects: Register repositories, set base refs, and make a project available on a host folder or by clone. `M`
- **CLI-11** Terminals: Create, list, read, send to, wait on and close live terminals from the CLI. `M`
- **CLI-12** Orchestration: runs and tasks: Coordinators create runs and tasks, and recover from lost mutation responses. `L` *(owner direction)*
- **CLI-14** Orchestration: supervised workers and dispatch: Start, watch, stop and release supervised agent workers, dispatch tasks to them, and delegate from a chat without the agent-choice card for bot tasks. `L` *(owner direction)* *(pv96)*
- **CLI-15** Browser tabs: Open, list, switch and close tabs in the built-in browser pane for a worktree. `S`
- **CLI-23** Automations: Schedule an agent prompt per trigger, with run history and a new worktree per run. `L` *(owner direction)*
- **CLI-24** Agent accounts: Sign in Claude and Codex accounts on the host; show usage per agent; spend a Codex reset credit; select or remove OpenCode and Devin profiles. `M` *(pv96)*
- **CLI-25** Agent status hooks: Enable or disable status hooks in agent CLI configs so the app shows agent status. `M`
- **CLI-32** Bot Teams commands: The owner and bots read and write Bot teams from the CLI: rooms, messages, tasks, routines, teams, memory, webhooks and team files. `L` *(owner direction)* *(pv96)*
- **CLI-35** Folder workspace commands: List, add, update, stop tracking and check plain-folder workspaces from the CLI. `S` *(pv96)*
- **CLI-36** Theme commands: Read and set the app theme (light, dark or system) from the CLI. `S` *(pv96)*

### Desktop app platform (10 of 30, [app-platform.md](app-platform.md))

- **PLAT-01** Installable builds per OS: Installs Orca from a Windows setup, a macOS disk image or zip, or a Linux AppImage, deb or rpm. `L`
- **PLAT-07** Single instance and profile lock: A second launch focuses the running window, and only one local core runs per profile. `S`
- **PLAT-08** Startup preflight and tool detection: Shows whether git, forge CLIs, shells and agent CLIs are installed before work starts. `M`
- **PLAT-10** Main window lifecycle and close confirmation: Closing asks before discarding unsaved files or running sessions, and the core keeps running; the window has no title bar and its controls sit on the session bar. `M` *(pv96)*
- **PLAT-14** Safe quit and durable state writes: Quit finishes within a bound, and state files survive a power loss. `M`
- **PLAT-18** Local trace log and redaction: Redacted local log files for debugging, with nothing sent anywhere. `S`
- **PLAT-24** Windows shell, process and tree handling: Git Bash and PowerShell detection, and reliable termination of process trees. `M`
- **PLAT-27** CLI on PATH and launch redirect: The orca command is installed, and one binary serves as desktop, server and CLI. `S`
- **PLAT-29** Visible product name MassOrca: Human-visible text says MassOrca while the command, install identity and wire names stay. `S` *(pv96)*
- **PLAT-30** Desktop shell: activity rail, side panel and session bar: One approved desktop layout: a rail with workspaces, tasks, calendar, settings, theme and usage rings; a side panel for terminals and browser tabs; one session bar. `M` *(owner direction)* *(pv96)*

### Settings, persistence and files (8 of 30, [settings-and-files.md](settings-and-files.md))

- **SET-01** Settings read, change and push: Preferences save once and reach every open window at once, including the shell and card-layout keys. `M` *(pv96)*
- **SET-02** Persistent state across restarts: Projects, sessions, settings and UI state, including the side panel and rail mode, reopen as they were left. `M` *(pv96)*
- **SET-03** Schema versions and rolling backups: Upgrades migrate old data, newer data is refused, five backups kept. `S`
- **SET-04** Sealed secrets on the device: API keys and account tokens stay encrypted; the Marketplace vault wraps its key with the OS store when usable, otherwise an owner-only key file. `M` *(pv96)*
- **SET-08** Import the Electron app's profile state: Owner's agent accounts and subscriptions carry over; old file untouched. `M`
- **SET-11** Default keyboard shortcuts: Each command shows its default shortcut in menus and settings; 93 named actions. `S` *(pv96)*
- **SET-29** Secret vault with scopes, links and in-chat requests: Store API keys, tokens and passwords per project, team or global scope, link them to other scopes, and hand them to agents without pasting them. `L` *(owner direction)* *(pv96)*
- **SET-30** App light and dark themes: Light and dark app themes, switched from the activity rail, with the full choice in settings; the CLI reads and sets the same value. `S` *(owner direction)* *(pv96)*

### Customisation and extensibility (3 of 11, [customisation.md](customisation.md))

- **CUS-01** orca.yaml setup and archive scripts with per-script trust: Committed setup and archive scripts run on worktree create and archive, and each script is confirmed once per content hash. `M`
- **CUS-04** Per-agent command, argument and environment overrides: Point an agent at a custom binary or wrapper, add default arguments and environment variables per agent, disable agents, and choose the default agent, from Settings or a host's Marketplace agent page. `S` *(pv96)*
- **CUS-08** Stable internal extension points in the core: Our own features and later extensions register into named core points (commands, tools, status sources, rules, views) without changing the core. `L`

### Prototypes and design system (4 of 13, [prototypes.md](prototypes.md))

- **PROTO-01** Prototype requests from a chat: Asks for directions in plain words or with /prototype, picks a count, and gets design files back. `M` *(owner direction)* *(pv96)*
- **PROTO-02** Request folder and capture contract: Each request's directions are plain files in the chat's visuals folder that the agent writes and the view reads. `M` *(owner direction)* *(pv96)*
- **PROTO-04** Prototypes list and set view: A page with project groups, agent filters, set rows and a grid of directions on desktop and mobile. `L` *(owner direction)* *(pv96)*
- **PROTO-05** Viewer with viewports, browse and annotate: Opens one direction at desktop or phone width and pins elements to annotate them. `L` *(owner direction)* *(pv96)*

## Largest things left out, and why

| Left out | IDs | Why |
|---|---|---|
| Agents beyond Claude Code and Codex: other structured adapters, ACP, discovery of other agents' history, transcript chat | AG-18 to AG-21, AG-24, HIST-04 | Each agent is its own adapter; the plan adds them one by one after Phase 4. Codex chat (AG-18, XL) is the biggest single item left out. |
| Browser automation for agents, DevTools, computer use | BRW-10, BRW-13 to BRW-15, BRW-23, BRW-24, CLI-16, CLI-19 | The pane itself is in; agents driving it depends on the engine choice (CDP proposed) and is XL. |
| Mobile emulator control and streaming | BRW-26, BRW-27, CLI-20 | The owner's Work panel has a Mobile emulator tab; assumed to show a mobile web view in a device frame through the browser pane first (see assumptions). |
| Hosting providers beyond GitHub, tracker browsing, review threads | SCM-04, SCM-11, SCM-12, SCM-18 to SCM-21, SCM-27 | GitHub sign-in, PR status and PR creation are in; the Tasks page starts with team Boards as its only source. |
| Plugin runtime, third-party plugins, installing host programs, app connections | SKL-20, SKL-21, SKL-30, SKL-35 to SKL-37 | The extension model is a Phase 0 research item; the first version keeps stable internal extension points (CUS-08) and a catalog page. |
| Remote extras: Windows and WSL hosts, managed servers on SSH hosts, ephemeral VMs, remote history and git through the relay | REM-18, REM-20, REM-21, REM-24, HIST-10, GIT-28, GIT-29, WS-27, WS-28 | Linux and macOS SSH hosts and a headless server come first. |
| Relay and cloud services | SRV-13, CLD-01 to CLD-18 | Owner accepted default: relay pairing is not in the first version (design open question 5). |
| Multi-server orchestration and orchestration over SSH | AUTO-22, AUTO-23, AUTO-09 | Orchestration runs on one host first. |
| Team memory and file versions, web address triggers | AUTO-31, AUTO-36, AUTO-29 | Not defined by the owner yet, or a network entry point without security rules. |
| Prototype editing, merging and the project design system | PROTO-06 to PROTO-13 | Prototypes is core (decided on the owner's behalf), but request, list and viewer are enough behind the Sidebar entry. |
| Editor, notebooks, AI text generation, usage analytics | SET-16, SET-25, AG-27, GIT-10, ACC-19 | Not needed for daily agent work; the editor is a Phase 5 item. |
| Auto-update | PLAT-03 | Manual updates first; signed builds come with PLAT-01. |

## Assumptions made overnight

The owner was not asked tonight. These assumptions shape the list; each is easy to reverse.

1. **Decisions "on the owner's behalf" count as decided.** `docs/design/pending-owner-review.md` records them. The one that moves features here is prototypes as core (item 12): it puts PROTO-01, PROTO-02, PROTO-04, PROTO-05 and WS-41 in the first version. If the owner restores the extension default, those five go back to later.
2. **Owner direction beats pv96 behaviour.** pv96 routes Conference room requests without a lead bot and runs routines on their own scheduler. The list keeps the owner's lead bot and one schedule engine; the difference is an open question.
3. **Every core feature in the first version gets its CLI entry point** (PLAN: the CLI is the test harness). This is why CLI-12, CLI-14, CLI-23 and CLI-32 are in.
4. **Mobile emulator tab:** assumed to start as a mobile web view in a device frame (design A37), using the browser pane (BRW-01, BRW-07). Full emulator control stays later.
5. **Marketplace rail entry:** assumed to open the catalog page (SKL-27) with skills and agents; installing host programs and connecting services stay later. Android has no Marketplace entry (pending review item 10).
6. **Structured chat is in** because the Conference room and the model picker need a chat surface. This is the largest cost in the list and the first trimming question.
7. **Agents:** the list assumes Claude Code and Codex as the first agents (terminal), with Claude as the only structured chat agent. The first-version agent set is still an open question.

## What this list does not cover

- It was built from code reading only. Nothing was run, and no feature was tried in the app.
- Renderer and mobile code were read only to learn behaviour. Tests were not read.
- Sizes are rough estimates for reimplementing the behaviour, not for porting code.
- Overlaps are counted once in the domain they belong to, but the list does not merge them: bring-back of child work (AUTO-32 and GIT-30), delegation (AG-26 and AUTO-20), and per-agent overrides (AG-11 and CUS-04).
