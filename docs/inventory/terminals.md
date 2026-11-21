# Terminals: functional inventory (TERM)

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits), read with read-only git (`diff`, `show` and `grep` on the commit) and Read. The earlier pass read worktree `massorca-server-language-820c84`, read-only. Paths are relative to the repository root unless marked otherwise. Nothing was built, run or changed there. CodeGraph was not used for this pass.

## Domain summary

Terminals give each workspace shells that keep running when the window closes. A per-user terminal host (internal name: daemon) owns the pseudo-terminal of every terminal, so terminals survive UI restarts. The desktop app and the headless server attach to that host over a local socket or named pipe, receive a screen snapshot and live output, and send input, resizes and kill requests. The host keeps a bounded live screen model per terminal. It can also persist checkpoints and an output log to disk, and rebuild a lost terminal's screen (cold restore). Shells start with optional integration files that report readiness, working directory and title. The subsystem also covers Windows ConPTY and PowerShell behaviour, macOS privacy wrapping, Linux service handling, environment assembly, busy detection and process-tree termination, built-in and imported colour themes, a credential guard for unattended agent terminals, and cleanup when workspaces are removed. Tabs and splits are described only where the core stores or changes them. WSL and SSH are noted as behaviour only. At pv96 each workspace's terminals and browser tabs sit in a right side panel (PLAT-30), local terminal spawns receive the Marketplace variables (TERM-16), and the headless host also stays busy while bot-team routines are scheduled (TERM-26).

## Source directories read

- `src/main/daemon/` (all non-test files, including `pty-subprocess/`, `fixtures/` and `AGENTS.md`). Read by header, key functions and constants; several files only by name.
- `src/main/pty/` (all non-test files).
- `src/main/orcad/` (supervision, census, instance lock, node-pty checks, idle exit, entry header). Browser-process files are outside this domain.
- `src/main/dsh/` (all non-test files).
- `src/main/ghostty/` and `src/main/warp-themes/` (all non-test files).
- `src/shared/terminal-themes/` (all non-test files) and terminal helpers in `src/shared/`: `terminal-scrollback-policy.ts`, `terminal-input.ts`, `terminal-startup-cwd.ts`, `terminal-theme-selection.ts`, `terminal-git-credential-guard.ts`, `git-credential-prompt-env.ts`, `default-global-settings.ts`, `osc-title-extraction.ts`, `terminal-mode-reset-profiles.ts`, `unix-socket-path-limit.ts`.
- `src/main/` root files matching `pty*`, `terminal*`, `shell*`, `zsh*`, `bash*`, `fish*`, `pwsh*`, `powershell*`, `win32*`, `line*`, `rolling*`, `lazy*`, `worker*`. Most were read; the generic ones are listed under "Not read".
- `src/main/ipc/pty.ts` and `src/main/ipc/pty/` (register, delivery, pane, provider, runtime, host-env and ipc subfolders), with channel names enumerated across `src/main`.
- `src/main/runtime/` terminal-ownership files (split layout, create, teardown, reconciliation, preview, credential guard, color query replies, render-desync evidence, scrollback limits).
- `src/main/ipc/` terminal and PTY handlers (`terminal-preview*.ts`, `terminal-startup-color-query-replies.ts`, `renderer-terminal-serializer-readiness.ts`, `pty-*.ts`).

pv96 delta (`git diff custom/integration d3c6311a531`): `git diff --numstat` over the daemon, PTY, orcad, dsh, ghostty, warp-themes, shared terminal and `src/main/ipc/pty*` paths showed three non-test changes. Read as diffs: `src/main/ipc/pty/ipc/spawn-env.ts` and `src/main/ipc/pty/provider/local-configure.ts` (Marketplace variables for local spawns), and `src/main/orcad/orcad-entry.ts` (the `automationsBusy` check). Read by header: `src/main/marketplace/marketplace-agent-env.ts`, `src/shared/persisted-ui-state-types.ts` (side panel fields), and `src/main/persistence/tracking-repos/worktree-identity-migration-session-refs.ts` (session references). Also `odd/tasks/desktop-simplification.md` (side panel and tab placement).

## Not read / low confidence

Not read, or read only by name or header:

- Renderer (`src/renderer`) and mobile UI: excluded by the brief. Not verified: the close-confirmation prompt, terminal UI behaviour, acknowledgement timing on the renderer side, and the mode-2031 push.
- SSH and relay internals (`ssh-*.ts`, `ssh-pty-*.ts`, `delivery/ssh-intake.ts`, `runtime/undelivered-ssh-kill.ts`): another worker covers remote hosts.
- Host internals: kill and shutdown bodies in `terminal-host*.ts`, the core of `daemon-pty-adapter.ts` and the router, checkpoint file layout, process table parsing, incarnation evidence, the history seed transfer protocol, and the body of `session.ts`.
- App-level spawn internals: `src/main/ipc/pty/runtime/*` (spawn and kill bodies), pane adoption, replacement and reservation (`pane/`), pending, payload, exit and interactive delivery files, provider liveness and listeners, `host-env/assembly.ts` (grep only), and the Codex, Pi and OpenCode environment paths.
- Shell files: `zsh-deferred-startup-line-init.ts`, `zsh-wrapper-dir-ownership.ts`, shell startup identity and output scanners, `pty-process-table-parser.ts` (name only), the Codex and OMP wrappers, and legacy terminal shims and tombstones.
- orcad: supervision, census, lock, idle exit and node-pty checks were read by header. `orcad-entry.ts` was read at the header level only.
- dsh: the UI trigger for removal was not traced.
- Warp: the save step and custom-theme normalisation were not traced.
- Generic utilities in the scoped root list, characterised only: `rolling-file-backup.ts` (backup writer that refuses symlinked targets), `lazy-worker-thread-host.ts`, `worker-thread-request-queue.ts` and `worker-thread-entry-path.ts` (worker-thread plumbing shared with other domains).
- Reference `docs/audits/*` (including the daemon and terminal audit notes) were not read.
- pv96: the bot-team routine host (`src/main/bot-teams/bot-team-routine-host.ts`) was not read; it is named only by the orcad diff.
- pv96: the renderer's side-panel terminal tabs and the split-by-reference logic were not read. TERM-02 and TERM-03 use the approved spec and the QA records only.
- pv96: no other terminal host file changed (checked for the daemon, PTY, dsh, ghostty, warp-themes and shared terminal paths).

Low confidence or inferred (marked again in place):

- An updated host kept alive by live terminals may still start new terminals (TERM-19).
- A machine reboot is handled as a crash (TERM-09).
- Whether an evicted attached client is told it was evicted (TERM-05).
- Cursor-position replies when no window is attached (TERM-15).
- Whether the history manager's disposal can mark a terminal ended while its host keeps it alive (TERM-09).
- Windows directory tracking depends on the user's shell (TERM-11).
- macOS login-session watch details (TERM-18).

## Feature index

| ID | Feature | Proposal |
|---|---|---|
| TERM-01 | Create a terminal in a workspace | first version |
| TERM-02 | Tabs and pane binding | first version |
| TERM-03 | Split panes | later |
| TERM-04 | Terminal host and reconnection after UI restart | first version |
| TERM-05 | Attach, detach and multiple clients | first version |
| TERM-06 | Output streaming and flow control | first version |
| TERM-07 | Scrollback and screen restore on reattach | first version |
| TERM-08 | Terminal history on disk | later |
| TERM-09 | Cold restore after a lost host or crash | later |
| TERM-10 | Shell detection and selection | first version |
| TERM-11 | Shell integration: readiness, working directory, titles | first version |
| TERM-12 | Busy state and close confirmation | later |
| TERM-13 | Closing and killing terminals | first version |
| TERM-14 | Input, paste, resize and terminal modes | first version |
| TERM-15 | Terminal query replies and colour answers | first version |
| TERM-16 | Terminal environment | first version |
| TERM-17 | Windows terminals: ConPTY, PowerShell, PATH | first version |
| TERM-18 | macOS and Linux platform handling | first version |
| TERM-19 | Host updates and crash recovery | first version |
| TERM-20 | Host log | later |
| TERM-21 | Built-in terminal themes | first version |
| TERM-22 | Theme import from Ghostty and Warp | later |
| TERM-23 | Per-workspace shell command history | later |
| TERM-24 | Credential guard and agent shell hooks | first version |
| TERM-25 | Terminal preview in the dashboard | later |
| TERM-26 | Headless-server terminals | first version |
| TERM-27 | Closing a workspace or worktree closes its terminals | first version |
| TERM-28 | WSL terminals (behaviour note) | later |

