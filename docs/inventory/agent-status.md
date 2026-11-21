# Agent status, hooks and notifications (STAT)

Functional inventory of the current MassOrca app (Electron, reference repository, read only). Describes behaviour and rules, not a Rust design.

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits).

## Domain summary

MassOrca shows, for each terminal pane that runs a coding agent, whether the agent is working, needs the user (blocked or waiting for input), or is done. For most supported agents the signal comes from hooks that Orca writes into the agent's own configuration; each hook posts lifecycle events to a loopback listener in the Orca process. Agents without a hook route are read through their terminal title and screen, and a status escape sequence in pane output is accepted as a further input. The status store is owned by the process that runs the agent: the desktop main process for local panes, and a relay deployed on each SSH host for remote panes, which forwards events to the desktop. Consumers are the sidebar and tab markers, desktop banners and sounds, tray and Dock indicators, phone alerts, and a keep-awake lock. The domain also covers what changes on the user's machine when hooks are installed, what happens when a remote link drops, and the risks of both. Most identifiers in the code still say "Orca" (for example ORCA_PANE_KEY and the ~/.orca/agent-hooks folder); this is the legacy name.

On pv96 (unmerged), the hook installers, the hook listener, the notification path and the remote relay's status rules are unchanged. What changed is small: a structured-session row can carry the link to the parent chat that delegated it (STAT-26), and blocked or waiting rows stay visible in the Tasks approvals list regardless of age (STAT-15).

## Source directories read

Paths are relative to the reference repository root. Read depth varies; see the next section.

- `src/main/agent-hooks/` and `src/main/agent-hooks/server/`: listener lifecycle, status disposition, status application, persistence, hydration, cleanup, remote ingest, terminal ingest, interrupt inference, tmux status, installer utilities, managed-hook controls and registry, Windows and POSIX command builders, hook stdin contract, first-work branch rename.
- `src/main/notifications/` (all three files).
- `src/shared/agent-hook-listener/` (source routing, envelope, listener state and limits, provider dispatch; Claude, Codex, Copilot and Pi-family event rules; other providers by header only).
- `src/relay/`: `agent-hook-server.ts` (replay section), `agent-hook-envelope-publication.ts`, `agent-hook-spool-ingest.ts`, `agent-hook-cached-pane-status.ts`, `relay-agent-hook-runtime.ts` (header), `claude-terminal-interrupt-status.ts`; other `agent-hook-*` modules and `wsl-agent-hook-relay.ts` by header only.
- `src/shared/`: `agent-status-types.ts`, `agent-status-freshness.ts`, `agent-hook-relay.ts`, `agent-interrupt-intent.ts`, `claude-terminal-interrupt.ts`, `agent-turn-outcome.ts`, `agent-status-hooks-setting.ts`, `computer-awake-mode.ts`, `notification-settings-types.ts`, `notification-settings-defaults.ts`, `claude-owed-task-notifications.ts` (constant); `agent-hook-spool.ts`, `agent-session-attention.ts`, `agent-process-presence.ts`, `agent-status-observation.ts`, `tmux-agent-hook-owner.ts` by header only.
- `src/main/` root: `agent-awake-service.ts`, `agent-awake-status-lease.ts`, `agent-state-file-reader.ts`, `hooks.ts` and `effective-hook-config.ts` (headers), `codex/hook-trust-promotion.ts` (rule comments), `ipc/notification-*.ts`, `ipc/native-notification-delivery.ts` (header), `dock/unread-badge.ts`, `tray/system-tray.ts` (grep), `runtime/push/push-dispatcher.ts` (header and mapping function).
- `native/notification-status-macos/main.swift`.
- `src/main/runtime/`: `rpc/methods/agent-hooks.ts`; `terminal-wait-detection.ts` and `tui-idle-evidence.ts` (headers); `agent-state-rules/` (file list and hook-authority values). Status-store handlers were located by grep (about 80 files); none were opened in full.
- `src/main/ssh/ssh-relay-session.ts` (status paths and the disconnect teardown); `src/main/startup/` (install and quit triggers).
- `src/main/<agent>/hook-service.ts` for each agent: config target paths by grep; Claude's service read in part.
- pv96 delta (commit `d3c6311a531`; diffs against `custom/integration`): `src/shared/agent-status-freshness.ts` (diff), `src/shared/agent-session-status-fields.ts` (new, read in full), `src/main/runtime/rpc/methods/structured-agent-session-status-stream.ts` (diff), `structured-agent-session-status-delegated-parent.ts` (new, read in full), `src/relay/relay-agent-hook-runtime.ts` (diff: one environment augmenter added), and the one call site of the display rule in `src/renderer/src/components/tasks-home/tasks-home-approvals.ts`. A name-level diff against `custom/integration` of `src/main/agent-hooks/`, `src/main/notifications/`, `src/shared/agent-hook-listener/`, the agent-hook files under `src/relay/`, `src/main/agent-awake-*`, `src/main/runtime/agent-state-rules/` and `rpc/methods/agent-hooks.ts` showed no change.

