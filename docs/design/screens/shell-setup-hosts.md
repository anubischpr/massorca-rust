# Screens: app shell, first run, agent accounts, hosts, session import, notifications, dialogs

Status: draft for owner review, 2026-11-12. English. Covers the desktop rows of sections 1, 2, 3, 4, 7, 13 and 17 of [screen-inventory.md](screen-inventory.md), the shared patterns X-DLG-01, X-DLG-02, X-PK-01, the Both rows X-AC-01, X-DG-01, X-CN-01, X-IM-05, X-NT-01, X-NT-02 and X-UP-01, and the cross-cutting states (host unreachable, protocol mismatch, auth failure, service not running).

Built on [design-system.md](design-system.md), [app-flow.md](app-flow.md), [glossary.md](glossary.md) and the components in [components/](components/). Decisions the owner did not make are in "Decisions for owner review" at the end.

Artboards live in `scratchpad/screens/project/` with the prefix `sh-`. Each is 1440 by 900, a real Frame (Activity rail, Sidebar, Title bar) with an inset Canvas and Status bar. Dark is the default; light uses `data-theme="light"` on the artboard root.

| Artboard file | Size | Title | Theme | Shows |
|---|---|---|---|---|
| `sh-empty-app.dc.html` | 1440 x 900 | Start page, no workspace | dark | D-SH-01, D-SH-09, D-SH-05, D-OB-12 entry, J1 step 9 |
| `sh-wizard-accounts.dc.html` | 1440 x 900 | Agent accounts, first run | light | D-OB-01 step 1, D-AC-04, D-OB-02 merged |
| `sh-accounts-result.dc.html` | 1440 x 900 | Agent accounts, migration result | dark | D-AC-05, failed and re-sign-in states |
| `sh-wizard-theme.dc.html` | 1440 x 900 | First run, theme pick | light | D-OB-03, both theme previews |
| `sh-wizard-notifications.dc.html` | 1440 x 900 | First run, notifications and usage data | dark | D-OB-01 step 3, D-OB-06, D-OB-10, X-NT-02 |
| `sh-setup-checklist.dc.html` | 1440 x 900 | Setup checklist | dark | D-OB-12, D-ST-01 entry |
| `sh-add-host.dc.html` | 1440 x 900 | Add host, SSH form and test | light | D-HO-02, D-HO-01 |
| `sh-host-test-failed.dc.html` | 1440 x 900 | Add host, failed test diagnostics | dark | D-HO-02 failed, X-DG-01, D-HO-06 entry |
| `sh-host-unreachable.dc.html` | 1440 x 900 | Host unreachable across the shell | light | D-HO-01, X-CN-01, D-SH-03, D-SH-05, D-NT-01, X-NT-01, cross-cutting |
| `sh-import-detect.dc.html` | 1440 x 900 | Session import, detect and map | light | D-IM-01, D-IM-02, D-IM-04 entry, X-PK-01 |
| `sh-import-result.dc.html` | 1440 x 900 | Session import, progress and result | dark | D-IM-03, partial result |
| `sh-notifications.dc.html` | 1440 x 900 | Notification cards and toasts | light | D-NT-01, X-NT-01, X-UP-01, X-AC-01 |
| `sh-confirm-destructive.dc.html` | 1440 x 900 | Confirm dialog, destructive with count | light | X-DLG-01 destructive variant, D-HO-03 |

At least one artboard per area is light: the shell (`sh-host-unreachable`), first run (`sh-wizard-theme`), accounts (`sh-wizard-accounts`), hosts (`sh-add-host`), session import (`sh-import-detect`), notifications (`sh-notifications`), dialogs (`sh-confirm-destructive`).

---

## How to read the rows

- Each row ID gets one of: a screen section below (designed), **Merged into** a named screen or pattern, **Dropped** with the reason, or **Extension**. Merged, Dropped and Extension rows get one line.
- Android rows (M-*) are owned by the Android writer and are not designed here. The list is at the end.
- Frame and Canvas rules (region order, widths, collapse order, full pages) come from design-system section 5 and the owner requirements of 2026-11-12. This file does not restate them except where a screen depends on them.

## Shared rules for these screens

- **Honest state.** A host that cannot be reached shows its last known state with its age: "Last known state, 5 min ago". Never "stopped", "offline" or "done". Queued messages wait and send only after the host confirms the session (A10). Saving a secret while the host is unreachable is refused (A32).
- **Copy.** Sentence case, plain words, glossary names (Work panel, Session summary, Project sidebar). Progressive words while work runs ("Importing", "Checking"); result words only after the result ("Imported"). No exclamation marks. No "OK", "Yes" or "Submit". Never write a result the system has not yet produced.
- **Dialogs.** X-DLG-01 for every confirm. Width 440 px. Focus starts on Cancel for destructive dialogs. Escape closes unless busy. Full rules in the Dialog component.
- **Notifications.** Toasts (X-NT-01) report short results and sit above the notification cards (D-NT-01) in the bottom-right of the Canvas, 16 px from the right edge and above the Composer and the Status bar, never covering the Composer. Cards never move when a toast arrives.
- **Full pages.** Setup checklist, import, Settings and the Bot Teams and rail pages open over the whole Canvas: the Sidebar is hidden, the Title bar shows Back and the page title, and the Work panel and Project sidebar give way (design-system 5.1).
- **Other states.** Empty states use EmptyState (one action, never an error). Errors use ErrorState (what failed, what is kept, Retry). Long content truncates with an ellipsis and the full text in a tooltip. Permission denied shows ErrorState inline with the reason and one action (for example "Open system settings"). Loading shows Skeleton in the same place; a spinner appears only after 200 ms.

---

## 1. App shell and navigation

