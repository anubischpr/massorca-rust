# MassOrca app flow

Validated by the owner on 2026-11-12. English. Changes since then, decided on the owner's behalf or taken from the pv96 comparison, are in the changelog at the end; the owner should re-confirm them. Items still open are listed under Gaps and Assumptions.

Region and concept names follow [glossary.md](glossary.md). Use those names in every step, table and diagram.

**Purpose.** This file shows how a person moves through MassOrca, step by step, on the desktop app (GPUI) and on the Android companion (React Native). Each step names a screen ID from `screen-inventory.md`, so the flow can be checked against the inventory: every screen a journey needs must exist there, and every missing piece is written down under "Gaps". The visual version of the same flows is `app-flow.html`.

## How to read

- Each journey is one table. Read a row left to right: the step, the screen, what the user does, what the system answers, and what can go wrong (error, empty, offline, disconnected host).
- Screen IDs are copied from the inventory. A step that needs a screen with no ID says `no ID (G#)`, and the gap is listed below. No ID is invented.
- Where the inventory merged a screen into another, the flow uses the target screen's ID. The merged row is named only when it helps.
- Where the inventory left a choice open, the flow picks a default and lists it under "Assumptions".
- Every keyboard shortcut in this file is written as `Ctrl+K / Cmd+K` and checked in "Keyboard".

## Legend

| Mark | Meaning |
|---|---|
| `D-`, `M-`, `X-` | Desktop screen, Android screen, shared concept on both platforms |
| `D-SL-`, `D-EX-`, `D-CU-` | Extension slot, extension screen, customisation preference |
| `(A#)` | Assumption: a default picked for an open question |
| `(G#)` | Gap: a step needs a screen or fact the inventory does not provide |
| `[state]` | A major state from the inventory, for example `[disconnected]` |
| `→` | Next step or next screen |

## Journeys

### J1 First launch and setup (desktop)

Goal: a working desktop with the local service running, agent accounts carried over from the current app, a theme chosen, and session import offered but optional.

| step | screen ID | user action | system response | states and branches |
|---|---|---|---|---|
| 1 | D-SH-07 | Opens the app. | Loads saved UI and session state. Starts the local service in the background, because the core runs as a local process. | [loading] [ready]. Service fails to start: no ID (G1), so the shell shows [failed] with a retry. |
| 2 | D-OB-01 | Starts setup: wizard step 1 of 3, agent accounts. | Shows agent accounts and the default-agent choice (D-OB-02 merged here). | Skip: X-DLG-01 confirm (D-OB-07), then the checklist (D-OB-12). |
| 3 | D-AC-04 | Chooses to find accounts from the current app. | Finds accounts and subscriptions and lists them by name. Secrets are never shown. | [none found] → step 5 (sign in by hand). [found] → the owner picks. [importing] [failed]: retry that account only. No OS secret store: the account stops and D-OT-04 says why (A6). |
| 4 | D-AC-05 | Reviews the migration result. | Lists migrated, needs sign-in and skipped, with one action per account. | A credential that fails its check moves to [needs sign-in] (A1). |
| 5 | D-AC-01 | Signs in the accounts still open, one provider at a time. | Runs the generic sign-in flow with the provider's steps. | [waiting for browser or code] [failed]. Later expiry uses the X-AC-01 card. |
| 6 | D-OB-03 | Wizard step 2 of 3: picks a theme. | Shows a live preview of the frame and canvas tokens (the schematic in `app-flow.html`). | Follows the OS theme until chosen (A8). |
| 7 | D-OB-01 | Wizard step 3 of 3: notifications and the telemetry choice. | Saves the preferences. Telemetry is one explicit choice, off by default (A15). | Notifications refused by the OS: X-NT-02 stays off for that source. |
| 8 | D-OB-12 | Reads the setup checklist. | Shows open and done items: add host (J2), create workspace (J3), import sessions (J9). | [in progress] [complete]. Import is a checklist item, not a wizard step (A2). |
| 9 | D-AG-01 | Finishes setup and lands on the start page. | Shows the start page with no workspace chosen. | [empty]: no workspace yet, so J3 comes next. |

### J2 Add a remote host and pair it

Goal: a headless server or an SSH host appears in the sidebar as a connected host, with its workspaces listed. "Pairing" here means the desktop trusting a server. Phone pairing is J12.

| step | screen ID | user action | system response | states and branches |
|---|---|---|---|---|
| 1 | D-WS-01 | Chooses "Add host" in the sidebar tree (A7). | Opens the add-host dialog. | With no hosts, the tree shows local projects only. In Bot Teams mode, switch to Workspaces first (D-SH-10). |
| 2 | D-HO-02 | Picks SSH or server. | SSH: lists hosts from the SSH config. Server: asks for the address and the pairing secret the server shows (G4). | [form] [SSH config picker]. |
| 3 | D-HO-06 | SSH only, when the key needs a passphrase: enters it. | Uses it for this connection and never logs or shows it again. | [prompting] [wrong: asks again] [saved]. |
| 4 | D-HO-02 | Runs the connection test. | Connects and checks the protocol version and the credentials. | [testing] [failed] → X-DG-01 (connection logs, troubleshooting steps, local network test). |
| 5 | D-HO-07 | Checks the new server under servers and access. | Shows the server state and any update it needs. | [connected] [reconnecting] [update available]. Auth failed: pair again. Protocol mismatch: see G3. |
| 6 | D-HO-01 | Sees each project in the sidebar tree with one row per host it lives on. | Lists the project's sessions under each host, with child sessions nested (A97; no workspace row). | [connected] [connecting] [disconnected] [unreachable] [auth failed]. |
| 7 | X-DLG-01 | Renames or removes the host (D-HO-03 merged here). | Rename is a one-field confirm. Remove names what is lost. | Cancel keeps the host unchanged. |

### J3 Create a workspace (git worktree or plain folder)

Goal: a new workspace exists under a project, optional setup has run, and the start page opens on it.

| step | screen ID | user action | system response | states and branches |
|---|---|---|---|---|
| 1 | D-WS-01 | Chooses "New workspace" on a project row (D-WS-02) or in the sidebar header (A7). The sidebar's + Create opens the start page instead (D-AG-01). | Opens the composer for that project. | Remote project whose host is down: X-CN-01 banner, creation disabled. In Bot Teams mode, switch to Workspaces first (D-SH-10). |
| 2 | D-WS-09 | Only if no project exists yet: picks a source (clone, create, nested import, remote, server start, GitHub account). | Adds the project. A folder that is not a git repo is added as a plain-folder project. | [non-git folder]: a No Git pill in the sidebar with an Initialize Git action (D-WS-11), a state of this step. [nested scan limit] inline notice (D-WS-13). |
| 3 | D-WS-14 | Picks the type, location, base ref and parent. Pickers are X-PK-01. | Validates the choices before anything is created. | [validating] [base ref missing]: the default branch is offered (A5). Type choice: see G5. |
| 4 | D-WS-14 | Creates the workspace. | Starts creation; the new row appears in the tree (D-WS-02). | [creating] → step 5. Auto-rename failure: X-NT-01 toast with a retry. |
| 5 | D-WS-15 | Watches creation. | Shows progress in a Work panel tab, in place of a terminal (the pv96 build is not verified). | [pending] [done] [failed]: the log stays visible; the partial workspace can be removed. |
| 6 | D-WS-20 | Only if the repo has hook scripts: trusts or declines them. | Runs hooks only after the explicit trust step. | [untrusted] [trusted] [changed]: asks again. |
| 7 | D-WS-14 | Accepts or declines the setup script offer. | Runs the script in a terminal pane (D-TM-02) when accepted. | [setup offered] [running] [failed]: X-NT-01 toast with the log. |
| 8 | D-AG-01 | Starts an agent (J4) or opens a terminal (J5). | Opens the start page with this workspace chosen. Its placement row (host, project, branch or new worktree) sits above the composer, with a Terminal / Chat switch beside Access (pv96 build). | [workspace chosen] [starting]. |
| 9 | D-WS-16 | Later: deletes the workspace. | Warns about unsaved and unpushed work and about nested children. | [clean] [unsaved] [unpushed] [nested children]. |

