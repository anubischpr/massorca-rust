# Android screens

Screen specs for the React Native Android app, written 2026-11-12 while the owner was away. English. Follows [glossary.md](../glossary.md), [design-system.md](../design-system.md) (section 5.4 Android layout, 8.6 TalkBack, 9 differences), the rows of [screen-inventory.md](../screen-inventory.md) (every `M-` row, and the Android side of the shared `X-` rows), the journeys in [app-flow.md](../app-flow.md) (J12 to J14b) and the component specs in `docs/design/components/`. The pending items in [pending-owner-review.md](../pending-owner-review.md) apply, in particular item 10 (Activity rail entries move to the Home top bar, the overflow menu and the host card) and item 9 (system light or dark with a manual override, no dynamic colour). Marketplace sits inside Settings, as pv96 builds it.

The artboards are in the Design canvas folder, prefix `an-` (index below). They are static comps: dark theme unless stated, 390 by 844 dp phones, real copy, Lucide icons, no emoji. The system status bar, the gesture bar and the keyboard are not drawn (the real ones render on top of the layout); the 24 dp inset at the top of each phone is left empty on purpose.

## 1. Conventions used by every screen

| Topic | Rule (source) |
|---|---|
| Window classes | Compact under 600 dp: one pane, no bottom tabs (M-SH-01). 600 dp and up: master pane 360 dp on `surface.frame`, detail on `surface.canvas` with the rounded top-left corner (`radius.canvas`), content column at most 720 dp (design-system 5.4). |
| Top app bar | Floating (owner, 2026-11-13; see 1.1). No bar band: the bar area is transparent over `surface.canvas`, 64 dp high, 12 dp side gutters. Leading: one 48 dp round floating button (back, or the mode switch on Home). Centre: a floating title pill (title 15 sp / 600 with an optional `caption` line, centred) or, on a session, the floating view group. Trailing: one action is a 48 dp round floating button; two or three actions share one floating group pill. A screen without a trailing action keeps a 48 dp spacer so the centre stays balanced. |
| Touch | Visual height 44 dp (`size.touch.default`), hit target 48 dp (`size.hit.touch`), primary action at the bottom 48 dp (`size.touch.large`). At least 8 dp between targets. Rows 48 dp, two-line rows 56 dp, settings two-line rows 72 dp. Screen gutter 16 dp. |
| Type | Android scale in sp: `body` 14, `label` 14 / 500, `title` 18, `heading` 22, `display` 28, `chat` 16 / 24, `mono` 13, terminal 12. Layouts use minimum heights and scale to 200 percent; chips, badges and the bar cap at 130 percent. |
| Bottom sheets | The Android form of popovers and menus (X-DLG-02): `surface.overlay`, `radius.2xl` top corners, 32 by 4 dp handle in `border.strong`, `scrim`, `shadow.e3`, maximum 90 percent of the screen height (760 dp on the artboard), pinned footer with 48 dp buttons, dismissed by swipe, scrim tap or system back. A confirm is a Dialog, never a sheet. |
| Status | Never colour alone. Agent state is the `AgentStatus` glyph plus a word ("Working", "Needs you", "Failed", "Done", "Unreachable"). Added and removed diff lines carry `+` and `-`. |
| Honest state | A host that cannot be reached shows its last known state with its age, dimmed, in the `agent.unreachable` style. Nothing is called stopped (design principle 6). |
| Theme | Follows the system by default; the Home top bar icon (sun in dark, moon in light) is a manual override; Settings, Appearance offers System, Light and Dark. No Material You colour. |
| Agent marks | Drawn as monogram plates (a lettered circle in the provider ring colour) as in the ModelPicker previews; the real marks are supplied assets. |
| Icons | Lucide at 20 and 24 dp (16 dp in dense chips), stroke widths as in design-system 4.6. |
| Elevation | Android elevation 1, 6 and 12 dp for e1, e2 and e3; in dark the surface step and border do most of the work. |
| Back | System back gesture goes up one screen; it closes a sheet first, and clears a search first. |

### 1.1 Floating layout (owner, 2026-11-13)

The phone uses floating controls instead of bars, following the owner's two reference screenshots (a workspace session and a Conference room). The references fix the arrangement and the use of floating components, not the final look.

- **Floating button.** 48 dp circle, `surface.raised`, 1 dp `border.default`, `shadow.e1`, 24 dp icon in `text.primary`. Used for back, the mode switch, overflow and single actions.
- **Floating group.** A 48 dp pill with the same surface that holds two to four 44 by 40 dp icon buttons, 2 dp apart. The selected one sits on `fill.selected.raised` in `text.primary`; the others are `text.secondary`.
- **Title pill.** 48 dp pill, same surface, centred title and optional caption, at most 232 dp wide; text truncates with an ellipsis. In Bot Teams the title pill is a team selector: team character, team name, chevron.
- **Session screen.** Centre of the bar: the view group Chat, Summary, Files, Terminal (message, clipboard, folder and terminal icons). Trailing: overflow. Under the bar, a floating session pill (40 dp): status dot in the agent state colour, agent mark, branch, `text.tertiary` session name, chevron and the session count in a 28 dp sunken badge; it opens the Sessions sheet (7.4). Project and host move to the Summary and to the Sessions sheet.
- **Composer.** A floating card, `radius` 24 dp, 8 dp padding, `shadow.e3`. The text field is a sunken inner field (`surface.sunken`, 16 dp radius, at least 52 dp high). Bottom row, left to right: Attach (+), the model chip (agent mark, model name, chevron; reasoning moved out of the chip), Reasoning (signal bars, the filled bars show the effort), Access (lock; open lock on `status.danger.bg` in `status.danger.fg` when access is Full access), a spacer, Dictation, Send or Stop (round). These are the desktop Composer controls in the same order.
- **Team composer.** One row: Attach, a rounded sunken field "Message the team", Dictation, Send. `@` in the field opens the mention menu (7.11).
- **Jump to latest.** A 48 dp floating round button with a down arrow, bottom right above the Composer, while the reader is away from the bottom.
- **Other screens.** Lists, settings, source control, files, tasks and pairing keep their content and adopt the floating bar: back circle, title pill, actions as a circle or a group. FABs and bottom sheets are unchanged. In-page tabs (Scan | Paste, Changes | Pull request | History) stay segmented controls under the bar.
- **Not adopted from the references.** The strip of green dots at the left edge of the screenshots belongs to the phone's system, not to MassOrca.
- **Tablet.** The master and detail layout (M-SH-01 from 600 dp) is not changed yet.

## 2. Artboards

| File | Size (w x h) | Shows |
|---|---|---|
| `an-pairing.dc.html` | 1330 x 1954 | First launch and pairing: connect hint, scanning, paste mode with an invalid code, camera denied, confirm the desktop, post-pair setup |
| `an-home.dc.html` | 2620 x 1042 | Home with the Projects row and hosts; Projects list (M-PJ-01) and its empty state; Home with hosts connected; Home with an unreachable host and one that needs pairing; protocol gate; Home overflow sheet |
| `an-workspaces.dc.html` | 2190 x 1954 | Projects and workspaces: list (loaded, overflow menu with Prototypes, loading, empty, catalog error), action sheet, filter sheet, new workspace form, folder without git |
| `an-session-chat.dc.html` | 1760 x 1042 | Session chat with status pill, approval card, model picker sheet, queued messages |
| `an-session-more.dc.html` | 1760 x 1954 | Question card, delegation card and bring back, delegation sheet (two levels), plan sheet, sessions sheet, background work sheet, new session and its options sheet |
| `an-summary-artifacts.dc.html` | 1330 x 1954 | Session summary (collapsed and Results expanded), artifacts grid, read-only viewer, loading and empty states |
| `an-terminal-files.dc.html` | 1330 x 1954 | Terminal (live with key row and dock, pending bar, engine error), quick commands sheet, file explorer, file preview |
| `an-source-control.dc.html` | 1330 x 1954 | Source control hub (Changes, conflict state, Pull request, History), diff review, review note sheet |
| `an-botteams.dc.html` | 1330 x 2866 | Bot Teams: home carousel, a team, Conference room, Conference room overflow sheet, Board, Routines, Calendar agenda |
| `an-tasks.dc.html` | 1760 x 1042 | Tasks list, create task sheet, per-provider states, empty |
| `an-settings.dc.html` | 1760 x 1954 | Settings menu, notifications (permission off), accounts and usage, voice model, Marketplace (Installed, item, credential form), edit host |
| `an-secrets.dc.html` | 1760 x 1954 | Secret request card, entry sheet (add, invalid name, host unreachable, link, saving), vault list and empty |
| `an-connection.dc.html` | 1760 x 1042 | Host disconnected banner, reconnecting, restored, connection log and troubleshooting |
| `an-tablet.dc.html` | 1360 x 1000 (the tablet is 1280 x 800 inside) | Master and detail on an expanded window |
| `an-light.dc.html` | 1330 x 1042 | Light theme: Home, chat with an approval, diff review |
| `an-prototypes.dc.html` | 1330 x 1954 | Prototypes by project with each design system, the prototype canvas, Play, the Design system tab, a section (Colors), a linked read-only design system |

## 3. Inventory coverage

Every Android row ends up designed (a section below), merged, dropped or marked not drawn. Rows without an artboard say so.