**D-SH-01 Main window frame.** Designed. Shown in `sh-empty-app` (dark, landing) and `sh-host-unreachable` (light, workspace view).
- Layout: a grid of 48 px (Activity rail, full height), 256 px (Sidebar, full height from the Title bar down to the window bottom), and the rest. The Title bar (36 px, Windows and Linux; 40 px macOS) spans the Sidebar and the Canvas column. The Canvas is inset with a 12 px rounded top-left corner and no border. The Status bar (24 px) sits under the Canvas column only.
- Regions and widths: Conversation fills, minimum 360; Session summary 288 (240 to 400); Work panel 480 (320 min); Project sidebar 288 (240 to 480). Collapse order when narrow: Session summary, Project sidebar, Sidebar, Work panel (pending-owner-review item 8).
- States: workspace view; landing with no active workspace (start page, no region chrome); region error (ErrorState inside that region only; other regions stay usable).
- Sidebar (Workspaces mode, agreements A9 and A21, as in `ws-session`): Create, Search, then Prototypes (`layers`, count), Tasks (`list-checks`), Automations (`calendar-clock`) and MassOrca Mobile (`smartphone`), each with its icon; the Projects header carries Notifications, Filter and sort, Add folder and New project.
- Start page composer in `sh-empty-app` (agreements A32, A33, A40): placement bar (host, project and worktree buttons, borderless, each with its chevron) attached to the Composer; placeholder "Ask anything, @ to mention files, / for commands"; bottom row +, access chip (`shield`, "Workspace write" ▾), Chat / Terminal with icons, the model picker with the agent icon ("Opus 5.5 Medium" ▾), microphone and a round Send.
- Keyboard: F6 and Shift+F6 cycle Title bar, Activity rail, Sidebar, Conversation, Session summary, Work panel, Project sidebar, Status bar.
- Android: not applicable (M-SH-01 is the Android writer's).
- **Built (desktop, PLAT-07).** A second launch for the same profile opens no second window: it raises the running window and exits with status 0. The first desktop holds `<profile>/desktop/instance.lock` and answers on a local endpoint, the named pipe `massorca-desktop-<profile>-<hash>` on Windows or `<profile>/desktop/instance.sock` elsewhere. The running core is unaffected; `massorca service status` shows it. The desktop has no CLI command for the endpoint. Automated launches (`MASSORCA_DESKTOP_BACKGROUND=1`) skip the lock. Not yet: when the app keeps running with no window (macOS), a launch only activates the app.

**D-SH-02 Top-level view switch.** Redesign. Thirteen views become: rail entries (Tasks, Calendar, Marketplace), full pages (Settings, Tasks, Calendar, Marketplace, Automations, Bot Teams pages), a mode (Bot Teams on the rail), the start page for create-chat, and Work panel tabs for terminal. Artifacts is not a view (Results card). Mapping: app-flow.md, Navigation map. Full pages hide the Sidebar and show Back in the Title bar.

**D-SH-03 Title bar.** Designed. Shown in `sh-host-unreachable` (session title with the host suffix) and `sh-empty-app` (no session).
- Order: Sidebar toggle `panel-left` (28 px button), Back, Forward (disabled with no history), session title (`title-sm`, truncated, full title in tooltip), flexible drag area, region toggles (Session summary, Work panel, Project sidebar; 28 px each), window controls on Windows and Linux (minimise, maximise, close; 46 by 36 px, close turns `status.danger.solid` on hover).
- macOS: traffic lights at the left inside the Frame with an 80 px gutter when the Sidebar is hidden.
- States: normal; the Sidebar toggle stays with the Sidebar; no session: "No session open" in `text.tertiary`; host suffix "(host unreachable)" in `status.warning.fg` with its word; full page: Back button labelled "Back" and the page title replace back and forward, region toggles hidden.
- Keyboard: toolbar with roving focus; Left and Right move between buttons.
- **Built (desktop).** The Title bar keeps its layout. Its close button and any close the OS asks for (Alt+F4, the platform frame) ask first while a session is running or waiting for you. The confirm "Close window?" says how many ("2 sessions are running or waiting for you."), then "The service keeps running; sessions continue." Cancel has the focus and "Close window" is danger-styled. With nothing running the window closes at once. Closing sends no call to the service. Unsaved editor files are not counted, because the desktop has no editor yet. To see what keeps running, run `massorca agent list`.

**D-SH-04 Project sidebar shell.** Designed as a shell here; no artboard. A Canvas region at the right edge with a 28 px tab strip. Tabs are the owner's four: File explorer (D-ED-11), Sessions list (the old "vault" tab, merged, D-SH-04 owner rule), Source control (D-DF-02), Checks (D-DF-04), plus extension tabs (D-SL-01). Width 288 (240 to 480). Open, closed, and hidden on full pages. The pv96 extra tabs (workspaces, PR checks, Ports for SSH hosts, plugin tabs) are not in the owner list; they are an open item and have no row here.

**D-SH-05 Status bar.** Designed. Shown in both artboards.
- 24 px strip under the Canvas column, `surface.canvas`, caption text (11 px). Items: agent status (AgentStatus glyph at 12 px, `size.icon.xs`, and a count such as "1 waiting for you"), memory ("1.22 GB"), terminals ("2 terminals" or "No terminals"), ports ("No ports" or "2 ports"), hosts ("2 hosts" with a connection dot: solid success for connected, ring for not connected).
- Unreachable host: the hosts item reads "1 of 2 hosts unreachable" in `text.tertiary`, and its tooltip gives the age.
- Right-click opens the visibility menu (one checkbox per item). Narrow windows drop the lowest priority item first; hidden items stay in the menu (open point: no overflow button).

**D-SH-06 Recoverable error screen.** Designed as the ErrorState page variant (design-system ErrorState), used in a failed Canvas page or Work panel tab. Title says what failed ("Could not show this view"), explanation says what is kept, actions Retry (secondary) and Show details (ghost, mono block with error code and time, hidden by default). One crashed surface does not take the shell down. No artboard.

**D-SH-07 Startup loading.** Designed as a gate, not a splash. The Frame paints at once; each region shows Skeleton bars (`fill.skeleton`, no shimmer, static under reduced motion). If the local service does not start, see cross-cutting "Service not running".

**D-SH-08 Zoom indicator.** Proposed, no artboard. A caption-size chip ("110%") at the bottom centre of the Canvas, surface.overlay with e2 shadow, shown for 1.5 seconds after a zoom change and removed with an instant swap under reduced motion. Shortcuts: Ctrl+= / Ctrl+- / Ctrl+0 (app-flow Keyboard).

**D-SH-09 Activity rail.** Designed. Shown in every artboard.
- Full height, 48 px wide, `surface.frame` (#1b1b1c dark), no seam to the Sidebar, no border. Always visible; the Sidebar toggle never hides it.
- Top group: mode switch (D-SH-10), Tasks (`list-checks`), Calendar (`calendar`), Marketplace (`store`). Bottom group: theme toggle (`sun` or `moon`), usage rings (one per provider, D-SH-15), Settings (`settings`). Items 36 px with `radius.md`; icons 20 px; gap 4 px. Active item: `fill.selected.frame` with a 2 px `accent.fg` bar on the rail's left edge.
- Extension icons go in a divider group under the core top group (D-SL-09), three visible at the minimum height, the rest under an overflow button.
- macOS: a 40 px top zone reserved for the traffic lights.
- Keyboard: one tab stop; Up and Down move; Home and End go to the ends; Enter or Space activates. Tooltips on the right after 400 ms, showing name and shortcut.

**D-SH-10 Mode switch.** Designed in the rail top slot. Shows the current mode: `layout-grid` for Workspaces (shown in the artboards), `bot` for Bot Teams (not drawn).
- A press switches the Sidebar's mode from the mode's home. From any other page the first press returns to the current mode's home; a press at home switches (pending-owner-review item 6).
- Morph: 200 ms cross-fade with scale and rotate as in design-system 4.7. Reduced motion: instant swap.
- Accessible name is the current mode ("Workspaces mode"); the tooltip names the target ("Switch to Bot Teams"). Announce the new mode once, politely.

**D-SH-11 Tasks entry (rail).** Rail item (`list-checks`) that opens the Tasks page (D-WS-24) as a full page. Merged into the rail entry; no second screen here.

**D-SH-12 Calendar entry (rail).** Rail item (`calendar`) that opens the Calendar page (D-AG-23) as a full page. Entry only.

**D-SH-13 Marketplace entry (rail).** Rail item (`store`) that opens the extension browser (D-EX-01) as a full page. Entry only.

**D-SH-14 Theme toggle (rail).** Rail item, `sun` in dark and `moon` in light. Switches the theme instantly (no animation, so no surface flashes). Tooltip "Switch to light theme" or "Switch to dark theme". The full choice is in D-ST-03.

**Built (desktop, SET-02).** The theme last applied is saved on each change, and the first frame of the next start paints it, so no default theme flashes first. The regions left open (Session summary, Project sidebar, Sidebar, Work panel) are saved and reopened too; an overlay is not. The state is `<profile>/desktop/state/ui-state.json` (`<home>/profiles/<name>/`), a version 1 JSON file written through a temporary file. A missing or damaged file gives the defaults. Automated launches neither read nor write it. Not yet: the service's `appearance.theme` is not read by the desktop, and the selected workspace, window size and position are not saved.

**D-SH-15 Usage rings and detail popover (rail).** Designed in the rail bottom group.
- Each ring is 32 px with a 2 px track (`usage.ring.track`) and an arc from 12 o'clock in `usage.ring.1` to `usage.ring.6` by provider slot; `usage.ring.warn` from 80 percent, `usage.ring.limit` from 95 percent. The agent icon (never a letter, A16) sits in the centre at 14 px. Every artboard in this file shows the same three rings (Claude 62 percent, Codex 35 percent, OpenCode 10 percent).
- Hover or click opens a popover (surface.overlay, e2, 280 px): title "Usage", rows "Resets in 2 days" and "Plan".
- States: loading (a 90 degree arc turning, `text.tertiary`); available; unavailable ("Usage unavailable", track only); error ("Usage could not be read. Last value 62 percent", 6 px danger dot).
- The ring's accessible name carries the number: "Claude usage 62 percent, resets in 2 days". Announce only when crossing 80 or 95 percent.
- Full view: D-AC-06 (section 3).

**D-SH-16 Settings entry (rail).** Gear icon at the bottom of the rail. Opens Settings as a full page (sections D-ST-01 to D-ST-11, owned by the settings writer). Keyboard: Ctrl+, / Cmd+,.

**D-SH-17 Work panel.** Designed as a shell region with its tab strip and empty state, not drawn. Tabs: Terminal (D-TM-01), Browser (D-BP-01), Markdown or file (D-ED-01), Mobile emulator (D-SH-18); a plus (28 px) adds a tab. Files and diffs open here as file tabs (owner, 2026-11-12).
- Empty state (owner, 2026-11-12): a centred column of four buttons with their icons, "New terminal", "New browser tab", "New Markdown", "New mobile emulator"; no title or explanation (see workspaces-sessions, Work panel).
- States: empty; tabs open; tab error (ErrorState in the tab body).
- Keyboard: Ctrl+T adds a terminal tab, Ctrl+Shift+B a browser tab, Ctrl+W closes the focused tab and never the session (app-flow, not checked against browser shortcuts).

**D-SH-18 Mobile emulator tab.** Assumed (A37). A Work panel tab that previews a mobile app or web view in a device frame. Device list, frame and run source (URL or the workspace's dev server) are not decided. States: empty (choose a device and a source), running (device frame with the page), error (ErrorState in the tab). Not drawn.

**M-SH-01 to M-SH-04.** Android rows. Owned by the Android writer (navigation stack, protocol gate, OTA web shell dropped, web twins dropped). The desktop side of the protocol gate is in the cross-cutting section below.

---

## 2. First run, onboarding and setup

The wizard is a full Canvas page. The Sidebar is hidden, the Title bar reads "Set up MassOrca" with window controls, and the Activity rail stays visible. Each step has an overline "Step n of 3 · name", a 4 px determinate Progress bar (33, 66 and 100 percent), a 20 px heading, a short explanation, the step's content in a card, and a footer: Back (secondary, absent on step 1), Skip (ghost), and the primary action. The wizard's step indicator is built from Progress and overline text, not a new component (see Component gaps).

**D-OB-01 Onboarding wizard.** Redesign. Three steps: agent accounts (step 1), theme (step 2), notifications and telemetry (step 3). Cut from five steps; the other items move to the setup checklist (D-OB-12).
- Artboards: `sh-wizard-accounts` (step 1, found), `sh-accounts-result` (step 1 after the migration), `sh-wizard-theme` (step 2), `sh-wizard-notifications` (step 3).
- States: step n of 3; skipped (after the X-DLG-01 confirm, the checklist opens); step error (ErrorState inline in the step card; the step can be retried).
- Keyboard: Tab moves through the step's controls; Enter on the primary action continues; Escape asks the skip confirm only from step 1 and later.

**D-OB-02 Step: default agent.** Merged into D-AC-01. The default agent is a field on the accounts step (a Picker-style trigger showing the Claude icon, "Claude" and a chevron under the account list, `sh-wizard-accounts`).

**D-OB-03 Step: theme.** Keep. `sh-wizard-theme` shows two mini frames side by side, each with its own `data-theme`: the dark preview and the light preview, each a schematic of the rail, Sidebar, Title bar and Canvas with the real tokens. A Radio group under the previews, each option with its icon: Dark (`moon`), Light (`sun`, selected in the artboard), and "Match system" (`monitor`), which follows the system theme until changed (A8). The label names no operating system so it reads the same on Windows, macOS and Linux. The live preview updates as the choice changes.

**D-OB-04 Step: integrations.** Merged into D-OB-12. Reachable from the checklist as "Sources and integrations" (D-ST-06).

**D-OB-05 Step: Windows terminal and Ghostty import.** Merged into D-ST-05 (terminal settings).

**D-OB-06 Step: notifications.** Merged into D-OB-01 as step 3 (`sh-wizard-notifications`): a Switch for agent notifications (`bell`), three source rows (Waiting for you, Errors, App updates), each with its glyph, with On and Off, and a status line from the OS permission ("Your system allows notifications for MassOrca" in success; warning when blocked, see X-NT-02). The Usage data card title carries `bar-chart`.

**D-OB-07 Skip-onboarding confirm.** Merged into X-DLG-01: a default confirm "Skip setup?" with the body "You can finish setup from the checklist at any time." Action "Skip setup" (primary), Cancel (secondary).

**D-OB-08 Inline setup command terminal.** Merged into D-TM-02 (the normal terminal pane, running, done or failed states).

**D-OB-09 First-launch banner.** Dropped. Replaced by the setup checklist (D-OB-12).

**D-OB-10 Telemetry consent prompt.** Redesign. One explicit choice on step 3 (`sh-wizard-notifications`): a Radio group under "Usage data", default "Do not share usage data" (off by default, A15), and "Share usage data" with the helper "Counts which features you use. No prompts, code or file names." That helper is proposed copy and needs privacy review. The choice can be changed in Settings, Privacy and advanced (D-ST-10).

**D-OB-11 Feature wall.** Dropped. Marketing surface with no user task.

**D-OB-12 Setup checklist.** Redesign. Artboard `sh-setup-checklist` (dark, full page, title "Setup", a 4 px progress bar and "4 of 8 done").
- Eight rows as ListRow with a status glyph (16 px), then the icon of the row's content (20 px: `user`, `server`, `folder`, struck-through folder, `download`, `moon`, `bell`, `bar-chart`), then the text and one action: Agent accounts (warning glyph, "Claude is signed in. Codex and OpenCode need sign-in.", Open), Add a host (not started, Add host with `server`), Create a workspace (not started, "Add a project" with `folder-plus`), Conversations without a project (done by default, owner 2026-11-21: "Saved in ~/MassOrca/Conversations on This PC. Default; you can change it.", Change… opens a folder picker; one folder per host, the same setting as Settings, General, A95), Import sessions (optional, "Found 20 sessions in Claude Code, Codex and OpenCode. Read only.", Import with `download`), Theme (done: Dark, Change), Notifications (done, Change), Usage data (done: Not shared, Change).
- Opened from the wizard and from Settings, General (D-ST-01). Setup is optional; the footer says where to find it again.
- States: in progress; complete ("Setup complete" heading with the same rows, no action buttons other than Change).

**D-OB-13 Feature tips queue and five tip dialogs.** Dropped. Tips, where needed, use the one dismissable pattern: a toast with Dismiss (X-NT-01).

**D-OB-14 Contextual tour overlay.** Dropped for the first version. Extension candidate.

**D-OB-15 Star request card and toast.** Dropped. Not a user task.

---

## 3. Agent accounts and subscription migration

**D-AC-01 Agent accounts list.** Redesign. One list with provider adapters, shown in the wizard (`sh-wizard-accounts`) and as the Settings content (D-ST-02). Mock rows use Claude, Codex, OpenCode and Cursor from the inventory's provider list; the other providers (MiniMax, Grok, Antigravity, zCode) follow the same row.
- Row: the agent icon in a 28 px plate (never a letter, A16), name in `body-strong`, a secondary line with the state, and a tag with the state word.
- State words: Signed in; Expired; Needs sign-in; Not set up; Migrated; Conflict; Not found on this PC.
- Actions: Sign in (secondary) for Needs sign-in and Expired; Retry for failed checks; Remove through X-DLG-01 (D-AC-03).
- Absorbs D-OB-02 (default agent) and the Accounts pane.

**D-AC-02 Provider sign-in flow.** Merged into D-AC-01. One generic flow inside a Dialog opened from the row: starting; waiting for the browser or a code (the code is a TextField, masked only where the provider needs it); done; failed (ErrorState inline with Retry). A Codex warning becomes the "Expired" state.

**D-AC-03 Account removal dialogs.** Merged into X-DLG-01 (destructive confirm that names the account and what is lost).

**D-AC-04 Migration from the current app.** Designed in `sh-wizard-accounts` (light). The step lists the accounts found in the current app, each with a checkbox (checked by default), the provider avatar, "Credentials found, checked on import", and a Found tag. Cursor is not found and its checkbox is disabled. Primary: "Import 3 accounts"; ghost: "Skip accounts".
- Rules: copy the stored credentials; check each one on import; store it in the OS secret store; never show a secret (A1). Found, none found (the step then offers sign-in by hand), selecting, importing (each row shows a spinner and "Checking" while it runs), imported, and failed (retry that account only).
- No OS secret store: the migration stops for that account and the D-OT-04 notice explains why (A6).

**D-AC-05 Migration result and re-sign-in queue.** Designed in `sh-accounts-result` (dark). Headline "1 account migrated, 2 need you". A warning Banner: "Sign in to 2 accounts to use them here. Nothing from a failed account is saved, and no secret is shown on this screen."
- Three groups with overline labels: Migrated (Claude, success tag "Migrated"), Needs sign-in or retry (Codex, "Needs sign-in", Sign in; OpenCode, "Import failed", Retry this account and Sign in), Skipped (Cursor, "Skipped", add later in Settings).
- Accounts that did not migrate are obvious: each sits in its own group with one action.
- Footer: "Finish later" (ghost) and "Continue" (primary).

**D-AC-06 Usage and subscriptions.** Redesign. Full view of the usage rings (rail, D-SH-15). Layout described, not drawn: one card per account with its plan name, the 5-hour and weekly windows as bars with percentages and reset times, thresholds at 80 and 95 percent, and a "Last checked" line. States: loading (Skeleton), available, unavailable ("Usage unavailable"). Chart palette beyond the rings is not covered (design-system section 10).

**X-AC-01 Account needs sign-in card.** Both. Desktop: a notification card of kind "Sign-in needed" (warning plate, `user` icon, title: the Claude icon then "Signed out", with "Claude" as tooltip and accessible label (A17), body "Sign in again to keep using Claude in this project.", one action "Sign in"), shown in the card stack (`sh-notifications`). It asks once, in place. Android: the same card inline on the home screen and in the session (Android writer). Absorbs D-OT-12 (Codex login link notice).

**M-AC-01 Accounts usage (per host).** Android. Owned by the Android writer.

---

## 4. Hosts, pairing and remote/SSH

**D-HO-01 Hosts in the sidebar.** Keep. Projects are the top level of the Sidebar tree (SidebarTree); a project can live on more than one host (owner, 2026-11-12). Under each project there is one host row per host it lives on: host glyph (`server`), host name, and a state word or workspace count on the right. The workspaces on that host sit under its row.
- Artboards: `sh-empty-app` ("No projects yet"), `sh-add-host` (oppm on This PC and VPS; massorca on This PC; NexusCare on VPS), `sh-host-unreachable` (each VPS row in `text.tertiary` with the unreachable glyph and "Last known state, 5 min ago" or its age; the This PC rows stay normal).
- States: connected ("Connected"); connecting (spinner after 200 ms); disconnected; unreachable (last known with age); auth failed ("Sign-in needed", warning glyph); update needed (protocol mismatch, see cross-cutting). Never "stopped".

**D-HO-02 Add host.** Redesign. One Dialog (440 px) with two paths in a SegmentedControl: SSH host and Server. Absorbs the SSH target form, the server-start step and the runtime environment add and edit dialogs.
- SSH path (`sh-add-host`, light): Name, Address (mono), Port (mono, 84 px wide), User (mono), SSH key (a Picker-style trigger listing the keys from the SSH config, helper "The passphrase is asked only when needed"). A secondary "Test connection" button; after a pass, a success Banner "Connected. Protocol 5 matches this app." Footer: "Test again" (secondary, left), Cancel (ghost) and "Add host" (primary).
- Decision: "Add host" stays disabled until the test passes (owner decision list).
- Copy not kept: the SSH key helper reads "Keys from ~/.ssh. A key with a passphrase must be loaded in your SSH agent first." in place of the artboard's line above. The artboard's line says the keys come from the SSH config and that the passphrase is asked when needed. The list comes from `~/.ssh`, and the transport runs ssh in batch mode, so ssh never asks for a passphrase. Reason and review: [fidelity-report.md](../fidelity-report.md), "Artboard copy we could not keep".
- Failed test (`sh-host-test-failed`, dark): the Dialog takes the error variant (role alertdialog). Danger Banner "Could not connect. The server refused your key." and "Nothing was added. Your host list is unchanged." Below it, the connection log in a mono block (`surface.sunken`, `border.subtle`), the first things to check (numbered), and "Run network test" (secondary, X-DG-01). Footer: "Copy details" (ghost, left), Cancel, Retry (primary).
- Server path: Address and a pairing code field (TextField, masked, reveal IconButton) that the server shows; closes gap G4. Not drawn.
- States: form; SSH config picker; testing (the button shows a spinner after 200 ms and its label becomes "Testing"); passed; failed; passphrase needed (D-HO-06).
- Keyboard: Tab order follows the form; Enter in the last field does nothing; Escape cancels unless testing.

**D-HO-03 Rename and remove host.** Rename: Merged into X-DLG-01 (one-field variant, "Rename host"). Remove: Merged into X-DLG-01 destructive variant, drawn in `sh-confirm-destructive`: title "Remove host VPS?", body "MassOrca stops showing VPS and the projects on it in this app. Nothing on the server is deleted.", the loss list with counts ("3 projects leave the sidebar", "2 saved SSH workspaces are forgotten", "4 past sessions on VPS are hidden here"), Cancel (focused) and "Remove host" (danger).

**D-HO-04 Forget saved SSH workspace.** Merged into X-DLG-01 (destructive confirm; the list names the workspace).

**D-HO-05 Widen host visibility gate.** Merged into X-WS-01 (Workspaces writer): an inline choice in the workspace filter.

**D-HO-06 SSH passphrase prompt.** Keep. A small one-field Dialog: title "Passphrase for ~/.ssh/id_ed25519", a masked TextField with reveal IconButton, one primary "Connect". States: prompting; wrong (error line "That passphrase did not work. Try again." and the field keeps focus); used for this connection only. Decision: saving the passphrase in the OS secret store is an owner choice (owner decision list). Secret entry is never logged or shown back. No artboard.

**D-HO-07 Servers and access.** Redesign. List of remote servers (ListRow, two-line) with the state word, the version, and actions: Connect, Reconnect, Update, Move, Access. States: connected; reconnecting; update available; updating; failed; protocol mismatch ("Update needed", see cross-cutting). Absorbs the Servers pane, update dialog, move dialogs and grant list. No artboard.

**D-HO-08 Local network connection test.** Merged into X-DG-01 (a "Run network test" action in the diagnostics).

**D-HO-09 Remote workspace folder picker.** Merged into X-PK-01 (folder mode of the Picker, breadcrumbs in mono, a parent row).

**D-PR-01 Phone pairing (desktop).** Redesign. One pairing screen, reached from the Sidebar entry "MassOrca Mobile" (A38) or from Settings (D-ST-07). Not drawn.
- Layout: a QR code (about 192 px square on a white tile with 16 px padding, in both themes), beside it the pairing code in mono and a "Refresh code" secondary button. Below: a Radio group "Connect over" listing this PC's local network addresses with the adapter name (for example "192.168.1.20, Wi-Fi"), and "Custom address" (TextField). A warning Banner shows the Windows firewall notice when the port is blocked, with "Open firewall settings" as the action.
- States: idle; showing QR; path choice; waiting for device (ProgressBar indeterminate with "Waiting for your phone"); paired (the device appears in D-PR-02); firewall warning.
- Relay is not in the first version (D-PR-04 dropped).

**D-PR-02 Paired devices.** Keep. Desktop only. A ListRow per phone (name, paired date, last seen when the service reports it, state word "Active" or "Revoked"), with one action "Revoke" (secondary). Revoke opens X-DLG-01 (destructive, "Revoke access for Pixel 8?", the loss list "The phone loses access to every host on this PC." and "Revoke" as the danger action). Revocation is one clear action. A paired device is the owner with full access: pairing offers no role or scope picker (the desktop asks for full access), and a row shows a "Limited: <scopes>" label only for a device holding less than full access.

**D-PR-03 Mobile page (view).** Merged into D-PR-01 (a Settings section and the Sidebar entry are enough).

**D-PR-04 Relay beta notice.** Dropped. Relay pairing is not in the first version (owner accepted default, open question 5: local network and SSH only).

**X-DG-01 Connection diagnostics.** Both. Desktop: a page or the failed-test dialog (`sh-host-test-failed`) with the connection log (mono block), the troubleshooting steps and the network test. Android: the Android writer's screen. States: ok, warning, failed. Actions: Copy details, Run network test, Retry. Absorbs D-HO-08.

**X-CN-01 Connection status banner.** Both. Banner (desktop) at the top of every region that depends on the host (Conversation, Work panel tab, host card). Variants and copy from the Banner spec. Shown in `sh-host-unreachable`:
- Unreachable (dashed unreachable glyph, `surface.raised`, `border.default`): "VPS cannot be reached. Showing the last known state from 5 min ago. Work that was running there may still be running." Actions Retry and Details.
- Connecting (info, spinner after 200 ms): "Connecting to VPS". Reconnecting (warning): "Reconnecting to VPS, attempt 3", action "Retry now". Restored (success, fades after 5 seconds, dismissible): "Reconnected to VPS". Auth failed (warning): "VPS needs you to sign in", action "Sign in".
- Banners are not dismissible while their state holds. Several notices: one banner with "+1 more". Absorbs D-TM-03, the runtime disconnect toast and the SSH reconnect overlay.

**M-PA-01 to M-PA-05, M-HO-01.** Android. Owned by the Android writer (connect hint, scan pairing code, paste sheet, confirm pairing, post-pair setup, edit host). The desktop side of pairing is D-PR-01.

---

## 7. Session import from other agent tools

Import runs on the host where the sessions live and copies the selected sessions into MassOrca's store. Source files are only read (A2).

**D-IM-01 Import entry.** Designed as the detect step of `sh-import-detect` (light, full page "Import sessions from other agent tools"). Reached from the checklist "Import" and from the start page (A7).
- Found on this PC: one row per tool with a status glyph and its count: Claude Code (14 sessions in `~/.claude/projects`), Codex (6 sessions in `~/.codex/sessions`), OpenCode (warning glyph: "Not found at the default path. Set where its data lives.", action "Set path"), Gemini CLI ("Not installed on this PC. Skipped.").
- States: detecting (Skeleton rows, no spinner for the first 100 ms); found per tool; none; access needed (warning, Set path). Detection says what it found before reading anything.

**D-IM-02 Import selection.** Designed in `sh-import-detect`.
- A table (Table component): checkbox, Session (title), Agent (only the agent icon at 16 px, the name as tooltip and accessible label, A20), Repository (mono path), Project (a Picker-style field), Status (Ready, Needs project, Already imported). Header row in overline.
- Unmapped sessions (no repository, or a repository with no project) are asked about once, in one batch: a warning Banner "Two sessions have no project. Choose one project for both." with one Picker. The choice applies to the whole group.
- Already imported rows are dimmed, unchecked and cannot be selected again.
- Footer: a status line "5 selected, 1 already imported"; Cancel (ghost); "Import 5 sessions" (primary), disabled until every selected session has a project, with the reason in the status line.
- States: list; selected; unmapped (batch); already imported; no sessions found.

**D-IM-03 Import progress and result.** Designed in `sh-import-result` (dark).
- Heading "Importing 5 sessions", a stat line "2 imported, 1 failed, 2 left", and a determinate 4 px Progress bar with the percentage. No info Banner: the one quiet line in the footer says it runs in the background (A42).
- One row per session with its status: Done (success glyph, "Imported to NexusCare"), Failed (danger glyph, the reason and the file name in mono, Retry and Skip), Copying (spinner glyph, a mini bar, "Copying 60%"), Queued (clock, "Waiting for its turn").
- A partial import keeps what succeeded; each failure is named; a missing source file is named in the result.
- Footer: a line "Runs in the background. Use Back to leave; a notification tells you when it ends." and "Open imported sessions" (primary, enabled only when the import has finished). No "Run in background" button: the import always runs in the background and the Title bar Back leaves the page (owner, 2026-11-12).
- States: running; partial (failures named, the rest kept); failed (every session failed, with the same row pattern); done.

**D-IM-04 Source connector setup.** Designed as a small Dialog from "Set path" (D-IM-01). A one-field form with a TextField in mono for the folder, a "Browse" secondary button (the folder variant of X-PK-01), and a helper line "Access is read-only. MassOrca reads the sessions and copies the ones you select." States: default path; custom path; not found (ErrorState inline: "No sessions found in this folder. Check the path."). No artboard.

**X-IM-05 Imported session marker.** Both. Desktop: a line in the transcript header, caption size, "Imported from Codex, 12 Nov 2026"; after the first message the line reads "Continued here". Android: a badge on the sessions switcher (Android writer). No artboard.

---

## 13. Notifications and toasts

**D-NT-01 Notification card stack.** Redesign. Artboard `sh-notifications` (light).
- Stack in the bottom-right of the Canvas, 16 px from the right edge and above the Composer and the Status bar, never covering the Composer. Cards are 360 px wide with `radius.lg`, e2 shadow and the kind plate (28 px). Up to three visible; the fourth and later fold into a "+1 more" row that expands the stack (maximum 60 percent of the Canvas height, then scrolls).
- Cards shown: Attention ("oppm: Run command? pnpm install", "[Codex icon] is waiting for approval in oppm-api." (A17), Review (primary), which opens the session in the floating workspace as approval rows do, A45), Sign-in (Claude icon and "Signed out", Sign in), Update ("MassOrca 1.4.2 is ready", "Restart now" and "Later").
- Priority: Attention and Error first, then Sign-in, Update, Info. Cards never time out. Dismiss with the x button. A card resolves itself when the cause is gone.
- The Sidebar bell (Projects header) expands the stack to show every card (decision, owner list).
- Keyboard: Mod+Alt+N focuses the stack; Up and Down move between cards; Delete dismisses; Escape returns focus.
- States: per kind (Attention, Error, Update, Sign-in, Info); folded row; disconnected host ("Last known: waiting for you, 5 min ago", actions still open, answer actions disabled, see `sh-host-unreachable`).

**D-NT-02 Profile save delay notice.** Merged into X-NT-01: a warning toast "Saving is taking longer than usual." with no action, shown only while the save runs.

**D-NT-03 Desktop notification settings.** Merged into D-ST-08 (per-source switches and sounds). The wizard step 3 uses the same three rows.

**X-NT-01 Toast.** Both. Artboard `sh-notifications` shows a success toast ("Session imported to oppm.", Open), a warning toast ("Setup script failed.", Open log), and in `sh-host-unreachable` an error toast with the duplicate counter ("Could not reach VPS. Try again later." x2, Retry).
- Anatomy: 360 px, `radius.lg`, e2 shadow, status glyph at 16 px (info 5 s, success 5 s, warning 8 s, error stays until dismissed), message up to three lines, one optional action, x close. Stack bottom-right of the Canvas above the cards; at most three visible.
- Duplicates within 2 seconds add a counter ("x2"). Undo toasts time out after 6 seconds. Toasts never take focus; Mod+Alt+N moves focus to the group only on request.
- Absorbs the setup script toast, the base-ref refresh, the auto-rename failure with retry, D-NT-02, D-TM-06 (terminal error) and the runtime disconnect (now X-CN-01).

**X-NT-02 OS notifications.** Both. The operating system notification only announces attention (decision logic is in the attention module). Settings are in D-ST-08. Wizard step 3 shows the OS state: "Your system allows notifications for MassOrca" (success) or, when blocked, a warning Banner "Notifications for MassOrca are blocked. Turn them on in your system settings." with the action "Open system settings". The copy names no operating system so it holds on Windows, macOS and Linux. Per-source toggles can switch off an OS notification for one source only.

**X-UP-01 App update notice.** Both. Desktop: an update card in D-NT-01 with states Available ("MassOrca 1.4.2 is available", Download, Later), Checking (spinner, "Checking for updates"), Downloading (4 px Progress, "Downloading, 64%"), Ready ("Restart to finish the update", Restart now, Later), Error ("Update failed: reason", Try again). Android: M-HM-01 card and M-ST-01 section (Android writer).

**M-NT-01 Notification settings.** Android. Owned by the Android writer.

---

## 17. Other dialogs and shared patterns

**X-DLG-01 Confirm dialog.** Both. Artboard `sh-confirm-destructive` (light) shows the destructive variant with its loss list.
- Container: `surface.overlay`, 1 px `border.default`, `radius.xl` (12 px), e3 shadow, 440 px wide, padding 16 px, scrim behind (`scrim` token, covering the whole window including the Frame).
- Title (title, 16 by 24, a question or noun phrase): "Remove host VPS?". Body (13 by 20, `text.secondary`, up to three lines). Loss list (up to five rows with counts in tabular figures, then "and N more").
- Footer: Cancel (secondary) left of the action (primary or danger, always last). Destructive: focus starts on Cancel, Enter on the default focus closes.
- Variants: default confirm (primary); destructive (danger, named for the loss); error (what failed and what is kept, action Retry, status line in `status.danger.fg`); one-field (rename, naming an untitled file; one TextField).
- States: default; destructive; busy (the action shows a spinner after 200 ms and its label stays, for example "Removing", Escape ignored); error; disconnected host (action disabled, "Host unreachable. Last known state, 5 min ago" as body text).
- Absorbs D-OB-07, D-AC-03, D-HO-03, D-HO-04, D-PR-02 (revoke), D-TM-04, D-TM-05, D-AG-16, D-ED-05, D-OT-14, D-VT-02, M-WS-07 and the other confirms listed in X-DLG-01.
- Behaviour: Escape closes unless busy; click on the scrim closes a default confirm only when nothing is typed; destructive dialogs close only by their buttons or Escape. Role alertdialog for destructive and error variants, dialog otherwise.

**X-DLG-02 Sheet, drawer and popover container.** Both. Desktop: popovers and menus: `surface.overlay`, `border.default`, `radius.lg`, e2, anchored to the trigger, 4 px move in 150 ms with the enter curve; Escape closes and returns focus to the trigger; only one open at a time. Android: bottom sheets (Android writer). Used by the rail usage popover (D-SH-15), the Status bar visibility menu, the notification centre expansion, and the Picker panel (X-PK-01).

**X-PK-01 Picker.** Both. One searchable picker for branches, base refs, folders, parent workspaces, repo sources and accounts. Trigger: a field-shaped button (28 px, `surface.sunken`, `border.strong`) that shows the current value or a placeholder that names the choice ("Choose a project").
- Panel: anchored Popover of 320 px with SearchField on top, rows (ListRow, compact), overline group headers, and an optional footer action. Variants: list (projects, accounts, branches), folder (breadcrumbs in mono; ".." parent row on Android), create-capable (a first row "Create branch "name"" only when nothing matches).
- States: loading (three Skeleton rows, no spinner in the list); empty ("No branches match "query"" or "This folder is empty"); error (ErrorState section size with Retry; the last good list stays above it); filtered (matches highlighted with `match.bg`); selected (check glyph and `fill.selected.overlay`); disconnected host (last known rows with "Last known state, 5 min ago" in the header; existing local paths can still be chosen).
- Used in `sh-import-detect` (Project per session and the batch), `sh-add-host` (SSH key trigger).
- Keyboard: Enter, Space or Down opens; typing filters; Up and Down move; Enter chooses; Escape returns focus to the trigger.

**D-OT-01 Crash report.** Redesign. A Dialog titled "MassOrca quit unexpectedly" or "A view failed", with "Show what would be sent" (a mono preview block), a checkbox "Send this report" (off by default) and actions "Send report" (primary, enabled only when the box is checked) and "Do not send" (secondary). Nothing is sent without consent. States: shown; sent ("Report sent" only after the send succeeds); dismissed. No artboard.

**D-OT-02 macOS permission prompts.** Merged into D-ST-10 (privacy and OS permissions section).

**D-OT-03 Linux package install recovery.** Keep. ErrorState section variant with a numbered list of recovery steps (for example "Run the package manager's repair command"), Retry and "Show details". States: failed; recovered (replaced by the normal content). No artboard.

**D-OT-04 Unsealed credential notice.** Keep. Warning Banner at the top of the Settings section it affects (accounts, secret vault). Copy: "No secure store is available on this PC, so credentials are not copied or saved here. Sign in to each agent instead." Stays until fixed. No artboard.

**D-OT-05 Dictation overlay.** Keep. A pill at the bottom centre of the Canvas, `surface.overlay` with e2, showing a state: idle (hidden), listening (microphone glyph, "Listening" and a level meter in `accent.default`), processing ("Transcribing"). Shortcut Mod+E. Open point: the key conflicts with shells (G16). No artboard.

**D-OT-06 Keep-awake card.** Merged into D-ST-02 (a setting).

**D-OT-07 Ephemeral VM dialogs and setup guide.** Dropped. Experimental cloud VM runtimes, not in the plan.

**D-OT-08 API key entry dialogs.** Merged into D-ST-08. Keys are entered in Settings and never pasted into notes (open question G21 on whether they move into the vault).

**D-OT-09 Skill usage example dialog.** Dropped. Documentation, not a product screen.

**D-OT-10 Icon and emoji pickers.** Merged into D-WS-04 (workspace edit).

**D-OT-11 CLI registration dialog.** Merged into D-ST-10 (a setting).

**D-OT-12 Codex login link notice.** Merged into X-AC-01 (sign-in card).

**D-OT-13 Theme import dialogs.** Merged into D-ST-03 (theme import in appearance settings).

**D-OT-14 Profile sign-out confirm.** Merged into X-DLG-01 (default confirm with a named account).

---

## Cross-cutting states

**Host unreachable.** Designed in `sh-host-unreachable` (light). Rules that apply everywhere:
- Sidebar header and rows dimmed in `text.tertiary` with the dashed unreachable glyph and "Last known state, 5 min ago". Rows keep their ages.
- Title bar suffix "(host unreachable)" in `status.warning.fg` with the word.
- Conversation: the unreachable Banner at the top; the transcript stays readable; the composer keeps the approved session body (+, access chip, model picker with agent icon, microphone, round Send, A33) and its placeholder reads "VPS cannot be reached. Messages wait in the queue."; queued messages show "Waiting for VPS. Sends after the host confirms the session." (A10).
- Session summary: the Environment card (title `server`) shows the host in its header with the unreachable glyph, then Repository (`book`), Branch (`git-branch`) and Last seen (`clock`), each with its name as tooltip (A22, A23); the Context card title carries `gauge`.
- Cards and toasts: "Last known: waiting for you, 5 min ago" on the card, action Review (A45); the toast "Could not reach VPS. Try again later." (never "stopped"). Save actions (secrets, A32) are refused with the host state shown.
- Status bar: "1 of 2 hosts unreachable".
- Closes gap G2. The state word is "Unreachable"; losing contact is not proof that a process died.

**Protocol mismatch (desktop).** Closes gap G3. The host row shows the state word "Update needed" with the warning glyph. The host's regions show a warning Banner: "VPS runs protocol 4. This app needs protocol 5. Update VPS to continue." with the action "Update VPS" when the server supports it, otherwise "How to update". Sessions on that host cannot start; reading the last known state still works. The block is on the host, not the whole app. Android keeps the gate M-SH-02 (Android writer).

**Auth failure.** Host: Banner warning "VPS needs you to sign in" with "Sign in"; host row "Sign-in needed". Agent account: the X-AC-01 card with one action. SSH key passphrase: D-HO-06. Secrets are never shown back or logged.

**Service not running (local).** Closes gap G1 for the desktop. If the local service does not start, the Frame stays and the Canvas shows an ErrorState page: title "MassOrca service is not running", explanation "Projects, agents and terminals run in the local service on this PC. Retry starts it. Show details has the last start error.", actions Retry (secondary) and Show details (ghost). The Status bar hosts item reads "Service not running". Copy stays away from claims about data safety until the owner confirms them. Not drawn; described here.

---

## Component gaps

1. **Wizard step indicator.** Not a component. Built from Progress (4 px bar, three steps) and an overline "Step n of 3 · name". A StepIndicator component is proposed if the wizard grows.
2. **Theme preview frame.** A schematic of the frame and canvas in one theme, used on the theme step (`sh-wizard-theme`). Built from tokens in a fixed 300 by 188 box. Proposed as a small component if more pickers need it.
3. **Session import table row with a mapping field.** Built from the Table component and a Picker-style trigger. No new component.

## Decisions for owner review

| # | Decision | Alternative | What changes with the alternative |
|---|---|---|---|
| 1 | The setup wizard is a full Canvas page with the Sidebar hidden and the Title bar reading "Set up MassOrca" | A modal over the shell | Only the first-run layout; the rail and Title bar stay the same |
| 2 | The Activity rail stays usable during the wizard; entries open pages while setup is incomplete | Lock the rail until the wizard ends | Navigation during first run; the checklist still reopens the wizard |
| 3 | "Add host" stays disabled until the connection test passes | Add the host at once; test is optional | Validation flow in D-HO-02; a host may be added without a working connection |
| 4 | The Server path asks for the server's pairing code in a masked field inside the same dialog (closes G4) | A separate pairing page | Dialog size and the D-HO-02 states |
| 5 | Remove host uses the copy "Nothing on the server is deleted" and lists counts | Different copy or no loss list | Wording of the destructive confirm; the loss list is required by X-DLG-01 |
| 6 | The Sidebar bell expands the notification card stack to show every card; there is no separate notification centre and no history in v1 | A notification centre page or popover with history | A new list surface and history storage; Dismiss would keep items |
| 7 | Session import is a full page (Sidebar hidden, Back in the Title bar), not a dialog | A dialog about 720 px wide | Layout of D-IM-02 and D-IM-03; the table needs the width |
| 8 | Sessions without a project are mapped once, in one batch with one Picker | One Picker per session | Less work for many sessions; a group choice may hide a wrong match |
| 9 | Desktop protocol mismatch blocks the host with a Banner and the state word "Update needed" (closes G3) | Block the whole app, or show nothing | Host-level versus app-level blocking |
| 10 | A failed local service shows a Canvas ErrorState with the Frame kept (closes G1) | A blocking full-window dialog | Whether the shell stays usable while the service is down |
| 11 | The telemetry helper says "No prompts, code or file names" | Other wording, or no description | Copy needs privacy review before release |
| 12 | The theme step selects "Match system" until the user chooses (A8); the light artboard shows Light chosen | Default to dark | Which theme new users see first |
| 13 | Mock account rows use Claude, Codex, OpenCode and Cursor (the D-AC-01 list); Gemini appears only as a session source in D-IM-01 | Add Gemini as an account | Provider list in the accounts step |
| 14 | Mock paths use `~/.claude` and `D:\projects\...` and host addresses from the documentation range 203.0.113.x; no real names or accounts | Real project paths | Artboard realism only |
| 15 | Superseded by the owner (agreement A43): the Work panel empty state is a centred column of four buttons, each as wide as its label (see D-SH-17) | | |

## Not covered

- Artboards for D-PR-01 (phone pairing), D-HO-07 (servers), D-AC-06 (usage detail), D-SH-17 (Work panel), D-SH-18 (mobile emulator), X-DG-01 as a standalone page, D-OT-01, D-OT-05, D-SH-04 (Project sidebar tabs), D-IM-04 and X-IM-05 as drawn elements. Each is specified in text above.
- Android rows: M-SH-01 to M-SH-04, M-PA-01 to M-PA-05, M-HO-01, M-AC-01, M-NT-01 (Android writer).
- Rows owned by other writers and referenced here: D-ST-01 to D-ST-11, D-WS-01 and D-WS-24, D-AG-23, D-EX-01, X-AG-01, X-WS-01, X-DG-01 Android side, D-TM-02.
- Brand marks for providers (placeholders used), the secret vault (section 19), the Session summary and the chat cards.
- Screen reader names beyond those in the component specs; GPUI accessibility support is an open question (design-system 10.1).
- Visual check in a browser: the artboards were checked for structure and tag balance, not rendered.