### J4 Run an agent in a workspace

Goal: a session runs in a workspace; the owner watches its status, answers approvals, and then finishes, continues or resumes it.

| step | screen ID | user action | system response | states and branches |
|---|---|---|---|---|
| 1 | D-WS-02 | Selects the workspace row. | Opens the workspace. The row shows the agent status through X-AG-01. | A sleeping workspace wakes first (state of D-WS-01). In Bot Teams mode, switch to Workspaces first (D-SH-10). |
| 2 | D-AG-01, X-AG-02 | Picks the agent, model and effort in the model picker (X-AG-02) and access in the composer chip next to it, the owner's component. Its More models fold holds the models beyond the featured set, and access is the composer's own chip (C3). Then writes the first message. | Starts the session on the chosen agent, model and effort. | [agent signed out] → X-AC-01 card first. [agent missing] → D-ST-02 shows it. |
| 3 | D-AG-03, D-AG-27 | Watches the Conversation open, with its Session summary cards (Checklist X-CH-11, Results). | Shows the transcript and composer; the session gate shows the start. The Session summary (D-AG-27) shows a card only when its first record arrives, never an empty card. | [loading] [session gate] [empty] [error: retry]. |
| 4 | X-AG-01 | Watches status without opening the chat. | Updates the row, the tab and the agents overview (D-AG-22). | Host unreachable: no status is claimed (G2). |
| 5 | X-CH-02 | Approves or denies a tool use, or answers a question (single choice, multi-select or free text). | Sends the answer and the run continues. | [pending] [submitting] [answered] [error: same card, retry]. |
| 6 | X-CH-03 | Sends a follow-up while the agent works. | Queues the message and sends it when the agent is ready. | [queued] [sending]. Host disconnected: waits until the host confirms (A10). |
| 7 | X-NT-02 | Is in another window when the agent needs attention. | Sends an OS notification; a click opens the card in D-AG-03. | Off for this source: no notification, but the row still shows the state. |
| 8 | X-AG-01 | Sees the run end. | Status returns to idle; the changes are ready for review (J6). | [error]: the last error stays on the card. |
| 9 | X-CH-05, X-CH-06 | Reads the plan and any delegated work. | Plan view; delegation card with the reason. | Plan [shown] [scrolled]; child work [running] [done]. |
| 10 | X-DL-01 | Watches the coordinator agent launch child agents. | Child agents, sequential or parallel, show as a tree inside the session, each with its status. Each child runs in its own worktree by default; the current worktree only for read-only children, folder workspaces or a manual pick on the card; a child writing in the current worktree makes the parent wait; parallel children always get their own worktree. In the sidebar, children nest under the parent with a Child session badge, and an automatic router picks the agent (pv96 build). | [sequential] [parallel] [child running] [child done] [parent waiting]. Depth limit: a setting in D-ST-02. |
| 11 | D-AG-05 | Continues or forks an earlier session. | One dialog for both; a fork opens in a new pane. | [choosing] [running]. |
| 12 | D-AG-06 | After an app restart, resumes runs. | Lists runs with the status of each. Resume opens the chat. | Per run [ok] or [failed]; [none]. |
| 13 | X-CH-08 | Brings a finished child back into the parent worktree (Bring back). | Merges the child's commits into the parent worktree (git merge --no-ff). Nothing is pushed. | [conflict states] (pv96 build, not validated). |

### J5 Terminals: open, split, detach, survive a UI restart

Goal: terminals keep running when the window closes, and they come back in their layout. Terminals live in the Work panel (D-SH-17).

| step | screen ID | user action | system response | states and branches |
|---|---|---|---|---|
| 1 | D-TM-01, D-SH-17 | Opens a terminal in the workspace as a new tab in the Work panel, with Ctrl+T / Cmd+T (pv96, not checked). | Starts a PTY in the workspace folder in the core and shows it in a new pane. | [starting] [restoring] [running]. Start fails: [exited] with the error. Ctrl+Shift+B / Cmd+Shift+B opens a browser tab the same way. Ctrl+W / Cmd+W closes the focused Work panel tab, never the session (pv96, not checked; see Keyboard). |
| 2 | D-TM-01 | Splits a pane by dragging it, or from the context menu (D-TM-08). | Splits the pane tree. Both panes keep running. | Drop overlays show where the pane lands while dragging. |
| 3 | D-TM-02 | Runs a shell command or an agent TUI. | Streams output with scrollback. File drop and paste work. | [running]. A terminal write to the clipboard follows the setting (D-TM-07). |
| 4 | X-DLG-01 | Closes a running pane. | Asks whether to end the process or keep it running (A9). | End: the process stops. Keep running: the pane closes and the process stays in the core (see G6). |
| 5 | D-SH-07, D-TM-01 | Closes the window and opens the app again. | The core kept the PTYs. The layout restores from saved state. | [restoring] → [running] or [exited]. Restored banner in D-TM-02. Layout save failed: D-TM-05. |
| 6 | X-CN-01, D-TM-02 | A terminal on an SSH host loses its connection. | The pane shows [reconnecting]; the banner shows [disconnected]. | [restored] when the link is back. Host unreachable: last output kept, no exit claimed (G2). |
| 7 | D-TM-18 | Another device drives the pane, for example the phone. | Overlay shows [driven]. It clears on [released]. | One overlay covers terminal and browser panes. |
| 8 | D-TM-13 | Uses the quick terminal panel. | Toggles, maximises and minimises the floating panel. | [hidden] [shown] [maximised] [minimised] [empty]. |

### J6 Review diffs and source control

Goal: the owner reviews what an agent changed, comments on it, stages and commits, then checks CI or opens a PR.

| step | screen ID | user action | system response | states and branches |
|---|---|---|---|---|
| 1 | D-SH-04, D-DF-02 | Opens the Source control tab in the Project sidebar after the run ends. | Lists staged and unstaged changes, the branch against its base with ahead and behind counts, a PR chip, and Commits in branch with +/- counts and a status letter per file. | [clean] [changes] [detached] [conflicts]. Git missing: banner in D-DF-07 (D-WS-21 merged). |
| 2 | D-DF-01, D-SH-17 | Clicks a changed file in the list or in Commits in branch. | The diff opens as a file tab in the Work panel, with inline change markers. Its toolbar: rendered or source, split, previous or next change, edit, more. | [loading] [empty] [large diff: asks before rendering] [conflicts skipped: notice]. |
| 3 | X-RV-02 | Writes comments on lines or files, then sends them to the agent. | Saves drafts. Sent notes arrive as messages in the agent chat. | [draft] [sent] [none]. Agent busy: notes join the queue (A10). |
| 4 | D-ED-07 | Resolves a conflicted file. | Opens the conflict review for that file. | [conflicted] [resolved]. |
| 5 | D-DF-02 | Stages files and writes the commit message (AI assist in D-DF-07), then clicks Commit. | Stages the files. Commit creates a commit on the branch. The menu beside Commit is in the reference but not designed. | Detached HEAD: the badge warns before commit. Repo hooks: trust first (D-WS-20). |
| 6 | D-DF-02 | Asks an agent to commit or open a PR (D-DF-03 merged here). | The agent runs the action; its progress shows in the chat (D-AG-03). | A failure appears in the chat with its reason. |
| 7 | D-SH-04, D-DF-04 | Opens the Checks tab in the Project sidebar, then checks CI for the branch or PR. | Shows the check runs with details. | [loading] [none] [failing] [passing]. |
| 8 | X-PR-01 | Links an existing PR, or creates one with a base chosen in X-PK-01. | Shows details, comments and checks of the PR. | [not linked] [linked] [creating] [error]. Branch not pushed: see G7. |

### J7 Browser pane: open a dev server preview and interact

Goal: a dev server running in a workspace opens in the browser pane. The owner clicks through it and can hand an element to the agent.

