# Desktop conformance audit

Every desktop surface built so far, compared with its current approved artboard and the agreements register. Read-only audit of 2026-11-24 against origin/main `26894ab9`. The code read is unchanged in later commits up to `824adeb4`. Nothing was changed in code.

## How it was done

- **Sources of the design.**
  - The artboards in `docs/design/artboards/` (snapshot of 2026-11-20).
  - The General rows of `docs/design/agreements.md`, including A97, the Sidebar ruling that amends A10.
  - The component specs in `docs/design/components/` and the screen specs in `docs/design/screens/`.
- **Precedence.** When they disagree, the artboard wins (artboards README).
- **Markup-based.** Windows renders no screenshots here, so each artboard's markup (text, structure, order, `aria-label`, icon references, sizes and states) was compared with the Rust code in `crates/desktop/src/shell/`, `crates/desktop/src/model/` and `crates/ui/src/components/`. Nothing was seen rendered.
- **Fan-out.** Six read-only audits, one per area. Every finding has both an artboard location and a code `file:line`. Line numbers are against `26894ab9`.
- **Severity.**
  - **wrong structure:** the hierarchy or layout differs.
  - **missing:** an approved element or state is absent.
  - **cosmetic:** label, copy, size, colour or icon.
- **Visibility.** 3 means seen on every launch or in every session, 2 means common flows, 1 means rare.
- **Not built.** Surfaces that are not built at all are listed once in their area, without one row per missing part.

## Summary

| Area | Deviations | Wrong structure | Missing | Cosmetic |
|---|---|---|---|---|
| 1. Shell frame and onboarding (title bar, rail, status bar, palette, setup, add host, notifications) | 44 | 2 | 30 | 12 |
| 2. Sidebar, start page, Composer, Model picker | 29 | 5 | 13 | 11 |
| 3. Session view, Session summary, delegation, source control, diff | 33 (+3 spec-only) | 6 | 17 | 10 |
| 4. Work panel, Project sidebar, Prototypes, Automations, Tasks | 26 | 5 | 8 | 13 |
| 5. Marketplace and Settings | 59 | 13 | 29 | 17 |
| 6. Bot Teams pages | 75 | 12 | 47 | 16 |
| **Total** | **266** | **43** | **144** | **79** |

**Not built at all:**
- **Shell:** the first-run wizard, the import screens, the remove-host dialog and the notification stack.
- **Work panel:** the browser, Markdown or file, and mobile emulator tabs.
- **Project sidebar:** the Files, Sessions and Checks tabs.
- **Pages:** Prototypes, Automations and Tasks.
- **Session:** the conversation transcript; the session body is the live terminal.
- **Bot Teams:** the Conference room, the routine form and the Calendar, which render placeholders.

## Top 15 deviations, most visible first

| # | Surface | Severity | Deviation | Artboard | Code | Agreement |
|---|---|---|---|---|---|---|
| 1 | Sidebar | wrong structure | A workspace row (branch icon) sits between the host row and the sessions. | ws-session (host 24 px, sessions 40 px) | shell/sidebar_rows.rs:61-88; model/sidebar.rs:158-164 | A97 (amends A10) |
| 2 | Session view | wrong structure | The session body is the live terminal. There is no transcript: no messages, steps, diff card, status line or approval cards. | ws-session, ws-diff, ws-delegation | shell/session_view.rs:194-203 | A33, A35, A36 (D-AG-03 defers it) |
| 3 | Title bar | missing | "New chat", "Terminal", "Split" and "Open in Work panel" are missing, and a "Work panel" toggle sits in their place. | ws-session title bar | shell/title_bar.rs:182-188 | TitleBar.md, RegionToggle.md |
| 4 | Session composer | missing | No + attach, no model chip, and the mic is disabled. | ws-session composer | shell/session_view.rs:386-388 | A33, A28 |
| 5 | Start page | wrong structure | The placement bar is a separate row 24 px above the composer instead of attached to its top edge. The empty start page has no host or worktree chip, and with a workspace all three chips are disabled. | sh-empty-app, ws-start | shell/home_start.rs:72-125; start_page.rs:170-244 | A31, A32, A40 |
| 6 | Sidebar | wrong structure | Session rows print the worktree or branch as a second line, with a branch icon on children. | ws-session | shell/sidebar_rows.rs:253-262 | A97 |
| 7 | Sidebar | missing | Project rows have no disclosure chevron and cannot collapse, and they lack the owner or org line. | ws-start, ws-session | shell/sidebar_rows.rs:56-58; model/sidebar.rs:138-152 | SidebarTree.md, SidebarRow.md |
| 8 | Bot Teams | missing | The Conference room is a placeholder: no header, thread, delegation block or composer. | bt-conference-room | shell/canvas.rs:181 | A34, A57, A59 |
| 9 | Marketplace | wrong structure | Installed reuses the Discover card grid, with no manager rows, no status words, and no More menu (Roll back, View log, Remove). | bt-extension-manager | shell/marketplace/catalogue.rs:172-181; extension_card.rs:394-426 | A68 |
| 10 | Marketplace | missing | The permission review has no risk notice and no permission rows. Its primary reads "Install" instead of "Allow and install". | bt-permission-review | shell/marketplace/dialogs.rs:50-139 | PermissionReview.md |
| 11 | Project sidebar | missing | Files, Sessions and Checks show "Not available yet."; only Source control is built. | ws-session, ws-light | shell/project_sidebar.rs:37-41 | A6, A50 |
| 12 | Diff | wrong structure | A diff opens as a Canvas page with a Back button, not as a Work panel tab. | ws-diff | shell/mod.rs:817; diff_page.rs:18-47 | A7 |
| 13 | Setup | missing | Checklist rows have no action buttons, and five rows read "Not available yet". | sh-setup-checklist | see section 1 | D-OB-12 |
| 14 | Settings | missing | Input and shortcuts has no keybinding rows and no conflict state. | bt-settings-keybindings | model/settings_sections.rs:312-316 | D-ST-04 |
| 15 | Bot Teams | missing | The team pill is a static block with no switch, hover or pill shape. Team rows in the team Sidebar use a users glyph instead of the team character. Routines has no team pill. | bt-board, bt-routines-empty | shell/teams/mod.rs:36; teams/sidebar.rs:112; teams/routines.rs:30 | A57, A59 |

## Agreements that are stale or contradicted

Rows whose text no later row amends but which the artboards or later rows contradict:

- **A10:** "that host's workspaces under the host row". Amended by A97 in PR #309; the workspace level is gone.
- **A27:** access sits in the Model picker footer. ws-start, ws-session, A33 and ModelPicker.md put access in the composer's own chip. Mark it superseded by A33.
- **A46 and A47:** typed, coloured file badges. The session audit found generic file glyphs on ws-session and ws-diff rows in the repo snapshot, and the code follows them. Either the artboards or the agreements need refreshing.
- **A5 and A91:** call the region "Sidebar". The artboards name the landmark "Workspaces", and SidebarTree.md disagrees with itself.
- **A61:** puts Board, Routines and Calendar in the Conference room "…" menu. It was asked on Android; the desktop room artboard has no overflow button.
- **A68 and A69:** the 2026-11-24 note on A68 removes the Settings "Extensions" row. The Settings artboards (2026-11-20), D-ST-09 and A69 still show it. The code follows the note.
- **A88:** Android-only, but the desktop code cites it to keep the New workspace form unreachable (shell/workspace_dialogs.rs:166-176).
- **A52:** the artboard shows section counts only where a section has items.
- **Doubts entry for A82 to A85:** desktop nesting is answered by A97.

Spec drift found along the way:
- **workspaces-sessions.md:**
  - :183 says no picker, attach or voice in a session, against A33.
  - :258 gives a card order without Child sessions.
  - :290 places the card inside "Subagents", against A96.
  - :471 makes the commit box sunken, against A49.
  - :585 uses a 2 x 2 grid, against A43.
- **SidebarTree.md:** icon names Prototypes `shapes` and Automations `workflow`.
- **AgentIcon.md:** row and rail sizes (14 and 24) match neither the artboards (14 and 20) nor the code (16 and 20).
- **teams-extensions-settings.md L353:** cites A12 for the permission grant.

## Artboard conflicts for the owner

- **Sidebar width:** 224 px on ws-session against 256 px on ws-start. Code and spec use 256.
- **Agent marks:** 14 px on the sh-* rails and ws-session rows against 16 px elsewhere. No 14 px icon token exists.
- **Sidebar footer:** a spacer, and "Panel" or "Toggle panel".
- **Composer key hints:** on ws-start, absent on sh-empty-app.
- **ws-diff:** "Commits in branch" sits over file rows.
- **ws-delegation:** the card lists 5 children while the tree lists 6.
- **Permission review:** the item page order differs between bt-permission-review (2026-11-20) and screen doc 7.2 (2026-11-22).
- **Settings:** the "Extensions" row (see A68 and A69 above).

---


## Shell frame and onboarding: conformance audit (section 1)

**Scope.** Approved design: `docs/design/artboards/sh-*.dc.html` (13 shell screens), `ws-palette.dc.html`, and the title bar, rail and status bar of `ws-session.dc.html`. `ws-reasoning-slider.dc.html` is excluded; `Main.dc.html` is the index only. Rules: `docs/design/agreements.md` (General rows and the rows cited here), the component specs TitleBar, ActivityRail, RailItem, UsageRing, StatusBar, StatusItem, FrameCanvas, RegionToggle, CommandPalette, Banner, NotificationCard and Toast, and `docs/design/screens/shell-setup-hosts.md`.

**Method.** The comparison is markup-based. Windows renders no screenshots here, so artboard tags, aria-labels, roles, icon references, sizes and CSS were compared with the Rust code. Code paths are relative to ``. That checkout has no `.codegraph` index, so Grep and Read were used. `codegraph init` was not run: the checkout is read-only, and the CodeGraph tool reports indexing as the user's decision.

**Scales.** Severity: missing (approved element or state absent), wrong structure (hierarchy or layout differs), cosmetic (label, copy, size, colour, icon or role). Visibility: 3 = every session view or launch (title bar, rail, status bar, start page); 2 = common flows (palette, setup page, notifications, host outage); 1 = first run, rare dialogs, or accessibility-only names.

**Not audited.** Pairing: no desktop artboard exists in the sh-* or ws-* set (`an-pairing.dc.html` is Android, out of scope). Work panel and Project sidebar contents of `ws-session.dc.html` are outside this brief.

### Title bar

Artboards: `ws-session.dc.html` title bar (toolbar "Title bar"); sh-* title bars.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| 1 | missing element | missing | 3 | ws-session.dc.html: button aria-label "New chat" (aria-pressed false) | crates/desktop/src/shell/title_bar.rs:182-187 (toggle set has no New chat) | No New chat control in the bar. RegionToggle.md lists it. | - |
| 2 | missing element | missing | 3 | ws-session.dc.html: button aria-label "Terminal" (aria-pressed true, selected fill) | crates/desktop/src/shell/title_bar.rs:182-187 | No Terminal toggle. The middle toggle in code is "Work panel" (row 3). | - |
| 3 | extra element | wrong structure | 3 | ws-session.dc.html: no "Work panel" toggle in the bar | crates/desktop/src/shell/title_bar.rs:185 ("toggle-work-panel", PanelBottom); crates/desktop/src/model/title_bar.rs:20-24 | Code draws a Work panel toggle where the artboard has Terminal. | - |
| 4 | missing element | missing | 3 | ws-session.dc.html: button aria-label "Split" (aria-pressed false) | crates/desktop/src/shell/title_bar.rs:182-187 | No Split toggle. RegionToggle.md lists it. | - |
| 5 | missing element | missing | 3 | ws-session.dc.html: button aria-label "Open in Work panel" (aria-pressed false) | crates/desktop/src/shell/title_bar.rs:182-187 | No "Open in Work panel" action. RegionToggle.md lists it. | - |
| 6 | missing element | missing | 1 | sh-host-unreachable.dc.html: span "(host unreachable)" in warning colour after "Fix login redirect" | no "(host unreachable)" suffix in crates/desktop/src/shell/title_bar.rs or model/title_bar.rs (the phrase appears only in status-bar copy and a gallery sample) | Session title lacks the host-unreachable suffix. TitleBar.md specifies it. | - |

Matches: bar height 36; Hide sidebar, Back and Forward buttons; window controls Minimise, Maximise, Close at 46 px; the Session summary and Project sidebar toggles use aria-pressed.

### Activity rail with usage rings

Artboards: `ws-session.dc.html` rail; `ws-palette.dc.html` rail; sh-* rails.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| 1 | label (landmark name) | missing | 1 | ws-session.dc.html: div role "navigation" aria-label "Activity rail"; sh-*: nav aria-label "Activity rail" | crates/desktop/src/shell/rail.rs:115-116 (id "activity-rail", no role and no name) | The rail has no navigation role or name. | - |
| 2 | label (role) | cosmetic | 1 | every ring in ws-session.dc.html, ws-palette.dc.html and sh-*: role "img", aria-label "Claude usage 62 percent" | crates/ui/src/components/usage_ring.rs:339 (Role::ProgressIndicator) | Code uses progressbar, as UsageRing.md says; artboards use img. Spec and artboards disagree; code follows the spec. | - |