## Not read or low confidence

- Tests (`*.test.ts`) were not read; a few test names are cited where they clarify a rule.
- The renderer (`src/renderer`) was not read, per the brief. Sidebar unread markers, sound playback in the UI, click-to-focus and terminal title observers are described only from core-side evidence.
- Provider event rules for most agents (amp, cursor, grok, the OpenCode family, the Pi family beyond state lines, droid, devin, kimi, muse, zcode, dsh, jcode, hermes, qoder, qwen-code, codebuddy, antigravity, command-code, gemini). Their install targets come from the registry and grep only, except where noted.
- Server internals not opened: `server-state.ts`, `server-status-application.ts` and `server-status-update.ts` (first 140 lines only), `server-ingest-structured*.ts`, `server-authority-*.ts`, `server-row-ownership.ts`, `server-tab-cleanup.ts`, `server-listeners.ts`, `server-opencode-binder.ts`. Pane-authority fencing is described from the disposition function only.
- WSL relay state machine, guest install, sentinel and recovery: headers only.
- Remote installers (`installer-utils-remote.ts`, `remote-managed-hook-installers.ts`) and TOML ownership (`managed-toml-ownership.ts`): headers or names only.
- Lock records, lock claims and owner identity: headers only.
- The TUI wait rule engine and rule JSON files (`agent-state-rules-engine.ts`): headers only.
- Native assertion internals: macOS uses a caffeinate child process and Linux uses systemd-inhibit (spawn level only). Windows keep-awake behaviour comes from code comments and the blocker path; nothing was run on a device.
- Phone push gateway, registration and `desktop-push-service.ts`: header only.
- First-work rename generation and folder rename, and the orca.yaml hook runner: headers only.
- Whether the sidebar shows `blocked` and `waiting` differently is not verified. Both map to "needs input" in notification wording and push.
- No production caller of `clearStatusEntriesForConnection` was found by grep in `src/` (outside tests). See STAT-17.
- The OSC status path (STAT-09) was checked for where it is ingested and when it yields to hooks, but not for sender authentication beyond the code read.
- No runtime behaviour was observed. Every statement comes from reading code and comments.
- CodeGraph (pv96 pass): the reference worktree now has a `.codegraph` index. Its semantic answers for the pv96 status and delegation modules did not match the code, so it looked stale. It was not synced, because that writes into the read-only worktree. Claims were checked with `git show d3c6311a531`, and with Read and Grep on the worktree, which matched the commit for these paths.
- pv96 not traced: the delegation-record query behind the parent link (`listDelegatedChildLinks`), which was read only through its call site, and the restart-resume phase field in the status summary.

## Main risks for the owner

1. Hooks are installed into user-global agent configurations at every launch, by default, for each agent whose CLI is found (STAT-04, STAT-06).
2. Prompts, tool input previews and last assistant messages are written in plain text to a file in the app data folder and kept for up to a week (STAT-13).
3. Hook scripts run in every agent session on the machine, not only in Orca panes. Outside Orca they exit quietly, but they still start on each tool use (STAT-06).
4. Hooks fail open. A broken hook never blocks the agent, but its status silently stops (STAT-02).
5. A dropped SSH link keeps the last state instead of clearing it. A stale busy row can remain until it is marked stale or replaced by a replay (STAT-15, STAT-17).
6. Codex runs with a separate Orca-managed home, and user hook approvals are copied back into the user's Codex trust config (STAT-04, STAT-06).
7. The OSC 9999 status path accepts status JSON from pane output. The ingest path read does not check the token that hooks carry (STAT-09).

## Feature index

Proposed first version (12): STAT-01, STAT-02, STAT-04, STAT-05, STAT-06, STAT-13, STAT-14, STAT-15, STAT-16, STAT-17, STAT-19, STAT-26.

Later (14): STAT-03, STAT-07 to STAT-12, STAT-18, STAT-20 to STAT-25.

pv96 (unmerged): changed STAT-01 and STAT-15; new STAT-26. The hook installers, the listener and the provider rules are unchanged in pv96.

The proposal assumes the first version supports one or two agents, which the owner chooses. The Phase 4 scope in the plan says the same.

---

### STAT-01 Agent status states and the status row

What the user sees: every pane that runs an agent has one status row. The state is one of four values: `working`, `blocked`, `waiting`, `done`. Blocked and waiting both mean the agent needs the user; notification wording and phone push treat them the same ("needs input").