| step | screen ID | user action | system response | states and branches |
|---|---|---|---|---|
| 1 | D-TM-02 | Starts a dev server in a workspace terminal in the Work panel. | The output shows the URL, and the port is offered as a candidate for the workspace. | Candidate list: pv96 has a Ports tab, for SSH hosts only (D-SH-04). The owner's Project sidebar list does not name it, so the candidate list still has no row (G8, partly answered). |
| 2 | D-BP-01 | Opens the candidate URL in a browser tab in the Work panel. | Loads the page under a stable local name for the worktree (name format open). | [loading] [error: error page with the reason, for example connection refused]. |
| 3 | D-BP-01 | Navigates with the address bar, back, forward, reload or a new tab. | Updates the title and URL. An error shows only for the newest navigation. | [error] stays until a newer navigation commits. |
| 4 | D-FD-01 | Finds text on the page: `Ctrl+F / Cmd+F`. | Highlights matches; next and previous move between them. | [closed] [open] [matches] [none]. Caveat in G16. |
| 5 | D-BP-02 | Grabs an element and annotates it for the agent. | Shows a confirm sheet; the element's HTML, styles and crop go into the agent prompt. | [grab] [confirm] [markup]. |
| 6 | no ID (G9) | Opens a site with an invalid certificate. | The warning page shows. The first version offers no trust decision (BRW-18). | Fails closed. No inventory ID exists for this page. |
| 7 | D-BP-06 | Signs in with a passkey. | The app's account picker opens; the owner picks an account or cancels. | Passkeys may come after the first version (G14). |
| 8 | D-TM-18 | Another device drives the pane. | Overlay shows [driven]; it clears on [released]. | Same overlay as terminals (J5). |
| 9 | D-BP-01, D-BP-05 | Opens a dev server that runs on an SSH host. | Pane traffic leaves from that host. The agent may drive the page only when D-BP-05 allows it. | Host disconnected: the page shows [error]. Traffic never falls back to a direct connection. |
| 10 | D-ED-01 | Opens a file that an agent session recorded. | Mod+Alt+click opens it in its own Work panel tab, beside the terminal. Only the session's written files can be read (pv96, not checked; see Keyboard). | [loaded]. Plain Mod+click keeps its dialog behaviour. |

### J8 Settings: change a preference and keybindings

Goal: the owner finds a setting, changes it, and reassigns a shortcut without leaving a conflict.

| step | screen ID | user action | system response | states and branches |
|---|---|---|---|---|
| 1 | D-SH-02, D-SH-16 | Opens settings: the settings view, from the rail (D-SH-16), the sidebar header menu (D-WS-07) or the command palette (D-CP-01). | Shows the settings view with its sections, D-ST-01 to D-ST-11. The owner's ten sections are kept; pv96 still uses the old taxonomy, with Share Skills listed (see the changelog). | Project settings (D-ST-11) sit in the project's own menu, not in this list. |
| 2 | D-CP-04 | Searches for a setting by name. | Jumps to the matching section and highlights the target briefly. | [results] [no match]. |
| 3 | D-ST-03 | Changes the theme or a layout option. | Applies at once; the frame and canvas follow the tokens (A11). | Theme import: D-CU-03 (merged here). |
| 4 | D-ST-04 | Opens the keybinding editor and records a new key for an action. | Shows the recorded key and any conflict with core or extension shortcuts. | [recording] [conflict] [filtered]. Escape while recording: not read (G10). The pv96 shortcuts in Keyboard are not yet checked for conflicts. |
| 5 | X-DLG-01 | Confirms a reassignment when the key is already taken. | Moves the binding; the previous action loses the key. | Cancel keeps both bindings unchanged. |
| 6 | D-ST-10 | Checks the privacy options. | Shows telemetry as off unless the owner turned it on. | Crash report: D-OT-01 previews what would be sent. Nothing is sent without consent. |

### J9 Import a session from another agent tool and continue it

Goal: a session started in another agent tool continues in MassOrca. Import runs on the host where the sessions live.

| step | screen ID | user action | system response | states and branches |
|---|---|---|---|---|
| 1 | D-IM-01 | Starts an import from the setup checklist (D-OB-12) or the start page (D-AG-01) (A7). | Detects installed tools (Claude Code, Codex, OpenCode, Gemini CLI, others) and says what it found before it reads anything. | [detecting] [found per tool] [none] [access needed] → step 2. |
| 2 | D-IM-04 | Only if access is needed or a path is not found: sets where a tool's data lives. | Stores the path. Access is read-only. | [default path] [custom path] [not found]: the tool can be skipped. |
| 3 | D-IM-02 | Selects sessions and checks each mapping. | Maps sessions by repository path. Unmapped sessions are asked about once, in one batch. | [list] [selected] [unmapped] [already imported]. |
| 4 | D-IM-03 | Runs the import. | Copies the selected sessions into MassOrca's store (A2) and shows progress. | [running] [partial: what succeeded is kept, each failure named] [failed] [done]. |
| 5 | X-IM-05, D-AG-03 | Opens an imported session. | The transcript header shows the origin tool and the import date. | [imported] [continued here]. A missing source file is named in the result. |
| 6 | D-AG-03 | Sends a message to continue the session. | Runs on an agent account and switches the marker to continued here. | Agent signed out: X-AC-01 card. Agent tool not installed: D-ST-02 shows [missing]. |
| 7 | M-SE-03 | On Android: sees the session with its badge. | The sessions switcher shows the X-IM-05 badge; history lists it in M-SE-06. | Host offline: the last known list is shown. |

### J10 Install an extension

Goal: the owner installs an extension from a source, reviews and grants what it asks for, enables it, configures it, and sees its panel appear.

| step | screen ID | user action | system response | states and branches |
|---|---|---|---|---|
| 1 | D-ST-09 | Opens Extensions settings (A7). | Lists installed extensions and the sources they come from. | [enabled] [disabled] [update available]. |
| 2 | D-EX-01 | Browses or searches the sources and reads about an extension. | Shows results grouped by source. From the rail, Marketplace (D-SH-13) opens this browser as a full page. pv96 builds it as a catalog of tools, agents, skills and services, with item pages (D-EX-08). | [loading] [empty] [error: per source] [freshness nudge]. |
| 3 | D-EX-02 | Opens an extension and reads what it asks for. | Shows the requested permissions and a risk notice before anything is installed. | [reviewing]. Not verified in pv96 (D-EX-02). |
| 4 | D-EX-02 | Confirms the install and grants the permissions (A12). | Installs the extension and records its version, so it can roll back. | [installing] [installed] [failed] [rollback]. |
| 5 | D-EX-03 | Checks it in the extension manager. | Shows it enabled (A13). Enable or disable from here. | [enabled] [disabled] [error: shows the log] [update available]. |
| 6 | D-EX-04, D-SL-06 | Configures the extension in its own settings section. | The section appears under settings. | [shown] [hidden]. |
| 7 | D-SL-01, D-SL-02, D-SL-07 | Sees the extension's panel appear. | The panel is a Project sidebar tab (D-SL-01). Status items (D-SL-02) and commands (D-SL-07) appear where the extension declares them. | Panel [open] [closed]. Shortcut conflict: resolved in D-ST-04. |
| 8 | X-DLG-01 | Removes the extension or rolls it back. | The confirm names what is lost. Rollback restores the previous version. | [busy] [error]. Removal and rollback both go through this confirm. |

### J11 Create my own extension or in-app application

Goal: the owner builds an extension from a template, runs it, iterates on its code, and disables or removes it when done.

| step | screen ID | user action | system response | states and branches |
|---|---|---|---|---|
| 1 | D-EX-05 | Starts a new extension from a template (from D-ST-09 or the command palette D-CP-01). | Creates the template files in a folder (G11). | [new]. |
| 2 | D-EX-05 | Loads the extension. | Loads it and shows its logs. | [loaded] [error: the log shows the failing line]. |
| 3 | D-ED-01, D-ED-02 | Edits the extension's code in the editor. | Saves the files. Edits made outside the app show the external change banner (D-ED-06). | [dirty] [saved]. |
| 4 | D-EX-05 | Reloads the extension and reads the logs. | Reloads the code and shows the new logs. | [loaded] [error]. |
| 5 | D-EX-06 | Runs the in-app application in a Project sidebar tab (D-SL-01) or a main view (D-SL-04). | Runs the application in its slot. | [running] [error: log and reload]. |
| 6 | D-CU-01 | Saves a theme or layout preset that the extension works with. | Saves and switches presets; a preset can be shared. | [saved] [active]. |
| 7 | D-EX-03 | Disables the extension when done, or removes it. | Disable keeps the files. Removal goes through X-DLG-01. | [enabled] [disabled] [error]. |
| 8 | no ID (G12) | Shares the extension with other people. | No screen publishes an extension. D-CU-01 shares presets only. | Blocked until G12 is decided. |