Matches: width 48; buttons 36 px; icons 20 px; active bar 2 px at -6 px; order (mode, Tasks, Calendar, Marketplace, theme toggle, usage rings, Settings); icons layout-grid, list-checks, calendar, store, sun/moon, settings; theme tooltips; rings 32 px; no Automations item in the rail.

### Status bar

Artboards: `ws-session.dc.html` status bar (role "contentinfo", aria-label "Status bar", line 859); `ws-palette.dc.html` same; sh-* footers.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| 1 | token/size | cosmetic | 3 | ws-session.dc.html: agent glyph before "1 running" at flex 0 0 16px | crates/ui/src/components/agent_status.rs:119 (XSmall = SIZE_ICON_XS, 12 px); crates/desktop/src/shell/status_bar.rs:81-82 | Agent glyph is 12 px in code and 16 px in ws-session. StatusBar.md says 12 px, so spec and artboard disagree. | - |
| 2 | label (role) | cosmetic | 1 | ws-session.dc.html and ws-palette.dc.html: role "contentinfo", aria-label "Status bar" | crates/desktop/src/shell/status_bar.rs:42-44 (Role::Status) | Role differs from the artboards. StatusBar.md specifies status. | - |

Matches: height 24; "1.22 GB"-style memory value; terminals with a 12 px icon; "1 port" / "2 ports"; "2 hosts" with a 6 px success dot; "1 of 2 hosts unreachable"; item order agents, memory, terminals, ports, hosts.

### Canvas and regions

Artboards: sh-* canvas (top-left radius 12); sh-host-unreachable (Conversation section, Session summary aside 288 px); `ws-session.dc.html` sidebar nav; sh-empty-app start page.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| 1 | label (landmark name) | missing | 1 | sh-host-unreachable.dc.html: section aria-label "Conversation", aside aria-label "Session summary"; sidebar nav aria-label "Workspaces" (ws-session.dc.html and sh-*) | crates/desktop/src/shell/canvas.rs:68-80, 292-304, 314-333 (no role or label); crates/desktop/src/shell/sidebar.rs (no role or aria-label) | Conversation, Session summary and Sidebar regions have no landmark names. Artboards name the sidebar "Workspaces"; see Stale agreements. | A5, A91 |
| 2 | state | missing | 3 | sh-empty-app.dc.html: placement chip "This PC" (role button, aria-haspopup menu, chevron-down); chip "Choose a project" (opens the project menu); only "New worktree" is dimmed | crates/desktop/src/shell/start_page.rs:170-173 (start-host disabled), 177-180 (start-project disabled), 185-192 (start-branch disabled) | All three placement buttons are disabled in code, and the host button has no menu or chevron. The artboard shows host and project active; only the worktree chip is dimmed. | - |

Matches: canvas top-left radius 12; sidebar 256 px (sh-*); Session summary 288 px; Frame has no border or shadow between parts (A1).

### Command palette

Artboards: `ws-palette.dc.html` (card at top 96, width 600, radius 12, input row 44, results max 480, footer row).

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| 1 | missing element | missing | 2 | ws-palette.dc.html: sunken footer row (32 px, top rule) with "Ctrl P" "Go to file", "Ctrl Shift F" "Search in files", "Ctrl ," "Settings" | crates/desktop/src/shell/palette.rs:126-153 (mode label and Mod+P only in the input row; Esc at far right) | No footer with the three mode hints. CommandPalette.md puts the mode label at the input's left, so spec and artboard disagree. | - |
| 2 | missing element | missing | 2 | ws-palette.dc.html, query "term": "New terminal tab" (Ctrl T), "Terminal: split pane", "Terminal: clear scrollback" | crates/desktop/src/model/palette.rs:250-295 (command list has no terminal commands) | Terminal commands shown in the results are absent. | A13 (Commands group) |
| 3 | missing element | missing | 2 | ws-palette.dc.html Tabs group: row "zsh, oppm" with detail "terminal in the Work panel" (selected) | crates/desktop/src/model/palette.rs:25 (Tabs label only; no item uses the group) | Tabs group has no rows. | A13 |
| 4 | structure (row composition) | wrong structure | 2 | ws-palette.dc.html Workspaces row: title "oppm / terminal-reconnect", trailing "waiting for you" | crates/desktop/src/model/palette.rs:215-218 (title = workspace name, detail = project name, state None) | Workspace rows show the wrong title and detail and no state. | A13 (pattern) |
| 5 | extra element (icon) | cosmetic | 2 | ws-palette.dc.html session row trailing: "Waiting for you" text, no glyph | crates/desktop/src/shell/palette.rs:274 (AgentStatus XSmall glyph before the state word) | Code adds a state glyph before the state word. | A13 |
| 6 | token/size | cosmetic | 2 | ws-palette.dc.html group header div.ov, height 24 px ("Workspaces", "Sessions") | crates/desktop/src/shell/palette.rs:160 (h SPACE_4 = 16 px) | Header is 16 px in code and 24 px in the artboard. CommandPalette.md says 16 px, so spec and artboard disagree. | - |

Matches: panel width 600, top 96, input row 44, results max 480, Esc key cap, group order Workspaces, Sessions, Tabs, Commands (Recent only when the query is empty), session row starts with a 16 px agent icon.

### Onboarding wizard and Setup checklist

Artboards: sh-setup-checklist; sh-wizard-accounts, sh-wizard-theme and sh-wizard-notifications (title bar "Set up MassOrca"); sh-accounts-result; sh-import-detect; sh-import-result.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| 1 | missing element | missing | 2 | sh-setup-checklist.dc.html: h1 "Setup" (28/36 px) above the card | crates/desktop/src/shell/setup.rs:14-66 (no heading drawn; "Setup" is only the page name) | No Setup heading on the page. | D-OB-12 (screen spec) |
| 2 | label, order | cosmetic | 2 | sh-setup-checklist.dc.html: intro under the progress bar: "Setup is optional and can be finished later. Open this checklist again from Settings, General." | crates/desktop/src/shell/setup.rs:61 (footnote at the bottom: "Setup is optional. Find it again in Settings, General and account.") | Copy and position differ. | - |
| 3 | structure, missing element | missing | 2 | sh-setup-checklist.dc.html rows: status icon (aria "Needs attention", "Not started" or "Done"), title, detail, trailing action button: "Open", "Add host", "Add a project", "Change...", "Import", "Change" | crates/desktop/src/shell/setup.rs:81-85 (ListRow with a trailing state word); crates/desktop/src/model/setup.rs:35-84 | Rows have no action buttons. Code puts a state word where the artboard has the action. | - |
| 4 | state, label | missing | 2 | sh-setup-checklist.dc.html "Agent accounts": "Needs attention", "Claude is signed in. Codex and OpenCode need sign-in." | crates/desktop/src/model/setup.rs:37 "Agent sign-in is not available yet." (NotAvailable) | Wrong state and copy. | - |
| 5 | label | cosmetic | 2 | sh-setup-checklist.dc.html "Add a host": "Run work on a server you reach over SSH or the local network." ("Not started") | crates/desktop/src/model/setup.rs:43 "This PC is connected." (Done) or "The service is not connected." | Copy differs; the state comes from the connection. | - |
| 6 | label | cosmetic | 2 | sh-setup-checklist.dc.html "Create a workspace": "Start in a project folder or a git repository." ("Not started") | crates/desktop/src/model/setup.rs:52-53 "No project yet." / "1 project on This PC." | Copy differs. | - |
| 7 | state, label | missing | 2 | sh-setup-checklist.dc.html "Conversations without a project": "Done", "Saved in ~/MassOrca/Conversations on This PC. Default; you can change it.", button "Change..." | crates/desktop/src/model/setup.rs:65 "...Changing it is not available yet." | Wrong state and copy. The artboard says the folder can be changed; the code says it cannot yet. | A95 |
| 8 | state, label | missing | 2 | sh-setup-checklist.dc.html "Import sessions": tag "Optional", "Found 20 sessions in Claude Code, Codex and OpenCode. Read only.", button "Import" | crates/desktop/src/model/setup.rs:71 "Session import is not available yet." | Wrong state and copy. | - |
| 9 | label | cosmetic | 2 | sh-setup-checklist.dc.html "Theme": "Dark", button "Change" | crates/desktop/src/model/setup.rs:74 "{theme} is in use." | Copy differs. | - |
| 10 | state, label | missing | 2 | sh-setup-checklist.dc.html "Notifications": "Done", "On for waiting for you and errors", "Change" | crates/desktop/src/model/setup.rs:77 "Notification settings are not available yet." | Wrong state and copy. | - |
| 11 | state, label | missing | 2 | sh-setup-checklist.dc.html "Usage data": "Done", "Not shared", "Change" | crates/desktop/src/model/setup.rs:82 "Usage data settings are not available yet." | Wrong state and copy. | - |
| 12 | missing element (screen) | missing | 1 | sh-wizard-accounts.dc.html: "Step 1 of 3 · Agent accounts", h1 "Bring your agent accounts", account rows with checkboxes and tags Found / Not found / Not signed in, footer "Skip accounts" and "Import 3 accounts" | none: no "Bring your agent accounts" or "Set up MassOrca" in crates/desktop/src | First-run wizard step 1 is not built. | D-OB-01 (screen spec) |
| 13 | missing element (screen) | missing | 1 | sh-wizard-theme.dc.html: "Step 2 of 3 · Theme", h1 "Choose a theme", Dark / Light / Match system choices, footer "Back" and "Continue" | none (the only "Choose a theme" string is a radio validation message, crates/ui/src/components/radio.rs:657) | Wizard step 2 is not built. | D-OB-03 (screen spec) |
| 14 | missing element (screen) | missing | 1 | sh-wizard-notifications.dc.html: "Step 3 of 3 · Notifications and privacy", h1 "Notifications and usage data", four switch rows, usage radios "Do not share usage data" / "Share usage data", footer "Back" and "Finish setup" | none | Wizard step 3 is not built. | - |
| 15 | missing element (screen) | missing | 1 | sh-accounts-result.dc.html: h1 "1 account migrated, 2 need you", warning banner "Sign in to 2 accounts to use them here.", groups Migrated / Needs sign-in or retry / Skipped | none | Accounts result screen is not built. | - |
| 16 | missing element (screen) | missing | 1 | sh-import-detect.dc.html: h1 "Import sessions from other agent tools", "Found on this PC" list, "Sessions to import" table with an Agent icon column, disabled "Import 5 sessions" | none (checklist row 8 only) | Import selection screen is not built. | A19, A20 |
| 17 | missing element (screen) | missing | 1 | sh-import-result.dc.html: h1 "Importing 5 sessions", per-session status, footer "Runs in the background. Use Back to leave; a notification tells you when it ends." | none | Import progress screen is not built. | A42, A93 |

Matches: "{n} of 8 done" progress format; the eight row labels; Theme row present; "Conversations without a project" row present with the default path text.

### Add host and pairing

Artboards: sh-add-host.dc.html (passed state); sh-host-test-failed.dc.html (failed state, role alertdialog); sh-confirm-destructive.dc.html (remove host).

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| 1 | label | cosmetic | 1 | sh-add-host.dc.html: SSH key helper "Lists the keys in your SSH config. The passphrase is asked only when needed." | crates/desktop/src/shell/add_host.rs:492 "Keys from ~/.ssh. A key with a passphrase must be loaded in your SSH agent first." | Copy differs. The screen spec (shell-setup-hosts.md L204) records the code text and says the artboard line was not kept; the artboard still shows it. | screen spec L204 (conflict) |
| 2 | label | cosmetic | 1 | sh-add-host.dc.html success notice: "Connected." then "Protocol 5 matches this app. Test passed at 09:41." | crates/desktop/src/model/add_host.rs:256-257 "The host runs Linux on {arch}." | Copy differs. | - |
| 3 | label (role) | cosmetic | 1 | sh-host-test-failed.dc.html: div role "alertdialog" aria-labelledby "hf-title" | crates/desktop/src/shell/add_host.rs:331-334 (Dialog Confirm, role dialog; crates/ui/src/components/dialog.rs:326-330) | Failed state is a dialog in code; the artboard uses alertdialog. | screen spec L205 |
| 4 | missing element | missing | 1 | sh-host-test-failed.dc.html: "Check these first" with numbered checks and button "Run network test" | no such strings in crates/desktop/src/shell/add_host.rs or model/add_host.rs | Failed-state checks and the network test button are missing. | - |
| 5 | missing element | missing | 1 | sh-host-test-failed.dc.html footer: ghost button "Copy details" at the left | no "Copy details" in crates/desktop/src/shell/add_host.rs (the label exists only in marketplace/dialogs.rs:130 and crates/ui/src/components/error_state.rs:216) | Copy details button is missing from the add-host failed state. | - |
| 6 | label, state | missing | 1 | sh-host-test-failed.dc.html footer: primary "Retry" | crates/desktop/src/shell/add_host.rs:353-359 ("Test again", secondary, left) and 338-340 ("Add host", primary, disabled until the test passes) | In the failed state the artboard's primary action is Retry; code shows Test again plus a disabled Add host. | - |
| 7 | missing element (dialog) | missing | 1 | sh-confirm-destructive.dc.html: alertdialog "Remove host VPS?", three loss lines, "Cancel" (focused), "Remove host" (danger) | none ("Remove host" absent from crates/desktop/src) | Remove-host confirmation is not built. | X-DLG-01 (screen spec) |