- Source: pv96 (unmerged). A structured-session row can also carry its delegated-parent link (parent session, task, dispatch and dispatch status) when it is a delegated child; older hosts omit it (STAT-26).
- There is no agent-level error state. A failed or stopped turn is `done` with a turn outcome (`success`, `failure`, `cancellation`, `superseded`; verdicts can also be `interruption` or `unconfirmed`). The word "error" in the code belongs to hook installation (STAT-04).
- Provider events that produce each state (verified for Claude, Codex, Copilot, Pi-family and OMP):
  - `working`: prompt submitted, tool started or finished, session started (Claude, Codex). Copilot also reports a recoverable error as `working`.
  - `waiting`: Claude permission request or AskUserQuestion tool use; Codex permission request or request_user_input.
  - `blocked`: Copilot blocking notification or ask prompt; Pi-family ask or UI prompt; OMP tool approval request.
  - `done`: Claude turn boundary and manual compact completion; Codex Stop or Interrupt; Copilot Stop, SessionEnd, or a non-recoverable error.
- Fields a reader can rely on: state and when it began (tool activity does not reset it); last prompt (kept across the turn); last tool name and a short input preview; the full question JSON while a question is pending; last assistant message preview, flagged when it came from tool output; the previous completed message; a capped state history; agent type (open string, custom agents allowed) and model; provider session identity, used only for exact CLI resume; turn start time; child work (subagents) and orchestration context for workers; connection identity (empty for local rows).
- Rules and edge cases:
  - Malformed rows are rejected. A malformed optional field drops only that field.
  - `interrupted` and `sessionBoundary` are valid only on `done`. A session boundary (for example Claude SessionStart landing idle) is not a completed turn and must not trigger notifications, unread markers or finished times.
  - Caps: tool name 60 characters; assistant message 8 000; interactive question JSON 16 000; JSON structure depth and token limits.
  - A pane can stay `working` after the main turn ends while background work is owed (STAT-11).

Clients and core: the core pushes rows to the renderer and other readers; the status IPC was read as main-to-renderer only. Hook events and terminal status payloads enter through the core's ingest paths.
Data: in memory; persisted as described in STAT-13.
Depends on: STAT-02, STAT-13, Terminals and panes.

### STAT-02 Hook listener and provider event mapping

What happens: the desktop (or the relay on a remote host) runs a loopback HTTP listener on 127.0.0.1 with a random port and a random token generated at each start. Each agent's hook script sends one event as a POST to `/hook/<agent>`, with the token in a header. The listener turns the event into a row through the provider's rules (STAT-01).

- Responses: 403 on a wrong token; 404 for a non-POST, an unknown path, or when status hooks are disabled; 204 for every accepted or malformed event, so a broken hook never blocks the agent.
- Malformed input is dropped without a user-visible error.
- Request bodies that stop arriving are cut after 5 seconds. A truncated body that is not caused by Orca's own timeout is counted as transport interference, per source.
- A hook script from a different protocol or build version is accepted and logged once; the same applies to a dev-versus-packaged mismatch.
- Pane keys longer than 200 characters are rejected. OpenCode text parts are capped at 8 000 characters per event.
- Two extra paths on the same listener: Claude's usage feed (STAT-08) and an OpenCode startup-prompt claim.
- Platform note: the POSIX hook writes events with curl; the Windows hook uses a PowerShell launcher and curl.exe (STAT-06).

Depends on: STAT-01, STAT-06, STAT-14.

### STAT-03 Agent coverage and detection paths

Accepted hook sources (27): claude, codex, qoder, qoder-cn, qwen-code, codebuddy, gemini, antigravity, amp, opencode, opencode2, mimo-code, cursor, pi, omp, prime-agent, droid, command-code, grok, copilot, hermes, devin, kimi, muse, zcode, dsh, jcode.

Three detection paths, from the code:

1. Managed hooks written into the agent's own configuration, through the installer registry (22 installers: claude, openclaude, codex, gemini, qoder, qoder-cn, qwen-code, codebuddy, antigravity, amp, cursor, droid, command-code, grok, copilot, hermes, devin, kimi, muse, zcode, dsh, jcode).
2. Installed outside the registry:
   - OpenCode (opencode and opencode2 sources): an overlay plugin directory that mirrors the user's OpenCode config through symlinks, so user edits still apply.
   - Pi, OMP and prime-agent: an extension source that is installed locally, in WSL, and pushed to SSH relays.
   - Mimo Code: environment prepared when a pane starts.
3. Terminal parsing only, for TUIs without a hook route. Rule profiles exist for antigravity, claude, cline, codex, cursor, dsh, gemini, omp, opencode, opencode2, pi, prime-agent and an unknown-pane profile. Cline has rules but no hook source, so it relies on title and screen alone (STAT-09).

Other detection details:
- Declared hook authority in the rule profiles: `authoritative` for omp, opencode, opencode2 and pi; `turn-end` for codex and dsh; other profiles declare none.
- Codex state files are read with a 4 MB size limit and a nesting limit (STAT-12).
- Transcript polling applies to Codex and Muse (STAT-12).
- Cursor is the one agent that the code says has no title-derived fallback. Without its hooks, its status stays idle.