### J12 Android companion: install, pair, follow a session, reconnect

Goal: from a fresh install, the owner pairs the phone with a desktop or server, follows an agent session, approves a request, uses the terminal, and recovers after a disconnect. iOS is out of scope.

| step | screen ID | user action | system response | states and branches |
|---|---|---|---|---|
| 1 | no ID (G13) | Installs the app from the store. | Happens outside the app. | Android only. |
| 2 | M-HM-01 | Opens the app with no host. | Shows the empty home. Its top bar holds the Bot teams toggle and Tasks (pv96). | [empty]. |
| 3 | M-PA-01 | Reads the three-step hint. | Explains how to connect the desktop. | [no hosts]. |
| 4 | M-PA-02 | Scans the QR code shown on the desktop (D-PR-01), or uses paste mode. | Reads the code and connects. | [scanning] [connecting] [invalid code] [camera denied: paste mode] [failed]. |
| 5 | M-PA-04 | Confirms the desktop by its name. | Pairs once the owner confirms. | [awaiting confirm] [connecting] [failed]. |
| 6 | M-SH-02 | Nothing to do unless the versions differ. | Checks protocol versions. Blocks use when they differ and says which side must update. | [compatible] [blocked]. Desktop side: see G3. |
| 7 | M-PA-05 | Sets the default view and notifications. | Shows only the steps still undecided. | [pending steps only] [error]. |
| 8 | M-NT-01 | Allows agent notifications. | Requests the OS permission and sends a test notification. In the Settings menu, Marketplace follows Notifications (M-EX-01; its Android place is open, A24). | [permission off] [on]. |
| 9 | M-HM-02 | Reads the host card on home. | Shows the host status and its agent usage rings (proposal, A24). | [ok] [warning] [unreachable] [auth failed] [credential missing]. |
| 10 | M-WS-01 | Opens the host and sees its workspaces. | Lists the workspaces. On wide layouts the list is a master pane (M-SH-01). | [loading] [catalog error: retries itself] [empty]. |
| 11 | X-WS-01, M-WS-02 | Filters with the sheet, or long-presses a workspace. | The sheet filters; the action sheet offers session, files and source control (A14). | [no match] (filter). |
| 12 | M-SE-01 | Starts a session in a workspace. | Opened from a FAB, the header + or Home. Chips and sheets pick the agent and base ref, then show progress (pv96 rebuild). | [progress] [composer row]. |
| 13 | M-SE-02, M-CH-01 | Follows the session and answers approvals. | Chat and status (X-AG-01), with a working status pill above the composer (M-SE-07). A Sessions chip opens the sessions sheet, and Resume an earlier session replaces Agent History (M-SE-03, M-SE-06). Approval cards (X-CH-02) answer in place. Summary (M-CH-07) and Artifacts (M-AR-01) are one tap away; Plan is in the overflow menu (M-CH-05). Prototypes (M-PT-01, M-PT-02) open from the overflow menu (pv96; G19 closed). | [loading] [waiting] [disconnected] [error] [pending terminal]. |
| 14 | M-SE-02, M-TM-01 | Uses the terminal with the key row (M-TM-02). | The terminal renders in the session screen. | [loading] [engine error] [live]. Renderer choice: see G17. |
| 15 | X-CN-01, M-HM-02 | The host drops off the network. | The banner shows [reconnecting] or [disconnected]; the host card shows [unreachable]; queued messages wait. | Last known status is shown with its age (G2). Reconnect shows the [restored] banner. |
| 16 | X-DG-01 | Reconnect keeps failing. | Shows logs and troubleshooting steps for the host (screen rewritten in pv96). | [ok] [warning] [failed]. |
| 17 | M-PA-02 | Pairs again when the credential is missing or the host refuses auth. | Same as step 4. | [credential missing] [auth failed] → pair again. |

### J13 Bot Teams: switch mode, talk to a team, track work

Goal: the owner switches the rail to Bot Teams, talks to a team in its Conference room, follows the tasks on its Board, schedules routines, and checks them on the Calendar. The Tasks page covers both modes.