| ID | Treatment |
|---|---|
| M-SH-01 | Designed, section 4.1, tablet artboard |
| M-SH-02 | Designed, 4.2 |
| M-SH-03 | Dropped (reason: Android is fully native; open question 3 answered) |
| M-SH-04 | Dropped (reason: typed routes replace the web twins and redirects) |
| X-AC-01 | Designed (Android side), 5.5 |
| M-AC-01 | Designed, 5.4 |
| X-DG-01 | Designed (Android side), 5.6 |
| X-CN-01 | Designed, 5.6 |
| M-PA-01, M-PA-02, M-PA-04, M-PA-05 | Designed, section 4.3 |
| M-PA-03 | Merged into M-PA-02 (paste is a mode of the scan screen) |
| M-HO-01 | Designed, 5.3 |
| M-HM-01, M-HM-02 | Designed, 5.1 and 5.2 |
| M-HM-03, M-HM-04 | Merged into M-HM-01 (Resume and Tasks rows in the Home list) |
| M-AC-02 | Merged into M-HM-02 (usage rings on the host card) |
| X-WS-01 | Designed (Android sheet), 6.4 |
| M-WS-01, M-WS-02, M-WS-05 | Designed, 6.1, 6.2, 6.5 |
| M-WS-03 | Merged into X-WS-01 |
| M-WS-04 | Merged into D-WS-14; the Android form of the fields is drawn, 6.3 |
| M-WS-06 | Merged into M-SH-01 (empty detail state, text in 4.1, not drawn) |
| M-WS-07 | Merged into X-DLG-01 |
| M-TK-01 | Designed, section 11 |
| X-AG-01 | Designed (Android side), 7.1 |
| X-AG-02 | Designed (sheet form), 7.5 |
| X-CH-02, X-CH-03, X-CH-05, X-CH-06 | Designed (Android side), 7.6 to 7.9 |
| X-CH-07 | Designed, 13.1 |
| X-CH-08 | Designed (Android side), 7.10 |
| X-CH-09, X-CH-10, X-CH-11 | Specified in text, not drawn, 7.11 |
| X-DL-01 | Designed as a flat sheet, 7.9 |
| M-CH-01 | Designed, 7.2 |
| M-CH-02, M-CH-03, M-CH-04, M-CH-05, M-CH-06 | Merged into M-CH-01, X-CH-02, X-CH-03, X-CH-05, X-CH-06 |
| M-CH-07 | Designed (reopened by pv96), section 8.1 |
| M-SE-01, M-SE-02, M-SE-03 | Designed, 7.3, 7.1, 7.4 |
| M-SE-04 | Merged into M-SE-02 (overflow menu and pending bar are drawn there) |
| M-SE-05 | Merged into X-AG-01 (child rows); the sheet is drawn, 7.4 |
| M-SE-06 | Merged into M-SE-03 ("Resume an earlier session" section) |
| M-SE-07 | Designed, 7.2 |
| X-IM-05 | Designed (Android badge), 7.4 |
| M-AR-01 | Merged into X-AR-01; the Android list and viewer are drawn, 8.2 |
| M-PT-01, M-PT-02 | Merged into D-AG-17 (and its sub-views). The Android entry is the session overflow row "Prototypes"; the screens are not drawn here (see Not covered) |
| M-EX-01 | Designed (inside Settings), 12.4 |
| X-QC-01, M-TM-01, M-TM-02 | Designed, section 9 |
| M-TM-03 | Merged into M-TM-01 (input dock) |
| M-TM-04 | Merged into M-TM-02 (the "more" key opens the custom key editor; not drawn) |
| M-TM-05 | Merged into M-ST-01 (Terminal settings page; not drawn) |
| M-TM-06 | Merged into X-QC-01 |
| M-FI-01, M-FI-02 | Designed, section 10 |
| X-RV-02, X-PR-01, M-DF-01, M-DF-02 | Designed, section 10 |
| M-DF-03 | Merged into X-PR-01 |
| M-DF-04 | Merged into M-DF-02 (History segment) |
| X-NT-01, X-NT-02, X-UP-01, M-NT-01 | Designed, section 12 |
| M-ST-01, M-ST-02 | Designed, section 12 |
| M-ST-03 | Merged into M-ST-02 |
| M-ST-04 | Merged into M-ST-01 (rows) |
| X-DLG-01, X-DLG-02, X-PK-01 | Designed (Android forms), section 14 |
| X-VT-01, M-VT-01 | Designed, 13.2 and 13.3 |
| Extension (rows) | None: extensions are desktop only in v1 (design-system 9), so no Android row is an Extension row |

Android forms of desktop-only rows that the brief asks for and that have no `M-` ID: D-WS-29 (team list), D-AG-24 (Conference room), D-WS-30 (Board), D-AG-25 (Routines), D-AG-23 (Calendar). They are in section 11 and recorded as decisions.

---

## 4. App shell, pairing and gates

### 4.1 M-SH-01 Navigation stack

- **Purpose.** One stack of single panes; home is the host list.
- **Layout.** Compact: Home, then a host (workspace list), then a session, then its sub-screens (summary, artifacts, files, source control, tasks, settings). No bottom tabs. The system back gesture goes up. The route table follows app-flow "Android". The Activity rail has no equivalent; its entries land on Home (item 10): mode switch (leading icon), theme and Settings (top bar), Calendar (overflow), usage (host card), Marketplace (inside Settings).
- **Medium and expanded (600 dp and up).** Workspace list becomes the 360 dp master pane on `surface.frame`; the detail pane is `surface.canvas` with `radius.canvas`; the title band over the detail is frame-coloured. Rows in the master pane use `fill.selected.frame` and a 2 dp `accent.fg` bar for the open workspace. Empty detail (M-WS-06 merged here): EmptyState, section size, message icon, title "Pick a workspace", line "Its session opens here."
- **States.** Compact, wide, empty detail. A blocked host shows the protocol gate (4.2) in the detail pane.
- **Artboard.** `an-tablet.dc.html`.

### 4.2 M-SH-02 Protocol gate

- **Purpose.** Blocks use of one host when the app and host protocol versions differ, and says which side must update. It blocks that host only, never the whole app.
- **Layout.** App bar with the host name. Page EmptyState, left aligned: 56 dp icon plate, `heading` title, explanation, a Card with both versions (`kv` rows), then pinned at the bottom: "Check again" (primary, 48 dp) and "Back to hosts" (secondary).
- **Copy.** Host older: "This PC needs an update" / "MassOrca on This PC speaks protocol 14. This app needs protocol 16 or newer. Update MassOrca on This PC, then check again." App older: "This app needs an update" / "This PC speaks protocol 18. Update MassOrca from Google Play, then open it again." with the primary "Open Google Play" instead of "Check again".
- **States.** Compatible (no gate), blocked host older, blocked app older, checking (spinner in the button).
- **Interactions.** Check again re-reads the version; back leaves to Home. The host card shows "Update needed" (warning badge) while blocked.
- **Artboard.** `an-home.dc.html` (third phone).

### 4.3 Pairing: M-PA-01, M-PA-02, M-PA-04, M-PA-05

**M-PA-01 Connect hint (empty Home).**
- Purpose: explain how to connect a desktop in three steps. Layout: the Home top bar (mode switch, theme, Settings, overflow), then `display` title "Connect your desktop", one `chat` line, three numbered steps (28 dp accent-subtle number, `body-strong` heading, `body-sm` line). Pinned: "Pair desktop" (primary 48 dp, qr-code icon) and "Paste a code" (ghost).
- Copy: "Follow your agents from the phone, answer their questions and approve their work." Steps: "Open MassOrca on your computer" (in the sidebar, choose MassOrca Mobile, which shows a QR code; the entry follows A38), "Scan the code", "You are connected".
- States: no hosts only. With a host the Home list replaces it.