Depends on: STAT-02, STAT-04, STAT-09, STAT-12.

### STAT-04 Installing managed hooks into agent configuration

What the user does: nothing by default. The global switch "agent status hooks" is on unless explicitly turned off. While the switch is on, Orca installs its entries at startup (for every build), and again when the switch or the per-agent disabled list changes. An explicit action in Settings also triggers an install.

Targets (from code):
- Claude: the settings file of the default Claude home, or of a Claude profile's config folder. Orca's entries are added beside the user's own hooks.
- Codex: an Orca-owned Codex home, used as CODEX_HOME when Codex starts. The user's `~/.codex` is not the install target (STAT-06 covers trust).
- Gemini: `~/.gemini/settings.json`. Antigravity: `~/.gemini/config/hooks.json`. Cursor: `~/.cursor/hooks.json`. Command Code: `~/.commandcode/settings.json`. Droid (Factory): `~/.factory/settings.json`.
- Devin: its config.json, which is JSONC. Hermes: `~/.hermes/config.yaml` plus a plugin folder. jcode: the `[hooks]` table of its config.toml. Kimi: config.toml in the Kimi home. Muse: its settings.json plus a managed hooks file. ZCode: its config.json.
- Grok: `$GROK_HOME/hooks/orca-status.json` (or the default Grok folder). Copilot: `hooks/orca.json` under `$COPILOT_HOME` (or the default Copilot folder).
- Qoder, Qwen Code and CodeBuddy: their settings files (not read).
- SSH hosts: the same installers run over SFTP against the remote home folder. The managed script is written to `~/.orca/agent-hooks` on that host.

States reported per agent: `installed`, `not_installed`, `partial`, `error`, `skipped`. Skip reasons: agent disabled, hooks disabled, CLI not found, CLI presence unknown. A missing CLI is a skip, not a failure.

Rules and edge cases:
- Claude: if settings.json cannot be parsed, the agent reports `error` with a detail line and nothing is written.
- Partial installs list the missing event names (Grok and Copilot), so a partial install is visible.
- A profile's statusLine is not touched (STAT-08).
- Installs can run on an explicit user action and at startup. Startup never removes entries (STAT-05).

Data written: agent configuration files, managed script files under `~/.orca/agent-hooks`, one backup file per configuration (STAT-06).
Depends on: STAT-06, STAT-16 (remote), STAT-05, Settings.

### STAT-05 Removing managed hooks and per-agent switches

What happens: removal is the reverse of STAT-04. It runs when the global switch is turned off (all agents), when one agent is disabled (that agent only), and at quit for Grok on Windows.

- Only entries whose command points at an Orca-managed script are removed. Matching is by managed script file name, so entries left by older or parallel installs are removed too. User-authored hooks in the same file are kept (test name: "preserving user-authored hooks").
- A profile whose switch is off never removes hooks at startup, because the files are user-global and other Orca instances may depend on them. Removal needs an explicit action on the Settings toggle.
- Grok on Windows: quit removes its entries under a time bound. The Windows command cannot express the pane-key guard and would keep running after quit. POSIX hooks exit when the pane-key variable is missing, so they need no quit cleanup.
- After removal, a TUI agent without hooks falls back to title and screen status (STAT-09), except Cursor.
- Removing the Claude statusLine entry follows STAT-08.

Depends on: STAT-04, STAT-06.

### STAT-06 Installer safety: atomic writes, backups, ownership and locks

Rules (verified in code):
- Atomic writes: content goes to a temp file that is then renamed over the target. A crash mid-write leaves the original intact.
- No churn: if the new content equals the file, nothing is written, so the backup is not rotated.
- One rolling backup: `<config>.bak` holds the content from before the last change. A second change overwrites it. There is no history.
- Scripts first: managed scripts are written before the configuration points at them, so a config never references a missing file.
- File mode is kept. On Windows, a restrictive folder permission is granted once and the write retried.
- JSONC (Devin) is edited in place to keep comments. TOML files (Codex, jcode, Kimi) go through a managed-ownership layer (not read).
- Concurrency: a lock serialises installs across Orca instances. Waiters retry every 20 ms for up to 10 seconds. Lock owners are identified by host and process identity, so a lock left by a dead owner can be recognised.
- Ownership: managed entries are recognised by script file name. Production, dev and parallel instances therefore write the same managed entry rather than racing, and a fresh install sweeps entries left by older or parallel installs.
- Command forms: POSIX uses sh and curl. Windows uses an encoded PowerShell launcher and fully qualified curl.exe, so a repository-local curl cannot intercept payloads. The Windows form drains stdin and exits when the Orca variables are missing.
- The POSIX stdin reader forwards the payload byte for byte and returns as soon as one JSON value is complete, because one agent does not close stdin.