| step | screen ID | user action | system response | states and branches |
|---|---|---|---|---|
| 1 | D-SH-10 | Clicks the mode switch in the rail's top slot, from grid (Workspaces) to bot (Bot Teams). From any other page, the first press returns to the current mode's home; a press at home switches the mode (decided on the owner's behalf; pending-owner-review.md, item 6). | The icon morphs to the bot icon, and the sidebar switches to Bot Teams. | [Workspaces] [Bot Teams]. Reduced motion: instant swap. |
| 2 | D-WS-29 | Looks at the sidebar. | Lists the teams, with search and All teams. Each team shows its Conference room and Board, with status hints, and a Projects section (Files, Memory, Routines). Projects also has Filters, New project and Show archived (pv96; not in the owner notes). | [empty]: no teams or bots yet. Bots are added in D-AG-13. |
| 3 | D-AG-24 | Opens the team's Conference room and states a requirement to the team's bots. | Detects the requirement and creates a task on the team's Board (D-WS-30). The room has an @ mention menu (X-CH-09), and a request choice card when no bot or team matches (X-CH-10). | [chat open]. Composite requirement (dashed): the lead bot takes the task and delegates the parts by competence, shown in X-DL-01. |
| 4 | D-WS-30 | Sees the new task on the team's Board. | Lists the task. | [no tasks] → [tasks listed]. Opening a task has no detail row yet (G20, partly closed: the task also has its own thread in the room). Per-project boards (D-WS-31) list their own tasks. |
| 5 | D-AG-24 | Asks a bot to schedule a task for later. | The bot creates a routine. | [routine created]. |
| 6 | D-AG-25 | Opens Routines in the team's Projects section. | Lists the routines, including the one the bot created. New routine adds another. Web addresses start one task each. New routine opens its own form in pv96 (D-AG-34); whether it opens the schedule editor (A26) is for the owner. | [No routines yet] [routines listed]. [No web addresses yet] [web addresses listed]. |
| 7 | D-AG-23 | Opens the Calendar from the rail. | Shows the routine on the timeline, with automations too; the filter chips separate them (A26; owner design kept over the pv96 legend). | [loading] [empty] [loaded]. |
| 8 | D-SH-11, D-WS-24 | Opens Tasks from the rail. | Shows one Tasks list that covers both modes. Bot Teams tasks come from the Boards (A25). | [loading] [connect] [error] [empty], per provider. No provider connected: connect first. |
| 9 | D-SH-10 | Switches back to Workspaces. | The icon returns to the grid, and the sidebar lists projects and hosts again. | [Workspaces]. |

### J14 Give an agent a secret

Goal: the owner gives an agent a credential (API key, token, password) without pasting it into the chat, from the desktop or the phone. The value is stored on the host, and the agent receives it only at run time.

| step | screen ID | user action | system response | states and branches |
|---|---|---|---|---|
| 1 | D-AG-03 | Asks in the chat how to give a secret, for example the OpenAI key. Or the agent finds that it needs one. | Shows a secret request card (X-CH-07) in the conversation, naming the variable. On Android the same card appears in the session chat (M-CH-01). | [user asked] [agent needs one]. The message never carries the value. |
| 2 | X-CH-07 | Chooses Add on the card. | Opens the entry modal (X-VT-01) with the variable name filled in from the card. | [pending] until the modal closes. Decline: the card shows [declined] and the agent is told (A33). |
| 3 | X-VT-01 | Types the variable name and the value. The value is masked; reveal is on request. | Checks the name and that the value is not empty. | [invalid name] [name in use in this scope] (A29). Fix and retry. |
| 4 | X-VT-01 | Picks the scope: project (preselected), team (Bot Teams) or global. Or picks Link an existing secret (J14b). | Shows which scope will hold the value. | [project] [team] [global]. |
| 5 | X-VT-01 | Saves. | Stores the value on the executing host, in its OS secret store or an encrypted store on a headless server (G22). Only the name and a stored state go back to the chat. | [saving] [stored] [failed: retry]. Host unreachable: nothing is saved and nothing is queued (A32). |
| 6 | X-CH-07 | Sees the card. | Shows the variable name and the scope, never the value. | [stored]. |
| 7 | D-AG-03 | Watches the agent continue. | At run time the agent gets the value as an environment variable or reference. The transcript shows the name only. | [continues]. A session already running keeps the value it started with (A30). Output that prints the value: G23. |
| 8 | D-VT-01 | Later, opens the vault to see the secret and which agents and sessions used it. | Lists the secret with its scope and usage: names and times, never the value. | [empty] [listed] [host unreachable]. Rotate and delete: J14b. |

### J14b Link a secret to another project, rotate it, delete it at the owner

Goal: the owner links one stored secret to a second project, rotates its value once, and sees every linked scope use the new value. Deleting it at the owner warns with the number of linked projects and teams. Only the owner's own secrets can be linked (A28).

| step | screen ID | user action | system response | states and branches |
|---|---|---|---|---|
| 1 | X-VT-01 | In project B, opens the entry modal from a card and chooses Link an existing secret. | Lists the owner's secrets held in other scopes, each with its owning scope. | [listed] [empty: no other secrets, so enter a new value as in J14]. |
| 2 | X-VT-01 | Picks the secret from project A and saves the link. | Creates a link, not a copy: both scopes refer to one stored value. The name stays the same (open question 10). | [linking] [linked] [failed]. |
| 3 | D-VT-01 | In project B, opens the vault. | Shows the secret as linked, owned by A and read-only in B. The value is not shown (A34). | [linked, read-only]. Unlink in B removes only B's link, with no confirm; the owner keeps the secret. |
| 4 | D-VT-01 | In project A, the owner rotates the secret once. | Sets the new value in the owning scope. Every linked scope uses it from its next run. | [rotating] [rotated]. A session already running keeps its old value (A30). |
| 5 | D-VT-01 | Checks project B. | Shows the same secret with nothing to do; the next run in B uses the new value. | [linked]. |
| 6 | X-DLG-01 | In project A, deletes the secret (D-VT-02 merged here). | Asks first, and names the number of linked projects and teams that lose it. | [default] [shared: linked count] [busy] [error]. Cancel keeps it. |
| 7 | D-VT-01 | The owner confirms the delete. | Removes the secret in every linked scope (A35). In B, the next request for that name returns as a pending card (X-CH-07). | [deleted] [failed]. |

## Cross-cutting states

| State | Where it shows | Rule | Gap |
|---|---|---|---|
| Disconnected host | X-CN-01 banner; D-HO-01 host row; M-HM-02 card; D-TM-02 [reconnecting]; D-AG-03 chat | Show the last known state with its age. Silence is not "stopped". Queued messages send only after the host confirms the session (A10). | G2 |
| Protocol version mismatch | M-SH-02 on Android (names the side to update); D-HO-07 [update available] on desktop | Block the use, not the whole app. Say which side must update. | G3 |
| Auth failure | X-AC-01 (agent accounts); D-HO-06 (SSH passphrase); D-HO-01 and M-HM-02 [auth failed]; X-DG-01 | Ask once, in place, with one action. Secrets are never shown back or logged. | none |
| Long-running tasks | D-WS-15 (creation); D-IM-03 (import); D-AC-04 (migration); D-EX-02 (install); X-UP-01 (update download); X-AG-01 (agents); D-AG-22 (agents overview); D-SH-05 (status bar) | Work runs in the core, so closing the window does not stop it (PLAN). Progress shows where the job started and in the agents overview. | G15 |
| Notifications | X-NT-02 (OS notification for agent attention; on, off or per source in D-ST-08); D-NT-01 (in-app card stack); X-NT-01 (short toasts); M-NT-01 (Android) | The OS notification only announces attention. Acting happens on the card. Toasts carry short results only. | none |
| Surface crash | D-SH-06 (recoverable error screen with retry); D-OT-01 (crash report preview) | One crashed surface does not take down the shell. Nothing is sent without consent. | none |
| Secret request | X-CH-07 (card); X-VT-01 (entry modal); D-VT-01 (vault) | The value never enters the transcript, the model context or logs. The chat shows only the name and a state. With the host unreachable, nothing is saved (A32). | G23 |

## Navigation map

### Desktop

The window has one Frame and one Canvas. The Frame is the Activity rail, the Title bar and the Sidebar as one surface, filled with `surface.frame`, with no seam or border between them (owner requirements of 2026-11-12). The rail is #1b1b1c in dark, not black as in the reference screenshot (A22). The rail is always visible: the sidebar toggle hides only the Sidebar, and with the Sidebar hidden, the Canvas's rounded top-left corner sits right next to the rail. The Canvas is inset in the Frame, with a rounded top-left corner where it meets the Sidebar and the Title bar. Inside the Canvas, left to right, are the Conversation, the Session summary, the Work panel and the Project sidebar. The Status bar is a strip at the bottom, under the Canvas column (A17). Dark tokens: `surface.frame` #1b1b1c and `surface.canvas` #151517 (owner requirement). Light values are in A16. Navigation runs rail → Sidebar → Canvas: the rail picks the mode and the top-level entry, the Sidebar picks a project, host or team, and the Canvas shows it. The schematic is in `app-flow.html`.

**Full pages (owner, 2026-11-12).** Tasks (D-WS-24), Calendar (D-AG-23), Marketplace (D-EX-01), Settings (D-SH-02, sections D-ST-01 to D-ST-11), Automations (D-AG-14) and the Bot Teams pages (D-AG-13) open over the whole canvas. The Conversation, Session summary, Work panel and Project sidebar give way, the Sidebar is hidden, and Back returns to the session. Open point: the team pages D-AG-25 and D-AG-26 are assumed to follow the same rule. Automations opens from its entry in the Workspaces Sidebar (owner decision, 2026-11-12), not from the rail. Artifacts has no page: its items sit in the Results card and open in a Work panel tab (owner decision, 2026-11-12; G26 closed).

| Region | Screens | Surface | Behaviour |
|---|---|---|---|
| Activity rail, full height, far left | D-SH-09 (entries D-SH-10 to D-SH-16) | Frame, filled with `surface.frame`, no seam to the Sidebar | Always visible; the sidebar toggle never hides it. From any other page, the mode switch first returns to the current mode's home. Top group: mode switch, Tasks, Calendar, Marketplace. Bottom group: theme toggle, usage rings, Settings. The mode switch decides what the Sidebar lists. Extensions add icons (D-SL-09). Flow: rail, then Sidebar, then Canvas. |
| Title bar, top, right of the rail | D-SH-03 | Frame | Sidebar toggle, back and forward, session title, region toggles including the Project sidebar toggle (A18) and the Session summary toggle (owner, 2026-11-12), window controls on the right on Windows and Linux and at the left (traffic lights) on macOS. Titlebar tabs stay dropped. This is a band of the Frame across the top (decided on the owner's behalf, pending-owner-review.md, item 2). The sidebar toggle hides only the Sidebar; the rail stays. States: normal; floating when the Sidebar is collapsed; hidden on the workspace view. |
| Sidebar | D-WS-01, D-WS-02, D-HO-01, D-WS-29 | Frame | Tree of projects, hosts and sessions. Projects are the top level; a project can live on several hosts, so each project shows one row per host, with its sessions under it and child sessions nested (A97; the workspace is part of the session, not a row). In Workspaces mode the top holds + Create, Search, Prototypes, Tasks, Automations and MassOrca Mobile, then the Projects section (D-WS-01). In Bot Teams mode (D-SH-10) the Sidebar lists teams, each with its Conference room, Board and Projects (D-WS-29). The sidebar toggle hides it, and full pages hide it too; the rail stays. |
| Conversation | D-AG-03, with the message minimap D-AG-28; Conference room D-AG-24 in Bot Teams mode | Canvas, leftmost | The chat of a session or a Conference room, its message minimap and the composer. |
| Session summary | D-AG-27 | Canvas | Column of summary cards. Only cards with information are shown; a card appears when its first record arrives (animated; instant with reduced motion); never an empty card. Cards: Environment, Plan, Checklist, Context, Results, Subagents, Connectors, Background tasks, QA servers. Can be hidden from the title bar (owner, 2026-11-12). Results items are URLs (Open, Copy URL), file paths and artifacts; files and artifacts open in a Work panel tab. |
| Work panel | D-SH-17; tabs D-TM-01 (terminal), D-BP-01 (browser), D-ED-01 (Markdown or file), D-SH-18 (mobile emulator) | Canvas | Tabs, with a + that adds one. The empty state offers four actions: New terminal, New browser tab, New Markdown, New mobile emulator. Terminals, the browser and the editor live here. Files and diffs open here as file tabs (owner, 2026-11-12). Full pages open over the canvas instead, see below. |
| Project sidebar | D-SH-04: File explorer D-ED-11 (Names or Content search, D-CP-03), Sessions list (the old 'vault' tab, merged), Source control D-DF-02, Checks D-DF-04; extension tabs D-SL-01 | Canvas, rightmost | Tabbed. Open and closed states. Hidden on full pages. |
| Status bar | D-SH-05: agent status, memory use, terminal count, ports, host connection; status slots D-SL-02 | Bottom, under the Canvas column | Shown or hidden through its visibility menu. Usage is on the rail (D-SH-15; full view D-AC-06). |
| Overlays | X-DLG-01 confirm; X-DLG-02 popover and sheet; X-PK-01 picker; X-AG-02 model picker (above the composer); X-VT-01 secret entry modal; X-NT-01 toast; D-NT-01 card stack (holds the update card, X-UP-01); D-CP-01 palette; D-FD-01 find bar; D-TM-13 floating terminal; D-OT-05 dictation | above all | Overlays stack above the canvas; the view behind them keeps its place. |

The top-level view switch (D-SH-02) has thirteen views, with calendar added for the rail. This flow keeps five and places the rest; terminal becomes a Work panel tab:

| D-SH-02 view | Where it goes |
|---|---|
| terminal | Work panel tab, D-TM-01 in D-SH-17 (owner, 2026-11-12); no longer a top-level view |
| settings | Full page: Settings view with sections D-ST-01 to D-ST-11, opened from the rail (D-SH-16) |
| tasks | Full page D-WS-24, opened from the rail (D-SH-11) |
| automations | Full page D-AG-14, from its Workspaces Sidebar entry (owner, 2026-11-12) |
| artifacts | No page. Items of the Results card (D-AG-27), opened in a Work panel tab (owner, 2026-11-12) |
| calendar | New full page D-AG-23, opened from the rail (D-SH-12) |
| create-chat | Merged into the start page D-AG-01 |
| activity | Dropped (D-AG-12) |
| space | Merged into the cleanup page D-WS-18 |
| skills | Merged into D-ST-09 and the extension browser D-EX-01 |
| prototypes | Core page with a Workspaces Sidebar entry (D-AG-17; decided on the owner's behalf, pending-owner-review.md, item 1) |
| bot-teams | A mode on the rail (D-SH-10), not a view. Page D-AG-13 is core (owner, 2026-11-12) |
| mobile | Merged into D-ST-07 (pairing content in D-PR-01) |

The rail's Marketplace entry (D-SH-13) opens the extension browser D-EX-01 as a full page, and is not a view of its own.

### Android

The stack follows M-SH-01: home is the host list, and wide layouts show the list as a master pane beside the detail.

```text
Start, no host:  M-PA-01 -> M-PA-02 -> M-PA-04 -> M-PA-05 -> Home
Home (M-HM-01):  host cards (M-HM-02, with usage rings, A24), resume card, tasks card
                 top bar -> M-ST-01 settings (A14)
  host card -> M-WS-01 workspace list -> long press -> M-WS-02 actions
               host edit: M-HO-01
  workspace -> M-SE-02 session (chat M-CH-01, terminal M-TM-01)
               M-SE-02 -> M-SE-03 sessions switcher | M-SE-06 history | M-SE-01 new session
  actions   -> M-FI-01 files -> M-FI-02 preview
  actions   -> M-DF-02 source control -> M-DF-01 diff review
  tasks card -> M-TK-01 tasks
Every host use sits behind the protocol gate M-SH-02.
The rail entries (D-SH-10 to D-SH-16) have no rail on Android; their proposed Home places are in A24.
Prototypes (M-PT-01) open from the overflow menu (pv96; G19 closed).
```

## Keyboard

Each shortcut is written as `Ctrl+K / Cmd+K`. "Basis" says whether the key comes from the inventory or from a default chosen here (A19).

| Action (screen) | Shortcut | Basis | Windows / macOS / Linux |
|---|---|---|---|
| Command palette (D-CP-01) | `Ctrl+K / Cmd+K` | A19 | Free on all three by default. Keys reach the app first from a browser pane, because the host reads them before the page (BRW-17 pattern). |
| Quick open (D-CP-02) | `Ctrl+P / Cmd+P` | A19 | Free on all three. |
| Search in files (D-CP-03) | `Ctrl+Shift+F / Cmd+Shift+F` | A19 | Free on all three. |
| Find bar (D-FD-01) | `Ctrl+F / Cmd+F` | Inventory (find key) | Free on all three. A full-screen program in a terminal may need the key: G16. |
| Dictation overlay (D-OT-05) | `Ctrl+E / Cmd+E` | Inventory (dictation key) | Free on all three. Shells use the same key for end of line: G16. |
| Recent tab switcher (D-TM-11) | `Ctrl+Tab / Ctrl+Tab` | Inventory (tab switch key) | The same key on macOS, where the system app switcher uses a different key. Free on Windows and Linux by default. |
| Settings view (D-SH-02) | `Ctrl+, / Cmd+,` | A19 | Free on all three. On macOS this is the usual preferences key. |
| Copy in terminal (D-TM-02) | `Ctrl+Shift+C / Cmd+C` | A19 | On Windows and Linux the shell's interrupt key must keep its meaning, so copy takes the Shift variant. |
| Paste in terminal (D-TM-02) | `Ctrl+Shift+V / Cmd+V` | A19 | Same rule as copy. |
| Zoom in, out, reset (D-SH-08) | `Ctrl+= / Cmd+=`, `Ctrl+- / Cmd+-`, `Ctrl+0 / Cmd+0` | A19 | Free on all three. Accept "+" as well as "=" on layouts where "+" needs Shift. |
| Model picker, choose models 1 to 4 (X-AG-02) | `Ctrl+1` to `Ctrl+4` / `Cmd+1` to `Cmd+4` | Inventory (owner requirement) | Active only while the picker is open; outside it, the keys keep their own meaning. Free on all three while the picker is open. |
| Add a terminal tab in the Work panel (D-SH-17) | `Ctrl+T / Cmd+T` | pv96 (desktop simplification S13), from pv96, check collisions | Not checked. A browser pane may use the same key for a new tab. |
| Add a browser tab in the Work panel (D-SH-17) | `Ctrl+Shift+B / Cmd+Shift+B` | pv96 (desktop simplification S13), from pv96, check collisions | Not checked against browser shortcuts. |
| Close the focused Work panel tab (D-SH-17) | `Ctrl+W / Cmd+W` | pv96 (desktop simplification S13), from pv96, check collisions | Never closes a session. On macOS, Cmd+W is the window-close key, which the check below excludes, so this row does not meet the rule as written. Decide before use. |
| Open a recorded file in its own tab (D-ED-01) | `Ctrl+Alt+click / Cmd+Option+click` | pv96 (remote recorded file read T4), from pv96, check collisions | Some keyboard layouts use Ctrl+Alt as AltGr; check that they still type. Plain Mod+click keeps its dialog behaviour. |

How this was checked: each key was compared with the default system shortcuts of Windows, macOS and common Linux desktops (GNOME, KDE, Xfce). None of them is an OS-reserved key, such as window close, app switch, quit, hide, system search, the Windows key, the Linux run dialog or the Linux terminal launcher. This is a check against default bindings, not a test on hardware. The four shortcuts marked "from pv96" were not checked this way; Cmd+W is the macOS window-close key, which this check excludes. Desktop environments can rebind keys, so every shortcut must be changeable in D-ST-04. An extension shortcut that clashes with a core key loses: that rule is assumed, because the inventory does not state it, and the clash is shown in D-ST-04 (D-SL-07).

## Assumptions

Defaults picked where the inventory left a question open, or where a step needs an entry point the inventory does not name.

- **A1** Open question 1 (accounts), owner accepted default 2026-11-12. Copy the stored credentials, check each one on import, and store it in the OS secret store. A credential that fails the check goes to the re-sign-in queue (D-AC-05). An account can be skipped.
- **A2** Open question 2 (session import), owner accepted default 2026-11-12. Imported sessions are copied into MassOrca's store, and the source is only read. Import is a setup checklist item (D-OB-12), not a fourth wizard step, because the wizard is capped at three steps (D-OB-01).
- **A3** Open question 3 (Android OTA web shell), owner accepted default 2026-11-12. Not kept. Every Android screen is native, so M-SH-03 stays dropped. M-SE-06 renders natively, although its inventory row mentions the web shell fallback.
- **A4** Open question 4 (core or extension), owner accepted default 2026-11-12. Bot Teams (D-AG-13) is core again, by owner decision of 2026-11-12, and is switched from the activity rail (D-SH-10). Prototypes (D-AG-17) is core, not an extension: decided on the owner's behalf, 2026-11-12 (pending-owner-review.md, item 1), which supersedes the accepted default. The other three (D-WS-08, D-WS-26, D-ED-10) stay extensions.
- **A5** Base ref missing (D-WS-14). The composer offers the project's default branch as the fallback and says so. The inventory names a fallback but not its target.
- **A6** No OS secret store. The migration stops for that account, and D-OT-04 explains why. The inventory does not say what happens without a store.
- **A7** Entry points the inventory does not name: "Add host" and "New workspace" from the sidebar tree (D-WS-01, D-WS-02); import from D-OB-12 and D-AG-01; the extension browser and create-extension from D-ST-09 (create also from the palette, D-CP-01).
- **A8** The theme follows the OS until the owner picks one (D-OB-03, or the rail's theme toggle, D-SH-14).
- **A9** Closing a running pane asks in X-DLG-01 whether to end the process or keep it running. "Keep running" is the detach action.
- **A10** Review notes and follow-up messages sent while the agent is busy join the queue (X-CH-03). While the host is disconnected, queued messages wait and send after the host confirms the session state.
- **A11** Preferences apply at once. There is no save button, and appearance needs no restart.
- **A12** Extension permissions are granted in the review step, before install (D-EX-02).
- **A13** An installed extension is enabled at once. The manager (D-EX-03) turns it off.
- **A14** On Android, the workspace action sheet (M-WS-02) holds session, files and source control entries. Settings open from the Home top bar (M-HM-01).
- **A15** The telemetry choice sits on wizard step 3 of 3 (D-OB-01), off by default (D-OB-10).
- **A16** Light tokens for the layout. Owner requirement (2026-11-12): the frame (activity rail, title bar and sidebar, one surface) is a slightly darker, tinted neutral than the canvas, and the canvas is the lightest surface, with the same rounded top-left corner. Proposed values: `surface.frame` #eeeef2 and `surface.canvas` #fbfbfc. A pure white canvas (#ffffff) also fits the rule. The owner picks the final values.
- **A17** Resolved (owner reference, 2026-11-12). The Status bar is a strip at the bottom, under the Canvas column. It shows agent status, memory use, terminal count, ports, and hosts with a connection dot (D-SH-05). Usage moved to the activity rail (D-SH-15). The provider switcher and workspace space left the bar, because the reference does not show them.
- **A18** The Project sidebar toggle stays in the Title bar (D-SH-03). The reference image puts a similar toggle at the top right of the canvas.
- **A19** Shortcuts the inventory does not name (command palette, quick open, search in files, settings, terminal copy and paste, zoom) use the defaults in "Keyboard".
- **A20** Top-level pages kept: settings, tasks, automations and calendar (new, opened from the rail or the Workspaces Sidebar). Artifacts is not a view: it sits in the Results card (owner, 2026-11-12). Prototypes is a core page, not an extension view (A4). Terminal is a Work panel tab (D-SH-17), not a view (owner, 2026-11-12). The start page (D-AG-01) takes create-chat. The other seven views merge into their topic, become extension views, are dropped, or, for bot-teams, become a mode on the rail (D-SH-10). The table in "Navigation map" has the detail.
- **A21** The browser engine is not chosen (Phase 0). The flows describe behaviour and assume nothing about the engine.
- **A22** The activity rail runs the full window height at the far left, filled with `surface.frame` (#1b1b1c dark), the same as the sidebar and title bar, with no seam between the rail and the sidebar. The reference screenshot shows the rail black; the owner's colour note overrides that. The title bar and sidebar sit to the right of the rail. The reference crops suggest this, but the owner should confirm the placement.
- **A23** Resolved (owner, 2026-11-12). The calendar (D-AG-23) shows routines, which are scheduled tasks, on a timeline, with automations too (A26). Routines come from the Routines page (D-AG-25) or from a bot in the Conference room (D-AG-24). Opening a routine goes to D-AG-25. Automations (D-AG-14) appear on the calendar with the Automations filter chip; see A26.
- **A24** Android placement of the rail entries is a proposal, because the rail has no Android equivalent. The mode switch, theme toggle and Settings go to the Home top bar; Calendar goes to the overflow; usage goes on the host card (M-HM-02), replacing the Home usage cards (M-AC-02). Tasks stays on the Home tasks card, which opens M-TK-01. Marketplace has no Android place yet, because extensions are not designed for Android. The owner has not decided these for Android.
- **A25** Bot Teams tasks on the Tasks page (D-WS-24). The owner says the Tasks page covers both modes, and that Bot Teams tasks come from the team Boards (D-WS-30). Assumed: the Tasks page lists the Board tasks of every team. The owner should confirm.
- **A26** Routines and automations (open question 8, answered by the owner on 2026-11-12). One core scheduling engine, two presentations: a routine (D-AG-25) is the Bot Teams form of an automation (D-AG-14), for non-technical users; Workspaces is for software engineers. A team can turn on Workspaces in a team. The calendar (D-AG-23) shows both, filtered by team chips and an Automations chip (owner, 2026-11-12). Assumed: New routine still opens the schedule editor (D-AG-15).
- **A27** Entry point for the vault (open question 11), owner accepted default 2026-11-12. The vault (D-VT-01) opens from the Settings view as its own section, not from the rail, which has no vault entry.
- **A28** Linking. A link is made from the entry modal of the receiving scope (X-VT-01). The list shows only the owner's secrets held in other scopes. The owner's note names no separate share action, so none is designed.
- **A29** Names and conflicts. A variable name uses letters, digits and underscores, and cannot start with a digit (assumed: the owner gave no rule). A name is unique within its scope. A name already in use is refused, with the choice to pick another name or link the existing secret. A linked secret keeps its name in every linked scope (open question 10, owner accepted default 2026-11-12).
- **A30** Rotation. A session that is running keeps the value it started with, and the new value reaches the next run. Rotation does not restart a session.
- **A31** Usage. The usage record keeps the secret's name, the agent, the session and the time. It never keeps the value.
- **A32** Host unreachable. Saving needs the executing host. While it is unreachable, the save is refused and shows the host state; nothing is queued, and the card stays [pending].
- **A33** Decline. The card shows [declined]. The agent is told the owner declined, then carries on without the secret or stops.
- **A34** Linked scopes. A linked secret is read-only where linked, and its value is never shown there. Unlink removes only that scope's link, with no confirm.
- **A35** Owner delete. Every linked scope loses the secret. Its next request for that name returns as a pending card (X-CH-07).
- **A36** Android. The phone shows the vault list and adds a secret through X-VT-01 (M-VT-01). Edit, rotate, delete and unlink stay on desktop (open question 12, owner accepted default 2026-11-12).
- **A37** Mobile emulator tab (D-SH-18). Assumed: the tab offers a short list of devices, shows the app or web view in a device frame, and loads it from a URL or the workspace's dev server. The owner's reference names the tab and its New action only. The device list, the frame and the run source are not decided.
- **A38** MassOrca Mobile entry in the Workspaces Sidebar. The owner's note names the entry, not its target. Assumed: it opens the phone pairing page (D-PR-01) on the desktop.
- **A39** Mode home and Prototypes placement. The owner did not name the home each mode returns to (D-SH-10). Assumed: the Workspaces home is the start page (D-AG-01), and the Bot Teams home is the team list (D-WS-29). Prototypes (D-AG-17) opens in the canvas; whether it is a full page is not decided.

## Gaps

Each gap is a step that the inventory cannot yet support. The owner decides it, or the next design pass fills it.

- **G1** Local service start and its failure. No screen. D-SH-07 covers UI loading only.
- **G2** No "host unreachable, status unknown" state. X-AG-01 and D-TM-02 show reconnecting or disconnected, but the PLAN rule says losing contact is not proof that a process died.
- **G3** A protocol mismatch on desktop has no explicit state. M-SH-02 covers Android only; D-HO-07 [update available] is the nearest.
- **G4** Desktop pairing with a headless server has no row for entering the server's pairing secret or address. This flow assumes it sits inside D-HO-02.
- **G5** The workspace type (git worktree or plain folder) is not a named state in D-WS-14.
- **G6** No screen lists running terminals that are open in no pane, so nothing reattaches a kept process after its pane closes (J5 step 4).
- **G7** Push, pull and fetch have no row. J6 assumes the branch is pushed before a PR is created.
- **G8** Partly answered. pv96 has a Ports tab, for SSH hosts only (D-SH-04). The owner's Project sidebar list (2026-11-12) does not name it, so the candidate-URL list for dev servers has no row.
- **G9** The certificate warning page (BRW-18) has no ID.
- **G10** Escape while recording a shortcut was not read (inventory "Not covered").
- **G11** Where an extension's files live when it is created (a folder, a repository or app data) has no row.
- **G12** Sharing an extension has no row. D-EX-07 (share skills) is dropped, and D-CU-01 shares presets only.
- **G13** Installing the Android app from the store happens outside the app, so there is no screen. iOS is out of scope.
- **G14** Passkeys: D-BP-06 is marked Keep, but BRW-18 puts passkeys after the first version. Owner decision.
- **G15** No single list of running jobs outside agents (workspace creation, import, migration, extension install, updates). D-AG-22 covers agents only.
- **G16** Key capture in a terminal: Ctrl+F / Cmd+F and Ctrl+E / Cmd+E are also used by shells and full-screen programs. The owner must decide which one wins inside a terminal pane.
- **G17** Android terminal renderer: WebView or native (M-TM-01, a Phase 6 decision).
- **G18** The owner's reference image shows an application menu in the title bar (Application, Edit). No row covers app menus.
- **G19** Closed (pv96). Android prototypes (M-PT-01) open from the overflow menu. D-AG-17 is core, not an extension view.
- **G20** Closed for the source (owner, 2026-11-12). Bot Teams tasks come from the Conference room (D-AG-24) and appear on the team's Board (D-WS-30). Whether the Tasks page lists them is A25. Still open: opening a Board task has no detail row yet, because D-WS-25 covers issues and PRs only.
- **G21** Existing key entries. The transcription key (D-ST-08) and the Linear and Jira keys (D-OT-08, merged into D-ST-08, which lists only the transcription key) are not decided: they stay in settings, or move into the vault.
- **G22** Headless store. The encrypted store on a headless server has no design: how it is keyed, unlocked after a restart, or backed up. D-OT-04 warns when credentials cannot be stored securely; what the vault does on such a host is not designed.
- **G23** Agent output. Nothing stops an agent from printing a variable into a tool result, which would put the value in the transcript. No redaction rule is designed.
- **G24** Team scope. Who besides the team's bots can see a team secret's name and usage is not decided.
- **G25** Several hosts. A secret lives on its executing host, and the owner rules out copies. A project on another host therefore cannot link the same stored value, and no rule says how the vault lists or links across hosts.
- **G26** Closed (owner, 2026-11-12; open question 13 in screen-inventory.md). Diffs and files open as file tabs in the Work panel (D-DF-01). Tasks, Calendar, Marketplace, Settings, the Bot Teams pages and Automations open as full pages over the canvas (see Navigation map). Artifacts (X-AR-01) is an owner decision (2026-11-12): items of the Results card, opened in a Work panel tab. The Work panel tabs are Terminal, Browser, Markdown or file and Mobile emulator, and the Canvas holds the Conversation, Session summary, Work panel and Project sidebar. Every one of them now has a place (see the Navigation map), so G26 is fully closed. Open question 13 in screen-inventory.md.

## Changes since owner validation (pv96 delta, 2026-11-12)

The owner validated the journeys on 2026-11-12. The changes below come from the unmerged pv96 build and from decisions taken on the owner's behalf that day. Each row names its source. The owner should re-confirm them. Full evidence: side-session notes file pv96-delta.md.

| Change | Source | Where |
|---|---|---|
| Title bar stays in the frame as a band. Titlebar tabs stay dropped. | Decided on the owner's behalf (pending-owner-review.md, item 2) | D-SH-03, Navigation map |
| Mode switch: from another page, the first press returns to the mode's home; a press at home switches the mode | Decided on the owner's behalf (item 14) | J13 step 1, D-SH-10 |
| Prototypes is core, with a Workspaces Sidebar entry. The A4 default is superseded. | Decided on the owner's behalf (item 12) | D-AG-17, A4 |
| Artifacts have no page. They are Results card items and open in a Work panel tab. G26 closed. | Owner decision, 2026-11-12 | X-AR-01, D-AG-27 |
| Workspaces Sidebar: + Create, Search, Prototypes, Tasks, Automations, MassOrca Mobile, then Projects. Automations opens as a full-canvas page. | Owner decision, 2026-11-12 | D-WS-01, D-AG-14, A38 |
| Journeys J3, J4, J5, J7, J8, J10, J12 and J13 carry the pv96 changes | pv96 delta, folded into the steps | Journeys |
| Keyboard: Ctrl+T, Ctrl+Shift+B, Ctrl+W and Ctrl+Alt+click (Cmd forms on macOS) added, flagged "from pv96" | pv96, not checked | Keyboard |
| Diagrams regenerated: title bar band, mode switch note, J4, J12, J13, keyboard | Follows the rows above | app-flow.html |

Owner decisions kept where pv96 differs (no change): the model picker is the owner's component, with its More models fold; the status bar keeps the owner's five items; the Project sidebar keeps the owner's four tabs; the Calendar keeps filter chips; Settings keeps the owner's ten sections; the frame and canvas colours keep the owner tokens.

### Open items for the owner

- Routine form: the schedule editor (A26) or pv96's own routine form (D-AG-34).
- Calendar: filter chips (owner design) or the legend pv96 built.
- Project sidebar: pv96's extra tabs (workspaces, pr-checks, Ports for SSH hosts, plugin tabs) in or out.
- Android: the Marketplace place (A24), and whether the Artifacts screen M-AR-01 stays.
- Keyboard: collisions for the four pv96 shortcuts, including Cmd+W on macOS.
- Mode home and Prototypes placement (A39), and the MassOrca Mobile target (A38).
- Tasks is on the rail (D-SH-11) and in the Workspaces Sidebar. Keep both?
- Delegation (X-DL-01, owner 2026-11-12): four rules for where a child session runs. Own worktree by default; current worktree only for read-only children, plain-folder workspaces or a manual pick; never two writers in one worktree (the parent waits); parallel launches always use own worktrees.
