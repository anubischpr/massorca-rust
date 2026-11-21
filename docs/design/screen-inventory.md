# MassOrca screen and surface inventory

Design input for the MassOrca rebuild. Written 2026-11-12, English. Edit this file as the design moves; keep IDs stable.

Region and concept names follow [glossary.md](glossary.md), the owner-approved list. Use those names; do not coin new ones for the same regions.

**Purpose.** One list of every screen and surface the design pass must decide on, with a proposal for each: keep, redesign, merge, drop, or new. It starts from the current app's screens and adds three new areas from `docs/PLAN.md`: agent account migration, session import, and customisation.

**Sources.** `desktop-screens.md` and `mobile-screens.md` (explorer notes on the current app, in the side-session-notes folder), `docs/PLAN.md` (product decisions), `docs/inventory/` (empty when this was written, so there are no first-version, later or dropped marks to align with). `current-style.md` was skimmed only. Nothing was re-checked in the reference source. Where a note said "not read", the row names the screen from its file name and says so in Rationale when that matters. The owner requirement notes of 2026-11-12 (`owner-layout-requirement.md`, `owner-model-picker-requirement.md`, `owner-activity-rail-requirement.md`, with their reference images) are authoritative for the activity rail, the model picker, the frame layout and Bot Teams. The Bot Teams note (`owner-botteams-requirement.md`, 2026-11-12, with its reference image) is authoritative for Bot Teams, routines, the calendar, orchestration and the status bar.

**pv96 delta (2026-11-12).** Rows whose Rationale says 'Source: pv96 (unmerged)' were checked against the unmerged commit d3c6311a531 and changed. The full delta is in the side-session notes file pv96-delta.md. Where pv96 differs from an owner decision, the difference is listed in open questions 14 to 16. Those three were decided on the owner's behalf on 2026-11-12 (see pending-owner-review.md, items 1, 2 and 6). The other differences keep the owner decision.

**Legend (Proposal).**
- **Keep**: carry over; behaviour kept, design still reviewed.
- **Redesign**: same job, new structure or design.
- **Merge (into ID)**: no screen of its own; its content lives in the named screen or pattern.
- **Drop**: removed from the product. Reason in Rationale. "Extension candidate" means it may return as an extension, not as core.
- **Extension**: leaves the core and ships as an extension through the extension slots (D-SL-*). Owner-confirmed for D-WS-08, D-WS-26 and D-ED-10 (open question 4, owner accepted default 2026-11-12).
- **New**: no equivalent in the current app.

**IDs.** `D-` desktop screen (GPUI on Windows, macOS, Linux). `M-` Android screen (React Native). `X-` one shared concept designed once for both platforms. `D-SL-` extension slots, `D-EX-` extensions, `D-CU-` customisation preferences. New items take the next free number; numbers are never reused.

**Platform.** Desktop, Android or Both. Android rows sit under the topic they belong to. Section 16 holds Android-only structure. iOS is out of scope.

**Consolidation.** About 20 small confirm dialogs become one pattern (X-DLG-01). The 38 settings panes regroup into 10 sections plus project settings (section 14, with the full mapping).

**Major states** are the states the notes showed. A dash means none was noted.

---