Risks:
- Orca changes files that belong to the user's agents, at each launch by default. Hand edits in managed areas may be replaced by a reinstall, although user-authored entries are kept.
- Only one backup is kept. A second change after a bad write loses the good copy.
- Codex: user approvals are copied back into the user's Codex trust config. Orca never writes trust for its own status hook there.
- Hooks are user-global. Every agent session on the machine runs them, including sessions that Orca did not start.

Depends on: STAT-04, STAT-05.

### STAT-07 Offline spool of hook events

What happens: when a POSIX hook cannot reach the listener (Orca closed or restarting), it appends the event as one JSON line to a per-pane file in a spool folder beside the endpoint file. On the next start the spool is replayed through the normal ingest path, fenced by launch token, so it cannot revive a retired pane.

- Progress events (PreToolUse, PostToolUse, PostToolUseFailure) are not spooled.
- No spool is written unless the pane key and endpoint variables are present and the endpoint file is readable. This stops stale variables from a parent shell from creating a folder.
- Caps: the hook writes to a pane file only while it is under 5 MB, and resets files older than 7 days. The replay side drains with limits of 5 MB, 1 024 files and 7 days.
- Windows coverage of the spool was not verified.

Depends on: STAT-02, STAT-06, STAT-14.

### STAT-08 Claude usage line feed

What happens: Orca can install a statusLine command in Claude Code that posts Claude's rate-limit and usage numbers to the listener. It feeds a usage display, not agent status.

- Installed only when the statusLine slot is empty. A user's own statusLine is never replaced.
- A user who deletes the managed statusLine is not reinstalled. A marker file records the first install.
- Not installed for Claude profile folders, only for the default Claude home.
- Removed with the managed hooks (STAT-05), and when the install plan marks it for retirement (the trigger was not traced).

Depends on: STAT-02, STAT-04. The usage display itself is outside this domain.

### STAT-09 Title and screen fallback, and the status escape sequence

What happens: for agents without hooks, the app reads the pane's terminal title (set by the TUI) and the screen tail. A spinner glyph in the title means working. An explicit ready, idle or done word, or the agent's ready prompt, means idle. A prompt waiting on the user is recognised from the tail. Rule profiles (STAT-03) hold the patterns, which ship as versioned data with a live-update path and a pattern safety check.

Where it is used:
- Terminal wait (CLI and orchestration waits for an agent to become idle). Evidence is ranked: hook row first for authoritative agents, then blocked prompts, then explicit ready markers, then quiet ready screens. A title that only names the agent is treated as idle for display, so it is not accepted alone; the runtime comments describe a busy Codex or Devin pane satisfying a wait at about 0 seconds mid-turn.
- Quiet screens: Codex's header and composer, and Muse's ready screen, are believed only after the screen has been quiet, because the same text is painted mid-turn.
- Panes without hook rows: a code comment says the runtime falls back to title status, except Cursor.