Matches: dialog width 440 (`.dlg` width 440 px; code DialogWidth::Medium = 440); title "Add host"; SSH host / Server segment; fields Name, Address, Port (84 px column), User, SSH key; footer Test again, Cancel, Add host with refresh and server icons; "Connected." lead.

### Notifications

Artboards: sh-notifications.dc.html.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| 1 | missing element (component stack) | missing | 2 | sh-notifications.dc.html: bottom-right stack with a command card "Run command? pnpm install" (Dismiss), a sign-in card "Sign in" (Dismiss), an update card "MassOrca 1.4.2 is ready" with "Restart now", and a "+1 more" fold | none: NotificationCard and Toast have no call site in crates/desktop/src; they are used only in crates/ui/src/gallery | No notification stack in the desktop app. The sidebar bell exists (crates/desktop/src/shell/sidebar.rs:167-170). | - (screen spec D-NT-01, X-UP-01) |

### Service banners

Artboards: sh-host-unreachable.dc.html.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| 1 | state | missing | 2 | sh-host-unreachable.dc.html: div role "status": "VPS cannot be reached." and "Showing the last known state from 5 min ago. Work that was running there may still be running.", buttons "Retry" and "Details" | crates/desktop/src/shell/service_banner.rs:16-18 (Unreachable tone returns div().hidden()); the copy would be "The MassOrca service cannot be reached." (crates/desktop/src/model/service_view.rs:190) | The unreachable banner is hidden, so no host-unreachable banner is shown. The code has a service banner only. | - (Banner.md X-CN-01) |

## Stale agreements