**M-PA-02 Scan pairing code** (M-PA-03 merged: paste is the second mode).
- Layout: app bar "Scan pairing code"; a two-segment control "Scan | Paste" (44 dp). Scan: a 358 by 360 dp viewfinder on black with four white corner brackets (the camera preview is the system's), the line "Point the camera at the QR code on your computer.", help line "It is under MassOrca Mobile in the sidebar. Both devices need to be on the same network.", ghost "Open a link instead" (deep-link entry, opens the same flow as `massorca://pair?...`). Paste: label "Pairing code", a multi-line field (mono, 112 dp), "Paste from clipboard" (secondary), "Connect" (primary, disabled until the field has text).
- States: scanning; connecting (viewfinder dims, "Connecting to This PC", spinner); invalid QR ("Not a valid MassOrca QR code", Banner danger, scanning resumes after 2 s); invalid pasted code (field border `status.danger.fg`, "Not a valid pairing code. Copy it from your computer and paste it again."; Connect disabled); camera denied (EmptyState page: camera-off plate, "Camera access is off", "MassOrca needs the camera to read the QR code. Allow it in Android settings, or paste the code instead.", "Open Android settings" primary, "Paste code instead" secondary); failed ("Pairing failed: This PC did not answer", Retry).
- Interactions: the segment switch keeps typed text; back goes to Home. A pairing deep link opens straight at M-PA-04.
- Artboard: `an-pairing.dc.html` (phones 2 to 4).

**M-PA-04 Confirm pairing.**
- Layout: app bar "Pair phone"; `heading` "Pair with this desktop?"; a Card with the desktop name (`title-sm` "This PC"), its OS and app version, and `kv` rows for address (mono), network and protocol; a `body-sm` safety line; a "Pairing log" row (opens the ConnectionLog of X-DG-01); pinned "Pair" (primary) and "Cancel" (secondary).
- Copy: "Check that this is your computer. A paired phone can see your workspaces and answer your agents."
- States: awaiting confirm; connecting (Pair shows a spinner, both buttons disabled); failed (Banner danger "Could not pair: This PC did not answer. Check that both are on the same network.", actions "Retry" and "Back to home"). System back while connecting asks nothing and cancels.
- Desktop counterpart: D-PR-01 (the desktop shows the QR and, when the phone arrives, its own confirm).

**M-PA-05 Post-pair setup** (absorbs the onboarding wizard and the notification preview).
- Layout: a modal screen, top bar "Finish setup" with a close icon, step dots (24 by 4 dp), `caption` "Step 1 of 2", `heading`, a line, option cards (Card, 12 dp padding, radio at the end, selected card in `accent.subtle.raised` with an `accent.fg` border), pinned primary and secondary buttons. Only undecided steps show: default view ("How do you want to follow sessions?", "Chat view" or "Terminal", buttons "Use chat view" and "Keep terminal"), then notifications ("Get told when an agent needs you", "Allow notifications" and "Not now", with a preview of one notification).
- States: pending steps only; none pending (the screen is skipped); error (Banner, announced as an alert). Swipe back is disabled; close skips with a confirm only when a step is undecided.

---

## 5. Home, hosts and connection

### 5.1 M-HM-01 Home

- **Purpose.** The host list plus what needs attention, in one scrolling list (the resume card, tasks card and top bar merge in).
- **Layout.** Floating top bar (1.1; owner asked for a clear mode change, 2026-11-13): Settings as the leading round button; in the centre the mode switch, a floating segmented pill with two labelled segments, "Workspaces" (`layout-grid`) and "Bot Teams" (`bot`), the active one on `accent.subtle.raised` in `accent.fg`; one tap changes the mode in place, no extra screen; then the overflow button. The mode switch replaces the title. Superseded: the unlabelled leading mode icon. Overflow sheet rows (48 dp, icon before label): Theme (sun-moon icon, value "System"; System, Light, Dark), Calendar, Pair another desktop, Connection log, Troubleshoot connection. The overflow phone of `an-home.dc.html` shows the same Home as the first phone (agreements pass, 2026-11-13). List, top to bottom: App update card (X-UP-01, when there is one), a grouped card with the Tasks row ("Tasks", "2 need your approval") and the Resume row (status glyph, the agent icon, "Resume feat/billing-export", "oppm, working 2:14"; the icon replaces the agent name, A17), then the overline "Hosts" with a "+ Pair" action and the host cards 12 dp apart. The grouped card starts with a Projects row (folder plate, "Projects", "3 projects · 1 needs you · 1 failed", chevron) that opens M-PJ-01 (owner, 2026-11-13: keep Home to hosts and reach projects from one button). Host cards are status only: no attention line, and a tap opens the host details (accounts and usage, edit, connection log). **M-PJ-01 Projects** (new; owner, 2026-11-13: clean, no cards, like the desktop Sidebar). Floating bar: back, title pill "Projects", then search, filter and overflow. Overflow sheet "Projects" (two-line rows, icon before label): Prototypes ("Screens and flows of each project", opens the prototype sets grouped by project), Design systems (one per project, shared ones marked), Archived workspaces, Sort by (owner, 2026-11-13). A plain list: each project is an overline header; when the project lives on more than one host, a small `caption` host label (host icon and name, `text.tertiary`) opens each host group. Workspace rows are the M-WS-01 rows (48 dp, status glyph, the icon of the agent that runs the session (16 dp), name, trailing child count, state word or age; waiting and failed rows tinted). Child sessions: a parent row with children shows the users icon and the count and a chevron; expanded, its children sit under it as 44 dp rows indented 80 dp, so the child agent icon lines up with the first letter of the parent name, with no connector lines (the indent is enough; owner, 2026-11-13), each with the agent icon (16 dp), the task in `body-sm`, its status glyph and word. Collapsed, only the count shows. FAB "New workspace". Tap a row: the session. Empty: a folder plate, "No projects yet", "A project is a repository or folder on one of your hosts. Start a workspace without a project, or create one from the project pill.", and the same FAB. New workspace opens M-SE-01 (7.3). The Projects row sits first in the Home grouped card in every Home state, including when hosts are unreachable. Artboard: `an-home.dc.html`, second phone.
- **States.** Empty (M-PA-01); hosts (below); the update card has its five states (12.3). Resume shows the unreachable glyph and "last known: working, 5 min ago" when its host cannot be reached.
- **Interactions.** Tap a card to open the host. Pull to refresh re-checks every host. The mode switch changes the list to teams (section 11); pressing it once always switches on Android, because the mode control only exists on Home (the desktop rule in pending item 14 does not apply here).
- **Artboard.** `an-home.dc.html`, `an-light.dc.html`.

### 5.2 M-HM-02 Host card

- **Layout.** Card (`surface.raised`, `border.default`, `radius.lg`, 12 dp padding, 16 dp side margin): a 40 dp plate with the host icon (monitor, server), the name in `title-sm`, a `body-sm` line (OS and link: "Windows, local network", "Linux server, SSH"), a status Badge at the end. Under a hairline: one UsageRing per provider (32 dp, 2 dp stroke, monogram inside) each followed by its percentage in `caption`, tabular. Under that a line with the attention summary ("1 session needs you, 2 working"; AgentStatus glyph 16 dp). The card is one 48 dp target; the rings are read only.
- **States and copy.**

| State | Badge | Card |
|---|---|---|
| ok | "Connected" (success) | As above |
| warning | "Check" (warning) | Reason line: "Slow connection" or "Update available" |
| unreachable | "Unreachable" (unreachable style) | Dimmed in `text.tertiary`; rings dimmed; "Last known state, 5 min ago: 1 session working. It may still be running."; "Retry" |
| auth failed | "Sign in" (warning) | "This PC no longer trusts this phone. Pair again to see its workspaces." and "Pair again" (primary) |
| credential missing | "Sign in" (warning) | "The pairing key is missing from this phone." and "Pair again" |
| protocol mismatch | "Update needed" (warning) | Opens the gate (4.2) |

- **Interactions.** Tap opens the host details (superseded: tap opens M-WS-01; projects now lead to workspaces, 5.1). A chevron after the Badge shows the card opens. Long press opens a sheet: Open, Accounts and usage (M-AC-01), Edit host (M-HO-01), Connection log (X-DG-01), Forget this host (X-DLG-01 destructive). The same actions are TalkBack custom actions.
- **Artboard.** `an-home.dc.html`.

### 5.3 M-HO-01 Edit host

- **Layout.** App bar "Edit host". Fields (48 dp): Name, Address, Port (104 dp wide, mono), an info Banner "Changing the address reconnects this phone to the new one.", a Card with `kv` rows (Paired date, Protocol), pinned "Save" (primary) and "Remove this host" (secondary with `status.danger.fg` text and `status.danger.border`).
- **States.** Pristine (Save disabled); invalid address ("Enter an address like 192.168.1.24 or vps.example.com"); saving (spinner, then Toast "Reconnected to This PC" on success, Banner danger on failure with the old values kept). Remove asks X-DLG-01: "Remove This PC? This phone forgets it. Workspaces and sessions on This PC are not deleted."
- **Desktop counterpart.** D-HO-03 (rename and remove in a confirm).
- **Artboard.** `an-settings.dc.html` (last phone).

### 5.4 M-AC-01 Accounts and usage

- **Purpose.** Account usage for one host, with the Codex reset-credit action.
- **Layout.** App bar "Accounts and usage" with the host name as caption (a host chip row appears when more than one host is paired). One Card per agent account: 32 dp mark, name, plan line ("Max plan, signed in on This PC"), then one block per window: label and percentage (tabular), a 4 dp Progress bar (`usage.ring.*`, `usage.ring.warn` from 80 percent, `usage.ring.limit` from 95 percent), and a reset line that counts live ("Resets in 1 h 48 min"; refreshed every minute without an announcement). Codex card: "Codex lets you reset your credit once a week." and "Reset credit" (secondary).
- **States.** Loading (skeleton cards); live; reset pending (button shows a spinner, "Resetting"); reset done (Toast "Credit reset"); unavailable ("Usage unavailable for Gemini"); signed out (X-AC-01 card, 5.5); host unreachable (rings dimmed, "last known 5 min ago"); error (ErrorState section with Retry).
- **Artboard.** `an-settings.dc.html` (third phone).

### 5.5 X-AC-01 Account needs sign-in (Android side)

- Card in the transcript header area and on M-AC-01: `agent.waiting.bg`, waiting glyph, "Gemini needs you to sign in", line "Its sign-in expired on This PC. Open MassOrca there to sign in again." Sign-in happens on the host. When the host supplies a sign-in link (Codex), the card has one 48 dp action "Open sign-in link". States: shown; dismissed (swipe or x, comes back after the next failed run); signed in (the card disappears and a Toast says "Gemini is signed in").
- Artboard: `an-settings.dc.html`.

### 5.6 X-CN-01 Connection banner and X-DG-01 Diagnostics

**X-CN-01 Banner.** Full width under the top app bar of every screen that depends on the host (session, workspace list, terminal), pushing content down (no overlay, no animation with "Remove animations"). Text `body`, a `body-strong` lead sentence, actions as 48 dp text buttons under the text, `accessibilityLiveRegion` polite (assertive for the first Disconnected).

| State | Variant | Copy | Actions |
|---|---|---|---|
| Connecting | info, spinner after 200 ms | "Connecting to VPS" | none |
| Reconnecting | warning, spinner | "Reconnecting to VPS, attempt 3. Retrying in 12 s." | "Retry now" |
| Disconnected | unreachable | "VPS can't be reached. Showing the last known state from 5 min ago. Work that was running there may still be running." | "Retry", "Details" |
| Restored | success | "Reconnected to VPS." (fades after 5 s, dismissible) | none |
| Auth failed | warning | "VPS needs you to sign in" | "Pair again" |

While disconnected: everything loaded stays; the running tool row shows the unreachable glyph and "Last known: running, 5 min ago"; the Composer stays enabled with the caption "VPS can't be reached. Messages wait in the queue." and Send becomes Queue; the queue dock rows read "Waits for VPS to confirm. Sends when it reconnects." and Send now is hidden. "Details" opens X-DG-01.

**X-DG-01 Connection log and troubleshooting** (merges the desktop connection test). App bar "Connection log" with the host as caption. A status line (host icon, name, last contact, Badge "Failed" or "Warning" or "Ok"), "What to try" (three numbered steps: "Check that VPS is on", "Check the network", "Run the network test"), "Network test" (rows: name lookup, port, secure handshake, each with glyph and result: "12 ms", "timed out", "skipped"), "Log" (mono, last 50 lines, timestamps), pinned "Run test" (primary) and "Copy log" (secondary). Nothing is sent anywhere; Copy log puts text on the clipboard. States: ok, warning, failed; running test (spinner per row).

- **Artboard.** `an-connection.dc.html` (all four phones), `an-home.dc.html` (card states).

---

## 6. Workspaces

### 6.1 M-WS-01 Workspace list

> **Superseded by M-PJ-01 Projects (owner, 2026-11-13; 5.1).** The list is now reached from the Projects row on Home, titled "Projects" instead of the host, with projects as headers, a host label when a project lives on more than one host, the agent icon on each row and child sessions nested under their parent. The row anatomy, tints and FAB below still apply. Artboard: `an-workspaces.dc.html` (retitled "Android: Projects and workspaces"), and `an-home.dc.html`.

- **Purpose.** The workspaces of one host. Master pane on wide layouts.
- **Layout.** App bar: back, host name, caption "Connected, local network" (with the status glyph), actions search, filter (a dot when a filter is active) and overflow (Edit host, Connection log). List grouped by project: overline header (project name), one-line 48 dp rows: AgentStatus glyph 20 dp in a 24 dp slot, workspace name (`body`, truncates), trailing items in this order: child-session count (users icon and number), "No Git" Badge, and a word in `caption` tabular: "Working", "Needs you", "Failed" or the age of an idle workspace ("3 h"). Rows in `waiting` are tinted `agent.waiting.bg`, rows in `error` `agent.error.bg`. Extended FAB "New workspace" (56 dp, `radius.full`, `accent.default`, 16 dp from the right and bottom, above the gesture inset).
- **States.** Loading (skeleton rows after 100 ms: a 24 dp circle and a bar, two groups); empty (page EmptyState "No workspaces yet", "A workspace is a git worktree or a folder where an agent works.", "New workspace"); catalog error (ErrorState page "Could not load workspaces from This PC", "The app is retrying by itself. Your sessions are not affected.", "Retrying in 8 s", "Retry now", details line in mono); filtered (a row at the top "Filtered: 6 of 7 workspaces" with "Clear"); no match ("No workspaces match this filter", "Clear filter"); disconnected host (Banner, rows keep their last glyphs, dimmed, with the age; the FAB stays but creating needs the host, so it opens the form with the host banner); long names truncate with an ellipsis, full text in the action sheet title.
- **Interactions.** Tap opens the latest session (M-SE-02), or the new session screen (M-SE-01) when the workspace has none. Long press opens M-WS-02 (also a TalkBack action "Actions"). Pull to refresh. Search filters by name.
- **Desktop counterpart.** D-WS-01 (a tree with collapsible projects, one row per host under each project).
- **Artboard.** `an-workspaces.dc.html` (phones 1 to 4).

### 6.2 M-WS-02 Workspace action sheet

Sheet titled with the workspace name and "project, host" as caption. Rows (48 dp, 24 dp icon): Open session, New session, Files (M-FI-01), Source control (M-DF-02), Copy path; a hairline; "Delete workspace" in `status.danger.fg`. Delete swaps the sheet content in place (no second modal) to the confirm of X-DLG-01: "Delete feat/billing-export?" with the warnings of D-WS-16 as a list (unsaved changes, unpushed commits, nested child workspaces), "Cancel" and "Delete workspace" (danger). States: default, confirm, busy, error ("Could not delete: <reason>. Nothing was removed."). Artboard: `an-workspaces.dc.html`.

### 6.3 New workspace form (M-WS-04 merged into D-WS-14; Android form)

> **Superseded (owner, 2026-11-13).** "New workspace" opens the New session screen (7.3), where project, worktree and base ref are pills; the base ref missing and setup states of this form move to those pills' sheets. The form, as drawn for reference, starts with a Host field (host icon, name, connection dot; the list holds the hosts where the chosen project lives) above Project.

Sheet "New workspace" with "on This PC" as caption. Fields: Project (picker row, X-PK-01), Branch name (focus ring while typing, help "A new branch and its own worktree are created."), Base ref (picker row, mono), "Run the setup script" row with the command in `body-sm` and a Switch. Base ref missing: inline warning Banner "origin/develop is missing. Using main, the default branch." (A5). Pinned "Create workspace" (primary). States: idle, validating (field spinner), base ref missing, creating (button spinner, fields read only, then the new row appears and the sheet closes), setup offered, error (Banner danger, form kept), host unreachable (Banner, Create disabled). Artboard: `an-workspaces.dc.html`.

### 6.4 X-WS-01 Filter sheet (M-WS-03 merged)

> **Updated (owner, 2026-11-13).** Title "Filter projects"; groups Status, Hosts (This PC, VPS, with counts) and Projects.

Sheet "Filter workspaces": overline "Status" with radio rows (All, Needs you, Working, Idle, each with a count), overline "Projects" with checkbox rows (name, count), a "Show archived" Switch row. Footer: "Clear" (secondary) and "Show 6 workspaces" (primary, the count updates live). States: all, filtered, no match (the primary reads "No workspaces" and is disabled). Artboard: `an-workspaces.dc.html`.

### 6.5 M-WS-05 Folder without git

Sheet "This folder is not a git repository" with "perulaunch-web is a plain folder, so there are no branches or diffs to show. Initialize Git to track changes and use worktrees." and an info line "Needs MassOrca 0.9.4 or newer on This PC." (the capability gate, shown only when missing). "Initialize Git" (primary), "Not now" (secondary). States: offered, running (spinner), done (the "No Git" badge goes away), error (Banner danger with the reason). Opened by tapping the "No Git" badge. Artboard: `an-workspaces.dc.html`.

---

## 7. Sessions and chat

### 7.1 M-SE-02 Session screen and X-AG-01 status

- **Layout, top to bottom.** Floating bar (1.1): back circle, the view group Chat, Summary (opens 8.1), Files (9 and 10), Terminal, and the overflow circle. The floating session pill (status dot, agent mark, branch, session name, chevron, session count; opens 7.4). Then the content, the status pill (7.2), the queue dock, the floating Composer. Superseded (owner, 2026-11-13): the app bar with the workspace name and the "Chat | Terminal" tab row.
- **Overflow sheet** (M-SE-04 merged): Plan, Artifacts, Source control, Prototypes, Quick commands, Connection log. Plan shows "No plan yet" with "Claude has not shared a plan in this session." when empty.
- **States.** Loading, waiting-session ("Starting Claude on This PC", progress), disconnected (5.6), error (ErrorState "Could not load this conversation", Retry), pending terminal (banner "Loading terminal. Starting the shell on This PC." with "Retry"; a second failure turns it into a danger Banner "Could not restore the terminal.").
- **X-AG-01 on Android.** The same five glyphs at 12, 16 and 20 dp, each with its word as accessible name: running arc, waiting circle with a question mark, idle ring, error triangle, unreachable dashed ring with a slash. Waiting and error rows and cards are tinted. A question folds into waiting. Child work is a count beside the parent glyph.
- **Desktop counterpart.** D-AG-03 and the Project sidebar's Sessions list.
- **Artboard.** `an-session-chat.dc.html`, `an-terminal-files.dc.html`.

### 7.2 M-CH-01 Chat view, M-SE-07 working status pill

- **Chat.** A virtualised list, full width, 16 dp gutter, text `chat` (16 / 24 sp). User messages: right-aligned bubble (`surface.raised`, `border.default`, `radius.lg`, 12 dp padding, up to 85 percent wide). Agent text sits on the canvas with no container. Blocks follow MessageBlocks: grouped tool calls collapsed to one 44 dp row ("4 steps", names in mono, duration), a diff snippet card (path, `+42 -3` with signs, six lines, "Show 36 more lines"), approval and question cards (7.6). Long press on a message opens a sheet: Copy, Fork from here, Retry. The list sticks to the bottom while the reader is near it; a 48 dp floating round button with a down arrow sits above the Composer at the right (1.1); "Waiting for you" stays in the status pill.
- **Composer.** Pinned above the keyboard and the IME inset, 1 to 6 lines, text 16 sp, placeholder "Message Claude". A floating card with a sunken inner field (1.1). Bottom row: Attach (40 dp, opens a sheet: files, camera, gallery), the model chip (agent mark, "Opus 5.5", chevron, 36 dp high, opens 7.5), Reasoning (signal bars, opens 7.5 at the reasoning list), Access (lock, opens the access list), dictation, Send (40 dp circle, 48 dp hit). Send is disabled when empty; Stop (square) while the agent runs with an empty field; Queue (same arrow) when the agent is busy and the field has text. A row of suggestion chips (44 dp) sits above the field while the draft is empty.
- **Working status pill (M-SE-07).** A floating pill above the Composer: AgentStatus glyph, "Working", elapsed clock (tabular, counts live), terminal count and agent count (each hidden at zero). Tap opens the Background work sheet (7.4). Pending approval: the pill reads "Waiting for you" with "Run command" and a tap scrolls to the card. Hidden when idle.
- **States.** Loading (three skeleton turns), awaiting transcript (EmptyState "Describe what you want done", Composer focused, no spinner), ready, streaming (the last turn ends with the running glyph and "Working"), error, disconnected (5.6), disabled Composer ("Sign in to Claude to continue" with its action).
- **Artboard.** `an-session-chat.dc.html`.

### 7.3 M-SE-01 New session

- **Purpose.** Starts an agent session in a workspace. The option pills replace the choice, base ref and more sheets; the progress view merges in.
- **Layout** (owner, 2026-11-13, from the owner's reference screenshot). Floating bar: Close circle, the host pill in the centre (status dot, host name, chevron; opens the "Run on" sheet with each host, its connection word, a check on the selected one and "Pair another desktop"), More circle. The prompt is written from the top: `heading` "What should Claude work on?" and under it the draft at 18 / 28 sp, placeholder "Describe the task to start the session…", growing downward. A new session has no conversation, so the space is the prompt, and the keyboard takes the bottom. Above the composer, one carousel of pills (36 dp, one line, scrolls sideways): Attach an image (icon only), project ("oppm"), "New worktree" (or the worktree name), base ref ("From main"), view ("Chat view"). Each pill opens its picker as a sheet. The project pill's sheet lists the projects on the chosen host, then "No project" (folder icon struck through; the session runs in the host's folder for conversations without a project, set in Settings, General, default `~/MassOrca/Conversations`; no worktree, the worktree and base ref pills hide) and "New project…" (name and location on the host, optionally git init) (owner, 2026-11-13). With no projects, the pill reads "No project". The composer is one floating row: the model chip wide (agent icon, "Opus 5.5", chevron), Reasoning (signal bars), Access (lock), and the round Start button (accent when the draft has text). Under it a `caption` line repeats the placement: "New worktree on oppm · This PC". Superseded: the app bar "New session", the field inside the composer, the options sheet and the agent and model pill.
- **Progress.** After Send the screen becomes a list "Starting Claude": "Creating the worktree", "Installing dependencies" (when the setup script runs), "Starting the agent", each with a glyph, then it opens the session. Failure shows a step in error with "Retry" and "Back".
- **States.** Composer row, options, progress, error, host unreachable (Banner, Send disabled with the reason).
- **Artboard.** `an-session-more.dc.html` (phones 7 and 8).

### 7.4 M-SE-03 Sessions, M-SE-05 Background work, X-IM-05 imported marker

- **Sessions sheet.** Title "Sessions", caption the workspace. Rows (two-line, 56 dp): status glyph, the agent icon (20 dp; A84), session title, "started 12 min ago" (the icon replaces the agent name, A17); the open one has the accent bar. Imported sessions carry a Badge "Imported" (info, download glyph) and the line "imported from Codex CLI, 3 Nov"; after the first message here the line reads "Continued here 9 Nov". Section "Resume an earlier session" lists past sessions of this workspace (history icon, then the agent icon). Pinned "New session" (primary). States: empty history ("No earlier sessions"), loading (skeleton rows), error.
- **Background work sheet** (from the pill). Sections "Running" and "Finished": terminal tasks (status glyph, name in mono, "Terminal, 2:14") and child sessions (status glyph, agent icon, the task as the title, "Child session"; no agent name (agreements pass, 2026-11-13)). Tap a child opens the delegation sheet (7.9) on it; tap a terminal task opens the Terminal tab at that output.
- **Artboard.** `an-session-more.dc.html`.

### 7.5 X-AG-02 Model picker (sheet form)

Title "Model". One column: a row of agent chips (Favourites star, then one per agent, 44 dp high, 48 dp hit; the active chip has an `accent.fg` border and text), a search field "Search models" (48 dp, accent underline when focused), model rows (56 dp: provider mark, name in `body-strong` when current, a check on the current one, a star that is always visible and a 48 dp target) with "Show more", then "Reasoning" with the current model on the right and the reasoning slider of A98 (a vertical bar 32 dp wide, one stop per level the model declares, 52 dp apart, labels to its right; gradient in the agent's colours; particles, glow and shimmer only at the top level; see components/ModelPicker.md). The footer is pinned and holds only "Standard | Fast" (the Fast segment has a zap icon): no border, on a `surface.sunken` fill, with the selected segment raised (`surface.raised`, `sh-e1`). Access is not in the picker: it lives in the lock button of the floating composer (A27, A30). The body scrolls; the sheet is at most 90 percent high. States: closed (the chip), open, searching (rows filter), no match ("No models match "xyz""), signed out ("Sign in to use Claude models"), disconnected ("Last known models, 5 min ago"). No shortcut badges on Android. A risky access mode adds a warning glyph to the chip.
Artboard: `an-session-chat.dc.html` (third phone).

### 7.6 X-CH-02 Approval and question cards

- **Approval.** Card on `agent.waiting.bg`, `border.default`, `radius.lg`, 12 dp padding: waiting glyph and "Waiting for you" in `label`, title ("Run command?") in `title-sm`, the command in a `surface.sunken` well (mono, wraps), a line saying what approval does ("Claude runs this once on This PC."), two equal-width 48 dp buttons: "Deny" (secondary) and "Approve" (primary). Answered: collapses to one row on `surface.raised` with a check or x glyph, "Approved" or "Denied", the title and the time. Submitting: both buttons disabled, spinner in the pressed one. Error: the same card with a line "Not sent: <reason>" and "Retry".
- **Question.** Same frame. Single choice: radio rows (48 dp). Multiple choice: checkbox rows. Free text: a multi-line field. "Send answer" is disabled until an answer exists (multi-select needs one pick). Answered: one line with what was sent.
- A pending card also tints the list row of the workspace (6.1), shows in the pill (7.2), and raises an OS notification (12.2).
- Artboards: `an-session-chat.dc.html`, `an-session-more.dc.html`, `an-light.dc.html`.

### 7.7 X-CH-03 Queued messages

A dock directly above the Composer, `surface.raised`, `border.default`, `radius.lg`, 8 dp gap to the Composer. Header "Queued (2)" from two messages, with a collapse chevron. Rows 56 dp: clock icon, the message (one line), the reason in `caption` ("Waits for the agent"), a "Send now" text button and an overflow button (48 dp: Edit, Remove). Send now swaps the row to the inline confirm: "Send now? The agent will stop what it is doing and read this." with "Cancel" and "Send now". More than three rows show two and "+3 more". States: queued (agent busy), queued (host not confirming: unreachable glyph, "Waits for VPS to confirm. Sends when it reconnects.", Send now hidden), sending (spinner, "Sending", actions disabled), error ("Not sent: <reason>", Retry, Remove), editing (inline field). Artboards: `an-session-chat.dc.html`, `an-connection.dc.html`.

### 7.8 X-CH-05 Plan sheet

Sheet "Plan" with "3 of 7 steps done" as caption. Step rows 44 dp or more: status glyph (done check circle, running arc, idle ring), step text (the running one in `body-strong`, done ones in `text.secondary`), an optional second line ("In progress, 2:14"). Opened from the overflow menu or the Plan card in the summary. States: shown, scrolled, empty ("No plan yet", "Claude has not shared a plan in this session."). Artboard: `an-session-more.dc.html`.

### 7.9 X-CH-06 Delegation card and why, X-DL-01 delegation sheet

- **Card in the chat.** One row: agent mark 28 dp (carries the agent name as its label), the task as the title (`body-strong`, one line), "Delegated" (`body-sm`) (agreements pass, 2026-11-13), the child's status glyph and word at the end; below it a sunken line "Why: <reason>", a "Runs in" selector ("Own worktree" preselected, "Current worktree" as the other choice) and two buttons "Open" (the child session) and "Why?" (toggles the reason). Done state: check glyph and "Done".
- **Where a child runs (A75).** (1) A child gets its own worktree by default. (2) "Current worktree" is used only when the child only reads, the workspace is a plain folder without git, or the user picks it on the card. (3) Two sessions never write in one worktree at once: a child writing in the current worktree makes the parent wait, shown as a floating strip above the composer ("Waiting for Codex", "It writes in this worktree. Messages queue until it ends.", duration, "Open child") and as "Waiting for Codex" on the coordinator row. (4) Parallel launches always give each child its own worktree; "Current worktree" is disabled there with the reason "Children that run in parallel each get their own worktree." A child in the current worktree carries a "Current worktree" chip (lock glyph) on its card and sheet row; other children show "Own worktree · <branch>".
- **Sheet (flat, two levels).** Title "Delegation", caption "3 agents, 2 working, 1 done". Group caption "Run in parallel" or "Run in order" (sequential children carry a step number). Rows 56 dp: the agent mark (the name is its label, never written), then the task (the root reads "This session" with a "Coordinator" Badge and the model line; Bot Teams shows "Lead"); status glyph, word and duration at the end. The indent puts a child's mark under the first letter of its parent's title (36 dp per level, A85), no connector lines, and stops at two levels; deeper rows collapse into a "+1 deeper" row that opens that subtree on its own. Footer shows "Review 2 commits ready" with the child's agent icon before it (label "Review Gemini: 2 commits ready") when a child has commits (opens 7.10). The bring back card title reads "[agent icon] finished with 2 commits". TalkBack reads "name, role, state, duration" and offers Open as an action.
- **Artboard.** `an-session-more.dc.html` (phones 2 and 3).

### 7.10 X-CH-08 Child work bring back (Android side)

A card after a finished child with commits: link icon, "Gemini finished with 2 commits", the branch, the commit list (hash in mono and subject), the line "Merges into feat/billing-export without fast-forward. Nothing is pushed." and "Bring back" (primary 48 dp). Blocked states say why and never lose work: predicted conflict ("2 files will conflict. Bring back anyway, or ask Claude to resolve them."), child with uncommitted work ("Gemini has uncommitted changes. Ask it to commit first."), parent moved ahead, conflicts after the merge (opens Source control, M-DF-02, in its conflict state), already brought back (compact row). Not reviewed by the owner (pv96 build). Artboard: `an-session-more.dc.html` (second phone, ready state only).

### 7.11 Specified, not drawn

- **X-CH-09 Mention menu.** Typing `@` in a composer opens a sheet (Picker) with sections Apps, Teams, Bots, Projects, a search field and 48 dp rows with icons or the team character; a pick becomes a chip (28 dp high with a remove x) above the field. The Bot Teams composer also has an `@` button (drawn). States: closed, open, no match ("Nothing matches "xyz"").
- **X-CH-10 Team request choice card.** In a Conference room, when no bot or team matches: a Card "No bot matches this request." with radio rows ("Create a bot for this", "Ask Ada"), a suggested team with a Select to change it, and "Continue" (primary). Pending and answered (one-line summary).
- **X-CH-11 Session checklist.** The Checklist card in the summary (8.1) is drawn. On the chat screen the checklist is a transcript row ("Checklist updated, 2 of 5") that opens a sheet with the items (glyph, text, 56 dp rows), "Add item" and, on tap, an item sheet (text field, Done switch, Delete). Empty: no card, no row.

---

## 8. Summary and artifacts

### 8.1 M-CH-07 Session summary

- **Purpose.** What is going on in this session at a glance: where it runs, the plan, progress, results and helpers. pv96 built it as agent status, environment tiles, results strip and connected chips; here it uses the same cards as the desktop Session summary (see decisions).
- **Layout.** The Summary tab of the session (1.1): the floating view group with Summary (clipboard icon) selected and the session pill under it (owner question, 2026-11-13: the clipboard is the Summary, not the Plan or the Checklist; Plan and Checklist are cards inside it). A status line (AgentStatus glyph 24 dp, "[agent icon] is working", "Opus 5.5, Medium, 2:14") with "Open chat". Subagent rows read "[agent icon] Write the invoices_csv migration" (A17). Then SessionSummaryCards stacked, 12 dp apart, full width: header row 48 dp (card icon 16 dp in `text.secondary`: Environment `server`, Plan `list`, Checklist `list-checks`, Context `gauge`, Results `wand-sparkles`, Child sessions `git-fork`, Subagents `users`; title `body-strong`, value `caption` tabular, chevron), collapsed rows (at most 6, one line, mono for repo, branch, path and URL). Environment rows each carry their icon on the left with the field name as accessible label: account `user`, repository `book`, branch `git-branch`, pipeline `workflow`, host `server` with its status dot (A22, A23). Only cards that have information appear, in the desktop order: Environment, Plan, Checklist, Context, Results, Child sessions, Subagents, Connectors, Background tasks, QA servers. Child sessions (delegated sessions) and Subagents (helpers inside this session) are separate cards (A96). A card appears when its first record arrives (instant with "Remove animations").
- **Results expanded.** Subtitle "URLs of the environment, file changes and visuals recorded in this chat." URL item: globe, the URL in mono wrapping at any character, "Open" and "Copy URL" (secondary, 44 dp). A URL on the host cannot be opened on the phone: "Open" is disabled and the note reads "This URL is on This PC. Open it there." File item: file-type icon (A46 to A48), path as a link (opens 10.2). Artifact item: file-type icon, name, kind, chevron (opens 8.2).
- **States.** One card only; many cards; expanded; nothing yet (EmptyState "Nothing to summarise yet", "Cards appear as the session produces information."); host unreachable (last known values, dimmed, with the age).
- **Artboard.** `an-summary-artifacts.dc.html` (phones 1 and 2).

### 8.2 X-AR-01 Artifacts (M-AR-01 merged)

- **Layout.** App bar "Artifacts" with "2 pages, 2 files". A filter chip row (All, Pages, Documents, Images; 44 dp). Section "Pages": a two-column grid of thumbnails (120 dp high, `radius.md`, live preview of the page) with the name (`body-sm`) and "Page, 4 min ago". Section "Other files": rows with the file-type icon (typed files in their colour, generic files a grey document; A46 to A48), name, kind and size. The viewer title carries the same icon.
- **Viewer.** App bar with the file name and "Page, 4 min ago"; an info Banner "Read-only preview. Scripts, links and forms are off."; the page in a sandboxed view; a bottom row "Copy path" and "Open on This PC" (secondary).
- **States.** Loading (grey thumbnail blocks), empty ("Nothing written in this chat yet", "Pages and files that Claude writes in this session show up here."), error ("Could not load this file", Retry), unopenable type (a row with "No preview for .zip" and Copy path).
- **Artboard.** `an-summary-artifacts.dc.html` (phones 3 to 6).

---

## 8b. Prototypes and design systems (owner, 2026-11-13)

The Android form of D-AG-17 and D-AG-29, with the same rules as desktop: a design system belongs to one project and can be shared with another as a link; its sections are Overview, Colors, Typography, Spacing, Corner radius, Shadows, Motion, Components, Assets, plus custom ones. Prototypes are drawn in their own colours, not MassOrca tokens. Artboard: `an-prototypes.dc.html`.

- **M-PT-01 Prototypes.** Opened from the Projects overflow (5.1) or a session overflow (7.1). Floating bar: back, "Prototypes", search. A plain list grouped by project (overline header). The first row of each project is its Design system (palette plate, version and section count; a link chip: "Shared with 1" on the source, the source project's name on a linked one; "No design system yet" when none). Then the prototype sets: a thumbnail, the set name, "4 screens · 3 directions · 2 h ago", chevron.
- **M-PT-02 Prototype canvas.** Floating bar: back, the set pill (name and chevron; switches set), a group with Note and overflow (Send to agent, Merge direction, Inspector). Under it a floating segmented control "Directions | Design system | Versions". The canvas fills the rest: dot grid on `surface.sunken`, every screen an artboard with its name above, arrows for the flow, the selected one with the accent outline; pan with one finger, pinch to zoom. Notes are sticky notes (author and time) pinned to an element with a dashed leader; they go to the agent with Send to agent. Floating at the bottom: the direction group (A with "Main", B, C), the zoom percent, and Play (round, accent). The element inspector (fill, border, radius, padding) opens from the overflow as a sheet, never by default.
- **M-PT-02 Play.** The prototype full screen. Floating Close at the top left and Note at the top right. A floating play bar at the bottom centre: previous, "Export options, 2 / 4", next; swiping left and right also steps.
- **M-PT-03 Design system tab.** The set pill becomes the project pill ("oppm"). A version banner ("Version 7 · 2 draft changes", Review), the share chip ("Shared with NexusCare", "Edits here reach linked projects"), then the Sections list (icon, name, count, chevron) and "Add section…". Overflow: Share with project…, Versions, Archive. Long press a custom section: rename, reorder, remove; built-in sections can be hidden, not deleted.
- **M-PT-03 Section.** A section opens full screen: back, title pill with the section and "oppm · Design system", search. Colors: filter pills (All 24, Brand, Surface, Text) and a two-column grid of swatches with token name and value. Tap a token: where it is used and "Ask the agent to change it".
- **M-PT-03 Linked design system.** In a project that links another project's design system: a card "Linked from oppm", "Edits happen in oppm and reach this project. Unlink to use NexusCare's own design system, or make a copy to change it here.", buttons Make a copy and Unlink; the sections show read only with a lock.
- **Not drawn.** Merge review, the Versions list and the element inspector sheet follow the desktop spec (workspaces-sessions.md, D-AG-30 to D-AG-32).

## 9. Terminal

### 9.1 M-TM-01 Terminal, M-TM-02 key row, M-TM-03 input dock

- **Layout.** The Terminal segment of the session screen. The terminal fills the content (`terminal.bg`, 12 / 16 sp mono, `terminal.*` colours, selection by long press with handles, pinch to change the size). Above the keyboard: the key row (surface.frame, 44 dp keys with a 48 dp hit target, 4 dp apart: Esc, Tab, Ctrl, the four arrows and "more" that opens the custom key editor (M-TM-04, not drawn); Ctrl is sticky and shows `accent.subtle` when armed; holding an arrow repeats), then the input dock (a one-line mono field "Type a command" and a Send-to-terminal button).
- **States.** Loading ("Starting shell"); pending terminal (info Banner "Loading terminal. Starting the shell on This PC." with Retry); live (live-input status as a caption when the terminal is read only); engine error (ErrorState page "The terminal could not start", "The terminal view failed to load on this phone. The agent keeps running on This PC.", "Retry", "Use chat view", details "engine init failed (code 3)"); process exited ("Process exited with code 1", "Restart"); restored ("Restored from earlier. This is history, not a running process."); host unreachable (Banner 5.6; output stays, last output time shown).
- **Accessibility.** Output is not announced; an action "Read last output" reads the last 10 lines.
- **Open.** Renderer (WebView or native text grid) is the Phase 6 decision (G17); the layout works with either.
- **Desktop counterpart.** D-TM-02 (a Work panel tab).
- **Artboard.** `an-terminal-files.dc.html` (phones 1 to 3).

### 9.2 X-QC-01 Quick commands (M-TM-06 merged)

Sheet "Quick commands", caption "Tap to run in this terminal". Rows (two-line, 56 dp): corner icon, name, the command in mono (one line), a scope Badge (Project or Global), an overflow button (Edit, Delete). Pinned "New quick command" (secondary) opens the editor sheet (name, command, scope Select). Tapping a row sends the command to the terminal and closes the sheet. States: list, empty ("No quick commands yet", "Save the commands you type often."), editing, error. Artboard: `an-terminal-files.dc.html` (fourth phone).

---

## 10. Files, source control and review

### 10.1 M-FI-01 File explorer

App bar "Files" with the path as caption (`oppm / src / billing`), search action. Rows 48 dp: folder glyph or the file-type icon (TS badge and the other typed colours, grey document for .md and untyped files; A46 to A48), name, a "M" or "A" change Badge (letter plus colour) for files with changes, size or item count in `caption` tabular. First row "up one folder" (chevron-up). Ignored files are dimmed. States: loading (skeleton rows), empty ("No files found"), error (ErrorState section, Retry), search (results with the path under the name). Tap a folder to enter, a file to open M-FI-02. Artboard: `an-terminal-files.dc.html`.

### 10.2 M-FI-02 File preview

Read-only preview of text, image or HTML. App bar: file name, "path, size", actions wrap lines and overflow. Text: line numbers (`caption` tabular) and `mono` 13 / 18 with `syntax.*` colours, horizontal scroll, on `surface.sunken`. Images: fit to width, pinch to zoom. HTML: the sandbox viewer of 8.2. States: loading ("Loading preview"), "Loading media" for images, error ("Could not open this file", Retry), too large ("This file is 8 MB. Open the first 200 KB or copy the path."), binary ("No preview for this file type"). Artboard: `an-terminal-files.dc.html`.

### 10.3 M-DF-02 Source control hub (M-DF-04 merged)

- **Layout.** App bar "Source control" with "branch, project" caption, actions refresh and overflow. A three-segment control "Changes | Pull request | History", each with its icon (diff, pull request, history; A21). Changes: a branch Card (branch icon, branch in mono, "2 ahead, 0 behind origin/main", PR Badge), the overline "Changes (4)", file rows (56 dp: status letter in `diff` colours with an accessible word, file-type icon, file name, directory in mono `caption`, `+42 -3` with signs), and a pinned commit area: a multi-line "Commit message" field (12 dp corners, `border.default`, `surface.raised`, e1 shadow, focus border on focus; A49 with the A32 radius), "Commit 4 files" (primary, 12 dp corners) and "Ask Claude" (secondary, hands the commit to the agent). History: rows with a commit glyph, subject, "author, time" (an agent author is its icon, A17) and the short hash in mono.
- **States.** Loading (skeleton), error (ErrorState with Retry), unavailable (EmptyState "Git is not available in this folder", "Initialize Git" when it is a plain folder), clean (EmptyState "No changes", "Everything is committed."), conflict (danger Banner "2 files have conflicts. Merging main stopped halfway. Resolve them on This PC, or ask Claude to resolve them." with "Ask Claude"; conflicted rows show a "Conflict" Badge; Commit is disabled with "Resolve the conflicts first."), detached head (warning Badge on the branch card, Commit asks first), empty history ("No commits yet").
- **Interactions.** Tap a file opens the diff (10.4). No staging UI on the phone: Commit takes every change (see decisions).
- **Artboard.** `an-source-control.dc.html` (phones 1, 2 and 4).

### 10.4 M-DF-01 Diff review and X-RV-02 review notes

- **Layout.** Full screen. Floating title pill: file-type icon and file name, "directory, +42 -3". Inline diff only: `mono` 13 / 18, one new-line-number column and a sign column, hunk headers sticky (`surface.raised`, mono), added lines on `diff.add.bg` with `diff.add.gutter`, removed lines on `diff.remove.bg` (no number). Bottom bar (48 dp targets on `surface.frame`): previous change, next change, "1 of 2", and "Send 2 notes" (primary, shown from the first note).
- **Notes (X-RV-02).** Tap a line opens the sheet "Comment on line 18": the line in a well with its file and number, a field "Your note for Claude", the help "Notes are kept as drafts. Claude gets them when you send.", "Add note". The line shows a `border.focus` outline until the note is saved, then a comment glyph. Sending posts the notes as one message (they join the queue when the agent is busy, A10). States: draft, sent (Toast "2 notes sent to Claude"), none.
- **States.** Loading, empty ("No changes in this file"), large diff (Banner "This diff has 4,800 lines. Show it anyway?" with "Show"), conflicts skipped (info Banner), binary.
- **Artboards.** `an-source-control.dc.html` (phones 5 and 6), `an-light.dc.html`.

### 10.5 X-PR-01 Pull request view (M-DF-03 merged)

The Pull request segment. Not linked: EmptyState "No pull request for this branch", actions "Link a pull request" (a field for the number or URL) and "Create a pull request" (asks Claude, with the base chosen in X-PK-01). Linked: a Card with the PR icon, state Badge ("Open", "Merged", "Closed"), title, "main ← feat/billing-export", "#128, opened by you, 2 commits"; "Checks" card: each check is one 28 dp line (status glyph, name, status word, chevron); workflow, duration and "View log" / "Re-run" live in an expandable detail under the row, and running or failed checks open expanded (A50) (agreements pass, 2026-11-13); comment cards (name or agent icon, time, text); pinned "Add comment" and "Open on GitHub" (secondary). States: not linked, linked, creating (progress in the button), error. Artboard: `an-source-control.dc.html` (third phone).

---

## 11. Bot Teams, Calendar and Tasks

### 11.1 Bot Teams mode (Android forms of D-WS-29, D-AG-24, D-WS-30, D-AG-25)

These rows have no `M-` ID. They are drawn because Bot Teams is a core mode and pv96 builds it on the phone (`mobile/src/bot-teams`). The mode switch on Home (bot icon) swaps the Workspaces list for the team list.

- **Bot Teams home (Home in Bot Teams mode; owner, 2026-11-13, following the pv96 mobile carousel in MobileBotTeamsScreen.tsx).** Floating bar with the mode switch on "Bot Teams". A horizontal carousel of the teams: each item is the team character at 56 dp (the Bot Teams characters with eyes), a running ring around it while any bot works, a waiting badge with the count of tasks that need the user, the team name and one state word ("Working", "Idle"). The last item is "New team" (dashed circle with +). Tap a team: its Conference room opens. Long press: the team sheet (edit, delete). Under it, the tasks of every team: filter pills "All", "Needs review", "Working" with counts; section "Needs your review" first (rows tinted waiting), then "Working now". Each 64 dp row: status glyph, task title, the team character at 16 dp with "team · bot", and at the end the state word and the age or running time. Tap a task: the task in its team (Board card or Conference room thread). Empty: "No teams yet", "A team is a group of bots that work from a Conference room.", "New team". No tasks: "Nothing running and nothing to review." Superseded: the search field, the "All teams" row and the team rows.
- **Team screen.** Floating bar with the team pill (team character, name, chevron to switch team), as in the Conference room; the host is only in the footer note. Rows: Conference room ("2 open"), Board ("3 tasks" with the waiting glyph), the overline "Bots" with each bot drawn as its team character (Desarrollo: the accent triangle with two white eyes, one tone of the team colour per bot so they stay distinct; the same marks in the Conference room, delegation tree and Board; A57), role line and AgentStatus (Ada has the "Lead" Badge), the overline "Projects" with Files, Memory, Routines. Footer note "Runs on VPS, the host Desarrollo uses."
- **Conference room.** Floating bar: back, the team pill (team character, "Desarrollo", chevron to switch team), and the overflow circle (owner, 2026-11-13). Overflow sheet rows (48 dp, icon before label): Board (kanban icon, "3 tasks"), Routines ("2"), Calendar (opens the Calendar filtered to this team), Memory, Files, Team settings. The Board is no longer a bar icon. Messages follow the Conversation list: user bubbles, bot messages with an author line (avatar 24 dp, name, "Lead" Badge, time), a Delegation card (X-DL-01, Bot Teams variant: avatars, competence chips such as "Writing", "Research", plain words "Working", "Needs you", "Done"; no model, no worktree), a task chip under the message that produced a Board task ("Task on the Board: Release notes 2.4"), and a "Routine created" card ("Every Monday at 09:00, Clara checks the changelog"; opens Routines). The Composer has no model chip (Bot Teams hides depth): one floating row with attach, the field "Message the team" (`@` in it mentions a bot), dictation, Send.
- **Team-level bar (Board, Routines, Calendar).** The same team pill as the Conference room (team character, "Desarrollo", chevron to switch team; A59), with back and the page action; the page name is a heading at the top of the list with its icon (kanban for Board, calendar for Routines, calendar-days for Calendar) (agreements pass, 2026-11-13). The Board row of the team screen uses the kanban icon (A25).
- **Board.** Status sections instead of columns (BoardColumn spec, Android proposal): "Needs you", "Working", "To do", "Done" with counts; rows (56 dp) with the bot avatar, task title, "bot, step 2 of 3" and the status glyph. "Move" is a row action (sheet), not a drag. Empty: "No tasks here." Opening a task opens its thread in the Conference room (G20, partly closed).
- **Routines.** Team pill bar with "New routine" (plus); heading "Routines". Intro line; section "Routines" (rows with calendar icon, name, "Every Monday at 09:00, Clara, next in 4 days", Switch); section "Web addresses" (EmptyState compact: "No web addresses yet", "Another app can call a web address, a webhook, to start one task in Desarrollo.", "New web address"); footer note. Creating a routine and a web address on the phone uses the simplified forms (not drawn).
- **States.** Host unreachable (Banner 5.6; rows dimmed with the age), team empty, loading skeletons, error ErrorState with Retry.
- **Artboard.** `an-botteams.dc.html` (phones 1 to 6; phone 4 shows the Conference room overflow sheet).

### 11.2 Calendar (Android form of D-AG-23)

Opened from the Home overflow menu (all teams) or the Conference room overflow (filtered to the team). From the Conference room the bar shows the team pill and the heading "Calendar"; from Home the title pill "Calendar". Header: previous and next (48 dp), the range title (`title-sm`, "9 - 15 Nov 2026"), "Today". A week strip of seven day cells (weekday letter, date tabular; today has a 2 dp `accent.fg` outline). Filter chips: the team ("Desarrollo", opens a Select sheet) and "Automations". Control "Day | Week | Month" (Week selected). Below: an agenda grouped by day ("Today, Thu 12 Nov"): rows with the time (tabular), the routine or automation name, "bot, team", the team character or an "Automation" Badge, and a done glyph. A pinned "Show done" Switch row. Tapping a routine opens Routines (A23); an automation shows its name and schedule in a sheet (editing is desktop only). States: loading, empty ("Nothing scheduled this week"), loaded, long names truncate. Month is a month grid (dots per day, tap a day to see its agenda; not drawn). Artboard: `an-botteams.dc.html` (seventh phone, from the Conference room).

### 11.3 M-TK-01 Tasks

- **Purpose.** One list for Workspaces and Bot Teams: approvals first, then issues, pull requests and Board tasks.
- **Layout.** App bar "Tasks" with a filter action. Filter chips (All, Needs approval, GitHub, Bot Teams). The overline "Needs your approval" with a count; rows (56 dp) tinted `agent.waiting.bg` with the waiting glyph, the request ("Run pnpm db:seed"), "workspace, project" or "team, bot" an origin Badge ("Workspaces" or "Bot Team") and a single "Review" button (A45; tapping the row does the same) that opens the session or the Conference room at the card. Then "Open": rows with a provider glyph, title ("#214 Invoice rounding is off by one cent"), "project, issue" and the provider name at the end. Extended FAB "New task".
- **Create task (sheet "New task").** Segmented control "Workspaces | Bot Team" (grid and bot icons); Bot Team shows a Team Select; a field "What needs doing"; "When it starts" segmented ("Start now | Later"; Later reveals a date and time row); pinned "Create task". Workspaces mode asks for the project and provider instead of the team.
- **States.** Loading (skeleton rows per provider), empty (EmptyState "No tasks", "Connect GitHub or Linear on This PC to see issues here, or create a task.", "New task"), per-provider error (Card "Could not load GitHub tasks", "The request timed out. Tasks from other providers are not affected.", "Retry"), not connected (Card "Connect Linear", "Sign in on This PC to see Linear issues here."), submitting, failed.
- **Desktop counterpart.** D-WS-24.
- **Artboard.** `an-tasks.dc.html`.

---

## 12. Settings, notifications, updates, Marketplace

### 12.1 M-ST-01 Settings menu (M-ST-04 merged)

App bar "Settings". At the top, when needed, the pending credential cleanup notice (warning Banner "2 old credentials are still on this phone. They were left by an earlier pairing." with "Review" and "Remove"), then the app update Card (12.3). Link rows (56 dp, icon, label, value, chevron) grouped by overlines that follow the desktop sections: General (Appearance: System, Light, Dark; Default session view), Agents (Accounts and usage), Terminal and browser (Terminal, Browser: Phone or desktop), Notifications and voice (Notifications, Voice model), Apps (Marketplace, after Notifications as in pv96; then "Extensions", puzzle icon, "Managed in Marketplace, Installed", ending with an arrow because it opens Marketplace, Installed; A68, A69), Hosts and security (Hosts, Secret vault), Privacy and advanced (Troubleshooting, Privacy policy), About (About, Support). External rows open the browser and show an external-link glyph. States: notice shown, update section states, disabled rows (with the reason in the helper line). Artboard: `an-settings.dc.html` (first phone).

### 12.2 M-NT-01 Notifications, X-NT-02 OS notifications, M-ST-02 Voice model

- **Notifications.** When the OS permission is off: a warning Banner "Notifications are off in Android settings. MassOrca cannot tell you when an agent needs you." with "Open Android settings", the rows below dimmed and disabled, "Send a test notification" disabled. When on: rows "An agent needs me" (approvals and questions), "A run finishes", "A run fails", "A Bot Teams task needs me", each a Switch row with a helper line; "Show message previews"; "Send a test notification" (primary) shows a real notification.
- **OS notification (X-NT-02).** One Android channel per row above. Title the workspace, text "Claude is waiting for you: Run command"; tapping opens the card in M-CH-01. The notification only announces attention; acting happens in the app. With a source off, no notification, but the row still shows the state.
- **Voice model (M-ST-02, M-ST-03 merged).** Line "Dictation runs on this phone. Your voice is not sent to a server." Radio rows (two-line): name, size and trait, and a state (Badge "Downloaded", a "Download" button, or a progress percentage while downloading). States: model list, downloading, download failed ("Could not download. Check the connection." with Retry), storage low.
- **Artboard.** `an-settings.dc.html` (phones 2 and 4).

### 12.3 X-UP-01 App update notice

Card (inline, no shadow) at the top of Home and a section at the top of Settings: states Available ("MassOrca 0.9.4 is available", "Update"), Checking (spinner), Downloading ("Downloading MassOrca 0.9.4", "You have 0.9.3. 62% of 18 MB.", 4 dp Progress), Ready ("MassOrca 0.9.4 is ready", "Restart the app to install it.", "Restart"), Error ("Could not download the update", "Retry"). Swipe or x dismisses a Ready or Available card until the next launch. Artboards: `an-home.dc.html` (Ready), `an-settings.dc.html` (Downloading).

### 12.4 M-EX-01 Marketplace (inside Settings)

- **Why here.** pv96 puts it in the Settings menu, after Notifications; pending item 10 said v1 has no Android entry, and this follows pv96 (see decisions).
- **Catalog.** App bar "Marketplace" with search. A segmented control "Discover | Installed | Updates" (compass, box and refresh icons; A68), Installed selected in the artboard. A host chip ("On This PC", opens a Select when several hosts are paired), a chip row (Tools, Agents, Skills, Services). Rows (two-line, 56 dp): 40 dp plate, name, one line ("Version control, detected on This PC"), a state Badge ("Installed", "Connected", "Not connected", "Detected"). Items follow pv96: Git, GitHub, Engram, OpenMetadata, "Your extensions" ("Made in Creator mode or added from a third party"; extensions live here, not in Settings), and the agents. Under the list the split button "Add extension ▾" (Create with chat, From the Marketplace, and under Third party From a Git URL…, From a folder…, From a file (.zip)…; A63).
- **Item.** Header (48 dp plate, name, line, state Badge); "Connection" (accounts as rows with "Add account"); "Who can use it" (a Switch per agent with its agent icon, and one for Bot Teams with the bot icon); "Danger zone" with "Disconnect GitHub" (secondary, danger text) which asks X-DLG-01 and names what stops working.
- **Credential form** (for example OpenMetadata). Line "Agents use it to look up tables and owners. The token is stored on This PC and never goes into a chat.", "Server address" (mono field), "Access token" (masked, Show, help "Shown once here, never again."), "Connect" (primary). Validation names the field; success returns to the item with the "Connected" Badge; failure keeps the typed values except the token's visibility.
- **States.** Loading (skeleton rows), listed, item, credential form, error (ErrorState "Could not load the Marketplace from This PC", Retry), host unreachable (Banner).
- **Artboard.** `an-settings.dc.html` (phones 5 to 7).

---

## 13. Secret vault

### 13.1 X-CH-07 Secret request card (Android)

- **Layout.** A card in the chat, `agent.waiting.bg` while pending: key icon in a 28 dp plate, title "[agent icon] needs a secret" (the icon replaces "Claude", A17; or "Add a secret for oppm" when the user asked), the state at the end (waiting glyph and "Waiting for you"), the variable name in a `mono-sm` chip (wraps at any character, at most 64), a reason line (plain text, at most 200 characters), the safety line with a lock "The value is stored on This PC and never goes into the chat.", then two stacked 48 dp buttons: "Add secret" (primary) and "Decline" (secondary).
- **Resolved rows (48 dp, `surface.raised`).** Stored: check circle, "Stored", name chip, "oppm", "Open vault". Linked: link icon, "Linked", "from project NexusCare". Declined: ban icon, "Declined". Failed: octagon-alert, "Not saved: <reason>", "Retry" and "Decline".
- **States.** Pending, entering (buttons disabled, "Entering secret"), stored, linked, declined, failed, host unreachable (pending stays pending; Add disabled; line "Can't reach This PC. Nothing can be saved until it is back." with the unreachable glyph; Decline still works).
- **Security.** Title, name and reason come from the agent and are plain text with length limits; the name is validated (letters, digits, underscores, not starting with a digit). No value ever travels over this card.
- **Artboard.** `an-secrets.dc.html` (first phone).

### 13.2 X-VT-01 Secret entry (bottom sheet)

- **Layout.** Sheet "Add secret" (full height when the keyboard is open). Info line "Agents get it as an environment variable when they run. It is stored on This PC and never goes into the chat." Fields (48 dp): Variable name (mono, prefilled from the card, help "Letters, digits and underscores. Cannot start with a digit."), Value (masked, mono, "Show" at the end as a 48 dp target; revealed values hide again after 10 seconds, on blur and on close; pasted multi-line values show "Multi-line value, 12 lines" while hidden), Scope (segmented "Project oppm | Global"; a Team segment appears in a Bot Teams context; caption "Only agents working in oppm can use it."), and the text button "Link an existing secret instead". Footer: "Save secret" (primary) and "Cancel".
- **States.** Entering (Save disabled until the name is valid and the value is not empty); invalid name (field border `status.danger.fg`, "Use letters, digits and underscores. It cannot start with a digit."); name in use ("OPENAI_API_KEY already exists in this scope." with "Pick another name" and "Link the existing secret"); saving (spinner in Save, fields read only, Cancel stays); stored (the sheet closes, the card shows Stored; no success screen); host unreachable (Banner "Can't reach This PC. Nothing can be saved until it is back."; Save disabled; nothing queued); failed (Banner "Not saved: <reason>. Nothing was stored."; the typed value stays for Retry); link mode (radio rows with the name in mono, the owning scope Badge and the last rotated date; empty: "No secrets in other scopes. Enter a new value instead." and "Back").
- **Security.** `FLAG_SECURE` on the window while the sheet is open (no screenshots, blank in recents); `secureTextEntry`; autofill off (`importantForAutofill="no"`); copy blocked while hidden; the buffer is zeroed on close; TalkBack reads "Secret value, hidden". Escape-equivalent (back or swipe down) closes without a confirm and discards the text.
- **Artboard.** `an-secrets.dc.html` (phones 2 to 6).

### 13.3 M-VT-01 Secret vault list

App bar "Secret vault" with the host as caption (reached from Settings, Hosts and security; A27). A info Banner "Edit, rotate and delete are on the desktop. Values are never shown." (A36). Sections by scope (Project oppm, Global; Team when it applies): rows (56 dp) with a key icon, the name in mono, "Used by [agent icon], 2 h ago" (agent icon and time, never values; A31), the scope Badge and, for linked secrets, a "Linked" Badge (read only). Extended FAB "Add secret" opens 13.2. States: empty (EmptyState "No secrets yet", "Keys and tokens that agents use. Values stay on This PC.", "Add secret"), listed, loading (skeleton), host unreachable (Banner, FAB disabled with the reason), error. Artboard: `an-secrets.dc.html` (phones 7 and 8).

---

## 14. Shared patterns on Android

- **X-DLG-01 Confirm dialog.** A Dialog (the one pattern that is not a sheet): 280 to 560 dp wide, `surface.overlay`, `radius.xl`, title in `title`, body in `body`, buttons at the bottom right (48 dp, secondary first, then primary or danger). The destructive variant lists what is lost. Busy: the primary shows a spinner and Cancel stays. Error: a line above the buttons. Android back cancels unless busy. Examples: "Remove This PC?", "Forget this host?", "Delete feat/billing-export?" (inline in its sheet, 6.2), "Disconnect GitHub?".
- **X-DLG-02 Sheet container.** See Conventions. Opens with `duration.slow`; focus moves into the sheet and returns to the trigger; the title is announced.
- **X-PK-01 Picker.** A full-width sheet at 90 percent height: a back row for nested folders, a search field at the top, 48 dp rows with a check on the current one; states loading (skeleton rows), empty ("No branches match"), error (ErrorState section, Retry), filtered.
- **X-NT-01 Toast.** A snackbar-style toast: full width minus 16 dp, at least 48 dp, one 48 dp action, anchored above the Composer or the bottom bar and clear of the keyboard and system insets. Kinds info, success, error, action; a toast with an action stays until dismissed except Undo (6 s).

## 15. Tablet (M-SH-01 wide)

`an-tablet.dc.html` shows an expanded 1280 by 800 dp window (the same layout from 600 dp). Master pane: 360 dp on `surface.frame` with the floating Home bar (Settings, the "Workspaces | Bot Teams" mode switch, overflow with the theme; A79), the heading "Projects" and the Projects list of M-PJ-01 (project headers, host labels, agent icons, nested child sessions with the 80 dp indent; A82 to A85, A88); the open session has `fill.selected.frame` and the accent bar. Detail pane: a 64 dp floating bar on the frame band (88 dp with the inset) with the session pill, the view group Chat, Summary, Files, Terminal and overflow (A76, A87), then the canvas (`radius.canvas` top-left corner) holding the conversation in a column of at most 720 dp, the status pill and the floating Composer (+, model chip, reasoning, access lock, microphone, Send). The Summary is the Summary view of the group, not a third column (agreements pass, 2026-11-13). Landscape phones use the compact layout.

---

## Decisions for owner review

| # | Decision taken | Alternative | What changes with the alternative |
|---|---|---|---|
| 1 | Marketplace is a Settings row on Android (after Notifications), following pv96 and this pass's brief. Pending item 10 and the design-system said no Android entry. | No Android Marketplace in v1. | Section 12.4 and the three Marketplace phones go; the Settings menu loses a row. |
| 2 | Android has a Summary screen (M-CH-07) built from the desktop summary cards in one column. The SessionSummaryCard spec says Android has no card column. | Keep only the sheets and the pill (plan, background work, status). | Section 8.1 and two phones go; the Summary action leaves the session bar; the Environment and Context cards have no phone home. |
| 3 | Bot Teams mode is drawn on Android (team list, team, Conference room, Board as sections, Routines, Calendar agenda) although the inventory has no `M-` rows for it and several component specs say "Android: none in v1". | Android Bot Teams is the Conference room only, reached from a short team list. | Board, Routines, Calendar and the team screen go; Tasks still lists Board tasks. |
| 4 | The Home top bar holds the mode switch (leading), the mode name as the title, theme, Settings and an overflow with Calendar; Tasks stays a Home list row. pv96 puts a Tasks icon in the bar. | Tasks as a top bar icon (pv96). | One more bar icon, the Tasks row leaves Home. |
| 5 | One press of the Android mode switch always switches mode (pending item 14, "return home first", is a desktop behaviour; the control exists only on Home). | Same rule as the desktop. | Needs a "home" state the phone does not have. |
| 6 | Superseded by 1.1 (owner, 2026-11-13): Chat, Summary, Files and Terminal form the floating view group and the Sessions chip became the floating session pill. Originally: the session screen has a Chat and Terminal control and a Sessions chip in one row under the bar, a Summary action, and an overflow holding Plan, Artifacts, Files, Source control, Prototypes and Quick commands. | Put those entries in the workspace action sheet (M-WS-02) only. | Fewer controls on the session screen; more taps to reach Plan and Artifacts. |
| 7 | Approve and Deny are equal-width buttons side by side, 48 dp (the desktop card right-aligns them). | Stacked full-width buttons (as the secret card). | Taller card; fewer accidental taps next to each other. |
| 8 | Source control on the phone has no staging: Commit takes every change, with an "Ask Claude" option; segments are "Changes, Pull request, History" (the inventory says Status, PR, History). | Per-file checkboxes for staging. | A checkbox column and "Stage all"; the commit button counts staged files. |
| 9 | Review notes are drafts sent together with a "Send N notes" button in the diff bottom bar. | Send each note at once. | No bottom send button; more messages to the agent. |
| 10 | Tapping a quick command runs it at once (as D-TM-09). | Tap inserts the command in the input dock, so the user presses Send. | Safer for commands like `kill`; one more tap. |
| 11 | The terminal key row has eight keys: Esc, Tab, Ctrl, four arrows, and "more" for custom keys. | Add the slash, dash and pipe keys as fixed keys. | The row scrolls horizontally or the keys shrink below 44 dp. |
| 12 | Host cards show usage rings with a percentage each, an attention summary line, and a long-press sheet (Open, Accounts and usage, Edit host, Connection log, Forget). | Rings only; usage detail from Settings only. | The long-press sheet goes. |
| 13 | The Android secret entry shows the scopes Project and Global and adds Team only in a Bot Teams context. The vault list shows "Used by Claude, 2 h ago" (read only). | Always show three segments; no usage line on the phone (usage is a desktop vault feature). | The segmented control and the list row change. |
| 14 | A URL on the host cannot be opened on the phone: "Open" is disabled with "This URL is on This PC. Open it there." (the desktop note says to forward the port). | Offer a port forward from the phone. | A new flow not in the inventory. |
| 15 | Sign-in for agent accounts happens on the host. The Android X-AC-01 card only says where; it gets an action only when the host supplies a link. | An in-app browser sign-in. | A new flow and its security review. |
| 16 | Calendar on Android is an agenda (week strip, filters, grouped list); automations are read only there. | A time-grid week view. | A denser screen that needs a landscape layout. |
| 17 | Chips and small buttons are 44 dp visual with a 48 dp hit target, like every control (the desktop chips are 24 px). | 40 dp chips. | Below the design-system touch rule. |
| 18 | Agent and team marks are monogram plates in the artboards. | Real brand marks. | Assets only; layout unchanged. |
| 19 | Settings groups follow the desktop section names (General, Agents, Terminal and browser, Notifications and voice, Apps, Hosts and security, Privacy and advanced, About). | The inventory's original flat list. | Section headers go. |

## Component gaps

Components the Android screens use that have no spec in `docs/design/components/`:

| Gap | Used by |
|---|---|
| AppBar (56 dp top app bar on `surface.frame`: leading, title with caption, up to three actions) | Every screen |
| Chip (tappable filter and option chip, 44 dp visual, selected and unselected) | Filters, suggestions, session options, Marketplace |
| HostCard (Card with plate, status Badge, UsageRings, attention line, dimmed unreachable form) | M-HM-02 |
| StatusPill (floating pill with glyph, word, clock and counts) | M-SE-07 |
| Fab (extended floating action button, 56 dp) | Workspace list, Tasks, Teams, Vault |
| BottomBar (pinned action bar: diff change bar, commit area, key row container) | M-DF-01, M-DF-02, M-TM-02 |
| KeyRow (terminal keys with sticky Ctrl and repeat) | M-TM-02 |
| ViewfinderFrame (camera frame with corner brackets) | M-PA-02 |
| TeamMark (the team character) | Bot Teams, Calendar |
| CalendarAgenda (week strip and grouped list) | Calendar on Android |
| ArtifactThumb (live thumbnail tile) | X-AR-01 |
| ChecklistSheet and MentionSheet | X-CH-11, X-CH-09 (not drawn) |

Token gaps: no `a-overline` type token in `massorca.css` (drawn as 12 / 16 / 600, capitals, +0.04em); no shadow token for the FAB (drawn with a 6 dp elevation value); the sheet drag handle (32 by 4 dp) and `layout.sheet.max-height` exist only in the foundations text. The token sheet defines `--shadow-e1`, `--shadow-e2` and `--shadow-e3` as objects, so the artboards declare the box-shadow values themselves.

## Not covered

- Prototypes on Android (M-PT-01, M-PT-02): merged into D-AG-17 and drawn by that area's writer; here only the entry (session overflow row "Prototypes") is specified.
- X-CH-09 mention sheet, X-CH-10 request choice card and X-CH-11 checklist sheets and item sheet are specified in text only (7.11); the bring back blocked and conflict states (7.10) are text only.
- The custom key editor (M-TM-04), the Terminal settings page (M-TM-05) and the Browser settings page are not drawn.
- Month and Day calendar views, a routine's detail sheet, the simplified routine and web address forms on the phone, the New team form, the bot form, team Files and Memory screens.
- Landscape phone layouts, foldables, a three-column tablet layout, picture-in-picture.
- Font scale at 200 percent, TalkBack reading order diagrams, high-contrast text and "Remove animations" renderings (the rules are written, the comps are drawn at default settings).
- The OS notification visuals, the Android share sheet, the camera permission dialog and the system keyboard (system UI).
- The app update Error state, the Google Play update path for an outdated app (copy only in 4.2), and the in-app browser used by the Browser setting.
- The pairing deep link when the app is closed (behaviour only, in M-PA-02), pairing over relay (not in v1), SSH-host pairing on the phone.
- Light theme is drawn for Home, chat and diff review only; every other artboard is dark.
- Pixel measurements were not run on a device; sizes are the design-system values in dp and sp.