## 1. App shell and navigation

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-SH-01 | Main window frame | Desktop | Lays out the Frame (Activity rail, Sidebar and Title bar) and the Canvas. The Canvas holds Conversation, Session summary, Work panel and Project sidebar, left to right; the Status bar sits at the bottom. | Workspace view; landing (no active workspace); region error | Redesign | Keep the regions; each region gets extension slots (D-SL-*). Region order (owner, 2026-11-12): Activity rail (D-SH-09), Sidebar (D-WS-01), Conversation (D-AG-03), Session summary (D-AG-27), Work panel (D-SH-17), Project sidebar (D-SH-04); Status bar (D-SH-05) at the bottom. Owner frame (2026-11-12): the activity rail, the Sidebar and the Title bar are one frame surface, filled with `surface.frame` #1b1b1c (dark), with no seam between them. The Canvas (`surface.canvas` #151517, dark) sits inset with a rounded top-left corner, one radius token. Light: the frame is a slightly darker, tinted neutral than the canvas, and the canvas is the lightest surface, with the same corner (proposed values in app-flow.md, A16). Source: pv96 (unmerged). pv96 removed the title bar, which the owner frame keeps (see D-SH-03; decided on the owner's behalf, pending-owner-review.md item 2). Its tabless main area and its side panel (Work panel, components/side-panel) are kept as built. The frame token is not applied. |
| D-SH-02 | Top-level view switch | Desktop | Switches between 13 fixed views (terminal, settings, tasks, activity, automations, calendar, space, skills, artifacts, prototypes, bot-teams, mobile, create-chat). | One active view | Redesign | Keep a few core views. Others become extension views (D-SL-04) or merge into their topic. Calendar (D-AG-23) is new and opens from the rail (D-SH-12) as a full page. Bot-teams is a mode on the rail (D-SH-10), not a view. Source: pv96 (unmerged). pv96: calendar and marketplace are rail pages (ui-slice-view-actions.ts); tasks is a page; bot-teams is a mode. |
| D-SH-03 | Title bar | Desktop | Sidebar toggle, navigation, session title, region toggles (Work panel, Project sidebar), window controls. Sits to the right of the activity rail (D-SH-09). | Normal; floating when sidebar collapsed; hidden on workspace view | Keep | Window controls: right on Windows and Linux, traffic lights at the left on macOS. Titlebar tabs stay dropped; terminal tabs live in the Work panel (D-TM-01). Part of the frame surface (D-SH-01), as a band across the top. The sidebar toggle hides only the workspace sidebar; the activity rail (D-SH-09) stays visible (owner rule, 2026-11-12). Source: pv96 (unmerged). pv96 removed the title bar (odd desktop-simplification S12): window controls moved to the session bar. Decided on the owner's behalf, 2026-11-12 (see pending-owner-review.md, item 2): the title bar stays in the frame, with the contents pv96 moved into its session bar. Open question 14 is answered. |
| D-SH-04 | Project sidebar shell | Desktop | Tabbed region at the right edge of the Canvas, with the four tabs the owner named (2026-11-12): File explorer (D-ED-11, with search and its Names or Content toggle, D-CP-03), Sessions list (the old 'vault' tab, agent session history), Source control (D-DF-02) and Checks (D-DF-04); plus extension tabs. | Open; closed; hidden on full pages | Redesign | Tabs become slots (D-SL-01). Owner rule (2026-11-12): the old right-sidebar 'vault' tab, titled 'Agents' and listing agent session history, is merged into the Sessions list here. It is not the secret vault (D-VT-01), and it keeps no row of its own (open question 9, answered). Ports and workspaces tabs are not in the owner's list and were not read, so they have no place here (see Not covered). Source: pv96 (unmerged). pv96 right-sidebar tabs (store/right-sidebar-route.ts): explorer, vault (agent session list), workspaces, pr-checks, source-control, checks, ports (SSH only), plugin tabs. Differs from the owner list above. The owner's list is kept; the extra tabs are an open item in app-flow.md. |
| D-SH-05 | Status bar | Desktop | Agent status, memory use, terminal count, ports, and hosts with a connection dot. Visibility menu. | Shown; hidden | Redesign | Owner reference (2026-11-12): these five items make up the status bar, a strip at the bottom of the window under the Canvas. Items become status slots (D-SL-02). Usage moved to the rail usage rings (D-SH-15); the full view is D-AC-06. The provider switcher and workspace space are not in the reference, so they leave the bar. Agent choice stays in the model picker (X-AG-02); workspace space stays on the cleanup page (D-WS-18). Source: pv96 (unmerged). pv96 (odd desktop-simplification S15): four right-aligned segments (keep awake, resource usage, ports, remote hosts). Not the five owner items. |
| D-SH-06 | Recoverable error screen | Desktop | Shows a failed surface with retry, so one crash does not take the shell down. | Error; retried | Keep | One error pattern for every page, modal and sidebar. |
| D-SH-07 | Startup loading | Desktop | Loads persisted UI and session state before the shell is shown. | Loading; ready; failed | Keep | Keep the gate, not a separate splash design. |
| D-SH-08 | Zoom indicator | Desktop | Shows the current zoom level briefly after a change. | Shown; hidden | Keep | Small and cheap. |
| D-SH-09 | Activity rail | Desktop | Narrow vertical icon rail at the far left of the window, part of the frame. Top group: mode switch (D-SH-10), Tasks (D-SH-11), Calendar (D-SH-12), Marketplace (D-SH-13). Bottom group: theme toggle (D-SH-14), usage rings (D-SH-15), Settings (D-SH-16). | Workspaces mode; Bot Teams mode | New | Owner requirement (2026-11-12). Filled with the frame token (`surface.frame`, #1b1b1c dark), the same as the Sidebar and Title bar, with no seam between rail and sidebar. The reference screenshot shows the rail black; the owner's colour note overrides that. Runs the full window height; the title bar and sidebar sit to its right (assumed, owner to confirm). Extensions add icons through D-SL-09. Always visible: the sidebar toggle hides only the workspace sidebar, never the rail. With the sidebar hidden, the canvas's rounded top-left corner sits right next to the rail. Android has no rail: each entry's Android place is in its row. Source: pv96 (unmerged). pv96 built (src/renderer/src/app-shell/AppRail.tsx, 52 px). Background is the normal background token, not surface.frame. |
| D-SH-10 | Mode switch (Workspaces / Bot Teams) | Desktop | One button in the rail's top slot. Its icon shows the current mode: grid for Workspaces, bot for Bot Teams. A press switches the sidebar (D-WS-01) between the two, but only from the mode's home. From any other page, the first press returns to the current mode's home (decided on the owner's behalf, 2026-11-12; see pending-owner-review.md, item 6). | Workspaces mode; Bot Teams mode | New | Owner clarification (2026-11-12). Highlighted when active. The icon morphs in about 200 ms: a cross-fade with scale and rotate of the two vector icons. Reduced motion: instant swap. Android: Home top bar (proposal). Source: pv96 (unmerged). pv96 built, but a press from another page first returns to the mode home; only a press at home switches (store/slices/ui/ui-slice-view-actions.ts). Open question 15 is answered on the owner's behalf (pending-owner-review.md, item 6). The owner's one-click wish is not kept. The mode's home is assumed (A39). |
| D-SH-11 | Tasks entry (rail) | Desktop | Checklist icon in the rail. Opens the Tasks page (D-WS-24) as a full page over the canvas. It covers both Workspaces and Bot Teams. | - | New | Owner requirement (2026-11-12). Entry only; there is no second Tasks screen. The Workspaces Sidebar has a Tasks entry too, which opens the same page (D-WS-01). Android: the Home tasks card (M-HM-04, merged into M-HM-01) opens M-TK-01. Source: pv96 (unmerged). pv96 built (AppRail.tsx: openTasksHome). |
| D-SH-12 | Calendar entry (rail) | Desktop | Calendar icon in the rail. Opens the calendar page (D-AG-23) as a full page over the canvas. | - | New | Owner requirement (2026-11-12). Entry only. Android: Home overflow (proposal; calendar is not designed for Android). Source: pv96 (unmerged). pv96 built (AppRail.tsx: openCalendarPage). |
| D-SH-13 | Marketplace entry (rail) | Desktop | Store icon in the rail. Opens the extension browser (D-EX-01) as a full page over the canvas. | - | New | Owner requirement (2026-11-12). Links to D-EX-01; no second store screen. Android: none; extensions are not designed on Android (section 15). Source: pv96 (unmerged). pv96 built (AppRail.tsx: openMarketplacePage). |
| D-SH-14 | Theme toggle (rail) | Desktop | Sun icon in the rail. Switches between light and dark. The full theme choice stays in D-ST-03. | Light; dark | New | Owner requirement (2026-11-12). Light and dark ship together, on the same frame and canvas structure. Android: Home top bar (proposal). Source: pv96 (unmerged). pv96 built (AppRail.tsx: RailThemeButton). |
| D-SH-15 | Usage rings and detail popover (rail) | Desktop | One icon per agent, with a circular progress ring for that agent's usage. Hover or click opens a usage detail popover. | Loading; available; unavailable (as D-AC-06) | New | Owner requirement (2026-11-12). Absorbs the status-bar usage switcher (D-SH-05). The full view is D-AC-06. Android: usage on the host card (M-HM-02, with M-AC-01), as a proposal. Source: pv96 (unmerged). pv96 built (app-shell/AppRailUsage.tsx, usage-ring-windows.ts): double ring for 5-hour plus weekly, single ring for weekly only. |
| D-SH-16 | Settings entry (rail) | Desktop | Gear icon at the bottom of the rail. Opens Settings as a full page over the canvas (D-SH-02), sections D-ST-01 to D-ST-11. | - | New | Owner requirement (2026-11-12). Entry only. Android: Home top bar (M-HM-01). Source: pv96 (unmerged). pv96 built (AppRail.tsx: openSettingsPage, with shortcut hint). |
| D-SH-17 | Work panel | Desktop | Region between Session summary and Project sidebar in the Canvas. Tabs for Terminal (D-TM-01), Browser (D-BP-01), Markdown or file (D-ED-01) and Mobile emulator (D-SH-18); a + adds a tab. | Empty (four actions: New terminal, New browser tab, New Markdown, New mobile emulator); tabs open | Keep | Owner requirement (2026-11-12). Replaces the workbench and the right side panel of the current app as one named region. Terminal, browser and editor content live here as tabs. Files and diffs open here as file tabs (owner, 2026-11-12; open question 13 answered; G26 closed). Keyboard (pv96, not checked; see app-flow.md): Mod+T adds a terminal tab, Mod+Shift+B a browser tab, and Mod+W closes the focused tab, never the session. Source: pv96 (unmerged). pv96 built as the side panel (components/side-panel/SidePanel.tsx). The empty state shows the four owner actions; the mobile emulator option depends on setting mobileEmulatorEnabled. |
| D-SH-18 | Mobile emulator tab | Desktop | Work panel tab that previews a mobile app or mobile web view in a device frame. | Empty; running; error (assumed, A37) | New | Owner requirement (2026-11-12): one of the four Work panel actions. Details are assumed (A37): the device choice, the frame and what the tab runs (a URL or the workspace's dev server) are not in the owner's reference and are not decided. Replaces the dropped emulator pane (D-BP-07). Source: pv96 (unmerged). pv96: offered only when setting mobileEmulatorEnabled is on (side-panel-new-tab-options.ts). |
| M-SH-01 | Navigation stack | Android | Stack of routes; home is the host list; host list is a master pane on wide layouts. | Phone (stack); wide (master and detail) | Keep | Current model has no tabs; keep it until a design pass says otherwise. |
| M-SH-02 | Protocol gate | Android | Blocks use when app and host protocol versions do not match. | Compatible; blocked | Keep | Required by the versioned protocol (PLAN rule). Say which side needs an update. |
| M-SH-03 | Web shell route (OTA) | Android | Renders some routes in a WebView on OTA builds, with a waiting frame and an unavailable state. | Waiting; route unavailable | Drop | Hybrid fallback for OTA web screens. Native screens remove the need. Open question 3 is answered (owner accepted default 2026-11-12): Android is fully native, so this stays dropped. |
| M-SH-04 | Web twins, catch-all page and redirect routes | Android | Web build twins, host catch-all page, and redirect-only routes (pr, history, notification opt-in). | - | Drop | Typed routes replace them. Old deep links need no compatibility (PLAN decision 3). |

## 2. First run, onboarding and setup

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-OB-01 | Onboarding wizard | Desktop | First-run setup in a short wizard. | Step n of 3; skipped; step error | Redesign | Cut from five steps to three: agent accounts (D-AC-01), theme, notifications. Others move to the setup checklist (D-OB-12). |
| D-OB-02 | Step: default agent | Desktop | Picks the default agent. | - | Merge into D-AC-01 | Agent choice belongs with account setup. |
| D-OB-03 | Step: theme | Desktop | Picks a theme with a live chrome preview. | - | Keep | The live preview is worth keeping. Stays a wizard step. |
| D-OB-04 | Step: integrations | Desktop | Connects task and source providers. | Per-provider status | Merge into D-OB-12 | Optional; reachable from the checklist. |
| D-OB-05 | Step: Windows terminal and Ghostty import | Desktop | Sets up the Windows shell and imports discovered terminals. | Terminals discovered | Merge into D-ST-05 | Windows-only detail; belongs in terminal settings. |
| D-OB-06 | Step: notifications | Desktop | Sets notification preferences. | - | Merge into D-OB-01 | Short; stays a step of the wizard. |
| D-OB-07 | Skip-onboarding confirm | Desktop | Confirms skipping the rest of onboarding. | - | Merge into X-DLG-01 | Generic confirm. |
| D-OB-08 | Inline setup command terminal | Desktop | Runs a setup command in an embedded terminal inside onboarding. | Running; done; failed | Merge into D-TM-02 | Reuse the normal terminal pane. |
| D-OB-09 | First-launch banner | Desktop | Dismissable first-launch notice. | - | Drop | Replaced by the setup checklist (D-OB-12). |
| D-OB-10 | Telemetry consent prompt | Desktop | Asks once for telemetry consent (internal cohort only today). | Asked; answered | Redesign | One explicit choice in D-OB-01, default off. Privacy first. |
| D-OB-11 | Feature wall | Desktop | Showcase of features with a tour panel. | - | Drop | Marketing surface; no user task. |
| D-OB-12 | Setup checklist | Desktop | Lists setup steps and their progress. Opened from the setup guide modal and General settings. | In progress; complete | Redesign | One checklist, reachable from first run and from D-ST-01. Absorbs the setup guide pane. |
| D-OB-13 | Feature tips queue and five tip dialogs | Desktop | Shows feature tips one by one, each with its own dialog. | Seen; unseen | Drop | Five near-identical dialogs. Replace with one dismissable tip pattern (X-NT-01). |
| D-OB-14 | Contextual tour overlay | Desktop | Step-by-step coach marks anchored to UI. | Step n | Drop | Not needed for the first version. Extension candidate. |
| D-OB-15 | Star request card and toast | Desktop | Asks for a GitHub star after value moments. | Card; toast | Drop | Not a user task. |

## 3. Agent accounts and subscription migration (NEW)

Required by PLAN: the owner's agent accounts and subscriptions must carry over so agents can be used.

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-AC-01 | Agent accounts list | Desktop | One list of agent accounts (Claude, Codex, MiniMax, OpenCode, Cursor, Grok, Antigravity, zCode) with sign-in state. | Signed in; expired; needs sign-in; not set up; migrated; conflict | Redesign | Replaces one pane per provider with one list and provider adapters. Absorbs the Accounts pane and D-OB-02. |
| D-AC-02 | Provider sign-in flow | Desktop | Signs one account in or out through its provider. | Starting; waiting for browser or code; done; failed | Merge into D-AC-01 | One generic flow with provider-specific steps. Codex warnings become a state. |
| D-AC-03 | Account removal dialogs | Desktop | Confirms removing an account. | - | Merge into X-DLG-01 | Generic confirm that names what is removed. |
| D-AC-04 | Migration from the current app | Desktop | Finds the owner's existing agent accounts and subscriptions and imports them. | Found; none found; selecting; importing; imported; failed | New | Lists what was found; the owner picks; each credential is checked on import and stored in the OS secret store, never shown. Copying was accepted as the method (open question 1, owner accepted default 2026-11-12). |
| D-AC-05 | Migration result and re-sign-in queue | Desktop | Shows what migrated and what still needs sign-in, with one action per account. | Migrated; needs sign-in; skipped | New | Accounts that did not migrate must be obvious. |
| D-AC-06 | Usage and subscriptions | Desktop | Usage windows, plan and limits per account. | Loading; available; unavailable | Redesign | Merges the Stats and Usage pane, status-bar usage, and chat usage cards. The rail's usage rings and popover (D-SH-15) are the quick view; this page is the full view. |
| X-AC-01 | Account needs sign-in card | Both | Says an account expired or was signed out, with one action. | Shown; dismissed; signed in | Redesign | Replaces the unexpected sign-out card and the Codex login-link notice. Same card on Android. |
| M-AC-01 | Accounts usage (per host) | Android | Account usage for one host, with the Codex reset-credit action. | Loading; live time; reset pending | Keep | Read first; keep the provider-specific reset action. Counterpart of D-AC-06. |

## 4. Hosts, pairing and remote/SSH

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-HO-01 | Hosts in the sidebar | Desktop | Shows, under each project, one row per host the project lives on, with its workspaces. Projects are the top level; a project can live on several hosts (owner, 2026-11-12). | Connected; connecting; disconnected; unreachable; auth failed | Keep | Hosts are first-class in the same tree as local projects. |
| D-HO-02 | Add host (SSH or server) | Desktop | Adds an SSH host or a remote server. | Form; SSH config picker; testing; failed | Redesign | One dialog with two paths. Absorbs the SSH target form, server-start step and runtime environment add/edit dialogs. |
| D-HO-03 | Rename and remove host | Desktop | Renames a host or removes it from the sidebar. | - | Merge into X-DLG-01 | Rename is a one-field variant of the confirm pattern. |
| D-HO-04 | Forget saved SSH workspace | Desktop | Forgets a saved remote workspace. | - | Merge into X-DLG-01 | Destructive confirm. |
| D-HO-05 | Widen host visibility gate | Desktop | Asks before widening which hosts are visible. | - | Merge into X-WS-01 | Becomes an inline choice in the workspace filter. |
| D-HO-06 | SSH passphrase prompt | Desktop | Asks for a key passphrase when a remote connection needs one. | Prompting; wrong; saved | Keep | Secret entry; never logged or shown back. |
| D-HO-07 | Servers and access | Desktop | Lists remote servers: connect, reconnect, update, managed servers, moves, access grants. | Connected; reconnecting; update available; updating; failed | Redesign | Absorbs the Servers pane, update dialog, move dialogs (SSH managed server and host delta) and grant list. |
| D-HO-08 | Local network connection test | Desktop | Tests whether the phone can reach this machine on the local network. | Running; passed; failed | Merge into X-DG-01 | Shared diagnostics screen. |
| D-HO-09 | Remote workspace folder picker | Desktop | Chooses a directory on a remote host. | Loading; listing; error | Merge into X-PK-01 | Same picker everywhere. |
| D-PR-01 | Phone pairing | Desktop | Shows the QR code and connection options so a phone can pair. | Idle; showing QR; path choice; waiting for device; paired; Windows firewall warning | Redesign | One pairing screen. Replaces the Mobile page (D-PR-03) and the pairing settings. Absorbs the runtime pairing URL, address picker, custom address and firewall notice. |
| D-PR-02 | Paired devices | Desktop | Lists paired phones and revokes them. | None; active; revoked | Keep | Revocation must be one clear action. Desktop-only. |
| D-PR-03 | Mobile page (view) | Desktop | Full page version of phone pairing. | - | Merge into D-PR-01 | Same content; a settings section is enough. |
| D-PR-04 | Relay beta notice | Desktop | Beta notice for relay pairing. | Shown | Drop | Relay is not in the first version (open question 5, owner accepted default 2026-11-12: local network and SSH only). |
| X-DG-01 | Connection diagnostics | Both | Shows connection logs, troubleshooting steps and the local network test, per host. | Ok; warning; failed | Redesign | Merges the desktop connection test (D-HO-08) and the Android connection log and troubleshoot screens. Source: pv96 (unmerged). pv96: mobile diagnostics rewritten; connection-diagnostics-submission.ts deleted. |
| X-CN-01 | Connection status banner | Both | Says a host, terminal or runtime is reconnecting, disconnected or restored. | Connecting; reconnecting; disconnected; restored | Keep | Absorbs the SSH reconnect overlay, remote runtime banner and runtime disconnect toast. |
| M-PA-01 | Connect hint (empty home) | Android | Explains how to connect the desktop in three steps. | No hosts | Keep | Keeps first-run pairing clear. |
| M-PA-02 | Scan pairing code | Android | Scans the QR code, with a paste fallback and deep-link entry. | Scanning; connecting; invalid code; camera denied; failed | Keep | Paste is a mode of this screen, not a separate sheet (M-PA-03 merges here). |
| M-PA-03 | Paste pairing code sheet | Android | Text entry for the pairing code. | Invalid | Merge into M-PA-02 | Paste is a mode of the scan screen. |
| M-PA-04 | Confirm pairing | Android | Asks the owner to confirm pairing with a named desktop. | Awaiting confirm; connecting; failed | Keep | Show the desktop name before confirming. Counterpart of D-PR-01. |
| M-PA-05 | Post-pair setup | Android | Shows only undecided steps: default view, then notifications. | Pending steps only; error | Redesign | Absorbs the mobile onboarding wizard and the notification onboarding preview. |
| M-HO-01 | Edit host | Android | Renames a host and edits its address. | Save; refresh | Keep | Counterpart of D-HO-03. |

## 5. Workspaces and sidebar

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-WS-01 | Sidebar (workspace tree) | Desktop | Tree of projects, groups, workspaces and hosts. In Bot Teams mode (D-SH-10) it lists teams instead. In Workspaces mode the top of the sidebar holds, in order: + Create (opens the start page, D-AG-01), Search, Prototypes with its count (D-AG-17), Tasks (D-WS-24), Automations (D-AG-14, a full-canvas page) and MassOrca Mobile (target assumed, A38). Then a Projects section with header actions (notifications, filter or sort, add folder, + new project, D-WS-09). Footer: help, locate and panel toggle. | Empty; collapsed; filtered; loading; Bot Teams mode | Redesign | Entries and footer follow the owner's Workspaces Sidebar note (2026-11-12). Automations has its entry here, not on the rail (owner decision). One tree. Absorbs project groups, external workspace inbox, hibernation state and header menu. The rail's mode switch decides which tree shows (owner, 2026-11-12). Part of the frame surface (D-SH-01), with no seam to the rail. Bot Teams mode lists teams as in D-WS-29. Source: pv96 (unmerged). pv96: one-line rows and a No Git pill (D-WS-11). The Bot Teams sidebar is D-WS-29. |
| D-WS-02 | Workspace row and menu | Desktop | Row for one workspace, with its actions. | Idle; working; unread; selected; sleeping | Keep | Status shows through X-AG-01. Source: pv96 (unmerged). pv96: one-line row with collapse, session menu, hover card with Environment, cross-worktree child rows, manual branch name (odd worktree-one-line-sidebar T1-T4). |
| D-WS-03 | Open-in and developer submenus | Desktop | Opens a workspace in other tools; developer actions. | - | Merge into D-WS-02 | One submenu. |
| D-WS-04 | Workspace edit | Desktop | Edits name, emoji, metadata, status appearance and icon. | - | Keep | Absorbs emoji suggestions, icon pickers (D-OT-10). |
| D-WS-05 | Project groups | Desktop | Creates, renames and deletes project groups. | - | Merge into D-WS-01 | Inline rename; delete goes through X-DLG-01. |
| D-WS-06 | Status appearance | Desktop | Sets a workspace's status colour and icon. | - | Keep | Feeds X-AG-01. |
| D-WS-07 | Sidebar header menu | Desktop | Header options: filters, settings and help. | - | Merge into D-WS-01 | Header of the tree. |
| D-WS-08 | Workspace board | Desktop | Board of workspaces by area, with drag and drop. | Area selection during drag | Extension | Owner accepted default 2026-11-12 (open question 4): the workspace board is an extension, not core. Shown through the extension view slot (D-SL-04). |
| D-WS-09 | Add project | Desktop | One entry for adding a project from any source. | Source choice; per-source step; progress | Redesign | Replaces the multi-step add-repo dialog with a source choice. |
| D-WS-10 | Add-project source steps | Desktop | Steps for clone, create, nested import, remote, server start, GitHub account. | - | Merge into D-WS-09 | One step per source. |
| D-WS-11 | Non-git folder and add-from-folder confirms | Desktop | Asks to add a folder that is not a git repo, or confirms adding a folder as a project. | - | Merge into D-WS-09 | Non-git folder becomes a state of the add flow. Source: pv96 (unmerged). pv96: a sidebar pill with popover and an Initialize Git action, local and SSH (odd no-git-folder-warning T1-T4). |
| D-WS-12 | Project added confirmation | Desktop | Confirms that a project was added. | - | Drop | Noise. A toast (X-NT-01) says the same. |
| D-WS-13 | Create-from picker and nested scan notice | Desktop | Picks the source for a new repo; warns when the nested scan hits its limit. | Limit reached | Merge into D-WS-09 | Inline state. |
| D-WS-14 | New workspace composer | Desktop | Creates a workspace: location, base ref, parent, dropped files, setup script offer. | Idle; validating; base ref missing; creating; setup offered | Keep | The one creation entry point. Absorbs the set-location dialog, base ref fallback, base ref and parent pickers (X-PK-01), setup script prompt card and toast, and auto-rename failure notice. |
| D-WS-15 | Workspace creation progress | Desktop | Shows progress in place of the terminal while a workspace is created. | Pending; done; failed | Keep | A Work panel tab (D-SH-17). A failure keeps the log visible. |
| D-WS-16 | Delete workspace | Desktop | Confirms deleting a workspace, with warnings for unsaved or unpushed work and lineage. | Clean; unsaved; unpushed; nested children | Keep | Highest-risk confirm; keeps its specific warnings. Absorbs the nested-workspace delete dialog and lineage notice. |
| D-WS-17 | Preserved-branch review | Desktop | Reviews branches kept after a delete. | Batch | Merge into D-WS-18 | Part of cleanup. |
| D-WS-18 | Workspace cleanup | Desktop | Bulk cleanup of finished workspaces, with disk usage. | Loading; blocked items; empty | Redesign | Absorbs the space page (D-WS-19). |
| D-WS-19 | Workspace space page | Desktop | Disk usage per workspace. | - | Merge into D-WS-18 | Same data. |
| D-WS-20 | Repo hooks trust | Desktop | Asks before running a repo's hook scripts from its config file. | Untrusted; trusted; changed | Keep | Running repo hooks executes code. Keep the explicit trust step. |
| D-WS-21 | Local git toolchain banner | Desktop | Warns about missing git tools. | Shown; dismissed | Merge into D-DF-07 | Belongs with git settings. |
| D-WS-22 | External workspace inbox | Desktop | Asks about worktrees created outside MassOrca, with a dismiss option. | Listed; dismissed | Merge into D-WS-01 | Inline row in the sidebar. |
| D-WS-23 | Sidebar feedback dialog | Desktop | In-app feedback form. | - | Merge into D-ST-01 | Support section. Source: pv96 (unmerged). pv96: image attachments removed from the feedback dialog (SidebarFeedbackImageAttachments.tsx deleted). |
| D-WS-24 | Tasks page | Desktop | Lists issues and PRs from connected task providers, and the Bot Teams tasks from the team Boards (D-WS-30). Opened from the rail (D-SH-11) or the Workspaces Sidebar's Tasks entry (D-WS-01) as a full page; the Sidebar is hidden there, and Back returns to the session. | Per provider: loading; connect; error; empty | Redesign | One list with a provider filter. Replaces per-provider pages. Covers both Workspaces and Bot Teams (owner, 2026-11-12). The Bot Teams source is the team Board (open question 6, answered); listing Board tasks here is assumed (A25). Source: pv96 (unmerged). pv96 built (components/tasks-home/TasksHomePage.tsx): approvals from WorkSpaces and Bot Team; create switch WorkSpaces / Bot Team; Start now or later. |
| D-WS-25 | Task item detail | Desktop | Issue or PR detail, with reviewer and duplicate pickers, and create-workspace from it. | Loading; saving; error | Redesign | One detail panel with provider sections. Absorbs the GitHub, GitLab, Jira and Linear dialogs and workspaces. |
| D-WS-26 | GitHub Projects views and dialogs | Desktop | Project boards and tables, item slugs, missing-repo dialog. | Loading; error; missing repo | Extension | Owner accepted default 2026-11-12 (open question 4): an extension, not core. |
| D-WS-27 | Sparse directory picker | Desktop | Chooses directories for a sparse checkout. | - | Drop | Niche. Extension candidate. |
| D-WS-28 | Workspace visibility dialog and help | Desktop | Chooses which workspaces are visible. | - | Merge into X-WS-01 | Replaced by the filter. |
| D-WS-29 | Sidebar in Bot Teams mode | Desktop | Lists the teams in Bot Teams mode (D-SH-10): a search box, All teams, and each team with its Conference room and Board, each with a status hint. Below, a Projects section (filter and +) with Files, Memory and Routines. | Empty (no teams); item selected | New | Owner requirement (2026-11-12), from the reference screenshot: the Conference room shows Nothing open and the Board shows No tasks. Files, Memory and Routines open D-ED-13, D-AG-26 and D-AG-25. Workspaces mode keeps the tree of D-WS-01. Source: pv96 (unmerged). pv96 built (components/sidebar/BotTeamsSidebar.tsx): labels Nothing open, No tasks, Files, Memory, Routines; Projects with Filters, New project, Show archived. |
| D-WS-30 | Team Board | Desktop | Team tasks. A requirement the user states in the team's Conference room (D-AG-24) becomes a task here. | No tasks (sidebar hint); tasks listed | New | Owner requirement (2026-11-12). The Board is the Bot Teams source of tasks (open question 6, answered). Opening a task has no detail row yet: D-WS-25 covers issues and PRs only. Source: pv96 (unmerged). pv96 built (components/bot-teams/BotTeamBoard.tsx): 'No tasks here.'; per-project boards are D-WS-31. |
| D-WS-31 | Team project brief, board and link | Desktop | A Bot Teams project: its brief and editor, its own board of tasks, an optional link to a Workspaces project, and delete. | Brief; board; no link (host cannot link); deleting | New | Source: pv96 (unmerged). components/bot-teams/BotTeamProjectBrief*.tsx, BotTeamProjectViews.tsx, BotTeamProjectWorkspace.tsx, BotTeamProjectDeleteDialog.tsx. Not in the owner notes; owner to validate. |
| X-WS-01 | Workspace filter | Both | Chooses which workspaces and projects are shown. | All; filtered; no match | Redesign | Desktop popover (absorbs D-WS-28, the project filter panel, D-HO-05). Android sheet (M-WS-03). |
| M-WS-01 | Workspace list | Android | Workspaces of one host; master pane on wide layouts. | Loading; catalog error (retries itself); empty | Keep | Counterpart of D-WS-01. Source: pv96 (unmerged). pv96: one-line rows and status glyphs (mobile/src/worktree/*.ts). |
| M-WS-02 | Workspace action sheet | Android | Long-press actions for one workspace. | - | Keep | Counterpart of D-WS-02. Source: pv96 (unmerged). pv96: WorktreeAgentRow changed (mobile/src/components/WorktreeAgentRow.tsx). |
| M-WS-03 | Filter sheet | Android | Filter options for the list. | - | Merge into X-WS-01 | Same filter, shown as a sheet. |
| M-WS-04 | New workspace button and form | Android | Floating button and form sheet to create a workspace. | Visible; submitting; failed | Merge into D-WS-14 | Same fields; the button stays. |
| M-WS-05 | Folder without git sheet | Android | Offers to initialise git in a folder. | - | Keep | Non-git state of the form. Source: pv96 (unmerged). pv96: no-git pill and sheet, gated on a capability (odd no-git-folder-warning T4). |
| M-WS-06 | Empty detail placeholder | Android | Placeholder in the wide-layout detail pane. | - | Merge into M-SH-01 | Empty state of the master-detail layout. |
| M-WS-07 | Confirm modal | Android | Destructive confirm. | - | Merge into X-DLG-01 | Generic confirm. |
| M-TK-01 | Tasks screen | Android | Task list with create and filter; provider and project pickers. | Loading; empty; provider error | Keep | Counterpart of D-WS-24. Create drawers stay inside it. |

## 6. Agents and sessions

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-AG-01 | Start page | Desktop | Starts an agent chat with or without a workspace. | Empty; workspace chosen; starting | Redesign | Replaces the landing page and the create-chat page (D-AG-02). Its composer uses the model picker (X-AG-02). Source: pv96 (unmerged). pv96 built (components/create-chat/CreateChatPage.tsx): placement row above the composer (host, project, branch or new worktree); Terminal / Chat switch beside Access (odd desktop-simplification S4-S6). |
| D-AG-02 | Create-chat page | Desktop | Starts a chat without a repo. | - | Merge into D-AG-01 | Same job. |
| D-AG-03 | Conversation | Desktop | The Conversation region: transcript and composer for one agent session, with its message minimap (D-AG-28), find, thread goal banner, and drag-and-drop files. | Loading; session gate; empty; ready; error | Keep | Primary agent surface. Uses X-CH-* cards. Its composer uses the model picker (X-AG-02). Find merges into D-FD-01. Counterpart of M-CH-01. Child agents show in its delegation view (X-DL-01), inside the session. Source: pv96 (unmerged). pv96: mention menu (X-CH-09), checklist section (X-CH-11), child bring-back (X-CH-08). |
| D-AG-04 | Access mode picker | Desktop | Sets the agent's access mode. | - | Merge into X-AG-02 | The access mode is the composer's own access chip (desktop) or lock (Android), next to the model chip; it is no longer in the model picker footer (consistency pass C3, 2026-11-13). |
| D-AG-05 | Continue or fork session | Desktop | Continues an earlier session, or forks it into a new pane. | Choosing; running | Redesign | One dialog for both. Absorbs the continuation and fork dialogs. |
| D-AG-06 | Resume after restart | Desktop | Lists sessions to resume after the app restarts, with status per run. | Loading; per-run ok or failed; none | Keep | Matches the PLAN rule that agents survive a UI restart. Counterpart of the Android resume card (M-HM-03). |
| D-AG-07 | Session history search | Desktop | Searches inside past agent sessions, per computer. | Indexing; results; none | Keep | Absorbs the Agent Session Search settings pane. Counterpart of M-SE-06. Source: pv96 (unmerged). pv96: 'This worktree' scope; automated runs hidden by default; child sessions nested under the parent (odd session-history-scope T2-T4). |
| D-AG-08 | Agent launch notice | Desktop | Notice on a pane that launched an agent. | - | Merge into D-TM-02 | Pane state. |
| D-AG-09 | Codex warnings and restart chip | Desktop | Old Codex terminal warning, shared server banner and fix, restart chip. | - | Drop | Workarounds for the Electron app's Codex integration. Phase 4 agent adapters replace them. |
| D-AG-10 | Agent hibernation gate | Desktop | Gate for sleeping agent workspaces. | Asleep; awake | Merge into D-WS-01 | Sleeping state of the tree. |
| D-AG-11 | Agent settings dialog | Desktop | Per-agent defaults. | - | Merge into D-ST-02 | Settings content. |
| D-AG-12 | Activity page | Desktop | Page labelled as a prototype. | - | Drop | No stated purpose in the current app. |
| D-AG-13 | Bot teams page and add-bot dialog | Desktop | Bot team setup. Core mode page, reached from the rail's mode switch (D-SH-10). Its pages open as full pages over the canvas (owner, 2026-11-12). | - | Redesign | Owner decision 2026-11-12: Bot Teams is a core mode switched from the activity rail. Redesign because the page's content was not read in detail. Bots are added here. The team's conversation is D-AG-24, and its sidebar entries are in D-WS-29. Orchestration is in the first version (open question 7, answered): the delegation design is X-DL-01. Source: pv96 (unmerged). pv96 built: Bot Teams page with routines, web addresses, memory and files (components/bot-teams/BotTeamsPage.tsx). Bot form is D-AG-33. |
| D-AG-14 | Automations page | Desktop | Lists scheduled automations, with run history and a detail pane. | Empty; skeleton; owner conflict; host filter | Redesign | One page for list, detail and runs. Absorbs run details, run dashboard, cron output, list filter and automation defaults. Same core scheduling engine as the Routines page (D-AG-25), in its Workspaces presentation (owner, 2026-11-12; open question 8, answered). Shown on the Calendar (D-AG-23). Owner (2026-11-12): opens as a full page over the canvas, with the Sidebar hidden. Its entry is Automations in the Workspaces Sidebar (D-WS-01; owner decision, 2026-11-12). A team can turn on Workspaces in a team to reach this depth inside Bot Teams. |
| D-AG-15 | Automation editor | Desktop | Creates or edits an automation with a schedule picker. | - | Keep | Custom cron stays behind an advanced toggle. |
| D-AG-16 | Automation delete dialogs | Desktop | Confirms deleting automations. | - | Merge into X-DLG-01 | Generic confirm. |
| D-AG-17 | Prototypes page | Desktop | Lists, views, edits and styles prototypes, with a design-system view. | List; viewer; unopenable; editing | Keep | Core page, not an extension: decided on the owner's behalf, 2026-11-12 (see pending-owner-review.md, item 1). This supersedes the default in A4 and open question 4. Its entry is Prototypes in the Workspaces Sidebar, with its count (D-WS-01). Counterpart of M-PT-01. Source: pv96 (unmerged). pv96 builds prototypes as a core page with a sidebar row (components/sidebar/SidebarPrototypesEntry.tsx); PrototypeListPanel.tsx removed. Open question 16 is answered (item 12). Sub-views D-AG-29 to D-AG-32. |
| D-AG-18 | Prototype links in chat | Desktop | Chat cards that link prototypes (count, link, proposal, system). | - | Keep | Prototypes is core (D-AG-17), so its link cards stay in the core chat, not an extension slot (D-SL-03). Source: pv96 (unmerged). pv96: prototype link cards still live in the core chat (native-chat/NativeChatPrototypeLinkCard.tsx); kept in core. |
| D-AG-19 | Usage cards in chat | Desktop | Usage, background status and Codex panels inside chat. | - | Merge into D-AC-06 | One usage home. |
| D-AG-20 | Inline diff card | Desktop | Shows a diff inline in chat, and the plan viewer. | - | Merge into D-DF-01 | Diff view owns diffs. |
| D-AG-21 | Visual panel and recorded file viewer | Desktop | Shows visual output and recorded files from a session. | - | Merge into D-ED-09 | Read-only viewers. |
| X-AG-01 | Agent status indicators | Both | Shows idle, working, waiting for input, question, error, and child work. | Idle; working; question; error; child rows | Keep | One status model fed by the status store (PLAN rule). Replaces the dot, spinner and question icon. Android shows the same states. Source: pv96 (unmerged). pv96: AgentSpinner.tsx removed on mobile; shared status glyphs (SessionStatusMark.tsx, session-status-glyph.ts). |
| X-AG-02 | Model picker | Both | One selector for agent, model and reasoning effort in every chat composer (access is its own chip, C3): start page (D-AG-01), Tasks (D-WS-24) and sessions (D-AG-03). Trigger: a chip in the composer's bottom row, showing the agent icon and the model and effort, for example Opus 5.5 Medium. | Closed; open; searching; no match | New | Owner requirement (2026-11-12). Popover above the composer, in three columns: agent rail (Favourites star, one icon per agent or provider, accent bar on the active one); model search and list (provider icon, name, favourite star on hover, Ctrl+1..4 / Cmd+1..4 badges, Show more); reasoning column (effort levels that depend on the model, checkmark on the selected one). Footer: Standard or Fast only; access moved to the composer chip (D-AG-04 merges there). Ctrl+1..4 / Cmd+1..4 work only while it is open. Extensions add agents and models (D-SL-10). Android composers (M-CH-02, M-SE-01): same parts as a sheet, not designed here (open). Source: pv96 (unmerged). pv96 differs: chip with featured models and More models; access picker is separate (create-chat/CreateChatAgentChip.tsx, native-chat/NativeChatAccessModePicker.tsx). No search, Favourites, three columns or Standard / Fast footer. Conflict; owner design kept. |
| X-CH-02 | Interactive cards | Both | Approve or deny a tool use; answer an agent question; single, multi-select or free text. | Pending; submitting; answered; error | Keep | Desktop: approval, question, structured and interactive cards. Android: M-CH-03. Multi-select submit enabled only with a pick. |
| X-CH-03 | Queued messages | Both | Shows follow-up messages waiting to send, and confirms sending one now. | Queued; sending | Keep | Desktop: queue card and confirm (replaces the send-now dialog). Android: M-CH-04. |
| X-CH-05 | Plan view | Both | Shows the agent's plan document. | Shown; scrolled | Keep | Desktop: plan viewer. Android: M-CH-05 sheet. |
| X-CH-06 | Delegation card and explanation | Both | Shows delegated sub-agent work and why it was delegated, and where each child runs (own or current worktree). | Running; done; parent waiting (a child writes in the current worktree) | Keep | Desktop: delegation card and notice. Android: M-CH-06 card and why-sheet. The child tree is X-DL-01. Source: pv96 (unmerged). pv96: automatic router across agents, rich card, why sheet, nesting (odd delegated-child-sessions T1-T10). |
| X-CH-07 | Secret request card | Both | Asks for a secret in the conversation, when the owner asks or the agent finds it needs one. Shows the variable name and a state, never the value. | Pending; stored; linked; declined; failed | New | Owner requirement (2026-11-12). Desktop: a card in the chat with Add and Decline; Add opens the entry modal (X-VT-01). Android: the same card in the session chat (M-CH-01). The value never enters the transcript, the model context or logs. Agents receive it as an environment variable or reference at run time. It is stored on the executing host, in its OS secret store, or in an encrypted store on a headless server (app-flow.md, G22). |
| X-CH-08 | Child work bring-back | Both | Brings a finished child session's commits into the parent worktree, in place: a merge without fast-forward on the host, with conflict handling. Nothing is pushed. | Commits ready; predicted conflict; child with uncommitted work; parent moved ahead; conflicts; already brought back | New | Source: pv96 (unmerged). components/native-chat/NativeChatChildBringBack.tsx; mobile/src/session/MobileChildBringBack.tsx (odd/tasks/child-bring-back.md, S1-S5). Rule: no silent loss; no stash; every blocked state says why. Not reviewed by the owner. |
| X-CH-09 | Composer mention menu | Both | An @ in a composer opens apps, teams, bots and projects; a picked item becomes a chip with its icon or team character. | Closed; open; no match | New | Source: pv96 (unmerged). components/native-chat/NativeChatMentionMenu.tsx, components/bot-teams/BotTeamComposerMenu.tsx, BotTeamMentionChip.tsx; mobile/src/bot-teams/MobileBotTeamMentionChip.tsx (odd/tasks/bot-teams.md, S20). |
| X-CH-10 | Team request choice card | Both | When a request matches no bot or team, asks whether to create one or route to an existing one; suggests a team with a choice the person can change. | Pending; answered | New | Source: pv96 (unmerged). components/bot-teams/BotTeamRequestChoiceCard.tsx and BotTeamPendingChoice.tsx (odd/tasks/bot-teams.md, S22, owner relayed). Reuses the automatic agent choice card. |
| X-CH-11 | Session checklist | Both | The agent's checklist for a session: summary card, transcript row, add and edit items; on the phone, sheets and a chip; also in the floating panel. | Empty (no card); items; editing | New | Source: pv96 (unmerged). components/native-chat/NativeChatChecklist*.tsx, NativeChatSessionChecklistSection.tsx; mobile/src/session/MobileChecklistSheet.tsx and MobileChecklistItemSheet.tsx (odd/tasks/checklist-card.md). The card is meant to show only when it has items; not verified. |
| X-DL-01 | Delegation view and engine | Both | One tree of child agents or bots under a parent, with each child's status. Shown inside the parent session or the Conference room, not in a floating panel. | Running; done; per-child status (X-AG-01) | New | Owner decision (2026-11-12): orchestration is in the first version. One core engine serves both modes. Bot Teams: the lead bot delegates parts by competence. Workspaces: a coordinator coding agent launches child agents, sequential or parallel. Where a child runs (owner, 2026-11-12): (1) by default each child gets its own worktree; (2) "Use the current worktree" is allowed only when the child only reads (research, review, explain), when the workspace is a plain folder without git, or when the user picks it by hand on the delegation card; (3) two sessions never write in the same worktree at once: if a child writes in the current worktree, the parent waits until it finishes (sequential, not parallel); (4) parallel launches always give each child its own worktree, with no option. Merges D-TM-14 and the Orchestration pane. Depth limit: a setting in D-ST-02. Android: the card X-CH-06 is shared; the tree is not designed for Android here (open). Source: pv96 (unmerged). pv96 partly built: child sessions nested in the sidebar and a background popover; Bot Teams lead routing via team delegation. Depth limit not verified. |
| X-AR-01 | Artifacts | Both | Artifacts that agents produced in a session: previewed and linked. On desktop they are items of the Results card (D-AG-27), not a page. | Loading; empty; error; preview; link panel | Redesign | Desktop: no page (owner decision, 2026-11-12). The Results card lists artifacts and opens each in a Work panel tab; the detail drawer and link panel are not designed for desktop yet. Android: M-AR-01 (list and viewer). Absorbs the artifacts settings pane. Its place is set by the owner's Results card note (2026-11-12), so G26 is closed. Android keeps M-AR-01 until the owner decides whether it stays (open item in app-flow.md). Source: pv96 (unmerged). pv96: mobile Artifacts screen built (grouped grid and list, read-only sandboxed viewer, live thumbnails). |
| D-AG-22 | Agents overview | Desktop | Running agents with their tool steps; kanban view; terminal popout for one agent; dashboard settings. | Empty; running; waiting; done | Redesign | One overview. Absorbs the dashboard drawer, kanban card, terminal dialog and dashboard settings menu. Counterpart of M-SE-03 (partly). |
| D-AG-23 | Calendar page | Desktop | Schedule view of routines and automations on one timeline, opened from the rail (D-SH-12) as a full page. Filter chips by team and one for Automations. | Loading; empty; loaded (proposed); Day, Week or Month view | New | Owner requirement (2026-11-12): a new screen, and the owner sets its purpose. Every routine appears here, whether it came from the Routines page (D-AG-25) or from a bot in the Conference room (D-AG-24). Opening a routine goes to D-AG-25. Automations (D-AG-14) appear on the same timeline (owner, 2026-11-12; open question 8, answered), and the filter chips separate the two. Opening an automation goes to D-AG-14 (assumed). Owner reference (2026-11-12): previous, Today and next buttons; a range title (for example 9 to 15 nov 2026); filter chips for a team (the reference shows Desarrollo) and for Automations; a Show done switch; a Day, Week and Month control; and the week's day columns, with today outlined. Android: not designed (open). Source: pv96 (unmerged). pv96 built (components/calendar/CalendarPage.tsx): Day, Week, Month; Show done; Upcoming runs; legend of team characters and Automations, not clickable filter chips. |
| D-AG-24 | Conference room | Desktop | Chat with the team's bots, from the team's entry in the Sidebar in Bot Teams mode. Detects the user's requirements and creates tasks on the team's Board (D-WS-30). Creates a routine when the user asks to schedule a task for later. Shows the lead bot's delegation (X-DL-01). | Nothing open (sidebar hint); chat open | New | Owner requirement (2026-11-12). A composite requirement goes to the lead bot, which delegates the parts by competence. Routines the room creates appear on the Routines page (D-AG-25) and the calendar (D-AG-23). Source: pv96 (unmerged). pv96 built (components/bot-teams/BotTeamRoom.tsx): tasks with their own thread, Save to Todo, Start, Stop, Continue, bot posts. |
| D-AG-25 | Routines page | Desktop | Routines of one team, opened from the team's Projects section (Routines). Header with the team's name and New routine. A routine starts a task on a schedule, and each run is kept in its history. Web addresses are webhooks that start one task in the team. Footer names the host the team runs on. | No routines yet; routines listed; no web addresses yet; web addresses listed | New | Owner requirement (2026-11-12), from the reference: both sections have empty states. Routines come from this page or from a bot in the Conference room (D-AG-24). New routine likely opens the schedule editor (D-AG-15), assumed (A26). The simplified Bot Teams form of an automation (D-AG-14): one core scheduling engine, presented for non-technical users (owner, 2026-11-12; open question 8, answered). The New web address form is not in the reference and is not designed here. Source: pv96 (unmerged). pv96 built (components/bot-teams/BotTeamRoutines*.tsx, BotTeamWebhook.tsx): routine list with run labels; web addresses section. New routine opens D-AG-34, not D-AG-15. |
| D-AG-26 | Team memory | Desktop | Memory of one team, opened from the team's Projects section (Memory). | - | New | Owner reference (2026-11-12) shows the entry only. What the memory holds, and whether the bots write to it, is not in the reference and not decided. Source: pv96 (unmerged). pv96 built (components/bot-teams/BotTeamMemory.tsx): Bot and Team tabs, 'What the team remembers', delete, host-unavailable state. |
| D-AG-27 | Session summary cards | Desktop | The Session summary region: a column of summary cards for the session. Only cards that hold information are shown. A card appears when its first record arrives, with an animated insert (instant with reduced motion). Never an empty placeholder card. Known cards: Environment, Plan, Checklist, Context, Results, Subagents, Connectors, Background tasks, QA servers. | Empty (no card); card shown per record type | Redesign | Owner requirement (2026-11-12). The current app shows every card at all times, and this changes that. The Session summary can be hidden from the title bar (owner, 2026-11-12). The owner reference shows no Session summary while a file tab is open; whether that is a rule is not stated. Each card has a chevron to a detail view; the detail views are not designed here. Card contents follow the owner's reference (Environment: host, user, repo, branch, pipeline; Context: % used; Results: count reported; Subagents: n running). Results card (owner note, 2026-11-12): URL items with Open and Copy URL (Open is disabled, with a note to forward the port, when the URL is on a remote host); file paths, shown as links that open in a Work panel tab; and artifacts, which open in a Work panel tab (X-AR-01). There is no Artifacts page. Source: pv96 (unmerged). pv96: checklist card added (NativeChatSessionChecklistSection.tsx); visibility setting nativeChatSummaryVisible. The rule for empty cards is not verified. |
| D-AG-28 | Conversation message minimap | Desktop | Vertical minimap on the right edge of the Conversation (D-AG-03): a scroll timeline of its messages. | Shown | New | Owner reference (2026-11-12) shows the minimap only. Its behaviour (for example, a click to jump, or a marker for the current message) is not in the reference and is not designed. Source: pv96 (unmerged). pv96: not read. |
| D-AG-29 | Prototype design system view | Desktop | Design system of a project: tokens, components, images and a selection panel; draft edits sent to the agent; version banner and archive. | Empty; loading; no design system; draft; versions | Keep | Source: pv96 (unmerged). Built in core in pv96 (components/prototypes/PrototypeDesignSystem*.tsx; odd/tasks/prototypes-design-system.md). Core, per open question 16 (item 12). Counterpart of M-PT-02. |
| D-AG-30 | Prototype merge review | Desktop | Merges a direction into the main prototype set: preview document, merge plan, progress and review. | Preview; merging; review; error | Keep | Source: pv96 (unmerged). components/prototypes/PrototypeMerge*.tsx and prototype-merge-preview-document.ts (odd/tasks/prototypes-section.md, MM1). Core, per open question 16 (item 12). |
| D-AG-31 | Prototype element inspector | Desktop | Picks an element of a prototype and edits its design values (token chips, colours, sizes, text) in an aside, with a Components tab. | Idle; element selected; draft edits | Keep | Source: pv96 (unmerged). components/prototypes/PrototypeEditPanel.tsx and PrototypeEditControls.tsx (odd/tasks/prototypes-design-system.md, E1-E4). |
| D-AG-32 | Prototype versions and restore | Desktop | Lists the versions of a prototype set and of the design system, and restores one. | Listed; restoring; error | Keep | Source: pv96 (unmerged). components/prototypes/PrototypeVersionsPanel.tsx, PrototypeDesignSystemVersionsPanel.tsx; components/version-history/VersionHistoryList.tsx and VersionRestoreDialog.tsx. |
| D-AG-33 | Bot form and advanced options | Desktop | Adds or edits a bot of a team: name, role description, agent, host and skills. Model and effort sit in Advanced options, hidden by default. | Empty; editing; saving; host picker | New | Source: pv96 (unmerged). components/bot-teams/AddBotDialog.tsx, BotAdvancedOptions.tsx, BotTeamHostPicker.tsx, BotTeamReassignButton.tsx (odd/tasks/bot-teams.md, S16-S19, owner relayed spec). The owner should validate the Advanced options default. |
| D-AG-34 | Bot team routine form | Desktop | Plain-language form for a routine: name, what to do, schedule (once, daily, weekdays, weekly, monthly), bot and host. | Empty; editing; saving | New | Source: pv96 (unmerged). components/bot-teams/BotTeamRoutineNew.tsx. Replaces the assumed use of the D-AG-15 editor (A26). Owner to confirm. |
| M-CH-01 | Chat view | Android | Agent transcript and composer, with loading, waiting and error states. | Loading; waiting; awaiting transcript; ready; error | Keep | Counterpart of D-AG-03. Composer and suggestions merge in. |
| M-CH-02 | Composer and suggestions | Android | Message input with suggestions. | - | Merge into M-CH-01 | Part of the chat view. |
| M-CH-03 | Permission and question prompts | Android | Approve or deny; answer single, multi-select or free text. | Submitting; sending | Merge into X-CH-02 | Shared cards. |
| M-CH-04 | Queued messages | Android | Messages waiting to send. | - | Merge into X-CH-03 | Shared pattern. |
| M-CH-05 | Plan sheet | Android | Shows the plan. | - | Merge into X-CH-05 | Shared view. Source: pv96 (unmerged). pv96: Plan in the overflow menu whenever the chat is shown; 'No plan yet' state (odd mobile-chat-summary-artifacts T4, T7). |
| M-CH-06 | Delegation card and why-sheet | Android | Delegated work and its explanation. | - | Merge into X-CH-06 | Shared card. |
| M-CH-07 | Chat summary | Android | Summary of a chat (name inferred from file name). | Not traced | Keep | Behaviour not verified; no clear job. Re-add only if a design pass finds one. Source: pv96 (unmerged). Re-opened from Drop. pv96 built (mobile/src/session/MobileChatSummaryView.tsx): agent status, environment tiles, results strip, connected chips. Owner to validate (odd mobile-chat-summary-artifacts T5). |
| M-SE-01 | New session | Android | Starts an agent session in a workspace, with choice, base ref and more sheets. | Progress; composer row | Redesign | One sheet instead of three. Progress view merges in. Counterpart of D-AG-01. Source: pv96 (unmerged). pv96 built: create screen with chips and sheets; FAB, header + and Home entries; old form under the overflow menu (odd mobile-create-session T1-T4). |
| M-SE-02 | Session screen | Android | Chat and terminal for one workspace, with header, command dock and pending bar. | Loading; waiting; disconnected; error; pending terminal (retry) | Keep | Header overflow and pending bar merge in. |
| M-SE-03 | Sessions switcher | Android | Switches between sessions in a workspace. | - | Keep | Counterpart of D-AG-22 (partly). Shows the X-IM-05 badge for imported sessions. Source: pv96 (unmerged). pv96: session switcher chip plus Sessions sheet (odd mobile-chat-summary-artifacts T2). |
| M-SE-04 | Session header overflow and pending bar | Android | Overflow menu and pending terminal bar. | - | Merge into M-SE-02 | Parts of the session screen. |
| M-SE-05 | Background work sheet | Android | Background child work of an agent. | - | Merge into X-AG-01 | Child rows of the status model. Source: pv96 (unmerged). pv96: Background sheet opened from the working indicator (odd mobile-chat-summary-artifacts T3). |
| M-SE-06 | Agent session history | Android | Past sessions of a workspace. | - | Merge into M-SE-03 | Counterpart of D-AG-07. Falls back to the web shell on OTA builds; see M-SH-03. Source: pv96 (unmerged). pv96: 'Resume an earlier session' in the Sessions sheet replaces Agent History (odd mobile-chat-summary-artifacts T2). |
| M-SE-07 | Working status pill | Android | Floating pill above the composer in structured chats: working state, elapsed clock, running terminal and agent counts (hidden at zero); tap opens the background sheet. | Working; idle (hidden) | New | Source: pv96 (unmerged). odd/tasks/mobile-chat-summary-artifacts.md, T11. |
| M-AR-01 | Chat artifacts list and viewer | Android | Files the agent wrote in a chat, and a preview. | Empty; loading | Merge into X-AR-01 | Shared artifacts pattern. Source: pv96 (unmerged). pv96: grouped grid and file list with filters; read-only viewer; live thumbnails (odd mobile-chat-summary-artifacts T6, T8, T9). |
| M-PT-01 | Prototypes: list, set, viewer, settings and sheets | Android | Lists prototypes, opens a set or viewer, drafts, pins and picks colours. | List; set; viewer; unopenable | Merge into D-AG-17 | Same concept as the desktop page. Source: pv96 (unmerged). pv96: entry from the overflow menu; set swipe, Edit and Note sheet, draft sheet, Send (mobile/app/h/[hostId]/prototypes). Closes G19 in app-flow.md. |
| M-EX-01 | Marketplace | Android | Catalog of tools, agents, skills and services, with item screens for Git, GitHub, Engram, OpenMetadata, your-apps and agents, and a credential form. | Loading; listed; item; credential form; error | New | Source: pv96 (unmerged). mobile/app/marketplace/*.tsx and mobile/src/marketplace/*.tsx. Entry is in Settings, not Home (A24 said none). Owner to confirm. |
| M-PT-02 | Prototype design system and merge | Android | Design system (tokens, components, images, versions) and merge sheets for prototypes on the phone. | Listed; sheet; merging; error | New | Source: pv96 (unmerged). mobile/src/prototypes/MobilePrototypeDesignSystem*.tsx and MobilePrototypeMerge*.tsx. Counterpart of D-AG-29 and D-AG-30. |

## 7. Session import from other agent tools (NEW)

Required by PLAN: work started in another agent tool continues here. Import runs on the host where the sessions live; Android sees the results through session history (M-SE-06).

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-IM-01 | Import entry | Desktop | Picks the agent tool to import from (Claude Code, Codex, OpenCode, Gemini CLI, others). | Detecting; found per tool; none; access needed | New | Detects installed tools' session stores and says what it found before reading anything. |
| D-IM-02 | Import selection | Desktop | Lists found sessions, lets the owner select them, and maps each to a workspace or project. | List; selected; unmapped; already imported | New | Maps by repository path where possible; unmapped sessions are asked once, in one batch. |
| D-IM-03 | Import progress and result | Desktop | Runs the import and shows what succeeded, what failed and why, and opens imported sessions. | Running; partial; failed; done | New | A partial import keeps what succeeded. Each failure is named. |
| D-IM-04 | Source connector setup | Desktop | Sets where each tool's data lives. Access is read-only. | Default path; custom path; not found | New | Read-only by default. Copying into MassOrca's store was accepted (open question 2, owner accepted default 2026-11-12). |
| X-IM-05 | Imported session marker | Both | Shows the origin tool and import date in a session. | Imported; continued here | New | Desktop: transcript header. Android: badge on the sessions switcher (M-SE-03). |

## 8. Terminal

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-TM-01 | Terminal tabs in the Work panel | Desktop | Tab groups and split panes in the Work panel (D-SH-17); drag to reorder, move or split. | Empty; split; dragging | Redesign | A pane tree. Absorbs split surfaces, tab group panel, drop overlays, tab context menus. Source: pv96 (unmerged). pv96: titlebar tabs removed (TerminalTitlebarTabs.tsx deleted); tabs live in the side panel (components/side-panel/SidePanelTabStrip.tsx). |
| D-TM-02 | Terminal pane | Desktop | One terminal or agent TUI, with header, paste, file drop and context menu. | Starting; restoring; running; exited; reconnecting | Keep | Absorbs the exit overlay with restart, restored banner, agent-launch notice and inline setup terminal. Counterpart of M-TM-01. |
| D-TM-03 | SSH reconnect overlay and remote runtime banner | Desktop | Shows that a remote terminal is reconnecting. | Reconnecting; restored; failed | Merge into X-CN-01 | Shared connection banner. |
| D-TM-04 | Terminal close confirms | Desktop | Confirms closing a pinned tab, a running pane, a terminal, or renaming a workspace. | - | Merge into X-DLG-01 | Keeps the "process still running" text. |
| D-TM-05 | Layout save failure dialog | Desktop | Reports a failed save of the terminal layout. | Failed | Merge into X-DLG-01 | Error variant of the confirm pattern. |
| D-TM-06 | Terminal error toast | Desktop | Reports a terminal error. | - | Merge into X-NT-01 | Toast. |
| D-TM-07 | Clipboard write from terminal | Desktop | Shows a notice when a terminal writes to the clipboard (OSC 52). | Shown | Redesign | Make it an explicit setting with a visible notice. No silent writes. |
| D-TM-08 | Terminal context menu | Desktop | Copy, paste, split, agent session actions. | - | Keep | Standard menu. |
| D-TM-09 | Quick commands editor and tab menu | Desktop | Saved shell commands; run from the tab bar. | List; editing | Merge into X-QC-01 | Shared quick-commands concept. |
| X-QC-01 | Quick commands | Both | Saved shell commands, with scope; runs them in the terminal. | List; empty; editing | Keep | Desktop: D-TM-09. Android: M-TM-06 (sheet, list, editor). |
| D-TM-10 | Terminal search bar | Desktop | Finds text in the active pane. | Open; matches; none | Merge into D-FD-01 | Shared find pattern. |
| D-TM-11 | Recent tab switcher | Desktop | Switches tabs in recent order (Ctrl+Tab). | Open | Keep | Fast switching. |
| D-TM-12 | Tab hover card | Desktop | Shows a preview when hovering a tab. | - | Drop | Low value; the tab name and the recent switcher cover it. |
| D-TM-13 | Floating terminal panel | Desktop | Floating workspace panel with toggle, maximise and minimise. | Hidden; shown; maximised; minimised; empty | Keep | Quick terminal. Setting in D-ST-03. Source: pv96 (unmerged). pv96: floating panel gains tabs Checklist, new session, host select and session picker (D-TM-19). |
| D-TM-14 | Floating orchestration dialog and card | Desktop | Orchestration setup from the floating panel. | - | Merge into X-DL-01 | Orchestration is in the first version (owner, 2026-11-12). The floating dialog is not kept: child agents show inside the session or the Conference room. |
| D-TM-15 | Pet overlay | Desktop | Optional experimental overlay. | - | Drop | No product task. |
| D-TM-16 | Native chat cover on terminal pane | Desktop | Covers a terminal pane when its agent runs as native chat. | - | Drop | A pane holds one kind of tab. Covering a pane is a workaround. |
| D-TM-17 | Legacy terminal surfaces (four files) | Desktop | Older workbench path kept beside the newer one. | - | Drop | Duplicate path. Not ported. |
| D-TM-18 | Driver overlay (terminal and browser) | Desktop | Shows that another device is driving a pane. | Driven; released | Keep | One overlay for terminal and browser. Absorbs the browser driver overlay. |
| D-TM-19 | Floating panel tabs | Desktop | Tabs of the floating workspace: Checklist of a session, new session (with host select), and a session picker to open an existing session. | Checklist; new session; picking a session; no checklist | Keep | Source: pv96 (unmerged). components/floating-terminal/checklist/*, create-session/*, host/*, session-picker/* (odd/tasks/floating-workspace-checklist.md). |
| M-TM-01 | Terminal panel | Android | Terminal rendered in a WebView, with live input status and an engine error state. | Loading; engine error; live | Redesign | Renderer choice (reuse the WebView terminal or go native) is a Phase 6 decision. Counterpart of D-TM-02. |
| M-TM-02 | Terminal key row | Android | Esc, Tab, arrows, Ctrl; hold for repeat. | - | Keep | Needed on touch keyboards. |
| M-TM-03 | Terminal composer dock | Android | Text input line for the terminal. | - | Merge into M-TM-01 | Part of the terminal screen. |
| M-TM-04 | Custom key modal | Android | Defines custom terminal keys. | - | Merge into M-TM-02 | Key row setting. |
| M-TM-05 | Terminal settings | Android | Terminal options and shortcuts. | - | Merge into M-ST-01 | Phone settings section. |
| M-TM-06 | Quick commands sheet, list and editor | Android | Saved shell commands; run or edit them from the session. | List; editing | Merge into X-QC-01 | Counterpart of D-TM-09. |

## 9. Editor and files

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-ED-01 | Editor tab | Desktop | Work panel tab for a file (D-SH-17): header with path, view toggle, save, find, word wrap. | Empty; loading; load error; dirty; saved | Keep | Standard editor frame. Source: pv96 (unmerged). pv96: recorded files open in their own tab with Mod+Alt+click (odd remote-session-recorded-file-read T4). |
| D-ED-02 | Code editor | Desktop | Text editing (Monaco), with gutter menu. | Editing; read-only; large file | Keep | Absorbs gutter and markdown-actions menus and the view toggle. |
| D-ED-03 | Markdown editor and preview | Desktop | Edits markdown with preview, table of contents, and diagram rendering. | Source; preview; too large (size gate) | Keep | The one markdown mode. Absorbs the table of contents, Mermaid viewer and size gate. |
| D-ED-04 | Rich markdown editor | Desktop | WYSIWYG editing with slash, emoji and doc-link menus. | - | Drop | Second markdown editing mode that duplicates D-ED-03. Reconsider after the first version. |
| D-ED-05 | New file dialogs | Desktop | Template choice for a new markdown file and naming an untitled file on save. | - | Merge into X-DLG-01 | Naming uses the one-field confirm variant. |
| D-ED-06 | External change banner and compare | Desktop | Shows a file changed on disk; reload or compare. | Clean; changed externally | Keep | Compare reuses D-DF-01. |
| D-ED-07 | Merge conflict review | Desktop | Resolves conflicts in a file. | Conflicted; resolved | Keep | Needed for the git workflow. |
| D-ED-08 | Diff surfaces | Desktop | Single-file diff, changes mode, combined diff of many files, large-diff prompt, skipped-conflicts notice. | Loading; large (prompt); conflicts skipped | Merge into D-DF-01 | Diff view owns all diffs. |
| D-ED-09 | Read-only viewers | Desktop | Views for images, audio and video, PDFs, paged CSV, recorded files, and image diffs. | Loading; too large (paged); unsupported | Redesign | One viewer host with type-specific views. PDF and media depend on the browser engine decision (Phase 0). |
| D-ED-10 | Notebook viewer and kernel setup | Desktop | Views .ipynb files and sets up a kernel. | Kernel missing | Extension | Owner accepted default 2026-11-12 (open question 4): an extension, not core. Kernel management is heavy for the first version. |
| D-ED-11 | File explorer (Project sidebar) | Desktop | Tree of workspace files, with search (Names or Content toggle, owner 2026-11-12); ignored files dimmed. | Loading; empty; filtered | Keep | Counterpart of M-FI-01. |
| D-ED-12 | Notes send menus | Desktop | Sends review notes from a file or diff to the agent. | - | Merge into X-RV-02 | Shared review-notes concept. |
| D-ED-13 | Team files | Desktop | Files of one team, opened from the team's Projects section (Files). | - | New | Owner reference (2026-11-12) shows the entry only. Likely design, assumed: the file tree of D-ED-11 and the editor of D-ED-01. Not designed yet. Source: pv96 (unmerged). pv96 built (components/bot-teams/BotTeamFiles*.tsx, BotTeamFileVersions.tsx): team folder plus linked project files, versions, previews. |
| M-FI-01 | File explorer | Android | Browses workspace files. | Loading; empty | Keep | Counterpart of D-ED-11. |
| M-FI-02 | File preview | Android | Read-only preview of text, image or HTML files. | Loading; error | Keep | Counterpart of D-ED-09. |

## 10. Diff, review and source control

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-DF-01 | Diff view | Desktop | Diff of one file, opened as a file tab in the Work panel (D-SH-17) with inline change markers; comment on lines. | Loading; large (prompt); conflicts skipped; empty | Redesign | Owner requirement (2026-11-12; open question 13 answered): a diff opens as a file tab in the Work panel, as a file does. Its toolbar holds rendered or source, split, previous and next change, edit, and more. Absorbs the editor diff surfaces (D-ED-08), compare and inline diff card. Keeps the change navigation keys. How all changes or many files are listed is not in the owner's reference and is not designed here. |
| X-RV-02 | Review notes | Both | Writes comments on diff lines or files and sends them to the agent. | Draft; sent; none | Keep | Desktop: diff comments and send menus (D-ED-12). Android: M-DF-01 line taps. |
| D-DF-02 | Source control panel | Desktop | Staged and unstaged changes, branch, commit composer, detached-HEAD badge. | Clean; changes; detached; conflicts | Keep | Counterpart of M-DF-02. Absorbs the agent commit action (D-DF-03). Owner reference (2026-11-12): a PR chip; the branch against its base (origin/main in the reference) with ahead and behind counts; a commit message box with a Commit button and a menu beside it (the menu items are not designed); a "Commits in branch" list with +/- counts and a status letter per file. A file click opens it as a file tab in the Work panel (D-DF-01). |
| D-DF-03 | Agent commit or PR action | Desktop | Runs a commit or PR action through an agent. | - | Merge into D-DF-02 | An action in the panel. |
| D-DF-04 | Checks | Desktop | CI check runs for a branch or PR, with run details. | Loading; none; failing; passing | Keep | Absorbs check run details and folder-workspace PR checks. |
| D-DF-05 | Pull request page | Desktop | Full PR view: details, assignees, viewed files, comments, link actions. | Not linked; linked; loading; error | Merge into X-PR-01 | Shared PR view. |
| X-PR-01 | Pull request view | Both | Shows and acts on the linked PR: details, comments, checks; links or creates a PR. | Not linked; linked; creating; error | Redesign | Desktop: page and panel (absorbs D-DF-05, widgets, link actions). Android: M-DF-03. |
| D-DF-06 | Base branch picker for a new PR | Desktop | Chooses the base branch. | - | Merge into X-PK-01 | Shared picker. |
| D-DF-07 | Git settings | Desktop | Git behaviour, commit-message AI, provider API budget, action recipes, repo AI section. | - | Merge into D-ST-06 | Settings content. |
| M-DF-01 | Diff review | Android | Reviews workspace changes; tap a line to comment. | Loading; empty; sheets | Keep | Counterpart of D-DF-01. Uses X-RV-02. |
| M-DF-02 | Source control hub | Android | Status, PR and history segments. | Loading; error; unavailable; conflict; empty | Keep | Counterpart of D-DF-02. |
| M-DF-03 | PR view on Android | Android | PR details, link form, comments, actions. | Empty; link form | Merge into X-PR-01 | Shared PR view. |
| M-DF-04 | Git history list | Android | Commit history. | Empty | Merge into M-DF-02 | Segment of the hub. |

## 11. Browser pane

Engine: GPUI has no webview, so a web engine is embedded for this pane only. Choosing it is a Phase 0 research item.

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-BP-01 | Browser pane | Desktop | Browser tabs in the Work panel (D-SH-17), with address bar, find, back, forward, reload. | Loading; error; client-hosted page | Keep | Absorbs the toolbar overflow menu, document preview (with its external-link confirm, X-DLG-01), and the reopen-on-server button. |
| D-BP-02 | Grab and annotate | Desktop | Grabs a page element, annotates it, and sends it to the agent. | Grab; confirm; markup | Keep | Useful for agents. Confirm sheet kept. |
| D-BP-03 | Cookie import | Desktop | Imports cookies from other browsers so sign-ins carry over. | Disclosure; machine notice; per profile | Redesign | Explicit, per profile, opt-in, off by default. Absorbs the disclosure, machine notice and session cookies section. |
| D-BP-04 | Browser profiles | Desktop | Separate profiles with their own sign-ins. | List; new | Merge into D-ST-05 | Settings content. |
| D-BP-05 | Agent browser access | Desktop | Lets an agent drive the browser: enable switch, skill setup, examples, computer-use notice. | Off; on; setup needed | Redesign | Per-agent permission. Lives in D-ST-02 with the agent settings. |
| D-BP-06 | WebAuthn account picker | Desktop | Picks the account for a passkey prompt inside the browser pane. | Prompting | Keep | Needed for sign-in flows; depends on the engine. |
| D-BP-07 | Emulator pane | Desktop | Mobile emulator tab. | - | Merge into D-SH-18 | Reversed by the owner (2026-11-12): the mobile emulator is a Work panel tab, D-SH-18. Not dropped. |
| D-BP-08 | Link routing preference | Desktop | Chooses where links open (browser, editor, other). | - | Merge into D-ST-05 | Settings content. |

## 12. Command palette and search

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-CP-01 | Command palette | Desktop | One palette to jump to workspaces, tabs, project actions and commands. | Empty; filtered; no match | Redesign | The single command entry point. Extensions register commands here (D-SL-07). Absorbs the filter menu and browser and simulator rows. |
| D-CP-02 | Quick open (go to file) | Desktop | Finds a file in the active workspace. | Empty; results; ripgrep missing | Keep | Shows install guidance when ripgrep is missing. |
| D-CP-03 | Search in files | Desktop | Text search across workspace files. | Searching; results; none | Keep | Content mode of the File explorer in the Project sidebar (D-ED-11). |
| D-FD-01 | Find bar | Desktop | Find in terminal, chat, editor, browser and settings. | Closed; open; matches; none | Keep | One pattern; scope per context. Mod+F means different things per context, which the find bar makes clear. |
| D-CP-04 | Settings search | Desktop | Finds settings and jumps to the matching section. | Results; no match | Keep | Highlights the target briefly. |
| D-CP-05 | Palette tip dialog | Desktop | Tip that introduces the palette. | - | Drop | Tips are removed (D-OB-13). |

## 13. Notifications and toasts

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-NT-01 | Notification card stack | Desktop | Stacked cards for agent attention and app updates. | Per card; dismissed | Redesign | One stack. Account sign-in cards use X-AC-01. Star request removed (D-OB-15). |
| D-NT-02 | Profile save delay notice | Desktop | Says that saving profile state is slow. | Shown | Merge into X-NT-01 | Toast. |
| D-NT-03 | Desktop notification settings | Desktop | Per-source notification toggles and sounds. | - | Merge into D-ST-08 | Settings content. |
| X-NT-01 | Toast | Both | Short status messages with a close button, optional action. | Info; success; error; action | Keep | One host. Absorbs setup script toast, local base ref refresh, runtime disconnect (X-CN-01), auto-rename failure (with retry). |
| X-NT-02 | OS notifications | Both | Native notification for agent attention. | On; off; per source | Keep | Decision logic is in the attention module; the UI is the OS notification plus the settings in D-ST-08. |
| X-UP-01 | App update notice | Both | Says an update is available, checking, downloading, ready or failed; installs it. | Available; checking; downloading; ready; error | Keep | Desktop: update card in D-NT-01. Android: M-HM-01 card and M-ST-01 section. |
| M-NT-01 | Notification settings | Android | Enables agent notifications, delivery and a test. | Permission off; on | Keep | Counterpart of D-ST-08. Absorbs the delivery section and the onboarding preview. |

## 14. Settings

The 38 desktop settings panes (plus per-repository sections) regroup into 10 sections. Project settings leave the list and move to the project's own menu. Android settings are in section 16.

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-ST-01 | General and account | Desktop | Account, updates and release channel, workspace and editor defaults, setup checklist, support. | Loading; update status | Redesign | Absorbs MassOrca account, setup guide, General and the feedback dialog. |
| D-ST-02 | Agents and accounts | Desktop | Default agent and launch, agent availability, agent accounts, usage, computer use, browser access, keep-awake, session search, delegation depth limit. | Per agent: detected; missing; signed out | Redesign | Lists D-AC-01 and D-AC-06 as content. Absorbs Agents, Accounts, Computer Use, Stats and Usage, Agent Session Search, and the Orchestration pane (its depth limit; the engine is X-DL-01). Source: pv96 (unmerged). pv96: settings navigation still uses the old taxonomy; Bot Teams settings pane (BotTeamsSettingsPane.tsx) and Jev decisions section (D-ST-12) added. |
| D-ST-03 | Appearance and layout | Desktop | Theme, fonts, chat look, window and sidebar, status-bar items, floating panel, zoom. | - | Redesign | Theme import (Warp, Ghostty) lives here. Absorbs Appearance, Chat, Floating Workspace. Light and dark ship together (owner, 2026-11-12); the rail's theme toggle (D-SH-14) switches between them. |
| D-ST-04 | Input and shortcuts | Desktop | Editing defaults, keybinding editor with conflicts, quick commands. | Recording; conflict; filtered | Redesign | Keybinding editor is the shortcut customisation point (D-CU-02). |
| D-ST-05 | Terminal and browser | Desktop | Terminal fonts, theme, scrollback, shell, rendering; browser home, search, zoom, profiles, link routing. | - | Redesign | Absorbs Terminal and Browser. Quick Commands stays in D-ST-04. |
| D-ST-06 | Sources and integrations | Desktop | Git providers, task providers, Linear agent skill, git behaviour and API budget. | Connected; not connected; error | Redesign | Absorbs Integrations, Linear, Git, Task Sources. Source: pv96 (unmerged). pv96: IntegrationsPane.tsx deleted; BotTeamsIntegrations.tsx added. Replacement not read. |
| D-ST-07 | Hosts and phones | Desktop | SSH hosts, servers, paired phones, pairing. | Per host state | Redesign | Absorbs SSH Hosts, Servers and Mobile. |
| D-ST-08 | Notifications and voice | Desktop | Per-source notifications and sounds, dictation, speech model, transcription key. | Model states | Redesign | Absorbs Notifications and Voice. |
| D-ST-09 | Extensions | Desktop | A link row to Marketplace > Installed, where installed extensions, sources and skills are managed (owner, 2026-11-13, A68). | Enabled; disabled; update available | Redesign | Absorbs Plugins. Replaces the Share Skills pane (dropped, D-EX-07). Source: pv96 (unmerged). pv96: Share Skills still listed in settings navigation (share-skills), although D-EX-07 is Drop. |
| D-ST-10 | Privacy and advanced | Desktop | Telemetry, diagnostic export, network, cache timers, OS permissions, CLI registration, experimental flags, developer tools. | - | Redesign | Absorbs Privacy, Developer Permissions, Advanced, Dev Tools, Experimental. Experimental cloud VMs dropped (D-OT-07). |
| D-ST-11 | Project settings | Desktop | Per-project identity, icon, hooks, worktree defaults, runtime, git author, GitHub account, AI actions, remove project. | - | Redesign | Moves out of the list into the project's menu. One collapsed section per project is hard to find. |
| D-ST-12 | Jev decisions | Desktop | Settings section for Jev decisions (the routing and model suggestion helper used by Bot Teams). | Not read | Redesign | Source: pv96 (unmerged). components/settings/JevDecisionsSection.tsx; runtime/runtime-jev-decisions-client.ts. Labels not read; the owner has not designed this. |

**Mapping of the 38 panes (old nav target to new section).**

| Old pane | New home | Note |
|---|---|---|
| Agents | D-ST-02 | |
| Accounts | D-ST-02 | Content is D-AC-01. |
| Orchestration | D-ST-02 (depth limit) | Merged into the shared delegation design X-DL-01 (owner, 2026-11-12; open question 7). |
| Linear (agent skill) | D-ST-06 | |
| Computer Use | D-ST-02 | |
| Voice | D-ST-08 | |
| MassOrca Account | D-ST-01 | |
| Setup Guide | D-ST-01 | Same checklist as D-OB-12. |
| General | D-ST-01 | |
| Integrations | D-ST-06 | |
| Mobile | D-ST-07 | Pairing content is D-PR-01. |
| Appearance | D-ST-03 | |
| Chat | D-ST-03 | |
| Input and Editing | D-ST-04 | |
| Notifications | D-ST-08 | |
| Shortcuts | D-ST-04 | |
| Stats and Usage | D-ST-02 | Content is D-AC-06. |
| SSH Hosts | D-ST-07 | |
| Servers (runtime environments) | D-ST-07 | Content is D-HO-07. |
| Developer Permissions | D-ST-10 | |
| Privacy | D-ST-10 | |
| Advanced | D-ST-10 | |
| Dev Tools | D-ST-10 | Behind the advanced toggle. |
| Experimental | D-ST-10 | |
| Plugins | D-ST-09 | |
| Git | D-ST-06 | |
| Task Sources | D-ST-06 | |
| Terminal | D-ST-05 | |
| Quick Commands | D-ST-04 | |
| Browser | D-ST-05 | |
| Floating Workspace | D-ST-03 | |
| Mobile Emulator | D-SH-18 | Work panel tab (owner, 2026-11-12); see D-BP-07. |
| Automations | D-AG-14 | Settings move into the page. |
| Artifacts | X-AR-01 | Settings move into the page. |
| Share Skills | Dropped | D-EX-07. |
| Prototypes | D-AG-17 | Settings move into the page. |
| Bot Teams | D-AG-13 | Core mode again (owner, 2026-11-12). Reached from the rail (D-SH-10). |
| Agent Session Search | D-AG-07 | |
| Per-repository sections (group "repositories") | D-ST-11 | Leaves the list. |

---

## 15. Customisation (NEW)

Principle from PLAN: users adjust the product and build their own applications inside it. Extension points are part of the core from Phase 1. Existing precedents (Project sidebar plugin tabs, status-bar items, plugin commands with shortcuts, plugin marketplace) are noted where they exist. Extensions on Android are not designed here.

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-EX-01 | Extension browser | Desktop | Browses, searches and reads about extensions and skills from their sources. | Loading; empty; error; per source | Redesign | Absorbs the plugin marketplace browser and preview, sources dialog, Skills page, sources popover, and the freshness nudge. Source: pv96 (unmerged). pv96: Marketplace (rail page) is a catalog of tools, agents, skills and services with install, uninstall and host picker (components/marketplace/*.tsx). Not only extensions; item pages are D-EX-08. |
| D-EX-02 | Install and permission review | Desktop | Shows what an extension asks for before install; installs; rolls back. | Reviewing; installing; installed; failed; rollback | Redesign | One review screen with a risk notice. Absorbs the plugin install, consent and rollback dialogs and the skill install, bundle, detail and update dialogs. |
| D-EX-03 | Extension manager | Desktop | Enables, disables, configures, updates, removes and rolls back installed extensions. | Enabled; disabled; error; update available | New | No single place today. Removal and rollback use X-DLG-01. |
| D-EX-04 | Extension settings sections | Desktop | Each extension can add its own section to settings. | Shown; hidden | New | Uses slot D-SL-06. |
| D-EX-05 | Create an extension | Desktop | Starts a new extension from a template, loads it, reloads it, shows its logs. | New; loaded; error | Redesign | Current development section, extended with a template and reload. |
| D-EX-06 | In-app application | Desktop | A user-built application that runs inside a view or panel. | Running; error | New | Built from slots D-SL-01, D-SL-04 and D-SL-05. Requested by the owner. |
| D-EX-07 | Share skills | Desktop | Shares a skill through links. | - | Drop | Sharing needs a service and is not in the first version. |
| D-EX-08 | Marketplace item page | Desktop | One page for each catalog row: host picker, connect or install on the picked host, who can use it, advanced options, and an uninstall or disconnect action. | Loading; not installed; installed; connected; error | Redesign | Source: pv96 (unmerged). components/marketplace/MarketplaceAppAccess*.tsx, MarketplaceAppAdvanced.tsx, MarketplaceAppConnection.tsx, MarketplaceAppDanger.tsx, MarketplaceAgentPage.tsx, MarketplaceBoardToolPage.tsx (odd/tasks/marketplace-parity-audit.md, S1-S9). Each action has an orca CLI equivalent. |
| D-SL-01 | Slot: Project sidebar tab | Desktop | Extension panel as a Project sidebar tab. | Open; closed | Keep | Precedent exists (plugin tabs). |
| D-SL-02 | Slot: status bar item | Desktop | Small extension item on the status bar. | Shown; hidden | Keep | Precedent exists (status-bar items). |
| D-SL-03 | Slot: chat card | Desktop | Extension card inside a chat transcript. | Shown | New | Extension chat cards. Prototype link cards stay core (D-AG-18). Precedent: native chat cards. |
| D-SL-04 | Slot: main view | Desktop | Full page view added by an extension. | Open; error | New | Hosts in-app applications. Prototypes are core (D-AG-17). |
| D-SL-05 | Slot: sidebar section | Desktop | Extension section under the workspace tree. | Shown; hidden | New | Hosts extension lists such as boards. |
| D-SL-06 | Slot: settings section | Desktop | Extension settings section (see D-EX-04). | Shown | New | Part of the settings model. |
| D-SL-07 | Slot: command and shortcut | Desktop | Extension commands in the palette, with default shortcuts. | Registered; conflict | Keep | Precedent exists (plugin commands add shortcuts). Conflicts shown in D-ST-04. |
| D-SL-08 | Slot: Work panel tab kind | Desktop | Extension tab type in the Work panel. | Open; closed | New | Lowest priority; decide during Phase 5. |
| D-SL-09 | Slot: activity rail icon | Desktop | Extension icon in the activity rail (D-SH-09). Opens an extension's main view or panel. | Shown; hidden | New | Owner requirement (2026-11-12): extensions may contribute rail icons. Which rail group an icon joins is for the design pass. Source: pv96 (unmerged). pv96: not built. AppRail.tsx has no extension slot. |
| D-SL-10 | Slot: model picker entry | Desktop | Extension adds agents and models to the model picker's agent rail and list (X-AG-02). | Shown; hidden | New | Owner requirement (2026-11-12): extensions that add agents or models contribute entries to the rail and list. Android: not designed. |
| D-CU-01 | Theme and layout presets | Desktop | Saves, switches and shares theme and pane layouts. | Saved; active | New | Customisation without code. |
| D-CU-02 | Keybinding preferences | Desktop | Keybinding editor, conflicts, extension shortcuts. | - | Merge into D-ST-04 | Same editor. |
| D-CU-03 | Theme import | Desktop | Imports terminal and app themes (Warp, Ghostty YAML). | Preview; imported; error | Merge into D-ST-03 | Same settings area. |

---

## 16. Mobile-specific

Android rows with no desktop counterpart, plus the mobile home and settings. Other Android rows sit under their topic above.

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| M-HM-01 | Home | Android | Paired hosts plus resume, tasks and app-update cards. | Empty; host states | Redesign | Cards become one list. Absorbs the resume card (M-HM-03), tasks card (M-HM-04) and top bar. Top bar and overflow take the rail entries (proposal; see D-SH-09 and its entries). Source: pv96 (unmerged). pv96: phone top bar has the Bot teams toggle and Tasks (host-screen-phone-top-bar.tsx); Marketplace is in the Settings menu. |
| M-HM-02 | Host card | Android | One card per paired host, with a status badge. | Ok; warning; unreachable; auth failed; credential missing | Keep | Uses X-CN-01 states. Carries the host's agent usage rings (proposal, D-SH-15); detail is M-AC-01. |
| M-HM-03 | Resume card | Android | Resumes the last session. | - | Merge into M-HM-01 | Card in the home list. |
| M-HM-04 | Tasks card | Android | Summary of tasks. | - | Merge into M-HM-01 | Card in the home list. |
| M-AC-02 | Home usage cards | Android | Agent usage summary on home. | - | Merge into M-HM-02 | Usage moves onto the host card (proposal, D-SH-15). Detail is M-AC-01. |
| M-ST-01 | Settings menu | Android | Top settings list: chat, terminal, browser, voice, notifications, troubleshooting, about, support, privacy, phone browser. | Pending credential cleanup notice; app update section | Redesign | Group into the desktop sections (D-ST-01 to D-ST-10). Pending credential cleanup becomes a notice at the top. Source: pv96 (unmerged). pv96: Marketplace added after Notifications (mobile/src/settings/mobile-settings-menu-items.ts). |
| M-ST-02 | Voice model | Android | Chooses the speech model for dictation. | Model list | Keep | Counterpart of D-ST-08. |
| M-ST-03 | Dictation setup sheet | Android | Setup for dictation (name inferred from file name). | Not traced | Merge into M-ST-02 | Same job. |
| M-ST-04 | Chat, browser and about settings | Android | Chat UI, browser choice, about and version. | - | Merge into M-ST-01 | Settings rows. |

---

## 17. Other dialogs and shared patterns

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| X-DLG-01 | Confirm dialog | Both | Confirms an action; the destructive variant lists what is lost. | Default; destructive; busy; error | Keep | Replaces about 20 confirm dialogs: pinned tab and running pane close, queued send, skip onboarding, remove folder, project group delete, host rename and remove, forget SSH workspace, automation delete, account removal, profile sign-out, kill session. Delete workspace (D-WS-16) keeps its own. |
| X-DLG-02 | Sheet, drawer and popover container | Both | Container for secondary content: popovers and menus on desktop, bottom sheets and drawers on Android. | Open; closed | Keep | Replaces the desktop popovers and the Android action sheet, drawer and picker modal. |
| X-PK-01 | Picker | Both | Chooses a branch, base ref, folder, parent workspace, repo source, or account. | Loading; empty; error; filtered | Redesign | One picker for the composer, PR base, remote folders and Android sub-pickers. Absorbs base ref and parent pickers, set-location, hosted-review base picker. |
| D-OT-01 | Crash report | Desktop | Shows crash details and offers to send a report. | Shown; sent; dismissed | Redesign | Preview what would be sent. Nothing is sent without consent. |
| D-OT-02 | macOS permission prompts | Desktop | Guides folder access and full disk access on macOS. | Needed; granted | Merge into D-ST-10 | Settings content with a prompt. |
| D-OT-03 | Linux package install recovery | Desktop | Recovery steps after a failed Linux package install. | Failed; recovered | Keep | Needed on Linux. |
| D-OT-04 | Unsealed credential notice | Desktop | Warns that credentials cannot be stored securely. | Shown | Keep | Security notice; stays until fixed. |
| D-OT-05 | Dictation overlay | Desktop | Shows while dictation is active. | Idle; listening; processing | Keep | Shortcut Mod+E. |
| D-OT-06 | Keep-awake card | Desktop | Keeps the machine awake while agents run. | On; off | Merge into D-ST-02 | Setting. |
| D-OT-07 | Ephemeral VM dialogs and setup guide | Desktop | Cloud VM runtimes (experimental). | - | Drop | Experimental; not in the plan. |
| D-OT-08 | API key entry dialogs | Desktop | Enters a transcription, Linear or Jira key. | Entering; saved | Merge into D-ST-08 | Key entry belongs in settings. Keys are never pasted into notes. |
| D-OT-09 | Skill usage example dialog | Desktop | Example usage of a skill. | - | Drop | Documentation, not a product screen. |
| D-OT-10 | Icon and emoji pickers | Desktop | Chooses a workspace or project icon or emoji. | - | Merge into D-WS-04 | Part of workspace edit. |
| D-OT-11 | CLI registration dialog | Desktop | Registers the CLI on the machine. | - | Merge into D-ST-10 | Setting. |
| D-OT-12 | Codex login link notice | Desktop | Shows a Codex sign-in link. | Shown | Merge into X-AC-01 | Sign-in card. |
| D-OT-13 | Theme import dialogs | Desktop | Warp and Ghostty theme import. | Preview; imported | Merge into D-ST-03 | Setting. |
| D-OT-14 | Profile sign-out confirm | Desktop | Confirms signing out of a profile. | - | Merge into X-DLG-01 | Generic confirm. |

---

## 18. Desktop to Android pairing

| Area | Desktop screens with an Android counterpart | Desktop-only |
|---|---|---|
| Pairing | D-PR-01 with M-PA-02 and M-PA-04 | D-PR-02 paired devices, D-HO-06 passphrase |
| Hosts | D-HO-03 with M-HO-01; D-HO-01 with M-HM-02 | D-HO-02, D-HO-07, D-HO-09 |
| Accounts | D-AC-06 with M-AC-01; X-AC-01 (both) | D-AC-01 to D-AC-05 (migration and management) |
| Workspaces | D-WS-01 with M-WS-01; D-WS-02 with M-WS-02; D-WS-14 with M-WS-04; D-WS-24 with M-TK-01 | D-WS-03 to D-WS-13, D-WS-15 to D-WS-23, D-WS-25 to D-WS-30 |
| Agents and sessions | D-AG-03 with M-CH-01; D-AG-07 with M-SE-06; D-AG-06 with M-HM-03; D-AG-01 with M-SE-01; D-AG-17 with M-PT-01; X-AG-01, X-AG-02, X-CH-02, X-CH-03, X-CH-05, X-CH-06, X-DL-01, X-AR-01 (both; X-AG-02 sheet form open; X-DL-01 tree open) | D-AG-05, D-AG-13, D-AG-14 to D-AG-16, D-AG-23 (calendar; Android open), D-AG-24 to D-AG-26 (Bot Teams), D-AG-27 and D-AG-28 (Session summary, minimap) |
| Import | X-IM-05 (both, Android badge only) | D-IM-01 to D-IM-04 (host-side only) |
| Terminal | D-TM-02 with M-TM-01; X-QC-01 with M-TM-06 | D-TM-01, D-TM-08, D-TM-11, D-TM-13, D-TM-18 |
| Editor and files | D-ED-11 with M-FI-01; D-ED-09 with M-FI-02 | D-ED-01 to D-ED-08, D-ED-10, D-ED-13 |
| Diff and source control | D-DF-01 with M-DF-01; D-DF-02 with M-DF-02; X-PR-01 and X-RV-02 (both) | D-DF-04, D-DF-06, D-DF-07 |
| Notifications and settings | D-ST-08 with M-NT-01; D-ST-01 with M-ST-01; X-NT-02, X-UP-01 (both) | D-NT-01, D-ST-02 to D-ST-07, D-ST-09 to D-ST-11 |
| Secret vault | X-CH-07 and X-VT-01 (both); D-VT-01 with M-VT-01 (proposal) | D-VT-02 (merged into X-DLG-01) |
| Activity rail | D-SH-10, D-SH-14 and D-SH-16 in the Home top bar; D-SH-12 in the overflow; D-SH-15 on the host card (proposals); D-SH-11 with M-TK-01 | D-SH-09 (the rail itself), D-SH-13 |
| Browser, command palette, customisation | None | D-BP-*, D-CP-*, D-EX-*, D-SL-*, D-CU-*, D-SH-17, D-SH-18 |

Android has no equivalent of the workspace shell, command palette, browser pane, extension surfaces, or the desktop-only settings. The activity rail has no Android equivalent; its entries land on Home as the row above says. Those stay desktop-only unless a later design pass says otherwise.

---

## 19. Secret vault (NEW)

Owner requirement (2026-11-12): a secret vault in the core, so the owner shares credentials (API keys, tokens, usernames, passwords) with agents without pasting them into the composer and without storing them outside the app. Sharing creates a link, never a copy. The owner's note calls its design rules proposed and marked one of them for confirmation (open question 10, answered 2026-11-12: the same variable name in every scope). The request card X-CH-07 sits in section 6 with the other chat cards.

| ID | Name | Platform | Purpose | Major states | Proposal | Rationale |
|---|---|---|---|---|---|---|
| D-VT-01 | Secret vault screen | Desktop | Lists the secrets of each scope (project, team, global) with the owning scope and any linked scopes. Edit, rotate, delete and unlink. Shows which agents and sessions used each secret. | Empty; loading; listed; host unreachable; error | New | Owner requirement (2026-11-12): list, edit, rotate and delete secrets, and see which agents and sessions used each one. A linked secret is read-only where linked: edit and delete happen only at the owning scope, and unlink is allowed locally (A34). Rotate sets one new value in the owning scope, and every linked scope uses it from its next run (A30). Deleting at the owner removes the secret everywhere (D-VT-02). Usage shows names and times, never values (A31). Empty state: no secrets yet, with Add (X-VT-01). Entry point: the Settings view (open question 11, owner accepted default 2026-11-12; A27). The old 'vault' tab in the Project sidebar is a different screen (D-SH-04). |
| X-VT-01 | Secret entry modal | Both | Enters a variable name and a value, picks the scope or links an existing secret, and saves. Also opens for edit and rotate from D-VT-01. | Entering; invalid name; name in use; saving; stored; host unreachable; failed | New | Owner requirement (2026-11-12). Fields: variable name; value, masked, with a reveal toggle used on request; scope picker with project (preselected, the default), team (Bot Teams) and global. Link an existing secret replaces the new value and lists the owner's secrets held in other scopes (A28). Validation: the name rules and a name already in use in that scope (A29). Save stores the value on the executing host and is refused while the host is unreachable (A32). Edit keeps the name fixed; rotate changes only the value. Android: the same modal works on the phone. Security: the value is never echoed in the chat, the model context or logs, and the field is cleared when the modal closes. |
| D-VT-02 | Delete secret confirm | Desktop | Confirms deleting a secret at its owning scope. A shared secret names the number of linked projects and teams that lose it. | Default; shared (with linked count); busy; error | Merge into X-DLG-01 | Destructive variant of X-DLG-01, with the linked count as the list of what is lost. Owner requirement (2026-11-12): deleting at the owner removes the secret for every linked scope. Unlink asks nothing (A34). |
| M-VT-01 | Secret vault list | Android | Lists secrets with their scope and whether they are linked, and adds one through X-VT-01. | Empty; listed; host unreachable | New | Owner accepted default 2026-11-12 (open question 12): the phone shows the list and adds a secret through X-VT-01. Edit, rotate, delete and unlink stay on desktop (A36). |

---

## Open questions for the owner

1. Should the owner's agent accounts migrate by copying the stored credentials from the current app, or should each provider be signed in again after migration? **Answered (owner accepted default 2026-11-12):** copy the stored credentials and check each one on import (A1).
2. Should sessions imported from other agent tools be copied into MassOrca's store, or read live from each tool's own files? **Answered (owner accepted default 2026-11-12):** copy them into MassOrca's store; the source is only read (A2).
3. Should the Android app keep the OTA web shell, which lets some screens update outside the store, or should every screen be native? **Answered (owner accepted default 2026-11-12):** every Android screen is native; the OTA web shell is not kept (A3).
4. Which of these stay in the core product rather than becoming extensions: the workspace board (D-WS-08), GitHub Projects (D-WS-26), prototypes (D-AG-17), and the notebook viewer (D-ED-10)? Bot teams (D-AG-13) is core since the owner's decision of 2026-11-12. **Answered (owner accepted default 2026-11-12):** the workspace board, GitHub Projects, prototypes and the notebook viewer are extensions, not core (A4). Bot Teams stays core. Prototypes later became core (open question 16).
5. Is pairing through a relay part of the first version, or are only local network and SSH needed? **Answered (owner accepted default 2026-11-12):** relay pairing is not in the first version; only local network and SSH (D-PR-04 dropped).
6. Where do Bot Teams tasks come from? **Answered (owner, 2026-11-12):** from the Conference room (D-AG-24). A requirement the user states to the team's bots there becomes a task on the team's Board (D-WS-30). A composite requirement goes to the lead bot, which delegates the parts by competence (X-DL-01).
7. Does orchestration (the Orchestration settings pane and D-TM-14) move into Bot Teams (D-AG-13), or stay dropped? **Answered (owner, 2026-11-12):** orchestration is in the first version. The Orchestration pane and D-TM-14 merge into one shared delegation design (X-DL-01), used by Bot Teams and by Workspaces. The depth limit is a setting in the agents section (D-ST-02).
8. Are the Workspaces automations (D-AG-14) and the Bot Teams routines (D-AG-25) one list? **Answered (owner, 2026-11-12):** one core scheduling engine, two presentations. A routine is the simplified Bot Teams form of an automation: Bot Teams is for non-technical users, Workspaces for software engineers. A team can turn on Workspaces in a team to get Workspaces depth. The calendar (D-AG-23) shows both on one timeline with a mode filter.
9. Should the old right-sidebar 'vault' tab, which is titled 'Agents' and lists agent session history, keep a row of its own? It is not the secret vault (D-VT-01), although the owner's note calls it 'Vault'. **Answered (owner, 2026-11-12):** merged into the Sessions list of the Project sidebar (D-SH-04); it keeps no row of its own.
10. The owner's note marks one rule for confirmation: may a linked scope use a different variable name for the same secret? Until the owner says yes, the name is the same in every scope (A29). **Answered (owner accepted default 2026-11-12):** no. A linked secret keeps the same variable name in every scope (A29).
11. Where does the secret vault screen open from? The rail has no vault entry. Assumed: the settings view (A27). **Answered (owner accepted default 2026-11-12):** the vault opens from the Settings view, as its own section (A27).
12. Should Android get the secret vault list and add (M-VT-01) in the first version? Proposed: read and add only, with edit, rotate, delete and unlink on desktop (A36). **Answered (owner accepted default 2026-11-12):** Android gets the list and add only; edit, rotate, delete and unlink stay on desktop (A36).
13. Where do the diff view (D-DF-01) and the top-level pages open (Tasks, Calendar, Automations, Artifacts, Settings, and the Bot Teams pages)? The owner's Work panel tabs are Terminal, Browser, Markdown or file and Mobile emulator, so none of these has a place yet (G26 in app-flow.md). **Answered (owner, 2026-11-12):** a diff opens as a file tab in the Work panel. Tasks, Calendar, Marketplace, Settings, the Bot Teams pages and Automations open as full pages over the canvas, with the Sidebar hidden and Back returning to the session (G26 in app-flow.md). Artifacts is an owner decision (2026-11-12): items of the Results card, opened in a Work panel tab (X-AR-01).
14. Keep the title bar in the frame? **Answered on the owner's behalf, 2026-11-12** (see pending-owner-review.md, item 2): the title bar stays in the frame (D-SH-03), as a band with the contents pv96 moved into its session bar. Titlebar tabs stay dropped. The owner can change this.
15. Mode switch: one click, or a return to the mode's home first from other pages? **Answered on the owner's behalf, 2026-11-12** (see pending-owner-review.md, item 6): from any other page the first press returns to the current mode's home; a press at home switches the mode (D-SH-10). The owner can change this.
16. Prototypes: extension or core? **Answered on the owner's behalf, 2026-11-12** (see pending-owner-review.md, item 1): prototypes is core, with its entry in the Workspaces Sidebar (D-WS-01). This supersedes the default in A4 (app-flow.md). The workspace board, GitHub Projects and the notebook viewer stay extensions.

---

## Not covered

- The Ports and Workspaces tabs of the current right sidebar were named in the notes but not read. The owner's Project sidebar list (2026-11-12) does not name them, so they have no row. The old 'vault' tab was read in the current app: it is titled 'Agents' and lists agent session history, not secrets. It is merged into the Sessions list (D-SH-04; open question 9, answered).
- Most desktop states marked "not read" in the explorer notes are named from file names; their states are not verified.
- Mobile items the mobile notes marked "name-inferred" or "not traced" (chat summary, dictation sheet, tasks and resume card states, connection log) are unverified.
- The shortcut editor's escape-confirm behaviour and the palette's keyboard behaviour were not read.
- The browser engine, extension language and sandbox, and distribution model are Phase 0 research items; this inventory does not decide them.
- The reference source was not opened, and `current-style.md` was only skimmed, so visual style is not part of this inventory.
- Platform-specific shortcut defaults (macOS and Windows differences) were not checked against the keybinding source.
- The activity rail's and the model picker's Android placements are proposals, not designed. The Bot Teams reference shows only the entries for team Files, team Memory and the New web address form, so their content is not designed (D-ED-13, D-AG-26, D-AG-25). The floating panel toggle at the bottom right of the Bot Teams canvas has no row yet.
- pv96 (unmerged, commit d3c6311a531) was not read for: onboarding, agent accounts migration, session import, hosts and pairing, extension manager and install review, diff view and source control details, Checks, the message minimap, the rule for empty Session summary cards, the Settings regroup, and mobile pairing and terminal. The secret vault (section 19) is not in pv96.

## Consistency pass (2026-11-13)

Every screen was checked against [agreements.md](agreements.md); the readings applied are in pending-owner-review.md, "Consistency pass". Inventory changes from that pass:
- D-EX-01 Extension browser: the Marketplace catalogue now carries the Installed page navigation (Discover, Installed, Updates, Categories) and the "Add extension" split button; the extension manager is Marketplace > Installed.
- D-IM-02 Import selection: the table column is "Agent" and shows only the agent icon; D-IM-03 drops the background banner for one quiet line.
- D-OB-03 theme step: "Match system" replaces "Match Windows".
- M-WS-01 is absorbed by M-PJ-01 Projects (new): project headers, host labels, agent icons, nested child sessions; Home gains the Projects row; M-WS-04 is superseded by the New session screen (M-SE-01).
- Android Conference room (Android form of D-AG-24): its "…" sheet holds Board, Routines, Calendar, Memory, Files and Team settings (new phone in an-botteams).
- Android Prototypes (new): M-PT-01 Prototypes by project, M-PT-02 canvas and Play, M-PT-03 design system tab, section and linked design system (an-prototypes).
- Desktop and Android Sidebars and lists: grouped by project with host rows (desktop) or host labels (Android), agent icons on every session row, child sessions nested by indent.