- **A5 and A91** name the region "Sidebar" (glossary and copy). The artboards name it "Workspaces": sh-* aside aria-label "Workspaces" and `ws-session.dc.html` nav aria-label "Workspaces". No later row amends A5 or A91, so the artboards contradict them. A9 already says "Workspaces Sidebar", so the agreements also disagree with each other.
- No other contradiction found in this area. Checked against artboard markup and consistent: A1, A2, A3, A4, A9, A11, A12, A15 (artboard accent #7461d8 and #b8a8f8 match; no #2c63dc remains under crates), A16, A26, A38, A40, A42, A92, A93, A95 (the artboard agrees, the code contradicts it, see row 7 above), A97.

## Artboard and spec conflicts (not counted as deviations)

1. Usage-ring mark: 14 px in sh-* (`width:14px;height:14px`) but 16 px in ws-session and ws-palette. UsageRing.md says 16 px; code uses 16 px (SIZE_ICON_SM).
2. Sidebar width: 256 px in sh-* and ws-palette, 224 px in ws-session (`width:224px`). Spec and code use 256 px.
3. Session summary width: 288 px in sh-host-unreachable, 264 px in ws-session (`width:264px`). Spec and code use 288 px.
4. Status bar memory label: "Memory 1.22 GB" in sh-*, "1.22 GB" in ws-session. Code (crates/desktop/src/model/shell.rs:157-160) follows sh-*.
5. Status bar agents item: ws-session shows "1 running", sh-* footers omit the item. Code always renders it and reads "Agents: idle" when idle (crates/desktop/src/model/shell.rs:148-155). Code follows ws-session and the spec.
6. Full-page Back: TitleBar.md says labelled "Back" on full pages; sh-setup-checklist shows an icon-only button aria-label "Back". Code uses icon-only except Tasks and Automations.
7. Unreachable banner fill: sh-host-unreachable uses `surface-raised`; Banner.md and crates/ui/src/components/banner.rs:41-52 use the agent.unreachable tint.
8. Command palette footer: ws-palette shows mode hints in a footer; CommandPalette.md places the mode label at the input's left (row 1).
9. Ring accessible name: artboards use aria-label "Claude usage 62 percent" and put "resets in 2 days" in the title only; UsageRing.md includes it in the name.

## Checks run

- Artboard outlines generated with a read-only parser (tags, aria, roles, icon refs, CSS) for all sh-* files, ws-palette and ws-session.
- Token values checked in crates/ui/src/tokens/generated.rs: rail 48, rail button 36, title bar 36/40, status bar 24, usage ring 32, icon sm 16, icon xs 12, sidebar 256, summary 288, work panel 480, conversation min 360, palette width 600, top 96, input 44, max 480, toast 360, z toast 60, canvas radius 12.
- Code claims re-checked by grep: no notification call sites in crates/desktop/src; no "(host unreachable)" title suffix or "Remove host" string; no terminal commands or Tabs palette items; start page placement buttons disabled at start_page.rs:173, 180, 192.


## Sidebar, start page, Composer and Model picker

29 deviations: 5 wrong structure, 13 missing, 11 cosmetic. The Model picker has no confirmed deviation. Code read at origin/main `26894ab9`. Paths: `shell/` = `crates/desktop/src/shell/`, `model/` = `crates/desktop/src/model/`, `components/` = `crates/ui/src/components/`, artboards in `docs/design/artboards/`. Visibility: 3 every launch, 2 common flows, 1 rare.

### Sidebar tree and rows

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| S1 | structure | wrong structure | 3 | ws-session Sidebar: host "This PC" (indent 24), sessions directly below (indent 40) | shell/sidebar_rows.rs:61-88 (TreeRow::Workspace); model/sidebar.rs:158-164; components/sidebar_row.rs:38-43, 109-120 | A workspace row (git-branch icon, folder for plain folders) sits between host and sessions. | A97 (A10 partly superseded) |
| S2 | structure | wrong structure | 3 | ws-session: session row = status glyph, agent icon, title ("Feed endpoints") | shell/sidebar_rows.rs:253-262; model/sidebar.rs:182 | Session rows print the worktree or branch as a second line; own-worktree children add a branch icon. | A97 |
| S3 | token/size | cosmetic | 3 | ws-session indents: project 8, host 24, session 40, child 62 | components/list_row.rs:35 (TREE_INDENT 12), 374-383, 596; shell/sidebar_rows.rs:272-275 | 12 px per level instead of 16. | A85; SidebarTree.md |
| S4 | token/size | cosmetic | 3 | ws-session: agent icon 14 px, child step 22 px | components/sidebar_row.rs:132 (16 px), 23 (child indent 24) | Icons 16 px, child step 24 px; no 14 px icon token exists. Docs disagree (AgentIcon.md 14, SidebarRow.md 16). | A84; owner decision |
| S5 | extra element | cosmetic | 3 | ws-session host rows: server icon and name only | components/sidebar_row.rs:93-95 | Connected hosts add a green dot and "Connected". The specs ask for them; the artboard does not show them. | spec vs artboard |
| S6 | missing | missing | 3 | ws-session "oppm" + "acme"; ws-start "NexusCare" + "acme" | shell/sidebar_rows.rs:56-58; model/sidebar.rs:138-144 | Project rows have no owner or org line. | SidebarRow.md, SidebarTree.md |
| S7 | missing | missing | 2 | ws-create-workspace "Project icon for launchpad-web" | shell/sidebar_rows.rs:56-58 (fixed Folder); model/sidebar.rs:138-144 | Custom project icon (A41) not implemented; no icon editor. | A41 |
| S8 | missing | missing | 3 | ws-start "NexusCare" collapsed (chevron-right); ws-session "oppm" expanded (chevron-down) | shell/sidebar_rows.rs:56-58; model/sidebar.rs:149-152 | No disclosure chevron and no collapse on project rows. | SidebarTree.md Behaviour |
| S9 | extra element | cosmetic | 3 | ws-session selected row: fill and bold, no bar | components/list_row.rs:728-738 | Selected rows also draw a 2 px accent bar. | SidebarRow.md open point |
| S10 | icon | cosmetic | 1 | sh-empty-app "No project" uses a struck-through folder | shell/sidebar_rows.rs:56-58; service_view.rs:50, 366 | A "No project" group row shows a plain folder. | A95 |
| S11 | missing | missing | 1 | ws-create-workspace "No Git" pill and "This folder is not a Git repository" popover | shell/sidebar_rows.rs:56-58 | Plain-folder projects have no "No Git" pill or popover. | D-WS-11, D-WS-14 |
| S12 | token/size | cosmetic | 3 | ws-session Projects header: 24 px row + 8 px top margin | shell/sidebar.rs:145-152 | Header about 6 px taller (38 px). | SidebarTree.md |

Matches: Create button, Search, the Prototypes, Tasks, Automations and MassOrca Mobile rows with their icons, Prototypes count, the four Projects header buttons.

### Start page

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| S13 | missing | missing | 3 | sh-empty-app: chips "This PC", "Choose a project", "New worktree" above the composer | shell/home_start.rs:72-87, 116-125 | Empty start page has only the project button; no host or worktree chip. | A32, A40 |
| S14 | structure | wrong structure | 3 | sh-empty-app: placement bar attached to the composer top edge (shared border, rounded top corners) | shell/home_start.rs:115, 125; start_page.rs:241, 244 | Placement is a separate row 24 px above the composer. | A32 |
| S15 | state | missing | 2 | ws-start: host, project and "New worktree from main" chips, all active with icons and chevrons | shell/start_page.rs:170-193 | All three chips disabled; icons and chevrons missing; "No branch" default. | A31, A32 |
| S16 | copy | cosmetic | 2 | ws-start: project name with a dotted underline | shell/start_page.rs:147 | Solid underline. | A40 |
| S17 | extra element | wrong structure | 2 | ws-start: composer then status bar only | shell/start_page.rs:220-233, 259 | Adds a "Workspace" facts card and an action row under the composer. | not in artboard |
| S18 | state | missing | 1 | sh-empty-app: "No project" enabled | shell/home_start.rs:45-63 | Disabled until the service offers `agent-no-project/1` (deliberate). | A95 |
| S19 | icon | cosmetic | 2 | sh-empty-app: folder with a diagonal strike | shell/home_start.rs:59; workspace_dialog_views.rs:32 (FolderX) | Drawn as folder-x (an X inside the folder). | A95 |

### New workspace form

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| S20 | state | missing | 2 | ws-create-workspace (D-WS-14 page) | shell/workspace_dialogs.rs:166-176 (`open_create` dead code outside tests); sidebar.rs:136-143 | The approved form cannot be reached; "+ Create" opens the start page. The dead-code note cites A88, which is Android-only. | A88 misapplied, A89 |
| S21 | structure | wrong structure | 2 | ws-create-workspace: page in the canvas, 640 px column, "New workspace" with subtitle "in launchpad-web, on This PC" | shell/workspace_dialog_views.rs:79-91 | Modal dialog instead of a page; no subtitle. | D-WS-14 |
| S22 | missing | missing | 2 | ws-create-workspace: first field "Host" | shell/workspace_dialog_views.rs:51-62 | No Host field. | A89 |
| S23 | missing | missing | 2 | ws-create-workspace: "Type" (Git worktree / Plain folder) and "Location" | shell/workspace_dialog_views.rs:51-76 | No Type control and no Location field. | D-WS-14 |
| S24 | label | missing | 2 | ws-create-workspace: "Base ref" and "Parent workspace" | shell/workspace_dialogs.rs:186-189 | Field reads "Base"; no Parent workspace. | D-WS-14 |
| S25 | extra element | cosmetic | 2 | ws-create-workspace: no project selector, Branch field or existing-branch checkbox | shell/workspace_dialog_views.rs:27-50, 75; workspace_dialogs.rs:180-184 | Three controls the approved form does not have. | D-WS-14 |
| S26 | missing | missing | 2 | ws-create-workspace: "Run the setup script after creating" | shell/workspace_dialog_views.rs:51-76 | No setup script checkbox. | D-WS-14 |
| S27 | missing | missing | 1 | ws-create-workspace: "This folder is not a Git repository" with "Initialize Git" | shell/workspace_dialog_views.rs:18-96 | No Git notice in the form. | D-WS-11, D-WS-14 |
| S28 | copy | cosmetic | 2 | ws-create-workspace: Name caption "Shown in the sidebar. Does not rename the folder." | shell/workspace_dialogs.rs:179 | Caption missing. | D-WS-14 |

### Composer

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| S29 | copy | cosmetic | 2 | ws-start: Send `aria-label="Send"` | components/composer/chrome.rs:304-306 | Accessible name "Send message". | A33 |

Matches: 12 px body, placeholder, Attach, "Workspace write" access chip, Chat / Terminal with icons, mic, round Send.

### Model picker (X-AG-02)

No deviation confirmed: rail, search, model rows with Mod+1 to Mod+4, "Show more", Reasoning column, Standard / Fast footer without access. Not verified: flip below the composer, the composer model chip.

### Stale agreements and spec drift (this area)

- A27 still lists access in the picker footer; ws-start and ModelPicker.md say otherwise. No superseded mark.
- SidebarRow.md "Session (under its workspace)" and the WS-35 child branch icon describe the removed workspace level (fixed by the A97 docs PR).
- SidebarTree.md names Prototypes `shapes` and Automations `workflow`; artboards and code use `layers` and `calendar-clock`.
- SidebarTree.md and SidebarRow.md ask for a host dot and state word; ws-session shows neither for connected hosts.
- AgentIcon.md says 14 px for desktop rows and 24 px for the picker rail; artboards show 14 and 20, code uses 16 and 20.
- The agreements.md Doubts entry on desktop nesting (A82 to A85) is answered by A97.
- A88 (Android) is cited in code to keep the desktop New workspace form unreachable.

### Artboard conflicts (owner to settle)

- Sidebar width: ws-session 224 px, ws-start 256 px; code and spec 256.
- Sidebar footer: ws-session, ws-start and ws-create-workspace show Help, spacer, Locate, Panel; sh-empty-app shows no spacer and "Toggle panel". Code matches sh-empty-app.
- Key hints under the composer: on ws-start, not on sh-empty-app; code always shows them.
- Attach label: "Attach" (ws-start) vs "Add attachment" (sh-empty-app).


## Session view, Session summary, delegation, source control and diff

33 artboard-backed deviations (6 wrong structure, 17 missing, 10 cosmetic) plus 3 spec-only gaps. ws-narrow was checked for frames and region toggles only, ws-light for the composer, Work panel header and Checks only. Paths are relative to `crates/desktop/src/shell/` unless they start with `crates/` or name a `crates/ui/src/components/` file (`session_summary_card.rs`, `delegation_tree.rs`, `composer/`, `queued_message*`, `work_panel_tabs.rs`, `diff_view.rs`, `text_field.rs`, `terminal_surface/`).

### Conversation

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| C1 | structure | wrong structure | 3 | ws-session Conversation column ("4 steps", "Open in Work panel", "Working, 1m 12s"); also ws-diff, ws-delegation | session_view.rs:194-203 (body is TerminalSurface); no transcript components | No transcript on desktop: no bubbles, prose, steps, diff card, status line or approval cards. D-AG-03 defers it. | A33, A35, A36 assume it |
| C2 | missing | missing | 2 | ws-session "Message minimap" | none | No minimap. | D-AG-28 |
| C3 | extra | wrong structure | 3 | ws-session: no header row, title in the Title bar | session_view.rs:154-180 (kind 176-177, Delegate 145-153, 180) | A kind label and a Delegate button that no artboard shows. | none |
| C4 | missing | missing | 3 | ws-session Title bar: "New chat", "Terminal" (pressed), "Split", "Open in Work panel" | title_bar.rs:182-188; regions.rs:43 | Four approved buttons missing; only region toggles. | TitleBar.md:69 |
| C5 | missing | missing | 3 | ws-session composer: "Attach"; chip "Opus 5.5 Medium" with agent icon | session_view.rs:387 `.attach(false)`, 388 `.model_picker(false)` | No attach and no model chip in session composers. | A33, A28 |
| C6 | state | missing | 3 | ws-session "Dictation" active | session_view.rs:386; composer.rs:186 | Mic shown disabled. | A33 |
| C7 | token | cosmetic | 2 | ws-session "Stop": dark circle, light glyph | composer/chrome.rs:318-324, 356-358 | Colours inverted. | none |

### Queued dock

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| Q1 | copy | cosmetic | 2 | ws-session dock "Queued" label | queued_message/state.rs:115-117 | Label only shows for 2 or more items. | X-CH-03 |
| Q2 | extra | cosmetic | 2 | ws-session: "Send now" text and remove | queued_message.rs:285-287, 296 | Icon-only send and an extra Edit control. | X-CH-03 lists Edit |

### Terminal sessions and Work panel empty state

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| T2 | token | cosmetic | 2 | ws-workpanel-empty: buttons 32 px high, 10 px apart, radius 8 | work_panel_tabs.rs:230-258; button.rs:55 | 28 px high, 8 px apart. | A43 |

### Session summary cards

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| SS1 | icon | cosmetic | 3 | ws-session Environment header: success dot, server icon, "VPS" | session_summary_card.rs:772-775, 834-836 | Uses an AgentStatus glyph; no server icon. | A22, A23 |
| SS2 | missing | missing | 3 | ws-session Environment rows with tooltips (Account, Repository, Branch, Pipeline), identifiers in mono | session_summary_card.rs:350-369, 405-417, 543 | No tooltips; identifiers not mono. | A22, A23 |
| SS3 | copy | cosmetic | 2 | ws-session Plan "Now: …", "Next: …" | session_summary_card.rs:450-456 | No Now / Next labels. | none |
| SS4 | state | missing | 2 | ws-session Checklist: done items struck, running item with glyph | session_summary_card.rs:460-478, 122-126 | No strike-through, no running state. | none |
| SS5 | state | missing | 2 | ws-delegation Subagents: name in mono, "Working" / "Done" | session_summary_card.rs:623-635 | No status word; name not mono. | A96, D-AG-27 |
| SS6 | state | missing | 2 | ws-delegation Child sessions: "Working", "Needs you", "Done", "Can't reach VPS", "Not started" | delegation_tree.rs:656-674 | Card rows show no state word. | A96 |

### Delegation

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| D1 | copy | cosmetic | 2 | ws-delegation "6 agents, 2 running, 1 waiting, 1 done" | delegation_tree.rs:219-242 | "needs you" instead of "waiting"; extra unreachable segment. | none |
| D2 | missing | missing | 2 | ws-delegation "Own worktree" / "Current worktree" (disabled in parallel, with reason) | delegate.rs:297-298 (Read only switch); delegation_tree.rs:699-703 | No worktree control. | A75, X-CH-06 |
| D3 | missing | missing | 2 | ws-delegation strip "Waiting for Claude to finish in this worktree", "Open child" | delegate.rs:363 | No waiting strip; no footer line. | A75 |
| D4 | structure | wrong structure | 2 | ws-delegation: delegation card in the transcript | delegate.rs:312; session_view.rs:145-153 | Delegation is a dialog, not a card. | A73, X-CH-06 |

### Source control and commit box

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| G1 | missing | missing | 2 | ws-diff "PR #313" chip | project_sidebar.rs:206-238 | No PR chip. | D-DF-02, X-PR-01 |
| G2 | missing | missing | 2 | ws-diff branch line: mono name, "+334 -15", "origin/main" with ahead/behind in warning colour | project_sidebar.rs:206-238 | No +/- counts; plain captions. | D-DF-02 |
| G3 | structure | wrong structure | 2 | ws-diff flat changed-file list | project_sidebar.rs:176-190, 298-303 | Group headers always shown (spec: only when the commit box has focus). | D-DF-02 |
| G4 | structure | wrong structure | 2 | ws-diff "Commits in branch", badge "14", "See all" | project_sidebar.rs:479-535 | Commit rows; no badge, no "See all". The artboard itself shows file rows under that heading. | D-DF-02 |
| G5 | token | cosmetic | 2 | ws-diff coloured status letters | project_sidebar.rs:353-358 | Letters in text.secondary. | none |
| G6 | missing | missing | 2 | ws-diff message box wand; split "Commit" button | project_sidebar.rs:276-283 | No AI wand, no split button. | D-DF-02, A49 |
| G7 | copy/token | cosmetic | 2 | ws-diff placeholder "Message"; raised field, radius 8, shadow e1 | mod.rs:462-466; text_field.rs:973; project_sidebar.rs:277-279 | "Commit message", 6 px radius, accent primary button. | A49 |
| G8 | missing | missing | 2 | ws-light Checks tab: one-line rows, detail, "View log", "Re-run", "Ask Claude to fix" | project_sidebar.rs:37-41 | "Not available yet." | A50, D-DF-04 |
| G9 | missing | missing | 2 | ws-session Files tab; Sessions (D-AG-07) | project_sidebar.rs:37-41 | Files and Sessions not built. | A6, D-ED-11, D-AG-07 |

### Diff page

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| F1 | structure | wrong structure | 2 | ws-diff: diff as a Work panel tab | mod.rs:817; diff_page.rs:1-3 | Diff opens as a Canvas page. | A7 |
| F2 | extra | cosmetic | 2 | ws-diff toolbar starts with badge and path | diff_page.rs:18-47 | Extra Back button and path heading. | A7 |
| F3 | missing | missing | 2 | ws-diff "vs origin/main" | diff_view.rs:1294-1302 | Base caption never set. | D-DF-01 |
| F4 | missing | missing | 1 | ws-diff Source / Rendered (Markdown) | diff_view.rs:1271-1342 | No toggle. | D-DF-01 |

Matches: diff toolbar order and counts, review bar, unchanged-lines fold, change ruler; Session summary order and value formats; delegation captions, role badge, branch chip, "Open to answer", "N commits ready"; change rows; "Nothing to commit."; Work panel empty state as a column; region collapse thresholds.

### Spec-only gaps (no artboard)

- Terminal overlays: only "Process exited" and Restart exist (terminal_surface/view.rs:430-455); reconnecting, restored, unreachable, Jump to bottom, paste confirm and size badge are missing (D-TM-02).
- Bring back is a dialog (bring_back.rs:83-116), not a transcript card (X-CH-08).
- No secret request card in the conversation; SecretRequestCard has no desktop call site (X-CH-07, A70).

### Stale or contradicted agreements and specs (this area)

- A27: access in the picker footer, contradicted by ws-session and A33.
- A46 and A47: typed, coloured file badges; this audit found ws-session and ws-diff rows drawing one generic glyph in the repo snapshot, and the code follows them. Owner decision: fix the artboards or the agreements.
- workspaces-sessions.md:290 (card inside "Subagents"; A96 says Child sessions), :471 (commit box sunken; A49 raised), :585 (2 x 2 grid; A43 column), :183 (no picker, attach or voice in a session; A33 draws them), :258 (card order without Child sessions), :46 (child rows name where they run; A97 governs).
- DelegationTree.md:49, 52 and SessionSummaryCard.md differ from the artboards (summary words, card count, header glyph, Plan wording).
- Artboard inconsistencies: ws-diff "Commits in branch" over file rows; ws-delegation's card lists 5 children, the tree 6; "Needs you" vs "Waiting for you".


## 4. Work panel and Project sidebar: conformance audit

## Basis

- Read-only audit. The comparison is markup-based (no screenshots): artboards were compared as DOM outlines and raw markup, and code was read from the checkout.
- Checkout: `` is on `docs/sidebar-session-rows` at 8bf75824, not origin/main (26894ab). The audited crate paths (`crates/desktop/src/shell`, `crates/ui/src/components`) are identical at both refs. The design docs differ: HEAD adds A97 and edits SidebarTree, SidebarRow, workspaces-sessions and app-flow. Rules were read as checked out (HEAD).
- Artboards: `ws-workpanel-empty`, `ws-browser`, `ws-prototypes`, `ws-automations`, `ws-tasks`, and the Work panel and Project sidebar of `ws-session`. Also `ws-diff`, which is outside the brief's list: D-SH-04 names it as the Source control artboard, and no listed artboard draws that view. The Checks view (`ws-light`) was not compared because the Checks tab is not built.
- Scope note: the terminal body inside the Work panel (TerminalSurface) was not compared; it is not in the brief's component list.
- Visibility: 1 = in the first view of the surface; 2 = in its normal view; 3 = only on close inspection or in one state.
- Totals: 26 deviations (8 missing, 5 wrong structure, 13 cosmetic). Surfaces that are not built have no rows.
- CodeGraph: the checkout had no `.codegraph/`. `gentle-ai codegraph init` created one, and `codegraph sync` cleared a truncated index. This leaves an untracked `.codegraph/` (about 105 MB) in the checkout; remove it if the checkout must stay clean. The broad `codegraph_explore` query returned unrelated symbols, so the rows use Grep and Read on the named files.

### Work panel tabs and empty state

Built: the strip, the terminal tab kind, and the empty state.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| W1 | missing element | missing | 1 | `ws-workpanel-empty`: strip (`padding:4px`, 36 px) with `aria-label="New tab"` above the four actions | `crates/ui/src/components/work_panel_tabs.rs:374-376` (empty tabs return before the strip is built); `crates/desktop/src/shell/work_panel.rs:606-609` | With no tabs the strip disappears, so the "New tab" (+) button is absent above the four actions. | none |
| W2 | missing element | missing | 3 | `ws-browser` and `ws-session`: unlabelled trailing icon button (3 dots) after the strip spacer; D-SH-17 names it "a More menu" | `work_panel_tabs.rs:421-437` (strip = tabs plus "New tab" only); no More control in `work_panel.rs` | No More menu at the end of the strip. | none (D-SH-17) |
| W3 | token/size | cosmetic | 1 | `ws-browser` and `ws-session`: `role="tablist" aria-label="Work panel"` strip, `height:36px` | `work_panel_tabs.rs:432` `.h(px(SIZE_CONTROL_DEFAULT))` = 28 | Strip is 28 px; artboards and D-SH-17 say 36 px (`SIZE_TOOLBAR`). | none (WorkPanelTabs.md says 28; stale, see Doc conflicts) |
| W4 | token/size | cosmetic | 2 | `ws-browser` tabs `height:32px` ("zsh, dev", "localhost:3000"); `ws-session` tab "zsh, oppm" `height:32px` | `work_panel_tabs.rs:307` `.h(px(SIZE_CONTROL_DEFAULT))` = 28 | Tab is 28 px; artboards and D-SH-17 say 32 px. | none |
| W5 | wrong structure | wrong structure | 3 | `ws-session` strip `padding:4px 4px 0; align-items:flex-end`: tabs sit on the bottom rule | `work_panel_tabs.rs:426-435` (`items_center`, no padding); tab row `:392-398` | Tabs are centred in the strip, so the active underline does not sit on the rule as in the artboard. | none |
| W6 | token/size | cosmetic | 3 | `ws-browser` and `ws-session` tabs: `border-radius:6px 6px 0 0; gap:6px` | `work_panel_tabs.rs:296-319` (no radius); inner gap `:306` `SPACE_1` = 4 px | Tab corners are square and the inner gap is 4 px (artboard: 6 px top corners, 6 px gap). | none |
| W7 | icon | cosmetic | 2 | `ws-session` tab icon and `ws-workpanel-empty` "New terminal" button: Lucide `terminal` (polyline and line) | `work_panel_tabs.rs:54` `IconName::SquareTerminal` (asset `crates/ui/assets/icons/square-terminal.svg`, boxed glyph); used by the tab at `:338` and the empty action at `:243-246` | Boxed square-terminal glyph where the artboard draws the plain terminal glyph. | none (WorkPanelTabs.md lists square-terminal as a proposal) |
| W8 | token/size | cosmetic | 2 | `ws-workpanel-empty` action buttons: `btn.s` with `height:32px` | `work_panel_tabs.rs:243-245` `ButtonSize::Default`; `crates/ui/src/components/button.rs:55` maps Default to 28 px | Action buttons are 28 px; artboard and D-SH-17 say 32 px. | none |
| W9 | token/size | cosmetic | 3 | `ws-workpanel-empty`: buttons `border-radius:8px`; column `role="group" aria-label="Open in the Work panel"` with `gap:10px` | `button.rs:411` `RADIUS_MD` (6); `work_panel_tabs.rs:240` `gap(SPACE_2)` = 8 | Button radius is 6 px (artboard and D-SH-17 radius.lg 8 px); column gap is 8 px (artboard 10 px). | none |
| W10 | state | missing | 2 | `ws-workpanel-empty`: all four action buttons enabled | `work_panel_tabs.rs:247-248` (disabled with tooltip); `work_panel.rs:107-111` passes Browser, Markdown and Mobile emulator as unavailable; reasons in `crates/desktop/src/model/work_panel.rs:17-19` | Three of four actions are disabled ("Browser tabs come later." and similar) because those kinds are not built. Clears when they ship. | none |

### Browser tab: Not built

The chrome component exists in `crates/ui/src/components/browser_chrome.rs`, but nothing in `crates/desktop` calls it. The tab kind is disabled in the empty state (`crates/desktop/src/shell/work_panel.rs:108`). Artboard: `ws-browser.dc.html`. Parts not listed.

### Work panel: Markdown or file and mobile emulator tabs: Not built

Both kinds are disabled in the empty state (`work_panel.rs:109-110`; `model/work_panel.rs:18-19`), so file and diff tabs (A7, A36) do not open. Artboards: `ws-session.dc.html`, `ws-diff.dc.html`. Parts not listed.

### Project sidebar tab strip

Built. It matches the artboards on height (36 px), padding (4 px and 8 px), gap (2 px), tab order, labels, icons, `aria-label="Project sidebar"`, tab size (28 px) and the 2 px underline.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| T1 | extra element | cosmetic | 2 | `ws-diff` and `ws-session`: Source control tab (`aria-label="Source control"`) shows no count badge | `crates/ui/src/components/project_sidebar_tabs.rs:77` `.count(changed_files, "changed")`; badge drawn at `:221-232` when count > 0; `crates/desktop/src/shell/project_sidebar.rs:30` | A changed-files count badge the artboards do not show. ProjectSidebarTabs.md calls counts a proposal. | none |

### Project sidebar: Source control

Built (`crates/desktop/src/shell/project_sidebar.rs`). Artboard: `ws-diff`, Project sidebar block.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| S1 | missing element | missing | 2 | Block top row: chip "PR #313" (accent-subtle) with Search and More icon buttons | `project_sidebar.rs:114-126` (the panel starts at `branch_line`); no PR chip anywhere in `crates` | No PR chip and no Search or More buttons. | none (D-DF-02) |
| S2 | missing element | missing | 2 | Branch line: `acme/sub-pipelines-partial` in mono 12 px, `+334` (diff add) and `-15` (diff remove) | `project_sidebar.rs:206-238` `branch_line` (name in `TYPE_BODY_STRONG`; no diff counts) | Branch line has no +/- counts. | none (D-DF-02) |
| S3 | label/icon/colour | cosmetic | 2 | Upstream line: `origin/main` in mono 12 px, arrow icon, ahead and behind counts in `status.warning.fg` with arrows, external-link icon | `project_sidebar.rs:222-230` (captions "→ upstream" and "↑n ↓n" in caption colour; no external-link icon) | Upstream and ahead/behind are plain captions, not mono with warning colour, icons and an external-link glyph. | none (D-DF-02) |
| S4 | wrong structure | wrong structure | 1 | Message box: 72 px field, `border-radius:8px`, `border.default`, `surface.raised`, `shadow.e1` | `crates/desktop/src/shell/mod.rs:462-466` `TextField::new(cx)` with no style; default height `text_field.rs:935` (`SIZE_CONTROL_DEFAULT`, 28 px); radius `text_field.rs:973` (`RADIUS_MD`); rest border `:851` `border.strong` | The message field is a default 28 px field with strong border and 6 px radius, not the 72 px soft-edged box. | A49 (code breaks it; artboard agrees) |
| S5 | copy | cosmetic | 2 | Message box placeholder "Message" | `mod.rs:465` `.placeholder("Commit message")` | Placeholder reads "Commit message"; artboard and D-DF-02 say "Message". | none |
| S6 | missing element | missing | 3 | Wand icon at the trailing edge of the message box (AI assist) | `project_sidebar.rs:241-286` (`commit_box`; no AI-assist control in `crates`) | No AI-assist control. | none (D-DF-02) |
| S7 | missing element | missing | 2 | Split "Commit" button: flex-1 main part plus a 28 px chevron segment | `project_sidebar.rs:274-284` (one button, `justify_end`; no chevron, no menu) | No split button and no Commit menu ("Ask agent to commit", "Ask agent to open a PR" per D-DF-02). Button is intrinsic width at the right. | none (D-DF-02; SplitButton has no component spec) |
| S8 | token/colour/icon | cosmetic | 2 | "Commit": secondary (`btn.s`), `border.default`, `surface.raised`, 8 px radius, leading icon | `project_sidebar.rs:278` `ButtonVariant::Primary`, no leading icon; `button.rs:411` radius 6 | Primary accent fill, 6 px radius and no icon, where A49 and the artboard use secondary style, border.default, 8 px and an icon. | A49 (code breaks it; artboard agrees) |
| S9 | extra element | wrong structure | 1 | Below the commit box: one flat list of changed files under "Commits in branch"; no group headings and no bulk buttons | `project_sidebar.rs:175-198` (groups "Conflicted", "Staged changes" and "Changes" with bulk buttons "Unstage all" and "Stage all", and "Untracked"); headings from `group()` at `:298-331` always render when rows exist | Group headings with counts and bulk actions are always shown. D-DF-02 shows the groups only while the commit box has focus. Heading copy is "Staged changes"; spec says "Staged". | none (D-DF-02 States) |
| S10 | extra element | wrong structure | 2 | File rows: file icon, name, folder, `+n -n`, status letter; no buttons | `project_sidebar.rs:410-421` `row_actions` (stage and discard `IconButton`s per row); `:423-445` `stage_button`; `:446` `discard_button`; rendered in `change_row` `:333-363`; no hover condition | Every change row carries stage and discard buttons that are always visible. | none |
| S11 | icon | cosmetic | 2 | `orchestration.py`: type badge (rect `fill="#3776ab"` with "PY" in `#ffd43b`); `data-model.md`: grey document (`fill="var(--text-tertiary)"`) | `project_sidebar.rs:336` `RowLeading::Icon(IconName::FileText)` for every row | No per-type file badge or colour; every file gets the generic document glyph. | A46 and A47 (code breaks them; artboard agrees) |
| S12 | colour | cosmetic | 2 | Status letters: `M` in `status.warning.fg`; `A` and `D` in `diff.add.fg` and `diff.remove.fg` | `project_sidebar.rs:356` (`text.secondary` for every letter) | All status letters are grey. | none (FileTree.md uses the same rule) |
| S13 | wrong structure | wrong structure | 1 | Under the "Commits in branch" overline: changed-file rows (as S10), the open diff row selected | `project_sidebar.rs:479-504` `history()` lists commits (short id, subject, author) via `commit_line` `:506-535` | The list under "Commits in branch" shows commits, not changed files. D-DF-02 describes file rows under this header, as the artboard does. The artboard header count (14) contradicts its own file rows; the file rows follow D-DF-02. | none |
| S14 | missing element | missing | 2 | "Commits in branch" header: count badge `14` and "See all" link | `project_sidebar.rs:479-486` (overline only) | No count badge and no "See all". | none (D-DF-02) |
| S15 | token/size | cosmetic | 3 | Block padding `8px` and gaps `8px` | `project_sidebar.rs:121-125` `gap(SPACE_3)` and `p(SPACE_3)` = 12 px | Panel padding and gaps are 12 px; D-DF-02 says the top block uses space.2 (8 px). | none |

Matches: the "Nothing to commit." copy (`project_sidebar.rs:163`) and the "Commit anyway" label for detached HEAD.

### Project sidebar: Files: Not built

Every tab except Source control renders `EmptyState` "Not available yet." (`crates/desktop/src/shell/project_sidebar.rs:38-40`). The file tree (`crates/ui/src/components/file_tree.rs`) has no desktop caller. Artboard: `ws-session.dc.html`. Parts not listed.

### Project sidebar: Sessions: Not built

Same placeholder (`project_sidebar.rs:39-40`). Artboard: `ws-workpanel-empty.dc.html`. Parts not listed.

### Project sidebar: Checks: Not built

Same placeholder (`project_sidebar.rs:39-40`). Artboard `ws-light.dc.html` (not compared; see Basis). Parts not listed.

### Prototypes: Not built

No `Page` variant (`crates/desktop/src/model/navigation.rs:52-68`). The Sidebar row has no page (`crates/desktop/src/model/sidebar.rs:104-108`), and no renderer exists under `crates/desktop/src/shell`. Artboard: `ws-prototypes.dc.html`. Parts not listed.

### Automations: Not built

`Page::Automations` exists (`navigation.rs:66`), but `crates/desktop/src/shell/canvas.rs:206-208` renders the generic placeholder "This page is not built yet." Artboard: `ws-automations.dc.html`. Parts not listed.

### Tasks: Not built

`Page::Tasks` (`navigation.rs:56`) reaches the same placeholder. The palette command "Open tasks" opens it (`crates/desktop/src/model/palette.rs:274`). Artboard: `ws-tasks.dc.html`. Parts not listed.

## Stale agreements

- **A27** (Composer, one Model picker): the text puts access inside the Model picker footer. `ws-tasks.dc.html` draws access as its own chip ("Ask for approval") beside the model chip, the same split that A33 and the Doubts section of `agreements.md` already record. No later row amends A27.
- **A52** (Prototypes design system sections): the text says each section shows an icon and a count. `ws-prototypes.dc.html` shows counts only where a section has items (Colors 24, Typography 12, Components 66, Assets 8). Overview, Spacing, Corner radius, Shadows and Motion have none, which matches D-AG-17 ("a count where it has items"). No later row amends A52.

Checked with no contradiction found: A6, A7, A8, A9, A12, A21, A31, A32, A43, A45, A46 to A48, A49, A51, A53. The code breaks A46, A47 and A49 (rows S8, S4, S11); the artboards agree with them.

## Doc conflicts (not agreements)

- `components/WorkPanelTabs.md` says the strip and tabs are 28 px ("decided"). D-SH-17 and both artboards say 36 px and 32 px, so the component doc is stale.
- `screens/workspaces-sessions.md` D-WS-24 gives approval rows "Open" and "Review". A45 and `ws-tasks.dc.html` show "Review" only.
- D-AG-17 describes a strip of direction thumbnails. The owner note and `ws-prototypes.dc.html` draw an infinite canvas.
- Decision 6 in the screen doc (a 2 by 2 grid for the Work panel empty state) is superseded by A43 and the artboard.
- D-AG-22 groups sessions under "Needs you", "Running" and "Recent". `ws-workpanel-empty.dc.html` shows no group headings.


## Marketplace and Settings

59 deviations: 29 missing, 13 wrong structure, 17 cosmetic. Paths: `marketplace/` = `crates/desktop/src/shell/marketplace/`, `model/` = `crates/desktop/src/model/`, other `.rs` in `crates/desktop/src/shell/` or `crates/ui/src/components/`. The item page and the sign-in dialog were compared only where an artboard shows them.

### Discover and catalogue (bt-marketplace-catalogue)

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| M1 | missing | missing | 2 | "Sources" button beside search | none | No Sources control. | screen doc 7.1 |
| M2 | structure | wrong structure | 1 | "Add extension" primary + "More ways to add" (Create with chat, From the Marketplace, From a Git URL, From a folder, From a file (.zip)) | marketplace/catalogue.rs:265-273 | One secondary button, no menu. | A63 |
| M3 | missing | missing | 2 | card footer "Not installed" | extension_card.rs (footer) | No status word. | screen doc |
| M4 | copy | cosmetic | 2 | byline "MassOrca extras · Marketplace · 1.2.0" | extension_card.rs:639-678 | Joined with commas; version split out. | none |
| M5 | icon | cosmetic | 2 | each chip has its own icon | marketplace/catalogue.rs:325 | Every chip uses Plus. | A21 |
| M6 | missing | missing | 2 | Gemini CLI chip "Installs on VPS" | extension_card.rs:307 | Never set. | none |
| M7 | copy | cosmetic | 2 | chip "Model picker entry" | model/marketplace/service_source.rs:150 | "Agent in the model picker". | none |
| M8 | state | missing | 2 | "Installed, enabled", "Update 2.1.0", Update | extension_card.rs:334-343 | No status line on an installed card with an update. | none |
| M9 | state | missing | 2 | "Needs MassOrca 1.6 or later" | extension_card.rs:363-392; model/marketplace/types.rs:60-73 | Failed, Installing and Incompatible states exist but nothing produces them. | none |

### Installed and extension manager (bt-extension-manager)

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| M10 | structure | wrong structure | 1 | Installed: manager rows (plate, name, meta, status, switch, More) | marketplace/catalogue.rs:172-181 | Reuses the Discover card grid. | A68, screen doc 7.4 |
| M11 | state | missing | 1 | row status "Enabled", "Update available 2.1.0", "Failed to start" + "View log", "Last known: enabled, 5 min ago" | extension_card.rs:394-403 | No row status words. | screen doc 7.4 |
| M12 | missing | missing | 1 | More menu: "Roll back to 0.3.0", "View log", "Remove" | extension_card.rs:420-426 | More opens the item page; no menu. | screen doc 7.4 |
| M13 | missing | missing | 2 | unreachable-host note and disabled switch | none | State not produced. | screen doc 7.4 |
| M14 | missing | missing | 2 | "Third party · 2.0.0" | sample/items.rs:62-98 only | No rule derives "Third party". | A63 |

### Item page (behind bt-permission-review)

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| M15 | structure | wrong structure | 2 | chip "Install on: This computer" | marketplace/host_pick.rs:102-118; marketplace_item_header.rs:235-244 | Host select plus "Connected" word; screen doc 7.2 agrees with code. | doc vs artboard |
| M16 | extra | wrong structure | 3 | no Copy link | marketplace/item.rs:126-128 | "Copy link" button. | none |
| M17 | missing | missing | 2 | "About" with "What it adds" chips | marketplace/item.rs | Absent. | screen doc 7.2 |
| M18 | structure | wrong structure | 2 | "Permissions" prose section | model/marketplace/item_page.rs:111-113 | Key/value row under Information. | screen doc 7.2 |

### Permission review dialog (bt-permission-review)

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| M19 | missing | missing | 1 | callout "Check. This extension can read files in your project and use the network." | marketplace/dialogs.rs:50-139 | No risk notice. | PermissionReview.md |
| M20 | missing | missing | 1 | "This extension asks for": Files, Network access, Notifications, each with reason and risk chip | marketplace/dialogs.rs:50-139 | No permission rows or risk labels. | PermissionReview.md |
| M21 | missing | missing | 2 | "What it adds" chips | marketplace/dialogs.rs | Absent. | none |
| M22 | missing | missing | 2 | "Install on" select | marketplace/dialogs.rs | Absent (spec: only with more than one host). | screen doc |
| M23 | copy | cosmetic | 1 | primary "Allow and install" | marketplace/dialogs.rs:71; model/marketplace/install_review.rs:29-30 | "Install" or "Add to MassOrca". | PermissionReview.md |
| M24 | missing | missing | 3 | subtitle with plate and "MassOrca extras · Marketplace · 1.2.0" | marketplace/dialogs.rs:70-71 | Title only. | none |

### Import and Add folder

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| M25 | extra | wrong structure | 2 | no approved design | marketplace/item.rs:246-256; marketplace/import.rs:115-158 | Account import flow with no artboard. | none |
| M26 | missing | missing | 2 | "From a file (.zip)…" | marketplace/add_folder.rs | Source absent. | A63 |
| M27 | structure | wrong structure | 2 | "From a Git URL…" menu item | marketplace/add_folder.rs:125-131 | Disabled control inside the folder dialog. | A63 |
| M28 | structure | wrong structure | 1 | main part of the split button opens a Creator mode session | marketplace/catalogue.rs:265-273 | Opens the folder install dialog. | A63, A65 |

### Settings navigation

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| M29 | missing | missing | 2 | "Extensions" row with arrow, "Opens Marketplace, Installed" | shell/settings.rs:173-187; model/settings_sections.rs:17-30 | Row absent. Code follows the 2026-11-24 note on A68; artboards predate it. | A68 note vs A69 |

Other items, order, icons and the 232 px width match A69.

### Appearance and layout (bt-settings-appearance)

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| M30 | structure | wrong structure | 2 | groups "Theme", "Accent", "Density and text", "Window" | model/settings_sections.rs | One group. | none |
| M31 | copy | cosmetic | 2 | Import theme helper and "Import theme" with upload icon | settings_sections.rs:233-235 | Shorter helper; button "Import". | none |
| M32 | copy | cosmetic | 3 | Layout preset helper | settings_sections.rs:238-240 | Different text. | none |
| M33 | copy | cosmetic | 2 | Accent helper and contrast line | settings_sections.rs:243-244; settings_planned.rs:153 | Different text. | none |
| M34 | icon | cosmetic | 3 | "Custom colour" plus swatch | settings_planned.rs:145 | Ghost text button. | none |
| M35 | copy | cosmetic | 3 | Density helper | settings_sections.rs:248-249 | Different text. | none |
| M36 | copy | cosmetic | 3 | font helpers | settings_sections.rs:261-267 | Different text. | none |
| M37 | copy | cosmetic | 3 | Text size and Session summary helpers | settings_sections.rs:253-254, 271-272 | Different text; second sentence missing. | none |
| M38 | copy | cosmetic | 3 | Status bar items helper | settings_sections.rs:276-277 | Different text. | none |

### Input and shortcuts (bt-settings-keybindings)

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| M39 | missing | missing | 1 | keybinding table (7 rows, From column, key chips) | settings_sections.rs:312-316 | Header only; no rows. | D-ST-04 |
| M40 | state | missing | 1 | "1 conflict", conflict block with Reassign and Cancel, recording hint | none | Absent. | D-ST-04 |
| M41 | copy | cosmetic | 2 | heading "Keybindings" | settings_sections.rs:313 | "Keyboard shortcuts". | none |
| M42 | copy | cosmetic | 2 | placeholder "Filter shortcuts" | settings_sections.rs:297-301 | No placeholder. | none |
| M43 | extra | wrong structure | 3 | no Tab width row | settings_sections.rs:289-296 | "Tab width" stepper. | none |
| M44 | missing | missing | 3 | footer on Ctrl / Cmd and extension shortcuts | none | Absent. | D-ST-04 |
| M45 | copy | cosmetic | 3 | Quick commands text, "4 saved commands", "Manage quick commands" | settings_sections.rs:318-320 | Different text, no count, "Manage". | none |

### Secret vault (bt-vault) and Add secret (bt-secret-request)

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| M46 | missing | missing | 2 | "Used by [Claude] in oppm, 3 sessions this week" | model/secret_vault.rs:126-139 | No usage line. | none |
| M47 | missing | missing | 2 | chip "Linked to 2 scopes" | model/secret_vault.rs | Absent. | A72 |
| M48 | missing | missing | 1 | "Unlink" on linked rows | model/secret_vault.rs | Absent. | A72 |
| M49 | structure | wrong structure | 2 | chips "Linked from Project oppm", "Read-only here"; next-run line | model/secret_vault.rs:126-139 | Read-only as detail text; no next-run line. | A72 |
| M50 | copy | cosmetic | 3 | "Team Desarrollo" | model/secret_vault.rs:161 | Shows the team id. | none |
| M51 | copy | missing | 2 | remove dialog final clause on the new request card | settings_remove_dialog.rs:116 | Clause missing. | none |
| M52 | state | missing | 2 | scope "Team Desarrollo" enabled | settings_secret_dialog.rs:424 | Team always disabled. | A71 |
| M53 | missing | missing | 2 | "Link an existing secret instead" enabled | settings_secret_dialog.rs:460-463 | Disabled. | none |
| M54 | copy | missing | 2 | value helper on the 10-second hide | settings_secret_dialog.rs:480-482 | Different helper. | SecretModal.md |
| M55 | state | missing | 2 | visible "Show" button | text_field.rs:596-601, 888-921 | Icon-only, only while focused. | none |
| M56 | extra | wrong structure | 2 | no Paste value | settings_secret_dialog.rs:482-485 | "Paste value" button. | SecretModal.md |
| M57 | structure | wrong structure | 3 | scopes "Project oppm", "Team Desarrollo", "Global" | settings_secret_dialog.rs:152, 423 | "Project" plus a separate select. | none |

### General (no artboard; screen doc)

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| M58 | state | missing | 2 | D-ST-01 and A95: no-project folder, changeable | settings_sections.rs:151-155 | Shows the value but reads "Not available yet". | A95 |
| M59 | copy | cosmetic | 3 | "Workspace defaults (root folder for new projects)" | settings_sections.rs:156 | "Workspace folder". | none |

### Stale agreements and doc conflicts (this area)

- A68 note (2026-11-24) removes the Settings "Extensions" row; artboards (2026-11-20), D-ST-09 and A69 still show it. Owner to confirm, then refresh one side.
- Screen doc 7.2 (2026-11-22) puts description and Permissions under Information; bt-permission-review (2026-11-20) shows About and a Permissions section.
- Screen doc L353 cites A12 for "the user grants permissions here and nowhere else"; A12 is the borderless button rule.


## Section 6: Bot Teams pages, desktop conformance

Scope: team home, team Sidebar, Conference room, Board, Routines and routine form, Calendar, team Files and Memory, bots and access, new project page.

## Method and limits

- Code: read-only checkout of origin/main at 8bf75824, ``. Paths are repo-relative to that checkout. The worktree had no CodeGraph index, so `gentle-ai codegraph init` created `.codegraph/` there. Git status stays clean, and no tracked file changed.
- The comparison is markup-based. The artboards were read as their HTML (text, classes, aria-labels, `use` symbol ids, inline styles and CSS rules). They were not rendered and no screenshots were taken. Animation (A57), reduced motion and hover states are not checked.
- Precedence: artboard over component doc over screen note, as the artboards README states. Where a component doc names a different glyph or size than an artboard, the artboard is applied and the conflict is listed under Notes.
- Surfaces with no desktop artboard (team home, Files, Memory, bots and access, project page, new project) are checked against `screens/teams-extensions-settings.md` sections 4.1 to 4.9 and the cited agreements. Their Artboard column says "none". Those rows are not artboard deviations.
- `an-botteams.dc.html` (Android) was not used. No agreement says desktop follows it: A58 is Android-only, and A61's desktop status is open in Doubts.
- Columns: Severity is `missing` (approved element or state absent), `wrong structure` or `cosmetic`. Vis is 1 (one state or sub-control), 2 (visible while the page is open) or 3 (on every visit, or the whole page is missing). Artboard is the file without `.dc.html`.

### Team home (Bot teams page)

Reference: no desktop artboard. Screen 4.5 (D-AG-13, D-AG-33), the New team dialog, A57, A59, G-1.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| H1 | icon | missing | 2 | none; bt-board `tpill` svg (role=img "Desarrollo team") shows the character | crates/desktop/src/shell/teams/home.rs:153; crates/ui/src/components/avatar.rs:175-186 | Team card mark is a Users glyph in the accent colour. The team character (triangle with two eyes) is absent. | A57; G-1 (Avatar.md:22 says "team icon"; A57 applied) |
| H2 | missing element | missing | 1 | none (4.5 card: "More (Archive, Delete)") | crates/desktop/src/shell/teams/card_menu.rs:81-90 | More menu has no Archive row. | 4.5 |
| H3 | extra element | wrong structure | 1 | none | card_menu.rs:83-84 | More menu adds "Edit team…" and a one-step access row, neither in 4.5. | 4.5; decision 0072 for the access row |
| H4 | extra element | wrong structure | 2 | none (4.5 card: name, lead, bot count, host chip, "Workspaces in this team: off") | home.rs:168 (calls access.rs:15-48) | Card adds a team-access line and a one-step change button after the Workspaces line. See Bots and access A1. | none (decision 0072, amendment 2026-11-23) |
| H5 | label or copy | cosmetic | 1 | none (4.5 host banner: "Can't reach VPS. Nothing can be created there until it is back.") | home.rs:39-43 | Banner reads "Bot Teams is not connected. Nothing can be created until it is back." | 4.5 |
| H6 | extra element | wrong structure | 2 | none (4.5 New team dialog lists its fields and helper only) | crates/desktop/src/shell/teams/dialogs.rs:1192-1199 (model/teams/access.rs:27-30) | New team dialog adds "Full access: bots run without asking for permission. Change it later on the team card." | none (decision 0072) |

Conforms: heading "Bot teams", New team button, "No teams yet" empty state, card Open (to Conference room) and Add bot, "Runs on VPS" chip, "Workspaces in this team: off", New team dialog title, fields, Switch helper text, Create team, 440 px width, Workspaces off by default (dialogs.rs:257), duplicate-name error text.

### Team Sidebar

Reference: artboards bt-board, bt-conference-room, bt-routines-empty and bt-routine-form (Sidebar in Bot Teams mode); screen 4.1; A24, A25, A55, A57.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| S1 | icon | missing | 3 | bt-conference-room: team row `svg role="img" aria-label="Desarrollo team"` (same in bt-board, bt-routine-form, bt-routines-empty) | crates/desktop/src/shell/teams/sidebar.rs:112-116 (`IconName::Users`) | Team row shows a generic users glyph. The approved team character (16 px, triangle with eyes) is absent. SidebarRow.md:28 also asks for it. | A57; G-1 |
| S2 | icon | cosmetic | 3 | bt-conference-room: "Conference room" row `#i-users` (same in bt-board, bt-routine-form) | sidebar.rs:154-157 (`MessagesSquare`) | Conference room row uses messages-square; the artboard uses the users glyph. Doc conflict, see Notes. | none (SidebarRow.md:29 and SidebarTree.md:41 name messages-square) |
| S3 | state | missing | 2 | bt-conference-room: "Conference room" hint `span.m` "2 running" (bt-board and bt-routine-form too) | crates/desktop/src/model/teams/sidebar.rs:98-101 (hint hard-coded "Nothing open") | The hint never shows the running count. Only "Nothing open" is produced. | 4.1 (hint "2 running") |
| S4 | icon | cosmetic | 3 | bt-routines-empty: "Routines" row `#i-repeat` (same in bt-routine-form and bt-conference-room) | sidebar.rs:194-198 (`CalendarClock`) | Routines row uses calendar-clock; the artboard uses the repeat glyph. `repeat.svg` is not in crates/ui/assets/icons. SidebarRow.md:29 also names calendar-clock. | none |
| S5 | extra element | wrong structure | 2 | bt-board: "Projects" lists Files, Memory and Routines only | model/teams/sidebar.rs:146-154 (`rows.extend(team.projects…)`); sample.rs:201-203 ("Desarrollo onboarding"); sidebar.rs:188-198 (FolderTree) | Every Bot Teams project of the team is listed under PROJECTS after Routines. 4.1 names three rows only. | A55; 4.1 |
| S6 | missing element | missing | 2 | none (4.1 States: empty sidebar has a New team button) | sidebar.rs:35-37 (note only) | With no teams, the sidebar shows "No teams yet" and no New team button. | 4.1 States |
| S7 | state | missing | 1 | bt-board: "Projects" header `#i-filter` and `#i-plus`, both live | sidebar.rs:76-79 with teams/mod.rs:74-82 (`waiting_icon`, disabled, "Not available yet") | Filters icon button is disabled. | 4.1 |
| S8 | token/size | cosmetic | 2 | bt-board CSS `.sr.ind{padding-left:30px}`; 4.1 "indented 30 px" | sidebar.rs:164-165 (`level(1)`); crates/ui/src/components/list_row.rs:35 (TREE_INDENT 12) and :596 | Child rows start about 20 px in (8 px padding plus a 12 px step), not 30 px. The doc (SidebarRow.md:29) gives a 12 px step, so the artboard and 4.1 are applied. | 4.1; see Notes |

Conforms: search field above "All teams"; "All teams" row with the grid glyph (`LayoutGrid`, matches the artboard); team row order; Board row with the kanban glyph (A25, sidebar.rs:155); Memory row with the brain glyph (A24, sidebar.rs:196); hints "5 tasks" and "No tasks"; unreachable team row "Last known state, N min ago"; PROJECTS overline with Filters and New project.

### Conference room

Reference: bt-conference-room.dc.html (artboard 1); screen 4.2; A33, A34, A57, A59, A73.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| C1 | missing element | missing | 3 | bt-conference-room: `main.cv` (header, thread, composer) | crates/desktop/src/shell/canvas.rs:181-184 | The whole Conference room is a placeholder: "The Conference room needs live agents and sessions, which are not built yet." | A55; A56; A73; 4.2 |
| C2 | missing element | missing | 3 | bt-conference-room: `header.chead` with `div.bots` (Mara lead, Bruno, Iris as characters) and "3 bots" | canvas.rs:181-184 | Header row (room name, team pill, bot characters, bot count) is not drawn. | A57; A59; G-2; 4.2 |
| C3 | missing element | missing | 3 | bt-conference-room: `div.thread` user bubble "We need magic link sign-in…" and bot message with 28 px character, Lead chip, status, time | canvas.rs:181-184 | Thread and message rows are not drawn. | A57; 4.2 |
| C4 | missing element | missing | 2 | bt-conference-room: `div.tcard` "Task added to the Board" with ghost "Open on Board" (`btn sm g`) | canvas.rs:181-184 | Task-added card and its Board action are absent. | A25; 4.2 |
| C5 | missing element | missing | 2 | bt-conference-room: `div.tcard` "Routine created" with "View routine" | canvas.rs:181-184 | Routine-created card is absent. | A56; 4.2 |
| C6 | missing element | missing | 3 | bt-conference-room: `div.deleg` "Delegation · 2 child bots" with lead row (Owns the task) and two child rows | canvas.rs:181-184; no desktop call site for the DelegationTree Bot Teams variant | Delegation block is not drawn. | A73; X-DL-01 (4.2) |
| C7 | missing element | missing | 3 | bt-conference-room: `div.comp` (placeholder, "Workspace write" chip, lead selector `title="Lead bot: Mara"`, Dictation, Send) | canvas.rs:181-184 | Session composer body with the lead-bot selector in place of the model picker is absent. | A33; A34; 4.2 |
| C8 | icon | missing | 2 | bt-conference-room: `div.deleg` child rows show a team character plus the agent icon (Codex, Gemini) | crates/ui/src/components/delegation_tree.rs:641 (AgentIcon for every variant); :76 (comment: stand-in for characters) | Bot Teams variant draws only the agent mark, with no bot team character. No desktop page renders this variant (ui gallery only, gallery/session.rs:394). | A57; DelegationTree.md:16 (28 px character) |
| C9 | missing element | missing | 1 | none (4.2 States: empty room line "Ask a bot, or describe what the team should do.") | canvas.rs:181-184 | Empty-room line is absent. | 4.2 States |

Conforms: the title bar reads "Desarrollo · Conference room" (shell/title_bar_tests.rs:35).

### Board

Reference: bt-board.dc.html (artboard 2, light); screen 4.3; BoardColumn.md; A25, A59.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| B1 | state | missing | 3 | bt-board: `button.tpill` aria-label "Team: Desarrollo. Switch team"; pill radius 14 px (CSS in bt-conference-room) | crates/desktop/src/shell/teams/mod.rs:36-58 (`team_pill` is a plain div); board.rs:71 | Team pill is not a control: no button role, no switch-team action, no hover fill, no pill radius. The same pill is used on Board, Memory, Files and the project page. | A59; 4.3 (borderless, hover fill, chevron switches team) |
| B2 | extra element | wrong structure | 2 | bt-board: `div.head` holds title, pill, note and Filter only | board.rs:72-80 (calls access.rs:15-48) | Board header adds a team-access line and its change button between the pill and the note. See Bots and access A1. | none (decision 0072) |
| B3 | state | missing | 2 | bt-board: `span.btn g` "Filter" with `#i-filter`, enabled | board.rs:88-94 (`.disabled(true)`, "Not available yet") | Filter button is always disabled. | 4.3 |
| B4 | token/size | cosmetic | 2 | bt-board CSS `.bcard{padding:10px; gap:6px}` | crates/ui/src/components/board_column.rs:147-148 (`gap` SPACE_HALF 2 px, `p` SPACE_2 8 px) | Card padding is 8 px (artboard and 4.3: 10 px); card gap is 2 px (artboard: 6 px). | 4.3 |
| B5 | token/size | cosmetic | 2 | bt-board CSS `.bcol{padding:8px}` | board_column.rs:387 (`p(SPACE_1)` 4 px) | Column padding is 4 px, not 8 px. Card width is therefore 272 px; BoardColumn.md says column width less space.2 gutters (264 px if each side has 8 px). | 4.3; BoardColumn.md (Anatomy) |
| B6 | state | missing | 2 | bt-board: `div.bcard.lost`; CSS `.bcard.lost{border-style:dashed; border-color:border-strong}` | board_column.rs:150-151 (solid `border.default` for every card) | Unreachable card keeps the solid resting border; the dashed border is absent. | 4.3 ("dashed border"); BoardColumn.md (Disconnected host) |
| B7 | label or copy | missing | 1 | bt-board: lost card meta "Bruno" and "· runs on VPS" | sample.rs:112-117 (no note); board.rs:157-181 (no host line) | Unreachable card omits "runs on VPS", which 4.3 names. | 4.3 |
| B8 | structure | wrong structure | 2 | bt-board: `div.bmeta` puts status, bot and time in one wrapping row (gap 6 px) | board_column.rs:173-213 | Status word, bot name and time stack on separate lines. | 4.3 (one meta line) |
| B9 | structure | cosmetic | 1 | bt-board: "Unassigned" as `span.chip` (bordered) | board.rs:166; board_column.rs:189-191 (plain meta text) | "Unassigned" is plain text, not the chip. | none |
| B10 | token/size | cosmetic | 1 | bt-board CSS `.bt{font:500 13px/18px}` | board_column.rs:168 (`TYPE_BODY_STRONG`, 13/20, 600) | Card title weight and line height differ from the artboard. 4.3 says body; BoardColumn.md says body-strong; the artboard is applied. | 4.3; BoardColumn.md |
| B11 | label or copy | cosmetic | 1 | bt-board Running card: "2 bots delegated", "Mara, lead", "Started 09:42 · Pull request not opened yet" | sample.rs:147-152 ("Two bots delegated"; bot "Mara"; no started line) | Running card copy differs from the artboard. 4.3 uses "two bots delegated", so the artboard is applied. | 4.3 |
| B12 | label or copy | cosmetic | 1 | bt-board Open card: time "08:50" | sample.rs:182 (time None) | Open card shows no time. Sample data only; 4.3 names no time for this card. | 4.3 |

Conforms: header 56 px with bottom rule; columns start 16 px below the header (board.rs:352 test); 280 px columns; titles "Open", "Running", "Waiting for you", "Done"; counts; "No tasks" in empty columns; note "Tasks come from the Conference room"; Filter glyph; status words; team pill content (character, name, chevron).

### Routines and routine form

Reference: bt-routines-empty.dc.html (artboard 3), bt-routine-form.dc.html (artboard 4); screens 4.6 and 4.7; EmptyState.md; A12, A17, A24, A56, A59.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| R1 | missing element | missing | 3 | bt-routines-empty: `header.head` with h1 "Routines", `button.tpill` (character, "Desarrollo", chevron), "New routine" | crates/desktop/src/shell/teams/routines.rs:30-40 | Team pill is missing from the Routines header. | A59; 4.6 |
| R2 | icon | cosmetic | 2 | bt-routines-empty: header `span.btn` (bordered) and empty-state `span.btn p`, both with `#i-plus` | routines.rs:84-94 (Primary, no leading icon, used in header and empty state) | Both New routine buttons lack the plus glyph. The header button is filled primary; the artboard draws it bordered. 4.6 cites A12 for a borderless toolbar button, but A12 covers rows, so the artboard is applied. | A12 (cited by 4.6) |
| R3 | icon | missing | 2 | bt-routines-empty: `div.empty > span.ib` with `#i-repeat` (Routines) and `#i-link` (Web addresses) | routines.rs:41-48 and 144-151 (no `.icon()`; crates/ui/src/components/empty_state.rs:138-140 offers it) | Empty states have no plate icon. EmptyState.md marks it optional; the artboard draws it. | EmptyState.md:12 |
| R4 | missing element | missing | 2 | bt-routines-empty: `span.btn` "New web address" with `#i-plus` | routines.rs:144-151 (EmptyState without action) | Web addresses empty state has no "New web address" button. | 4.6; EmptyState.md (one action) |
| R5 | token/size | cosmetic | 1 | bt-routines-empty CSS `.intro{font-size:13px; color:text-secondary}` | routines.rs:66-69 (note, TYPE_BODY_SM 12 px, text.tertiary; note fn at :197-200) | Intro line is 12 px tertiary. 4.6 and the artboard say body (13 px) in text.secondary. | 4.6 |
| R6 | token/size | cosmetic | 2 | bt-routines-empty CSS `.h2{14/20 600}` and `.sec{gap:12px}` | routines.rs:70-73 and 182-187 (section = TYPE_CAPTION 11 px secondary); :57-61 (page gap 24 px) | Section headings are 11 px caption instead of 14/20 600, and the heading-to-content gap is 24 px instead of 12 px. | none |
| R7 | missing element | missing | 2 | none for list state; 4.6 row: More menu (History, Edit, Pause, Delete), next run, enable Switch | routines.rs:97-141 | Routine card has no More menu and no next-run line. Decision 4 adds Run now and Pause. | 4.6; teams-extensions-settings.md section 9, decision 4 |
| R8 | missing element | missing | 1 | none (4.6 web address row: Copy address, Regenerate, Delete) | routines.rs:157-177 (Delete only) | Address card has only Delete. | 4.6; decision 7 |
| R9 | label or copy | missing | 1 | none (4.6 host unreachable: "Can't reach VPS. Routines can't change until it is back.") | routines.rs:84-94 (disabled button, no sentence) | Disabled New routine gives no reason. | 4.6 States |
| R10 | missing element | missing | 3 | bt-routine-form: `main.cv` (whole form) | canvas.rs:196-198 (`Canvas::Team(page)` → "This page is not built yet."); routines.rs:1-3 says the form is not built | The routine form is not built. "New routine" opens a placeholder. The title bar "Desarrollo · New routine" already matches the artboard. | 4.7; A56 |
| R11 | missing element | missing | 2 | bt-routine-form: `div.fld` "Name" (input "Check staging sign-up errors") and "What should the bot do?" text area with help | canvas.rs:196-198 | Name field (required, 64 characters) and instruction area (3 lines, 2,000 characters, help text) are absent. | 4.7 |
| R12 | missing element | missing | 2 | bt-routine-form: "When" `span.seg` (Once, Daily, Weekdays, Weekly, Monthly), `div.days` (M T W T F S S), time "09:00" with clock, "Time zone: this computer." | canvas.rs:196-198 | Schedule group (SegmentedControl, day chips, time field) is absent. | 4.7 |
| R13 | missing element | missing | 2 | bt-routine-form: "Bot" select (Bruno with Codex icon) and "Runs on" select (VPS with server glyph), with help text | canvas.rs:196-198 | Bot and Runs-on selects are absent. | 4.7; A17 |
| R14 | missing element | missing | 2 | bt-routine-form: `div.prev role=status` "Runs every Monday at 09:00 on VPS" and next-run line | canvas.rs:196-198 | Live preview card is absent. | 4.7 |
| R15 | missing element | missing | 2 | bt-routine-form: header with `span.ib` back icon, h1 "New routine", helper "Desarrollo · a simple form. Routines are plain-language automations." | canvas.rs:196-198 | Page header with Back and helper line is absent. | 4.7 |
| R16 | missing element | missing | 2 | bt-routine-form: `footer.foot` with Cancel (`btn g`) and Create routine (`btn p`) | canvas.rs:196-198 | Form footer is absent. | 4.7 |

Conforms: routine heading and intro wording; both empty-state titles and explanations (exact copy); web address empty copy; "Runs on VPS, the host Desarrollo uses." footer; section order; title bar "Desarrollo · Routines" and "Desarrollo · New routine"; Sidebar Routines row selected on the page.

### Calendar

Reference: bt-calendar-week.dc.html (artboard 5); screen 5; A8, A56. CalendarViews.md was not compared because the page is not built.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| K1 | missing element | missing | 3 | bt-calendar-week: `main.cv` (whole page) | crates/desktop/src/shell/canvas.rs:206-208 (`Canvas::Page(page)` → "This page is not built yet.") | Calendar page is a placeholder. The rail entry (model/rail.rs:19), the title bar with Back icon and "Calendar" (model/title_bar.rs:121) and the hidden Sidebar are in place. | A8; A56; 5 |
| K2 | missing element | missing | 2 | bt-calendar-week: `div.top` with previous and next buttons, Today, "9 to 15 Nov 2026", "Show done" switch, Day / Week / Month segments | canvas.rs:206-208 | Header controls are absent. | 5 (header row) |
| K3 | missing element | missing | 2 | bt-calendar-week: `div.filters` with "Team: Desarrollo" (team character) and "Automations" (workflow glyph) | canvas.rs:206-208 | Filter chips are absent. | 5 |
| K4 | missing element | missing | 2 | bt-calendar-week: `div.grid` with 08:00 to 18:00 gutter, Mon 9 to Sun 15, today outline | canvas.rs:206-208 | Week grid is absent. | 5 |
| K5 | missing element | missing | 2 | bt-calendar-week: `div.ent` and `div.ent.routine` (word, glyph, time, title; selected entry has accent bar) | canvas.rs:206-208 | Routine and automation entries are absent. | A56; 5 |
| K6 | missing element | missing | 1 | bt-calendar-week: `div.foot` "Routines come from the Routines page or a bot in the Conference room. Automations come from Workspaces." | canvas.rs:206-208 | Footer note is absent. | 5 |

### Team Files and Memory

Reference: no desktop artboard. Screens 4.8 and 4.9; A46, A48.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| M1 | structure | wrong structure | 2 | none (4.8: ListRow with a More menu holding Delete) | crates/desktop/src/shell/teams/memory.rs:108-123 | Memory row carries a direct trash icon button, not a More menu. | 4.8 |
| M2 | label or copy | cosmetic | 2 | none (4.8 empty state: the sentence only) | memory.rs:46-47 | Empty state adds the title "Nothing remembered here yet". | 4.8 |
| M3 | state | missing | 1 | none (4.8 host unreachable: last known entries with their age, Delete disabled) | memory.rs (no reachability handling) | No unreachable state for memory entries. | 4.8 |
| F1 | icon | cosmetic | 2 | none; A46 applies to every file name | crates/desktop/src/shell/teams/files.rs:89-92 (`FileText`) | Team file rows use a generic document glyph, not the file-type badge (TS, RS, MD) with the grey generic file. | A46; A48 |
| F2 | missing element | missing | 2 | none (4.9: explorer "tree and search") | files.rs:38-55 | Team file tree has no search field. | 4.9 (D-ED-11) |
| F3 | missing element | missing | 2 | none (4.9: editor in a Work panel tab, versions list with Restore in the file header menu) | files.rs:56-64 (placeholder "Opening a file needs the editor, which is not built yet.") | Editor pane is a placeholder; there is no Work panel tab and no versions list. | 4.9 |
| F4 | state | missing | 1 | none (4.9 States: host unreachable; external change banner D-ED-06) | files.rs (no such states) | Unreachable and external-change states are absent. | 4.9 |

Conforms: "Team folder" and "Linked project files" headings; empty-folder copy "No files yet. Files a bot creates for the team show here."; 288 px tree (files.rs:12); link glyph and project name on linked files; Memory tabs "Bot" and "Team"; footer "Only the team's bots and you can read this memory."; team pill on both pages.

### Bots and access

Reference: no desktop artboard. Screen 4.5 (Add-bot dialog, D-AG-33); decision 0072 at docs/decisions/0072-bot-teams-core.md:475 (Amendment 2026-11-23: team access).

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| A1 | extra element | wrong structure | 2 | none (no design row or artboard for team access) | crates/desktop/src/shell/teams/access.rs:15-48; model/teams/access.rs:10-25 (used at home.rs:168 and board.rs:72-80) | A team-access line and a one-step change ("Ask for permission" or "Run without asking") appear on every team card and in the Board header. | none (decision 0072 amendment only) |
| A2 | missing element | missing | 2 | none (4.5 Add-bot: Skills, chips with a picker, none by default) | crates/desktop/src/shell/teams/dialogs.rs:1224-1237 (add_bot_view: Name, Role, Agent, Host, unavailable links) | Skills field is absent from Add bot. | 4.5 (D-AG-33) |
| A3 | missing element | missing | 2 | none (4.5 Add-bot: "Advanced options" disclosure, collapsed, with Model and Effort selects) | dialogs.rs:1224-1237 | Advanced options disclosure is absent. | 4.5 (D-AG-33; X-AG-02) |
| A4 | extra element | wrong structure | 1 | none (4.5 has no Edit team form) | crates/desktop/src/shell/teams/team_bots.rs:13-48; dialogs.rs:1176-1183 (Edit team: Shape select, Bots section with Edit bot and Remove bot, Add a bot) | Edit team form and its Bots section are not in the approved design. | none |
| A5 | extra element | wrong structure | 1 | none (4.5 Agent select: Claude, Codex, Gemini, installed agents) | dialogs.rs:1269-1282 and 1233-1237 | Add bot shows "Open {agent} in Marketplace" for an agent kind that is not installed. The design has no such state. | 4.5 |

Conforms: Add-bot dialog title and action "Add bot", 560 px width (`DialogWidth::Large`, dialogs.rs:1247), Name and Role fields (placeholder "What this bot is responsible for"), Agent and Host selects.

### New project page

Reference: screen 4.4 (project page, D-WS-31; no artboard, specified in text by decision 9); screen 4.1 (New project icon). The New project dialog (new_project.rs) has no design at all.

| # | Kind | Severity | Vis | Artboard | Code | Deviation | Agreement |
|---|---|---|---|---|---|---|---|
| P1 | missing element | missing | 2 | none (4.4: "Page with Back") | crates/desktop/src/shell/teams/project.rs:101-114 (no back control); crates/desktop/src/model/title_bar.rs:36 and :82 (Bot Teams pages get BackControl::None) | Project page has no Back control. | 4.4 |
| P2 | structure | wrong structure | 2 | none (4.4: brief text area, 6 lines, edited in place, Save) | project.rs:44-59 (read-only text and "Edit brief" button); dialogs.rs:1284-1301 (edit_brief_view) | Brief is read as text and edited in a dialog. The file header (project.rs:2) says this is deliberate. | 4.4 |
| P3 | missing element | missing | 1 | none (4.4: "Saved" status when the store confirms) | project.rs (none) | No "Saved" status. | 4.4 |
| P4 | structure | wrong structure | 2 | none (4.4: "a Select with the projects on the same host") | project.rs:131-157 (`SegmentedControl` with "None" and project names) | Link control is a segmented control, not a Select. | 4.4 |
| P5 | state | cosmetic | 1 | none (4.4: Delete project as ghost button with danger text) | project.rs:60-64 (`ButtonVariant::Danger`) | Delete project is a filled danger button. | 4.4 |
| P6 | label or copy | missing | 2 | none (4.4 delete dialog: loss list "its routines stop, its tasks are removed from the Board") | crates/desktop/src/shell/teams/dialogs.rs:1327 ("Its tasks are removed from the Board. This cannot be undone.") | Delete project dialog omits "its routines stop". | 4.4 |

Not counted: the New project dialog (new_project.rs:165-203) shows helper copy "A project holds the why. Its tasks hold the how.", a Goal field and a Code source select (Workspaces only). No artboard or screen row covers it, so there is nothing to compare.

Conforms: project name heading with team pill; sections "Brief", "Board" and "Link to a Workspaces project"; Board columns; link helper text and disabled sentence exact; Delete dialog title "Delete project {name}?" with destructive variant; New project icon in the Sidebar header (4.1).

## Stale agreements

- **A61** (Conference room overflow; scope General, applies-to "Android; see doubts for desktop"): bt-conference-room.dc.html contradicts it for desktop. The room header has no overflow button, and the Sidebar lists Board, Routines, Files and Memory as rows. No later row amends A61, and Doubts leaves desktop open. The owner must say whether desktop follows it. A60 is history (superseded by A61) and is not listed.
- **A27** (the Model picker holds access in its footer): bt-conference-room.dc.html composer and the approved composer body in A33 show access as its own "Workspace write" chip, outside the picker. No later row amends A27, and Doubts (A27 and A33) already flags it.

## Notes

- Artboard-internal differences: bt-conference-room shows "Board 4 tasks" where bt-board and bt-routine-form show "5 tasks" (code follows bt-board). The Conference room hint reads "2 running" in three artboards and "Nothing open" in bt-routines-empty.
- Doc-versus-artboard glyph and size conflicts (artboard applied, owner to align the docs): SidebarRow.md:29 and SidebarTree.md:41 name messages-square for Conference room (artboard: users, S2); SidebarRow.md:29 names calendar-clock for Routines (artboard: repeat, S4); SidebarRow.md:29 gives a 12 px child step where 4.1 and bt-board give 30 px (S8); Avatar.md:22 says "team icon" for team marks where A57 and G-1 say team character (H1). The code follows the docs in all four.
- The DelegationTree Bot Teams variant is not used by any desktop page (only the ui gallery, gallery/session.rs:394).
- Not checkable from markup: A57 motion and reduced motion, and hover and focus states beyond the CSS quoted above.

## Counts

75 deviation rows: 47 missing, 12 wrong structure, 16 cosmetic. Surfaces with a whole page missing (Conference room 9 rows, routine form 7, Calendar 6) account for 22 of the missing rows.