## Features

### TERM-01 Create a terminal in a workspace

- Source: pv96 (unmerged). Terminals start from the workspace's side panel (New Terminal, Mod+T) in addition to the earlier entry points, and local spawns now also receive the Marketplace variables (TERM-16).

What the user does: opens a terminal in a folder workspace or a git worktree. A client (CLI, mobile, or an agent launch) can also ask for a terminal in a workspace.

What happens:

- The terminal starts in the workspace root: the stored folder path for a folder workspace, the worktree path for a worktree. A relative directory may point outside the root. A missing directory falls back to the root only when the root exists. A floating terminal keeps its own directory.
- The shell is the explicit override if given, otherwise the default-shell setting (see TERM-10). Background-created terminals start at 120 by 40 cells until a window reports its real size (TERM-14).
- Agent-hook and child-session variables are removed from the spawn environment. An agent launch gets a per-launch token. The startup command goes through the startup-delivery path, which waits for shell readiness on supported shells (TERM-11).
- Background creation: when a client asks for a terminal in a workspace and the spawn is not owned by a focused window (CLI, mobile, or no window open), the main process creates the terminal itself. Otherwise the window does.

Rules and edge cases:

- Refused: a startup agent without a workspace selector; runtime unavailable; the requesting client disconnected during creation; a Claude account switch in progress; a spawn whose pane identity has changed, so output never binds to the wrong pane; an SSH host without a connected relay (remote, see the remote-hosts worker).
- Folder-path usability is checked for folder workspaces only. Root-like directories are refused as a working directory. On POSIX, existence is always checked. On Windows, existence is checked for explicit paths only. WSL UNC existence is not probed.
- If the reveal step fails after a successful spawn, the terminal keeps running and a warning is shown.

Protocol surface (observed): `pty:spawn` (with spawn state and commit steps), `pty:spawned`, `pty:incarnation`, `pty:isCodexOnSharedServer`, `pty:stopCodexSharedServer`, `pty:disableCodexSharedServerAutoStart`. The background path has no channel.

Persisted: the pane-to-terminal binding and layout (TERM-02); the default-shell and PowerShell implementation settings.

Depends on: Workspaces (folder versus worktree); Settings; Agents (launch token, startup command, shared Codex server); TERM-02, TERM-04, TERM-10, TERM-16; WSL and SSH (TERM-28 and the remote-hosts worker).

Proposal: first version (M). Every terminal depends on it.

### TERM-02 Tabs and pane binding

- Source: pv96 (unmerged). Tabs live in the workspace's side panel rather than a strip on the session bar, and a split pane can show another workspace's session by reference, which keeps that workspace when the reference's last tab closes.

What the user does: opens a terminal tab in the workspace's side panel (Mod+T), switches tabs, closes a tab.

What happens: each tab holds a layout of terminal panes. The core records which pane is bound to which terminal (tab, pane, terminal id and a generation counter). An app restart or window reload therefore reattaches each pane to its own terminal and never to another.

Rules and edge cases:

- Closing the last pane of a tab closes the tab. Closing a pane that is not the last keeps the tab.
- Every close bumps a topology revision. A layout saved from an older revision cannot bring a closed pane back.
- Moving a pane to another tab updates the durable layout first, then the status and orchestration keys that refer to that pane.
- The active pane and the expanded pane are stored per tab.
- Headless (server) tab groups keep their own group layout (`headless-tab-group-split-layout.ts`).

Protocol surface (observed): `pty:moveLeafToNewTab`, `pty:hasPty`, `pty:listSessions`.

Persisted: per-tab layout tree (panes, split direction, active and expanded pane), the pane-to-terminal map, and the topology revision. The store details were not traced.

Depends on: TERM-01, TERM-03, TERM-05; Workspaces.

Proposal: first version (S). One terminal per tab is the minimum. The stored tree is the base for TERM-03.

### TERM-03 Split panes

- Source: pv96 (unmerged). Split view can hold a reference pane that shows another workspace's session, each pane keeps its own header row, and the unfocused pane is dimmed; terminal sessions in split view were not re-checked.

What the user does: splits a pane horizontally or vertically. The new pane's terminal starts beside the source pane.

What happens and what the core must do:

- The new pane is inserted next to its source in the chosen direction and becomes active. A temporary placeholder pane is removed once the split completes.
- Refused when the source pane is missing, its terminal has exited, its handle is stale, or the terminal session is unavailable.
- Agent-team splits require the source pane to be an Orca terminal (Agents).
- The core must: allocate ids before spawning; store the tree per tab; apply exact-direction splits and remove placeholders; resolve a close as pane, last pane or tab, and bump the revision; move panes durably; track the active and expanded pane; split and move headless tab groups.
- The UI owns focus, geometry reports, visibility marks and the timing of reveal.

Persisted: as TERM-02.

Depends on: TERM-01, TERM-02, TERM-14.

Proposal: later (M). The tree in TERM-02 already exists, so splits can be added without changing the base.

### TERM-04 Terminal host and reconnection after UI restart

What the user does: quits or restarts the app, or reloads the window, and expects running terminals to still be there.

What happens:

- Terminals run in a separate per-user background process, the terminal host. The app starts it, it runs detached from the app, and it owns each shell's pseudo-terminal child. One host exists per protocol version (current protocol 43, named into the endpoint).
- The app connects over a local socket (Unix) or named pipe (Windows). The handshake requires the exact protocol version and a per-user token file. Hello from a client with a different token is rejected.
- Quit: the app disconnects without killing terminals. For terminals kept in durable history it writes a final checkpoint first (TERM-08). It then asks the host to retire only if the host has no sessions (250 ms budget). Terminals with live processes keep running.
- Relaunch: a healthy running host is adopted when its entry path matches, and for packaged builds when its app version matches (TERM-19). The app repairs the host's identity record, opens a control channel and a stream channel (5 s connect timeout; concurrent attempts share one connect), and reattaches panes (TERM-05).
- The host retires by itself when idle: no authenticated client within 2 minutes of starting, or no client, transport or live session remaining.
- Shutdown: on SIGTERM or SIGINT the host disposes all sessions (bounded at 5 s). A shutdown request can kill sessions or leave them running.
- Endpoint ownership rules (from the host's design notes): only a host that publishes itself on the canonical endpoint may replace it, and only after proving the incumbent is dead. No actor removes a name it did not create. Only a refused or missing connection proves death; a timeout does not. A host never creates a session on an endpoint it no longer holds.

Requests the app sends to the host (observed set): create-or-attach, cancel create, write, resize, pause and resume output, set background, kill, signal, list sessions, detach, get working directory, get foreground process, inspect process, confirm foreground, buffer actions, get snapshot, get size, take pending output, ping, health checks, shutdown, shutdown-if-idle, closing startup query authority, setting colour query reply colours, and history seed transfer.

Persisted: host identity record (pid, app version, entry path, launch nonce, start time), token file, and the socket or pipe name. On Unix the socket lives in the app data folder and moves to a short per-user `/tmp` folder when the path is too long (TERM-18). The pipe name on Windows includes the protocol version and a hash of the data root. Per-terminal history: TERM-08. Log: TERM-20.

Depends on: TERM-05, TERM-06, TERM-07, TERM-19, TERM-26; Desktop app platform (quit and update behaviour).

Proposal: first version (L). The PLAN's Phase 2 exit requires that a terminal opened from the CLI is still alive after disconnect and reconnect.

### TERM-05 Attach, detach and multiple clients

What the user does: a pane reconnects to a live terminal after restart, reload or a dropped connection. A client also detaches from a terminal it no longer shows.

What happens and the rules:

- Create-or-attach: when a live terminal with that id exists, the client attaches and receives the current screen snapshot (TERM-07), then live output. Otherwise a new terminal starts. An attach-only request never creates a terminal, so a vanished terminal is not silently recreated.
- A new attachment detaches earlier attached clients of the same terminal, which then stop receiving output. Whether the earlier client is told is not traced.
- Detach applies only to the client that owns the attachment. When a client disconnects, its attachments detach and the processes keep running. When no client is attached, output resumes so the shell is not left paused.
- Reconnecting with the same client identity replaces that client's old connections.
- A creation can be cancelled by the client that issued it (matched on terminal, client and request id). A creation that finishes after the client gave up is detached, so no orphan remains. The create request can carry a cancel-after deadline, capped at 300 s. A creation still unfinished at that deadline is cancelled, so a client that timed out cannot leave an orphan.
- Client timeouts: 30 s per request. A cancel must settle within 5 s. A cancel that cannot be delivered drops the connection.
- While the host is retiring, creates and attaches are refused. After the endpoint is lost, new terminals are refused but attaches are still allowed.
- Listing: each terminal reports its state (created, spawning, running, exiting, exited), shell readiness, process id, working directory, size and creation time.

Protocol surface (observed): `pty:hasPty`, `pty:listSessions`, `pty:replay`, `pty:data`, `pty:exit`, `pty:modelRestoreNeeded`.

Persisted: none in memory (the in-memory model is TERM-07). History: TERM-08.

Depends on: TERM-04, TERM-07, TERM-09.

Proposal: first version (M). Reconnection needs it. The eviction rules can be simplified.

### TERM-06 Output streaming and flow control

What the user sees: heavy output (a build, a long agent answer) stays responsive. Background terminals neither freeze the window nor grow memory without limit.

Host to app:

- Output is batched every 2 ms. Output of up to 1 KiB that follows input within 100 ms is sent at once, so typing echo stays fast.
- When the connection's send buffer is above 128 KiB, bulk output is held and sent in 64 KiB slices. A terminal with more than 4 KiB queued is held. Holding stops above 32 MiB queued.
- Backpressure: when unread output across terminals reaches 4 MiB, shell output is paused until the backlog falls to 1 MiB. One terminal at 64 KiB pauses until it falls to 32 KiB. A pause releases itself after 5 s if the app never resumes. A consumer stalled for 60 s has its backlog dropped and a gap marker recorded.
- Hidden or background terminals keep only the newest tail (64 KiB to 512 KiB each, within a 2 MiB total budget). Older bytes are dropped and replaced by a gap marker. The shell is not paused for them.
- Output that arrives while nobody is attached is kept up to 2 MiB. Beyond that, the next attach gets a full snapshot.
- Daemon-backed terminals have no per-chunk acknowledgements. Flow control is the pause described above.

Host to window (main-process delivery):

- Output is queued per terminal, sent in 16 KiB chunks, at most two writes per round, on a 2 ms timer.
- Unacknowledged output per terminal is limited to 512 KiB (plus 512 KiB for the active terminal). The total is limited to 8 MiB (plus 256 KiB for interactive traffic). Acknowledgements carry cumulative processed characters. After 5 s of silence a resync probe runs. After 10 s without acknowledgements while data is in flight, the accounting can heal.
- The window's own producer pause applies at 256 KiB unprocessed, resuming at 32 KiB.
- Hidden terminals: when a window marks a terminal hidden and the hidden-delivery gate is on (default on), bytes are dropped after the terminal model has processed them, not queued. The first drop emits a restore marker. Showing the terminal again emits one if anything was dropped. Drop state survives window reloads. After a reload, a 10 s watchdog opens delivery if the window never confirms.

Protocol surface (observed): `pty:data`, `pty:ackData`, `pty:setActiveRendererPty`, `pty:setHiddenRendererPty`, `pty:setRendererPtyVisible`, `pty:setPtyDeliveryInterest`, `pty:rendererDispatcherReady`, `pty:requestDeliveryResync`, `pty:deliveryResyncResponse`, `pty:reportRendererDeliveryState`.

Persisted: none. A diagnostic file is written only when an explicit environment variable is set.

Depends on: TERM-04, TERM-05, TERM-07, TERM-14.

Proposal: first version (M). Bounded memory and responsive typing are required. The numbers are tuning values that the rewrite should prove with tests.

### TERM-07 Scrollback and screen restore on reattach

What the user sees: after a restart or reload the terminal shows the same screen, cursor, colours and scrollback. Keyboard and mouse behave as before.

Rules:

- The host keeps a live terminal model per terminal, including scrollback. The live window is 1000 rows by default. An environment variable can raise it to 100 to 5000 rows. The reason given for the small live window: 100 or more terminals at full depth exhausted memory.
- The user's scrollback setting (default 5000 rows; allowed 1000 to 50000; presets 5000, 10000, 25000, 50000) governs what the window keeps. Requests above 1000 rows overlay durable history (TERM-08) under a 5 s deadline. If the deadline passes, the live snapshot is returned and history compaction continues.
- A warm reattach receives a rendered snapshot of the live model (screen and scrollback), not a raw byte replay. Live output that follows is de-duplicated by sequence number.
- Restored state: cursor position and visibility, the saved cursor, the normal and alternate screen buffers (kept separately), the modes in TERM-14, wide characters (Unicode 11 widths, matched to the window), and hyperlink ranges. A real resize drops hyperlink ranges.
- A full-screen program in the alternate screen gets its modes and cursor restored. Its alternate frame is redrawn by the program.
- Mobile clients receive a smaller view: 1000 rows and 512 KiB.

Protocol surface (observed): `pty:getMainBufferSnapshot`, `pty:getAuthoritativeBufferSnapshotCapabilities`, `pty:replay`, `pty:clearBuffer`, `pty:declarePendingPaneSerializer`, `pty:settlePaneSerializer`, `pty:clearPendingPaneSerializer`, `pty:reportRendererSerializerReady`.

Persisted: the live model is in host memory. The durable copy is TERM-08.

Depends on: TERM-05, TERM-06, TERM-08 (optional overlay), TERM-14.

Proposal: first version (M). Reattaching without the screen breaks the promise that a terminal is still alive.

### TERM-08 Terminal history on disk

What the user sees: nothing directly. This is what lets scrollback come back after the host dies or the machine restarts (TERM-09). It also backs workspace-level scrollback snapshots.

Rules:

- Per terminal, a folder in the app data folder holds: a checkpoint of screen and modes, an append-only output log with a header and a generation number, metadata (working directory, size, start and end times, exit code), and a legacy scrollback file from older builds.
- When the log reaches 5 MiB, a new checkpoint is written, the generation increases, and the log restarts.
- Checkpoints are capped at 200 MB by dropping the oldest rows. Metadata is capped at 64 KiB. Stored dimensions are capped at 1000 columns and 500 rows.
- A torn last record is truncated on recovery. A gap in sequence numbers makes the log unreadable. Unreadable files move to a recovery quarantine folder and are never deleted.
- A write error disables history for that terminal until it is reopened.
- Closing a terminal moves its folder into a pending-delete area (a fast rename), then removes it asynchronously. Retries run at 30 s and 120 s. Leftovers are removed at the next start. A crash during removal is recoverable.
- The host remembers up to 1000 killed terminal ids in memory, so a killed terminal cannot be reattached.
- On POSIX, folders are owner-only (0700) and files 0600. A one-time repair runs per data root and is recorded by a marker file. Windows skips permission repair.
- Workspace-level scrollback snapshots: a `terminal-scrollback` folder holds one file per snapshot. The newest 5 MiB is stored and 512 KiB is read back. Unreferenced files are deleted.
- Erasure: an explicit close erases the terminal's history. Removing a workspace erases its history (TERM-27). Hibernating a workspace keeps it. App quit keeps it.

Persisted: `terminal-history/<encoded terminal id>/` (checkpoint, output log, metadata, legacy file); `terminal-history/.pending-delete`, `.recovery-quarantine`, and the permissions marker; `terminal-scrollback/`.

Depends on: TERM-07, TERM-09, TERM-27.

Proposal: later (M). Disk history is needed only to recover after a host crash or machine restart. The first version can accept losing scrollback in those cases. This is the owner's decision.

### TERM-09 Cold restore after a lost host or crash

What the user sees: after a host crash, a machine restart or any other loss of running shells, an open terminal reappears with its last screen and scrollback. A fresh shell then starts in the same folder. The previous process is gone.

Rules:

- Trigger: the terminal's metadata shows it did not end cleanly (no end time). Terminals the user closed, and terminals marked ended when the app disposes its history, are not restored. A host crash does not mark its sessions ended.
- A terminal still live in the host is reattached instead (TERM-05).
- Restore payload, in order: a mode reset, the restored screen (the normal buffer only if the terminal was in the alternate screen), a process-boundary marker, then a new shell seeded with the restored text.
- Sources, in order: checkpoint plus output log (generations must match); checkpoint alone; the legacy scrollback file cut before any open alternate screen. Replay runs serially in 64 KiB slices. A failure falls back to the checkpoint alone.
- Working directory: the last reported directory (TERM-11), otherwise the recorded one. On Windows, WSL paths are normalised. The directory's existence is not checked. Only a proven permission denial is reported.
- Payloads wait in a 16 MiB in-memory cache until the window acknowledges them. Repeated spawn requests receive the same payload until then.
- Seeding: up to about 1 MiB of text inline, larger text in 512 KiB chunks. If seeding fails, the new shell starts unseeded and history writing pauses so the old files survive.
- Code does not treat a machine reboot specially. It is handled as a crash (inferred).

Persisted: reads TERM-08 files. Writes nothing beyond the normal history files.

Depends on: TERM-07, TERM-08, TERM-10, TERM-11, TERM-28.

Proposal: later (M). Visible continuity after a crash or reboot. Without it, the user gets a fresh shell and loses that terminal's screen and scrollback.

### TERM-10 Shell detection and selection

What the user does: sets a default shell in settings, or picks a shell for a new tab.

Rules:

- Windows: the default is `powershell.exe`, overridable by an environment variable. The PowerShell choice can be `auto` (prefers PowerShell 7, `pwsh`, when it answers a version check), `powershell.exe` or `pwsh.exe`. cmd.exe and WSL distributions are also choices (TERM-28).
- POSIX: the `SHELL` variable, else `/bin/zsh`. An invalid absolute path falls back to `/bin/zsh`, then `/bin/bash`, then `/bin/sh`, and then errors.
- The shell kind is recognised by file name, including versioned names and Nix store paths: zsh, bash, fish, pwsh, powershell, cmd, other.
- Windows fallback: when the pseudo console rejects a PowerShell request, the host tries `powershell.exe`, then `cmd.exe`.
- Interactive profile arguments apply only to POSIX interactive shells with no startup command, no agent, no SSH and no explicit override.
- Working-directory validation: TERM-01.
- Settings involved: default shell, PowerShell implementation, preferred WSL distribution, interactive profile arguments.

Depends on: TERM-01, TERM-11, TERM-17, TERM-28; Settings.

Proposal: first version (S). A terminal must open the user's shell.

### TERM-11 Shell integration: readiness, working directory, titles

What the user sees: startup commands (agents) run only after the shell is ready. The tab follows the working directory and title. Typing before readiness is held, not lost.

Rules:

- Shells with integration files:
  - zsh: a generated startup directory; prompt and command markers; a ready marker.
  - bash: a generated rc file with prompt and command markers and a ready marker. It also enables bracketed paste (TERM-14).
  - fish: startup initialisation; ready when the prompt is drawn.
  - PowerShell: an encoded startup bootstrap with prompt markers. The command-start marker needs PSReadLine. Integration is skipped in constrained language mode. There is no ready marker.
- Other shells (cmd, unknown): no integration. Readiness is reported as unsupported and input is not held.
- Ready barrier: input from the app (a startup command, or typing held before readiness) waits for the ready marker, up to 15 s. After the timeout the held input is flushed. For a Codex launch on fish without ready delivery, the timeout is 300 ms.
- A line-editor scanner detects that a line editor is ready from the output: bracketed-paste enable, 8-bit meta mode, or the prompt-start marker.
- Readiness probe: waits 50 ms after the prompt settles, at most four probes.
- If the integration files cannot be written, the shell starts unwrapped. It has no ready marker and no tracking.
- Working directory: the last valid `file://` directory report (OSC 7) in the output. It is read back from the terminal model, else from the process (Linux `/proc`, macOS `lsof`). Windows has no process lookup, so directory tracking depends on the user's shell emitting OSC 7. No Orca integration emits OSC 7 on Windows (observed; medium confidence).
- Titles: the last OSC 0, 1 or 2 title, up to 1024 characters. Kept in checkpoints.

Persisted: generated integration files in the app data folder; title and working directory in checkpoints and metadata.

Depends on: TERM-10, TERM-07, TERM-08, TERM-18; Agents (startup command).

Proposal: first version (M). Startup-command delivery and directory tracking are needed. zsh, bash and PowerShell come first. fish and other shells can follow.

### TERM-12 Busy state and close confirmation

What the user sees: a terminal running a command or agent is known to be busy. Closing it can prompt for confirmation.

Rules:

- Busy means a foreground process other than the shell is present. On POSIX: the deepest running process in the terminal's foreground process group. On Windows: the process name reported by the pseudo console, else the shell's name.
- Hosts that predate this query count as busy.
- Proving that the shell owns the foreground uses a fresh process-table read. An unreadable table means "unverifiable", never "exited".
- Process lookups are cached for about 1 s.
- After a command-finished marker while the alternate screen is active, output is held for up to 750 ms.
- The close prompt itself (renderer) was not traced.

Protocol surface (observed): `pty:hasChildProcesses`, `pty:getForegroundProcess`, `pty:confirmForegroundProcess`, `pty:inspectProcess`, `pty:getCwd`.

Persisted: none.

Depends on: TERM-11 (markers), TERM-17, TERM-18, TERM-13.

Proposal: later (M). Safety polish. The first version can close a busy terminal without the warning.

### TERM-13 Closing and killing terminals

What the user does: closes a terminal or an agent tab. Removing a workspace also stops its terminals (TERM-27).

Rules:

- A user close marks the stop as intentional and asks the host to kill the terminal. If the window never saw an exit, a synthetic exit is shown.
- Plain shell: the process group (POSIX) or job object (Windows) receives a termination request. If the shell is still alive after 5 s, it is force-killed. One retry runs after 250 ms on failure.
- Agent terminal (process tree): a snapshot of the process table is taken before the root is signalled. Graceful stop: the root is signalled at once, descendants whose recorded identity still matches receive termination, then a force-kill follows after 2 s. Immediate stop: descendants first, then the root, then waiting up to 8 s for exit.
- Identity checks: process start times have one-second resolution in the process listing. Processes started in the same second with a reused id are never signalled. If the table cannot be read, POSIX falls back to a root-only kill.
- Windows: the tree is terminated through the job object, with a `taskkill /T /F` fallback capped at 5 s.
- Quit: quitting the app kills in-process terminals only. Host terminals survive (TERM-04).
- Hibernation is a reversible stop. It keeps the terminal's wake binding and its history.
- A failed kill for a remote terminal is recorded so it can be retried (remote; see the remote-hosts worker).

Protocol surface (observed): `pty:kill`, `pty:signal`, `pty:exit`.

Persisted: intentional-stop marks (store not traced).

Depends on: TERM-12, TERM-17, TERM-18, TERM-08 (erasure, if history is kept).

Proposal: first version (M). Killing must not leave agents running.

### TERM-14 Input, paste, resize and terminal modes

What the user does: types, pastes multi-line text, resizes the window, and uses full-screen programs with the mouse.

Rules:

- Input is accepted only from the main window, and only for owned terminals that are not mobile-owned (wording as observed; the mobile ownership rule was not traced). Input is written in 16 KiB UTF-8-safe chunks. A single send over 16 MiB is refused with a clear error.
- A write that cannot be delivered emits a write-unavailable event. After a host failure, all panes are marked write-unavailable so they re-attach.
- Paste: the core does not change pasted text. Bracketed-paste mode is tracked and restored on reattach. The bash integration enables bracketed paste, so a multi-line startup prompt arrives as one paste. Only shells with integration advertise the paste-safe startup barrier.
- Tracked and restored modes: the alternate screen, bracketed paste, cursor-key mode, mouse tracking with its encodings, and the Kitty keyboard flags (stack per screen, at most 16). A renderer request can reset input modes (`pty:resetInputModes`). After cold restore, the new shell starts with modes reset (TERM-09).
- Resize: the window reports its geometry. The host applies the size to the terminal and its model, and keeps the applied size. A real size change drops hyperlink ranges.
- Remote viewers can claim a terminal's grid size (TERM-25).

Protocol surface (observed): `pty:write`, `pty:writeAccepted`, `pty:writeUnavailable`, `pty:resize`, `pty:reportGeometry`, `pty:getSize`, `pty:claimViewport`, `pty:resetInputModes`.

Persisted: modes are kept in checkpoints (TERM-08).

Depends on: TERM-05, TERM-06, TERM-07, TERM-15.

Proposal: first version (M). Typing, pasting and full-screen programs need it.

### TERM-15 Terminal query replies and colour answers

What the user sees: full-screen programs and shells that ask about colours or terminal capabilities get answers. They draw correctly even when the window is hidden or not yet open.

Rules:

- Window attached: the window answers queries (device attributes, cursor position, colour queries) from its own state.
- No window attached: the host's model replies to very few queries. The exception is primary device attributes while the POSIX ready gate is pending. The application-side model answers only output that the hidden-delivery gate dropped, and only when no remote subscriber is attached. The cursor-position reply in this path is inferred.
- Colour queries (dynamic colours 10 to 12, palette entries 0 to 15, and the dark or light query) are answered from the last palette the window pushed. Before any push there is no answer.
- On native Windows pseudo consoles, the device-attributes answer is the pseudo console's own identity string.
- Colour-scheme change notifications (mode 2031): subscriptions are recorded. The unsolicited push is sent only by the window, so background subscribers get no push (low confidence).
- Startup colour replies use the theme colours (TERM-21).

Persisted: the last palette, in memory only.

Depends on: TERM-21, TERM-06, TERM-14.

Proposal: first version (S). Agent TUIs send these queries at startup. Without answers they may stall or draw wrong colours (inferred).

### TERM-16 Terminal environment

- Source: pv96 (unmerged). Local host spawns (not SSH or WSL) also receive the Marketplace variables (the OpenMetadata token, and the Engram Cloud token and server URL when stored, plus the archify update-check variable), with no per-agent filter at the spawn.

What the user does: nothing. Programs in a terminal see a colour-capable terminal with the workspace's environment.

Rules:

- Base: the host's inherited environment minus build-mode flags, plus the caller's variables. The terminal sets `TERM` (the caller may override it), `COLORTERM=truecolor`, `TERM_PROGRAM=Orca` and `FORCE_HYPERLINK=1`.
- Removed: colour-suppressing variables (`NO_COLOR`, `FORCE_COLOR=0`, `CLICOLOR=0`); pane-identity variables unless supplied for this terminal; an incoherent conda activation state; the Chromium session-bus variable (set to disabled); AppImage variables on Linux.
- Added by the app for agent terminals: agent configuration folders (for example `CODEX_HOME`), the hook endpoint, and browser routing.
- Windows PATH is merged (TERM-17). WSL variables are bridged with WSLENV (TERM-28). Git prompts are disabled for unattended sessions (TERM-24).

Persisted: none.

Depends on: TERM-01, TERM-17, TERM-18, TERM-24, TERM-28; Agents (per-agent overlays).

Proposal: first version (S). Colour and PATH behaviour are needed from day one.

### TERM-17 Windows terminals: ConPTY, PowerShell, PATH

What the user does: opens PowerShell or PowerShell 7 terminals on Windows and runs commands with their PATH.

Rules:

- Terminals run through the Windows pseudo console (ConPTY) using a bundled library. The host joins a kill-on-close job object before the first terminal exists, so children die with it.
- Boot warm-up: a throwaway `cmd /c exit` process warms the pseudo console. It is killed after 10 s if still running.
- The default shell is `powershell.exe` (TERM-10). PowerShell profiles are read as text and never executed. Dynamic values become placeholders when the environment is rebuilt. Profile PATH entries are considered.
- Shell integration is skipped in constrained language mode (TERM-11).
- Inline startup commands for cmd or PowerShell: at most 6000 characters, and no double-quote characters. Command-line length caps of 8,191 and 28,000 characters are defined alongside these rules.
- PATH: the machine (HKLM) and user (HKCU) `Path` values are merged. The persisted order wins. Entries from the launch-only environment come first. Comparison is case-insensitive. If one hive fails, its last good value is used. The result is cached for 30 s and invalidated by the system's settings-change message.
- Process identity: the root process identity is verified within 3 s and up to 4 hops. Tree termination: TERM-13.
- Working directory: no process lookup on Windows. It depends on OSC 7 (TERM-11).
- History files are not permission-repaired on Windows (TERM-08).

Depends on: TERM-10, TERM-11, TERM-13, TERM-16; WSL (TERM-28).

Proposal: first version (L). Windows is a first-phase target, and this is where its shell quirks live.

### TERM-18 macOS and Linux platform handling

What the user sees: terminals work on macOS with its privacy permissions, and on Linux with long data paths, AppImage builds and correct process groups.

macOS rules:

- Spawn attribution: a terminal may start through the system login program, so privacy prompts are attributed to the app and not to the host. A probe decides between wrapped and direct spawn. A rejected verdict is re-checked after 30 min. An inconclusive probe spawns directly. An environment variable turns the wrapper off.
- A wrapped child reports no exit status to the app.
- The host records the app's code identity for telemetry only.
- Login-session watch: a host started from the GUI watches for the login session dying (three PAM rejections spanning at least 120 s, with an unhealthy resolver). It then exits without tearing down terminals, so its replacement can cold-restore them (TERM-09). Medium confidence.
- The foreground process group receives window-size changes, because the process root can be the login program.

Linux rules:

- Long data paths: the host's socket moves to a short per-user folder under `/tmp` when the normal path would exceed the Unix socket path limit. The folder must be owned by the user and private.
- Service supervision: when a systemd user session with linger is available, the host runs in a transient scope unit named after its launch. The stop timeout is 5 s. Otherwise the host is forked directly. A watcher stops the scope after the host dies.
- Process groups: group termination uses the process listing. PTY names come from the terminal library.
- AppImage environment is removed (TERM-16).

Depends on: TERM-04, TERM-13, TERM-16, TERM-26.

Proposal: first version (M). macOS privacy prompts and Linux long paths are real support issues. The systemd scope and the login-session watch are hardening that could move later if the owner prefers.

### TERM-19 Host updates and crash recovery

What the user sees: after an app update, running terminals keep running. If the host dies, its terminals end, the user can open new ones, and nothing loops forever.

Update rules:

- A running host is stale when its recorded app version differs from the packaged app, or when it was started from another app path (development rebuilds).
- At launch, a stale host is replaced only after verifying zero live terminals. If any terminal is alive, the host is kept and adopted, so the old build keeps running those terminals. As far as the code shows, it can start new terminals until it retires (inferred).
- Replacing a host kills its terminals. That is why replacement waits for zero live terminals.
- Adopt-or-replace has an absolute 32 s budget. A wedged host gets up to 11 grace retries before replacement.
- Manual restart: a seven-step action that kills the current host and all its terminals, then starts a new host. It is explicit and destructive.
- macOS: replacement can also happen before a new terminal starts, when the bundle is stale or privacy attribution is lost (macOS only).
- Windows: the host runs from a per-user copy of its bundle, one per app version, so an installer upgrade does not kill it. If the copy cannot be made, it runs from the install folder. Old copies are removed unless a live host still uses them.
- Hosts of older protocol versions (1 to 42) are adopted as legacy adapters that never respawn. The PLAN's decision of no protocol compatibility means this part does not carry over.

Crash rules:

- If the host dies, its terminals die with it. The app starts a host once for the failing request and retries (single flight).
- Crash-loop guard: at most five host starts per 60 s. Beyond that, creating terminals is refused, with a message that terminals will not start until the underlying failure is fixed. The window slides, so the system recovers without intervention.
- Host start timeout 10 s. The child's error output (last 8 KiB) is kept. The child is killed on failure.
- Degraded mode (proposal: drop): when an adopted host fails its spawn probe while holding live terminals, new terminals run inside the app process and die with it. The probe is retried at most every 30 s. A restart kills those terminals.

Persisted: host identity record (protocol version, app version, entry path, launch nonce, start time, scope unit); token file; relocated bundle copies; host log (TERM-20).

Depends on: TERM-04, TERM-05, TERM-09, TERM-20; Desktop app platform (app updates).

Proposal: first version (L). The rule that no live terminal is lost by an update is required. The relocation and legacy-adapter machinery is not. Degraded mode is dropped.

### TERM-20 Host log

What the user sees: nothing. Support can read why the host failed to start or stopped.

Rules:

- A log file `daemon.log` sits in the app's logs folder. It holds JSON lines, is owner-only, and rotates at 5 MB into two numbered backups.
- Logged: startup, readiness, shutdown with reason, endpoint publish refusals and ownership loss, rejected client handshakes, and session created, attached, exited and killed. Terminal output and tokens are never logged. A logging failure never blocks terminals.
- Health checks: a handshake, then a terminal-spawn probe, with 3 s and 4 s budgets. Verdicts: healthy, unreachable, rejected, spawn-unhealthy.
- Telemetry is separate from the log: lifecycle replacement and retirement reasons; identical audit observations are re-sent at most every 5 min.

Persisted: the log file (2 backups).

Depends on: TERM-19.

Proposal: later (S). This is developer and support diagnostics, not a user-facing feature. The core will need logs, but their format need not ship as a product feature.

### TERM-21 Built-in terminal themes

What the user does: picks a terminal colour theme in settings. Optionally picks a separate light theme and follows the operating system's light or dark mode.

Rules:

- 22 built-in themes: 2 defaults (based on Ghostty's dark palette, with raised selection contrast), 1 classic, 11 popular dark, 8 popular light.
- "System" follows the operating system's preference. A separate light theme applies only when the separate-light option is on.
- Unknown theme names fall back to the default dark theme, or to the built-in light theme in light mode.
- Duplicate theme names are refused at load time.
- Up to 200 custom themes. Custom ids are checked before built-in names.
- The same palette feeds colour query answers (TERM-15) and startup colours.

Persisted: theme settings. Custom theme storage was not traced.

Depends on: TERM-15; Settings; Desktop app platform (light and dark mode).

Proposal: first version (S). One dark and one light default are needed. The full catalogue and custom themes can follow.

### TERM-22 Theme import from Ghostty and Warp

What the user does: imports colours and font from an existing Ghostty configuration or from Warp theme files.

Ghostty rules:

- The configuration is looked up in the XDG config folder (or `~/.config/ghostty`), in macOS Application Support, or in `%APPDATA%\ghostty` on Windows. Files `config.ghostty` and then `config` are read, and later values win. Palette entries accumulate.
- `theme=` resolves in the user's theme folder, then in the bundled folder. This resolution applies on macOS and Linux only. Only colour keys are imported from a theme file.
- Mapped: foreground, background, cursor, selection, palette 0 to 15, opacity, padding (0 to 512), line height (an adjustment up to +200%), font family, size, weight (100 to 900), cursor style, focus and mouse behaviour. The option-as-alt key maps on macOS only.
- Refused or unsupported: light and dark theme pairs, missing theme files, and keys with no equivalent (for example selection word characters and bold colour). Blur maps to on or off only.
- Preview runs in the main process. The window applies only the values that changed.

Warp rules:

- Theme folders are found per release channel: on macOS under `~/.warp*`, on Linux under XDG data folders named `warp-terminal*` or `warp-*`, on Windows under `%APPDATA%\warp\<channel>\data\themes`.
- Automatic scan caps: 200 files, depth 3, 80 folders, 500 entries per folder. Duplicates are removed by real path.
- Manual import: a file dialog for `.yaml` and `.yml` files, up to 200 files. Ids come from a content hash. Paths are not persisted.
- Files up to 1 MB. Parsed in a worker with a 1 s cap, and a 5 s preview budget. A theme needs a background, a foreground and an ANSI colour.
- Saving an imported Warp theme was not traced.

Depends on: TERM-21; Settings.

Proposal: later (M). Convenience for users moving from other terminals. Not needed for daily agent work.

### TERM-23 Per-workspace shell command history

What the user sees: the up-arrow history in a terminal holds that workspace's commands only.

Rules:

- zsh and bash: each workspace gets its own history file inside the app's terminal-history tree, keyed by a 16-character hex id. fish ignores history-file paths, so its history is isolated by session name in fish's own data folder.
- A history location the app itself set is not inherited into nested app launches. A user's own history location is respected.
- Cleanup of zsh and bash history: folders for workspaces that no longer exist are pruned. The pass runs 10 s after the window starts, only when the live set is readable and non-empty, and only for folders older than 5 min. It is cancelled at quit. Daemon folders have no workspace id and are never pruned this way.
- fish history is deleted with its workspace (TERM-27).

Depends on: TERM-11, TERM-27, TERM-08; Workspaces.

Proposal: later (S). Shell convenience. The first version can share one history file.

### TERM-24 Credential guard and agent shell hooks

What the user sees: agent terminals never stop to ask for a git password. Agent activity hooks work in the user's shell.

Credential guard rules:

- Applies to unattended sessions, recognised agent launches (including one-shot headless runs), and sessions whose policy setting says to guard.
- Sets git's prompt-disabling variable to 0 and blanks the askpass helper variables (or keeps them as configured). The policy variable is stripped from the child environment.
- Host-launched terminals defer git configuration appends.

Shell hook rules (these install the agent-status hooks; the meaning of the events belongs to Agents):

- Writes a marked block into the patch file of an external shell-environment loader (DSH, under its home folder, `cordis.patch.yml`). Only Orca's block is touched.
- Writes a hooks file (`~/.orca/agent-hooks/dsh-hooks.json`) for five events (session start, prompt submit, pre-tool, post-tool, stop). Writes a hook script that posts to the app's local endpoint. When the endpoint is missing, the script spools events. It always exits successfully, so a hook never blocks an agent.
- Install skips unchanged content and keeps a backup. A flow-style patch file that is not append-safe is refused.
- Status is read from disk: installed, partial, not installed, error.
- Removal strips only the marked block, restores the empty list if that was the file's content, and deletes the hooks file. The script stays.
- Remote installs use the same files over SFTP (remote-hosts worker).

Depends on: Agents (status and hook events); TERM-16; Remote hosts.

Proposal: first version (M). The credential guard keeps unattended agents from hanging. Hook installation is needed only when agent status is in the first version.

### TERM-25 Terminal preview in the dashboard

What the user does: opens a live view of a terminal from the dashboard or a pop-out window. Optionally fits the view to the terminal's size.

Rules:

- Only the dashboard pop-out and trusted app windows may connect.
- Connecting returns an atomic snapshot, then a live feed. Scrollback is 0 to 1000 rows. The view resyncs after a grid change or an overflow.
- Input from the preview is forwarded to the terminal runtime. Acknowledgements are capped at 64 KiB.
- Fit: the preview claims the terminal's grid through a viewer registry. The claim is released when the viewer unsubscribes or its window closes.
- Stream: 5 ms batches, 64 KiB per batch, 512 KiB in flight, 256 KiB pending. Memory only; nothing is persisted.

Depends on: TERM-06, TERM-07, TERM-14; Dashboard (other domain).

Proposal: later (M). A convenience for watching terminals, not needed to do the work.

### TERM-26 Headless-server terminals

- Source: pv96 (unmerged). The headless runtime's busy check (`automationsBusy`, in `orcad-entry.ts`) now also counts bot-team routines, so the host does not idle out while they keep it busy; an idle stop still never kills terminals.

What the user does: runs the app as a headless server (for example on a VPS). Clients create terminals that run on that host.

Rules:

- The headless runtime (orcad) runs the same runtime service without the UI and serves runtime requests. It supervises the terminal host, which it starts detached so that a restart, update or rollback of the runtime does not kill terminals. Stopping the runtime does not stop the host. The host retires only when it proves zero live terminals across all its generations.
- Single instance per data folder, checked before the profile loads. The lock does not cover the terminal host.
- Startup check: the native terminal library is first loaded in a child process. An incompatible system library then yields a readable message rather than a crash. Prebuilt binaries are selected by N-API, C library (glibc floor 2.28 on Linux server builds) and CPU architecture.
- Census for managed stop and update: live terminal count (host plus in-process fallback), terminals started since activation, and the host protocol version. Reported as unknown when any host cannot answer.
- Idle exit applies only to a runtime that a client launched. A runtime with live terminals does not idle out. An idle stop never kills terminals.
- Network exposure and pairing are covered by the server domain.

Depends on: TERM-04, TERM-05, TERM-18, TERM-19; Headless server (server domain); Remote hosts.

Proposal: first version (L). The headless server must run terminals and keep them across restarts. This is the PLAN's stated "work survives the client going away".

### TERM-27 Closing a workspace or worktree closes its terminals

What the user does: removes a git worktree or a folder workspace.

Rules:

- Worktree removal requires proof that the worktree's terminals and structured agent sessions are stopped. Without proof, removal is refused with a hint to use Force. Force proceeds with a warning. The sweep order is: structured agent sessions, the runtime's terminal graph, the host's inventory, then the local terminal registry. The sweep has a 10 s budget. Structured sessions are retired only after the refusals have passed.
- Folder workspace removal is best-effort and never refuses. It closes structured sessions and stops terminals.
- Missing worktree (its folder is gone): its terminals are killed, four at a time, from one process snapshot. Best-effort.
- Erasure: a removed worktree loses its terminal history (TERM-08) and shell history (TERM-23), including fish files and WSL roots.

Depends on: TERM-13, TERM-08, TERM-23; Workspaces; Agents (structured sessions).

Proposal: first version (M). Removing a workspace must not leave agents running.

### TERM-28 WSL terminals (behaviour note)

What the user does: on Windows, opens a terminal inside a WSL distribution. Behaviour only; not analysed in depth.

Rules:

- Distribution choice: from a UNC working directory, else the workspace's distribution, else the preferred setting, else the system default.
- Launched through `wsl.exe`. It changes to the mapped path and then starts a login shell. Validation uses the Windows path.
- Environment variables are bridged with WSLENV flags (path-translated or plain).
- WSL UNC existence is not probed before spawning.
- Restored directories (TERM-09) are normalised for WSL paths.
- A Codex home mapping for WSL exists (Agents domain).

Depends on: TERM-09, TERM-10, TERM-16; Remote hosts.

Proposal: later (M). Windows-only. Decide together with the remote-hosts worker.

## Appendix A: Limits and constants (as found)

| Area | Constant | Value | Evidence (file) |
|---|---|---|---|
| Host client | connect timeout / request timeout / cancel settle | 5 s / 30 s / 5 s | `daemon/client.ts`, `daemon/daemon-client-rpc-request.ts` |
| Host respawn | starts allowed per window | 5 per 60 s | `daemon/daemon-respawn-throttle.ts` |
| Host recovery | adopt-or-replace budget; grace retries | 32 s; 11 | `daemon/daemon-recovery-budget.ts`, `daemon/daemon-replacement-preflight.ts` |
| Host start | start timeout; error tail kept | 10 s; 8 KiB | `daemon/daemon-launched-child.ts` |
| Host shutdown | shutdown bound; idle start window | 5 s; 2 min | `daemon/daemon-entry.ts`, `daemon/daemon-server.ts` |
| Host health | health and resolver checks | 3 s; spawn probe 4 s | `daemon/daemon-health.ts`, `daemon/pty-subprocess/spawn-preflight.ts` |
| Degraded mode | re-probe interval | 30 s | `daemon/degraded-daemon-fresh-spawn-routing.ts` |
| Output (host) | batch interval; bulk gate; slice | 2 ms; 128 KiB; 64 KiB | `daemon/daemon-stream-data-batcher.ts` |
| Output (host) | valve above which holding stops | 32 MiB | `daemon/daemon-stream-data-batcher.ts` |
| Backpressure | total pause / resume; per-terminal pause / resume | 4 MiB / 1 MiB; 64 KiB / 32 KiB | `daemon/daemon-stream-backpressure.ts` |
| Backpressure | failsafe resume; stall shed | 5 s; 60 s | `daemon/session-producer-pause.ts`, `daemon/daemon-stream-backpressure.ts` |
| Hidden terminals | tail per terminal; global keep budget | 64 to 512 KiB; 2 MiB | `daemon/daemon-stream-keep-tail-drop.ts` |
| Reattach | pending output before forced snapshot | 2 MiB | `daemon/session-output-plane.ts` |
| Log | daemon log size; backups | 5 MB; 2 | `daemon/daemon-file-log.ts` |
| Permissions | files; folders | 0600; 0700 | `daemon/daemon-private-file-modes.ts` |
| Scrollback (setting) | default; min; max; presets | 5000; 1000; 50000; 5000/10000/25000/50000 rows | `src/shared/terminal-scrollback-policy.ts` |
| Scrollback (host live) | default; override range | 1000 rows; 100 to 5000 | `daemon/daemon-session-scrollback-window.ts` |
| Scrollback (restore) | durable restore depth | 5000 rows | `daemon/daemon-restore-scrollback-depth.ts` |
| Scrollback (mobile) | rows; bytes | 1000; 512 KiB | `src/main/runtime/scrollback-limits.ts` |
| Snapshot overlay | durable overlay deadline | 5 s | `daemon/daemon-pty-buffer-snapshots.ts` |
| Checkpoints | tick; deadline; concurrency; cooldown; queue | 5 s; 15 s; 4; 45 s; 3 | `daemon/daemon-pty-runtime-state.ts`, `daemon/daemon-checkpoint-session-queue.ts` |
| History log | rotation size | 5 MiB | `daemon/terminal-history-session-writer.ts` |
| History checkpoint | cap | 200 MB | `daemon/terminal-history-file-limits.ts` |
| History metadata | cap | 64 KiB | `daemon/terminal-history-file-limits.ts` |
| History legacy file | cap | 16 MiB | `daemon/terminal-history-file-limits.ts` |
| History dimensions | max columns; max rows | 1000; 500 | `daemon/terminal-history-dimensions.ts` |
| Tombstones (disk) | retries; removal queue | 30 s and 120 s; 64 | `daemon/terminal-history-session-tombstone.ts`, `src/main/terminal-history-deletion.ts` |
| Tombstones (memory) | killed-terminal ids remembered | 1000 | `daemon/terminal-host.ts` |
| Repair | delay; entries; depth | 10 s; 200,000; 3 | `daemon/terminal-history-permission-repair.ts` |
| Shell history GC | delay; minimum age; workers | 10 s; 5 min; 16 | `src/main/terminal-history-gc.ts` |
| Cold restore | payload cache | 16 MiB | `daemon/cold-restore-payload-cache.ts` |
| Cold restore | replay turn | 64 KiB | `daemon/cold-restore-replay-writer.ts` |
| Seed | inline size; chunk size | about 1 MiB; 512 KiB | `daemon/terminal-history-seed-chunks.ts` |
| Seed transfers | concurrent; chunks; idle TTL; total | 8; 4096; 30 s; 200 MB | `daemon/terminal-history-seed-transfer-registry.ts` |
| Shell ready | ready timeout; Codex fish timeout | 15 s; 300 ms | `daemon/session-shell-ready-barrier.ts` |
| Readiness probe | settle; max probes | 50 ms; 4 | `src/main/shell-prompt-readiness-probe.ts` |
| Recovery barrier | hold time; queued bytes | 750 ms; 256 KiB | `daemon/terminal-shell-recovery-barrier.ts` |
| OSC scan | scan tail | 4 KiB | `daemon/terminal-osc-cwd-title-scanner.ts` |
| Title | maximum length | 1024 characters | `src/shared/osc-title-extraction.ts` |
| Busy lookup | process cache | about 1 s | `daemon/pty-subprocess/foreground-process-tracker.ts` |
| Kill (plain) | force-kill after; retry after | 5 s; 250 ms | `daemon/session-termination-controller.ts` |
| Kill (tree) | descendant SIGKILL after; immediate exit wait | 2 s; 8 s | `src/main/pty-descendant-termination.ts`, `daemon/immediate-kill-reply-budget.ts` |
| Kill (Windows) | tree-kill cap; warm-up kill | 5 s; 10 s | `src/main/windows-process-tree-kill.ts`, `daemon/windows-conpty-warmup.ts` |
| PATH (Windows) | cache; registry query | 30 s; 5 s | `src/main/pty/windows-environment-path.ts` |
| Command line (Windows) | inline startup max; length caps | 6000 characters; 8,191 and 28,000 | `src/main/providers/windows-shell-args.ts` |
| macOS spawn | privacy probe; re-check after rejection | 500 ms; 30 min | `src/main/providers/macos-tcc-login-shell.ts` |
| Process identity | start-time tolerance (POSIX; Windows) | 1.5 s; 10 s | `daemon/daemon-process-start-time.ts`, `daemon/daemon-pid-identity.ts` |
| Linux service | scope probe; scope migration; linger probe | 2 s; 5 s; 2 s | `daemon/daemon-cgroup-scope.ts`, `daemon/daemon-user-manager-lifetime.ts` |
| Renderer delivery | batch; flush chunk; writes per round | 2 ms; 16 KiB; 2 | `src/main/ipc/pty/delivery/constants.ts` |
| Renderer delivery | in-flight per terminal (+ active); total (+ interactive) | 512 KiB (+512 KiB); 8 MiB (+256 KiB) | `src/main/ipc/pty/delivery/constants.ts` |
| Renderer delivery | resync probe; heal silence; watchdog | 5 s; 10 s; 10 s | `src/main/ipc/pty/delivery/constants.ts`, `src/main/ipc/pty/ipc/resize-visibility.ts` |
| Renderer flow | producer pause high / low | 256 KiB / 32 KiB | `src/main/ipc/pty-producer-flow-control.ts` |
| Input | chunk; maximum per send | 16 KiB; 16 MiB | `src/shared/terminal-input.ts` |
| Worktree removal | teardown budget | 10 s | `src/main/runtime/worktree-teardown.ts` |
| Missing worktree | concurrent kills | 4 | `src/main/runtime/missing-worktree-terminal-reconciliation.ts` |
| Themes | custom themes | 200 | `src/shared/terminal-custom-themes.ts` |
| Ghostty import | config size; theme file size; line-height adjustment | 1 MB; 256 KiB; up to +200% | `src/main/ghostty/index.ts`, `src/main/ghostty/theme-resolution.ts`, `src/main/ghostty/mapper.ts` |
| Warp import | theme file; scan files / depth / folders / entries; parse; preview budget | 1 MB; 200 / 3 / 80 / 500; 1 s; 5 s | `src/main/warp-themes/index.ts`, `src/main/warp-themes/theme-file-scanner.ts`, `src/main/warp-themes/parser-runner.ts`, `src/main/warp-themes/preview-operation-budget.ts` |
| Preview | connect scrollback; batch; in flight; pending | 0 to 1000 rows; 5 ms / 64 KiB; 512 KiB; 256 KiB | `src/main/ipc/terminal-preview.ts`, `src/main/ipc/terminal-preview-output-stream.ts` |
| Hidden-delivery gate | settings default | both on | `src/shared/default-global-settings.ts` |

## Appendix B: Platform differences at a glance

| Behaviour | Windows | macOS | Linux | WSL (Windows side) |
|---|---|---|---|---|
| Default shell | `powershell.exe`, or PowerShell 7 with `auto` | `$SHELL`, else `/bin/zsh` | `$SHELL`, else `/bin/zsh` | `wsl.exe` with a distribution |
| Pseudo terminal | ConPTY (bundled library) | Unix pty | Unix pty | Linux pty inside the distribution |
| Host endpoint | named pipe | Unix socket in the app data folder | Unix socket; short `/tmp` folder when the path is long | host runs on the Windows side |
| Host start | per-user bundle copy; detached | from the app; login watch when started from the GUI | systemd transient scope when available, else direct | n/a |
| Privacy wrapper | none | login program wrapper when the probe accepts | none | none |
| Working directory | OSC 7 only | OSC 7, or `lsof` | OSC 7, or `/proc` | OSC 7 |
| Busy detection | pseudo-console process name | foreground group listing | foreground group listing | pseudo-console process name |
| Tree kill | job object, `taskkill` fallback | process groups | process groups | job object |
| History permissions | not applied | 0700 folders, 0600 files | 0700 folders, 0600 files | Windows side |
| Ghostty theme `theme=` lookup | not applied | user and bundled folders | user and bundled folders | Windows side |
| PowerShell integration | encoded bootstrap; skipped in constrained mode | n/a | n/a | n/a |

## Proposal summary

Of 28 features, 19 are proposed for the first version and 9 for later. Most first-version items are the terminal core that daily agent work needs: creation, reattachment, streaming, shell start, closing, input, and the Windows and macOS behaviour that cannot be skipped. The largest single decision is TERM-08 and TERM-09 (disk history and cold restore): they are proposed for later, which means a crash or machine reboot ends running terminals and loses their screens.