Status escape sequence: pane output can carry a status JSON inside an OSC 9999 escape sequence. The core ingests it per pane. It yields to a hook that reported the same pane more recently, and a process-based state (derived from the pane's foreground process) is accepted as a separate origin. The ingest path that was read does not check a token of the kind hooks carry, so anything printed to the pane can set its status. Confirm before relying on this as a secure channel.

Depends on: STAT-01, Terminal output stream (Terminals).

### STAT-10 Interrupt inference (Escape and Ctrl+C)

What happens: when the user cancels a turn, the core can mark the pane `done` with the `interrupted` flag, even if the agent's own cancellation hook never arrives. The renderer sends the key intent; the core decides.

- Inference applies only if the row is still `working` and unchanged since the key press (same update time, state start and prompt), was confirmed live in this run, and is not provider-session-only. A 500 ms settle period is defined for this rule.
- A plain Escape also closes views in Claude, Codex, OMP, Pi and prime-agent, so it proves nothing by itself. Claude adds a check: after an Escape, the Claude terminal title must leave its working state within a 5-second confirmation window. The other agents need provider evidence.
- For some agents a single Escape is not enough (two Escape presses are required, by the code's list).
- Ctrl+C is ignored for Codex and Droid, where it can copy text or leave a side chat.
- For Claude, an Escape on a pending AskUserQuestion prompt can mark the question answered instead of ending the turn.
- Remote panes: the relay that owns the terminal input and output decides; the desktop only forwards the request.

Depends on: STAT-01, STAT-02, STAT-09, terminal input.

### STAT-11 Claude subagents, background tasks and wake-ups

What happens: a Claude pane can carry child rows (subagents and teammates) and background work (shell tasks, background agents, cron). Owed task notifications keep the pane `working` after the main turn ends, so the pane looks busy until the work reports back.

- The parent row shows child states. A child's permission prompt stays visible over the parent's own progress.
- A background launch creates an owed notification. If no notification arrives, a 60-second lease timer restates the pane so it does not stay busy forever.
- The execution host reports whether it waits for a wake-up (`notification` or `finishing-turn`), so the pane can show the reason.
- An active session cron is recorded as work for the pane; its effect on the displayed state was not verified.
- A manual /compact ends at an idle prompt with no Stop event, so the post-compact event is the only clearing signal. An automatic compact runs inside a turn and claims nothing.
- On restart, subagents are seeded from the saved roster, idle ones are dropped, and a roster for a pane with no live process is reaped (STAT-15).

Depends on: STAT-01, STAT-02, STAT-13, STAT-15.

### STAT-12 Transcript reads for the last message, and Codex and Muse polling

What happens: some hook payloads omit the last assistant message or the final result. The core reads the agent's transcript file to fill them in. For Codex and Muse it also polls the transcript while the pane needs more information.

- After a hook that needs a message, up to 5 read attempts run 50 ms apart.
- Codex and Muse: a transcript watcher polls every second while the row needs it. The poll stops when a newer event replaces the row (checked when the poll fires).
- Grok result discovery re-reads pending results. In the relay, deferred re-normalisation applies late Codex results.
- A nested agent of another kind inherits the pane variables. The code keeps the poll alive for that case rather than clearing it.
- Codex state files are read with a 4 MB size limit and a nesting limit of 128.

Depends on: STAT-02, STAT-03, agent transcript files (AI vault domain).

### STAT-13 Status store on the execution host, and persistence

Ownership: the store lives where the agent runs. On the desktop, the main process owns rows for local panes. For an SSH host, the relay on that host produces the events (STAT-16), and the desktop keeps a mirror tagged with the connection identity. Rows are keyed by pane (`tabId:leafId`). The headless server mode of the product was not read.

Persistence (verified):
- File: `last-status.json` in the agent-hooks folder under the app data folder. Format version 2.
- Writes are debounced by 250 ms after the last change, then go to a temp file that is renamed over the target. On POSIX the folder is 0700 and the file 0600. A synchronous flush runs at quit.
- Not stored: structured-session rows (the session journal is the source), per-process observation counters, terminal handles, replay flags, unconfirmed flags, raw launch tokens (only a hash is kept).
- Stored in plain text: last prompt, tool name and input preview, question JSON, last assistant message.

Restart:
- Rows received more than 7 days earlier are dropped at the next start. No runtime expiry was found.
- A row that was not `done` comes back as unconfirmed: visible, never fresh, and unable to seed an interrupt inference until a live event confirms it.
- A `done` row comes back as it was.
- A file with another version, invalid JSON or the wrong shape is ignored whole, with a warning. There is no migration.
- Codex and Claude child state is seeded from the saved snapshot.

Clients and core: the rows observed in the core are written from hooks, the terminal status path (STAT-09) and the remote relay (STAT-16); the status IPC was read as main-to-renderer only.
Depends on: STAT-01, STAT-14.

### STAT-14 Pane identity, closure and reuse rules

What happens: a status row belongs to one pane. These rules decide what happens when the pane closes, is reused, or changes identity.

- Closing a tab or a pane retires the pane. Later events for it are suppressed. Up to 1 024 closed tab identities and 1 024 closed pane keys are remembered, and the oldest are forgotten.
- A new turn (a new prompt, a session start, or a new process that proves a new run) revives a retired pane under a new launch token. While that token fence is active, events with a different token are rejected, so an older process cannot reclaim the pane.
- Replayed events never revive a retired pane. Replay selection drops panes whose surface is gone.
- Pane-key aliases map an old or moved key to its current key (up to 1 024 aliases).
- Orchestrated worker panes record their parent pane and dispatch context in the row.
- Legacy numeric pane keys from the earlier layout are not needed by the new product (drop). The earlier code flags PTYs with such keys as migration-unsupported.

Depends on: STAT-13, Terminals and panes, Orchestration.

### STAT-15 Stale rows and dead-process cleanup

What happens: a row for an agent that has gone away must not keep a pane busy.

- Source: pv96 (unmerged). For display, a blocked or waiting row stays in the Tasks approvals list until its pane reports again or closes, whatever its age. The 30-minute rule still governs control-flow readers, and a row restored from disk and not yet confirmed gets no exception.
- Freshness: a row that is not `done` is fresh only while its evidence is less than 30 minutes old. The clock is when the authority observed the evidence, not when the last relay delivery happened. Child work is marked unverifiable when parent evidence is stale or the transport is not live. `done` rows are not expired by freshness.
- Process identity: a row can carry the agent process identity (process id, platform, start time). A hook from a different process casts doubt on the owner and triggers a presence check. Verdicts: live, unverifiable, exited. An exited process reconciles the pane's presence; the exact resulting row was not traced.
- Pane exit: terminal exit removes the pane's cached events, so a dead pane cannot come back on reconnect. A test named for PTY-exit reconciliation covers the status side.
- Restored Claude subagents with no live local agent process are reaped by a sweep after restart. A PTY that died while Orca was down never runs its teardown, so without this sweep the pane would stay busy for good.

Depends on: STAT-13, STAT-14, Terminals (process inspection).

### STAT-16 Remote host status over SSH

What happens: an agent on an SSH host runs there. Orca deploys a relay on that host. Hook scripts on the host post to the relay's loopback listener. The relay forwards each accepted event to the desktop over the SSH channel. The desktop checks it again, stamps the connection identity, and applies the same rules as local rows.

- Agent plugin and extension sources ship from Orca on each connect and when the switch or the disabled list changes, so agent changes do not need a relay redeploy. Shipping is best effort; a failure costs status on that host only.
- Frame budget: an event too large for one relay frame drops fields in this order: last assistant message, then subagents, then the question JSON, which stays last because a blocking question must stay answerable. Dropped field names are sent so the desktop can restore them from its cache. State and pane key always survive. If nothing fits, the frame is dropped.
- Redelivery: a rejected state snapshot is retried up to 40 times at 250 ms intervals. At most 64 panes wait at once.
- The relay caches up to 256 panes for replay.
- Remote hook installation uses SFTP (STAT-04).
- Plan note: the current transport is a JSON-RPC bridge. The plan's rule (status lives on the execution host, and every UI reads it) is what must carry over; the bridge itself need not.

Clients and core: the desktop mirrors remote rows. CLI and phone read through the core.
Depends on: STAT-01, STAT-02, STAT-04, STAT-13, Remote hosts.

### STAT-17 Status during disconnect and reconnect

What happens: when the SSH link drops, Orca does not mark the remote agent as exited. The teardown comment says connection loss makes the remote status unverifiable, not exited, and keeps the last observation until a replay or a certified process teardown updates or removes it.

- Child work turns unverifiable while the transport is not live. Non-`done` rows whose evidence is older than 30 minutes stop being fresh (STAT-15).
- On reconnect the desktop asks the relay to replay cached pane statuses. Replays carry a replay flag and are refused for retired panes. A relay channel that drops while SSH stays up is detected and triggers a reconnect.
- Reconnect does not extend freshness: the evidence time is kept apart from the delivery time, so a replayed row keeps its real age.
- A function that clears every row of a lost connection exists (`clearStatusEntriesForConnection`), but no production caller was found. Current behaviour is therefore that rows survive a disconnect until replay, expiry or pane teardown. Confirm before relying on this.

Depends on: STAT-16, STAT-15, Remote hosts.

### STAT-18 WSL agent relay

What happens: agents inside a WSL distribution on Windows report status through a Node relay that Orca deploys into that distribution. The guest relay listens on the loopback port that Windows issued and forwards each hook envelope back to Orca over its standard streams.

- A failed relay restarts after a cooldown that grows with each failure (capped). A missing Node runtime gets a longer cooldown. A one-shot reinstall timer follows failures.
- Per-distribution state keys. A default-distribution setting selects the target.
- Pi, OMP and prime-agent extensions are installed in the guest. Codex's WSL home is prepared before a pane starts; this RPC is available to the local CLI only.
- Guest readiness is checked for Pi and OMP.

Depends on: STAT-02, STAT-04, Windows and WSL support.

### STAT-19 Attention and unread markers

What the user gets: a pane that needs input, or a finished turn the user has not yet seen, is marked. The mark clears when the user looks at it.

- Desktop: a banner and an unread marker for structured sessions, plus an announced-notification record per pane. A record that carries a journal position is retired only by a read that covers that position. A record without one is retired by the next acknowledgement of that pane. Turn completions that are session boundaries do not count (STAT-01).
- Tray: on a completion or terminal bell while the main window is not visible, the tray icon gets an attention dot. The dot clears when the window is shown or restored.
- Dock (macOS only): an unread count is set through the app's badge call and cleared at quit. The count's source is in the renderer and was not read.
- Phone: the same edge is sent to paired phones (STAT-22).
- Orchestration: coordinators receive fleet attention categories for their worker panes (orchestration domain).

Depends on: STAT-01, Agent sessions (journal), Orchestration.

### STAT-20 Desktop notifications

What the user does: receives a system notification when an agent turn ends, when a terminal bell rings, or when the Settings test button is pressed. Clicking the notification focuses the pane.

- Settings and defaults: notifications enabled; agent-task-complete on; terminal bell off; suppress when focused on (a notification for the active worktree is suppressed while the window is focused). Per-source mutes are kept on this client. A mute removes desktop delivery only; it does not affect phone alerts.
- Burst cooldown: one desktop notification per key within the cooldown. The key is the attention key if present, else the worktree, else the worktree label, else a global key. The Settings test bypasses it.
- Wording: the title shows the worktree context and agent name. The status text is "needs input" for blocked or waiting, "working" if the state is working (a still-working agent is never announced as finished), "failed" for failure or interruption, "stopped" for cancellation, superseded or unconfirmed, and "finished" otherwise. The body uses the agent's last message when available.
- macOS: a helper binary reads the app's notification authorization and alert settings, because the Electron API cannot. If permission is denied or not yet decided, nothing is shown, and the renderer shows its own fallback; the reason is that macOS would drop the notification silently. Other platforms deliver directly.
- A custom sound makes the native notification silent (STAT-21).

Depends on: STAT-19, STAT-21, Settings, Terminals and panes.

### STAT-21 Notification sounds

What the user does: picks a sound and a volume in Settings.

- Built-in sounds: system (default), two-tone, bong, thump, blip, sonar, blop, ding, clack, beep (nine bundled files plus the system sound).
- Custom file: mp3, ogg, wav, m4a, aac or flac, up to 10 MB, selected by absolute path. The core checks the extension and passes the path to the app's audio bridge.
- Volume: 0 to 100, default 100.
- With the system sound, the OS notification plays its own sound (macOS needs the default sound requested explicitly). Any other choice makes the native notification silent and the app plays the file.
- Limit: a custom sound needs the app to be running. This is inferred from the design; the playback code was not read.

Depends on: STAT-20.

### STAT-22 Phone alerts and desktop-away gating

What the user gets: paired phones receive the same agent alerts, and the phone knows whether the desktop would show them too.

- Every accepted alert goes to connected phones first. The same alert is also offered to a push gateway for phones with no open connection. Retry after 2 seconds; at most 20 phone registrations per send; title up to 80 characters; body up to 180.
- The push state mapping applies to completion notifications only. Blocked and waiting map to "needs input"; done and finished map to "finished"; any other state (for example working) maps to no value. The mapping was read; the send path was not traced.
- An alert the execution host already pushed to phones is not sent again.
- Desktop away: the desktop counts as away when the system has been idle for 180 seconds or is locked. A phone can register to receive pushes only while the desktop is away, and each alert carries the away flag. Unknown presence never blocks a push.
- Test notifications are excluded.

Depends on: STAT-19, STAT-20, Mobile pairing and push gateway (Android domain).

### STAT-23 Keep-awake while agents work

What the user does: chooses On, Auto or Off in Settings.

- On: keeps the computer awake while Orca runs.
- Auto: keeps it awake only while at least one agent row is `working` and was observed in this run. Rows restored from disk do not count, and a working row that has not renewed for 2 hours stops counting.
- Off: the default when nothing is stored.
- Mechanism: an Electron power-save blocker that prevents display sleep. On macOS a system-sleep assertion through a caffeinate child process, with the blocker as fallback. On Linux a systemd-inhibit lid-sleep inhibitor, retried every 30 seconds after a failure. On Windows lid close is not covered, because doing so would change the user's global power plan.
- The active state is published to the UI. A resume from sleep re-evaluates the decision.
- An older boolean setting is kept in sync.

Depends on: STAT-01, Settings, OS power interfaces.

### STAT-24 Rename workspace from first agent work

What happens: when the first agent turn runs in a new workspace whose branch still has an automatically generated name, Orca replaces the branch with a short name derived from the work, using the agent's own text generation. For folder workspaces it also renames the folder and the workspace title.

- Only the first such event renames. Replayed events do not.
- Names are made unique against existing branches.
- Remote repositories use the remote git provider.

Depends on: STAT-02, Git and workspaces, text generation.

### STAT-25 Repository setup and archive hooks (orca.yaml)

Name collision: this is not an agent hook. The root `hooks.ts` and `effective-hook-config.ts` implement repository lifecycle scripts declared in orca.yaml. Setup scripts run when a workspace is created and archive scripts when it is removed. Shared and local script sources are combined by policy (local only, shared only, or both). Scripts run through the local shell or the WSL target. An archive gate (`worktree-archive-hook-gate.ts`) governs removal when an archive hook cannot run (not read). Only headers and names were read.

Depends on: Workspaces and git (workspace domain). Decide in that domain.

### STAT-26 Delegated parent link on status rows

What the user sees: in a paired client, a structured child session that a parent chat delegated appears under that parent, with its dispatch state.

- Source: pv96 (unmerged). New feature: the status summaries of delegated child sessions name their parent, so clients can nest them.
- The link is read from the host's delegation record for each status snapshot and each status event of a structured session. It carries the parent session, the task, the dispatch identity and the dispatch status.
- A paired client has no other way to learn the link, and the link outlives the child's dispatch.
- A failed lookup leaves the rows as they are: they still list, only without nesting. Older hosts send no link, so clients show no nesting.
- The link applies to structured-session rows only. Terminal panes keep their own parent record (STAT-14).

Depends on: AG-26, AG-15, STAT-01, STAT-14.
