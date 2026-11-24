# Bot Teams, Calendar, Settings, Customisation and Secret vault: desktop screen specs

Status: draft for owner review, 2026-11-12. English. Desktop rows only (GPUI on Windows, macOS and Linux). Android rows of these areas are not designed here.

Source rows: `docs/design/screen-inventory.md` sections 5 (Bot Teams rows), 14 (Settings), 15 (Customisation), 19 (Secret vault), plus the Bot Teams and chat rows the owner reference needs. Terms follow `glossary.md`. Components follow `docs/design/components/*.md`. Tokens follow `design-system.md` and the token file used by the artboards.

## 1. Artboards

Fourteen artboards, each 1440 x 900 px, in `screens/project/`. Artboard numbers in this file follow the order of the table below. Dark is the default; `data-theme="light"` on the artboard root gives the light theme. Every area has at least one light artboard.

| File | Shows | Theme | Rows covered |
|---|---|---|---|
| `bt-conference-room.dc.html` | Conference room: a requirement becomes a task, a lead bot delegates to two bots, a bot schedules a routine | Dark | D-AG-24, D-WS-29, X-DL-01 (inline) |
| `bt-board.dc.html` | Team Board with four columns, a delegated task and a host that is unreachable | Light | D-WS-30, D-WS-29 |
| `bt-routines-empty.dc.html` | Routines page with both owner-reference empty states | Dark | D-AG-25 (empty states) |
| `bt-routine-form.dc.html` | Plain-language routine form, weekly schedule, live preview | Dark | D-AG-34 |
| `bt-calendar-week.dc.html` | Calendar, week view, routines and automations, team and Automations filters | Light | D-AG-23 |
| `bt-marketplace-catalogue.dc.html` | Marketplace catalogue, six items in all their states | Dark | D-EX-01 |
| `bt-permission-review.dc.html` | Permission review before install, over the item page | Dark | D-EX-02, D-EX-08 (item page behind) |
| `bt-extension-manager.dc.html` | Extension manager in Marketplace > Installed, with a row menu open | Light | D-EX-03, D-ST-09 |
| `bt-build-extension.dc.html` | Create an extension: a session in Creator mode with a live preview, files and logs | Dark | D-EX-05 |
| `bt-extension-app.dc.html` | Advanced extension example: Sticky Wall as a full page with a Sidebar entry | Dark | D-EX-05 (example) |
| `bt-extension-slot.dc.html` | An extension's panel in the Project sidebar slot, with its rail item and status item | Dark | D-SL-01, D-SL-02, D-SL-09, D-EX-06 |
| `bt-settings-appearance.dc.html` | Settings with search "theme", Appearance and layout section | Light | D-ST-03, D-CP-04, D-CU-01, D-CU-03 |
| `bt-settings-keybindings.dc.html` | Input and shortcuts: keybinding editor with a recorded shortcut that conflicts | Dark | D-ST-04, D-CU-02, D-SL-07 |
| `bt-secret-request.dc.html` | Secret request card in a conversation, with the Secret modal open | Dark | X-CH-07, X-VT-01 |
| `bt-vault.dc.html` | Secret vault with a shared secret and the delete confirm with a linked count | Light | D-VT-01, D-VT-02 |

## 2. Row coverage

Every row this file owns ends up in one of these states: designed in an artboard, specified in text here, merged into another row, dropped, or an extension.

| Row | State | Where |
|---|---|---|
| D-WS-29 Sidebar in Bot Teams mode | Designed | 4.1; artboards 1, 2, 3, 4 |
| D-WS-30 Team Board | Designed | 4.3; artboard 2 |
| D-WS-31 Team project brief, board and link | Specified | 4.4 |
| D-AG-13 Bot teams page, team creation and add-bot | Specified | 4.5 |
| D-AG-24 Conference room | Designed | 4.2; artboard 1 |
| D-AG-25 Routines page | Designed (empty states) and specified (list, web addresses) | 4.6; artboard 3 |
| D-AG-26 Team memory | Specified | 4.8 |
| D-AG-33 Bot form and advanced options | Specified | 4.5 |
| D-AG-34 Bot team routine form | Designed | 4.7; artboard 4 |
| D-ED-13 Team files | Specified (reuses D-ED-11 and D-ED-01) | 4.9 |
| D-AG-23 Calendar page | Designed | 5; artboard 5 |
| D-ST-01 to D-ST-12 Settings sections | D-ST-03, D-ST-04 and D-ST-09 designed; the rest specified | 6.3 |
| D-CP-04 Settings search | Designed on the Settings page | 6.2; artboard 11 |
| D-EX-01 Extension browser (Marketplace) | Designed | 7.1; artboard 6 |
| D-EX-02 Install and permission review | Designed | 7.3; artboard 7 |
| D-EX-03 Extension manager | Designed | 7.4; artboard 8 |
| D-EX-04 Extension settings sections | Specified | 7.5 |
| D-EX-05 Create an extension | Designed | 7.6; artboard 9 |
| D-EX-06 In-app application | Specified; the slot is shown | 7.7; artboard 10 |
| D-EX-07 Share skills | Dropped | 7.10 |
| D-EX-08 Marketplace item page | Designed behind the review | 7.2; artboard 7 |
| D-SL-01 Slot: Project sidebar tab | Designed | 7.8; artboard 10 |
| D-SL-02 Slot: status bar item | Designed | 7.8; artboard 10 |
| D-SL-03 Slot: chat card | Specified | 7.8 |
| D-SL-04 Slot: main view | Specified | 7.8 |
| D-SL-05 Slot: sidebar section | Specified | 7.8 |
| D-SL-06 Slot: settings section | Specified | 7.8 |
| D-SL-07 Slot: command and shortcut | Designed (conflict) | 7.8; artboard 12 |
| D-SL-08 Slot: Work panel tab kind | Specified (one rule) | 7.8 |
| D-SL-09 Slot: activity rail icon | Designed | 7.8; artboard 10 |
| D-SL-10 Slot: model picker entry | Specified | 7.8 |
| D-CU-01 Theme and layout presets | Designed (row) and specified | 7.9; artboard 11 |
| D-CU-02 Keybinding preferences | Merged into D-ST-04 | 7.9 |
| D-CU-03 Theme import | Merged into D-ST-03 | 7.9 |
| D-VT-01 Secret vault screen | Designed | 8; artboard 14 |
| X-VT-01 Secret entry modal | Designed | 8; artboard 13 |
| D-VT-02 Delete secret confirm | Merged into X-DLG-01; the destructive variant is shown | 8; artboard 14 |
| X-CH-07 Secret request card | Designed (desktop) | 8; artboard 13 |

Rows not owned here but shown: D-SH-12 (Calendar rail entry, shell writer), X-DL-01 and X-CH-10 (delegation and team request choice, chat writer), X-CH-09 (mention menu, chat writer), D-SH-09 to D-SH-16 (rail, shell writer). Artboards use them as they are specified there.

## 3. Shared rules for these screens

- **Frame.** Activity rail (48 px, `surface.frame`), Sidebar (256 px, same fill, no seam), Title bar (36 px on Windows, a band of the frame), Canvas (`surface.canvas`, rounded top-left corner `radius.canvas` 12 px), Status bar (24 px, at the bottom of the Canvas column). Window controls sit at the right of the Title bar on Windows.
- **Full pages.** Calendar, Marketplace, Settings and the extension pages open over the whole Canvas. The Title bar shows Back and the page title. The Sidebar is hidden there, as in the owner reference for Calendar. Exception: the Bot Teams pages keep the Sidebar, as in the owner's Routines reference (decision 1).
- **Dialogs.** The Dialog (X-DLG-01), the permission review and the Secret modal use the shared scrim (`scrim`, 0.30 to 0.80 alpha) and `surface.overlay`. They use core components only; no extension content can enter them.
- **Sample data.** Project oppm, NexusCare, perulaunch-web, massorca; hosts This PC and VPS; agents Claude, Codex and Gemini; team Desarrollo; bots Mara (lead), Bruno and Iris. Extension publishers "MassOrca extras" and "Community" are placeholders. Routine and task names are examples.
- **Dates.** The artboards use today, Thursday 12 Nov 2026. Week view is 9 to 15 Nov 2026, as in the owner reference.
- **Keys.** `Mod` is Ctrl on Windows and Linux, Cmd on macOS. Shortcut names in the artboards use Ctrl. Defaults taken from the design docs: Ctrl+W closes the active Work panel tab (pending-owner-review item 12); Ctrl+T adds a terminal tab (D-SH-17). Ctrl+K, Ctrl+P, Ctrl+B and Ctrl+Alt+N are example defaults, not decided.
- **Honest state.** Nothing says "saved", "deleted" or "installed" before the host confirms it. A disconnected host shows its last known state with its age, dimmed, in the `agent.unreachable` style. Nothing is queued silently; where a save is refused, the screen says so.
- **Status never by colour alone.** Each state has a glyph and a word.

## 4. Bot Teams

### 4.1 Sidebar in Bot Teams mode (D-WS-29)

**Purpose.** Lists the teams and their Conference room and Board, and the Projects section for the selected team (Files, Memory, Routines).

**Layout.** Sidebar 256 px. Search field (28 px, `surface.sunken`) at the top. "All teams" row (grid glyph), then each team row with its team character (Desarrollo: triangle in `accent.default` with two eyes, as in the Android team pill; A57, see component gap G-1). Under a team: Conference room and Board rows (Board with the kanban glyph, columns of different heights; A25) (28 px, indented 30 px), each with a status hint in `caption` at the right ("2 running", "Nothing open", "5 tasks", "No tasks"). Section header PROJECTS (overline) with Filters and New project icon buttons (28 px). Rows Files, Memory, Routines.

**States.** Empty (no teams): one line "No teams yet" with the "All teams" row and a New team button, which opens the team creation dialog (4.5). Item selected: `fill.selected.frame`. Host unreachable for a team: the team row shows the unreachable glyph and "Last known state, 5 min ago".

**Keyboard.** Tab stops are rows. Up and Down move through rows; Right opens a team's children; Enter opens the row. Search is the first stop.

**Artboards.** 1, 2, 3, 4.

**Desktop and Android.** Desktop only here.

### 4.2 Conference room (D-AG-24)

**Purpose.** The team's chat. A requirement the user states becomes a task on the Board (D-WS-30). A composite requirement goes to the lead bot, which delegates the parts by competence (X-DL-01). A request to schedule work becomes a routine (4.6, 4.7).

**Layout.** Conversation in the Canvas, column 720 px centred (`layout.conversation.reading`). Header 44 px: the room name in `title-sm`, the team pill (team character, "Desarrollo", chevron to switch team; borderless, hover fill; A59), the bots as 22 px team characters (Desarrollo triangle with eyes, one shade per bot: Mara, lead; Bruno; Iris; A57), and the bot count ("3 bots") in `caption` at the right. Thread: user messages right-aligned in a `surface.raised` bubble (`type.chat`, 14/22). Bot messages: 28 px team character, name in `body-strong`, a Lead chip for the lead, status (dot and word), time in `caption`.

Message blocks, in order of use:
- **Task added** card (tcard): plate with the Tasks glyph, overline "Task added to the Board", title, meta "Desarrollo Board, Running, made from your message", and a ghost "Open on Board" action with the kanban glyph (A25).
- **Delegation** block (X-DL-01): header "Delegation, 2 child bots"; the lead as the first row, then one row per child with a left rule, its status dot and word, the agent icon (14 px, name as tooltip and accessible label; A17), the part it owns, and the elapsed time.
- **Routine created** card: repeat glyph, overline "Routine created", title, meta with the schedule, the host and the next run, and "View routine" (repeat glyph).

Composer: the session Composer body (A33, A34), 720 px column, 12 px corners, light shadow: placeholder "Ask anything, @ to mention files, / for commands"; bottom row +, the access chip "Workspace write", the lead-bot selector (Mara's character, "Mara", lead) in place of the model picker, microphone and a round Send.

**Content.** The example conversation shows: "We need magic link sign-in for oppm. Keep the admin password login as it is." Mara answers that it is a composite requirement, delegates the API part to Bruno and the copy part to Iris, and later schedules "Check staging sign-up errors" every Monday at 09:00 on VPS.

**States.** Nothing open (the sidebar hint, and an empty room with the composer and one line "Ask a bot, or describe what the team should do."). Chat open. Bot running (dot and word). Delegation partly done (a child shows Done with its time). Request choice (X-CH-10, when no bot or team matches: the choice card is shown in the thread; its spec belongs to the chat writer). Host unreachable: the composer keeps the typed text and shows "Can't reach VPS. Your message is kept in the box, not sent." (decision 8). Long content: messages wrap at the column; a delegation block with more than four children collapses to the first three and "Show 2 more".

**Interactions and keyboard.** Enter sends; Shift+Enter adds a line. @ opens the mention menu (X-CH-09): Up and Down move, Enter picks a bot, Escape closes it. Tab order: thread, delegation rows (Right and Left expand and collapse), composer, Send. Enter on a delegated child opens that child's session (assumed).

**Desktop and Android.** Desktop only here. Android is not designed for the room in v1.

**Artboard.** 1.

### 4.3 Team Board (D-WS-30)

**Purpose.** The team's tasks, generated from the Conference room.

**Layout.** Header 56 px: "Board", the team pill (team character, "Desarrollo", chevron to switch team; borderless, hover fill; A59), the note "Tasks come from the Conference room", and a Filter ghost button at the right. Four columns (BoardColumn): Open, Running, Waiting for you, Done. Column width 280 px (`layout.board.column.width`, see section 11), `surface.sunken`, 8 px padding, gap 8 px. Columns keep their width; the Board scrolls sideways in a ScrollArea. Cards: `surface.raised`, `border.default`, `radius.lg`, 10 px padding, title in `body` (two lines, then ellipsis), a meta line with the status (dot and word) and the bot name, and the time in `caption`.

**Content.** Open: "Write release notes for oppm", Unassigned. Running: "Magic link sign-in for oppm" (Mara, 09:42, two bots delegated); "Login page copy and error states" (Iris, 10:05); "Nightly summary to the team" (Bruno, runs on VPS, "Last known state, 5 min ago", dashed border). Waiting for you: "Approve the magic link email template" (Iris, "Needs your reply in the Conference room"). Done: "Weekly digest posted to the Board" (Mara, 08:00).

**States.** Empty column: "No tasks" centred in `text.tertiary`. Empty Board: "No tasks" in the sidebar hint, and the Board shows four empty columns. Loading: card-shaped skeletons. Error in one column: ErrorState inside that column; other columns keep working. Host unreachable: the card shows the unreachable glyph and the last known state. Focused card: 2 px focus ring, offset 2 px.

**Interactions and keyboard.** Space picks up the focused card; Left and Right move it to the next column; Up and Down move it within a column; Space drops; Escape returns it. Each move is announced ("Moved to Running, position 2 of 3."). Pointer: drag with a 2 px drop line. A move changes the column only; it does not start or stop an agent (proposed in BoardColumn). Opening a task has no detail row yet (G-7): Enter does nothing beyond focus.

**Desktop and Android.** Desktop only; no Android Board in v1.

**Artboard.** 2 (light).

### 4.4 Team project brief, board and link (D-WS-31)

**Purpose.** A Bot Teams project inside a team: its brief, its own board of tasks, an optional link to a Workspaces project, and delete.

**Layout.** Page with Back; header with the project name and the team chip. Section "Brief" (a text area, 6 lines, `body`, with "Saved" status when the store confirms). Section "Board" (BoardColumn set with this project's tasks, same rules as 4.3). Section "Link to a Workspaces project": a Select with the projects on the same host, and the text "Links the Board to a Workspaces project's tasks. Optional." When the host cannot link, the Select is disabled with "This host can't link projects. Connect it, or leave the link off." Danger row at the bottom: "Delete project" (ghost, danger text), which opens the Dialog destructive variant: "Delete project Desarrollo onboarding?" with the loss list (its routines stop, its tasks are removed from the Board) and "This cannot be undone."

**States.** Brief (read); brief editing (text area focused, Save appears); board (lists); no link; link set (chip with the project name and host); deleting (busy Dialog); host unreachable (the brief save is refused with "Can't reach VPS. The brief is not saved." and its text stays in the box).

**Keyboard.** Tab order: brief, save, board columns, link Select, delete. Escape cancels a delete while idle.

**Desktop and Android.** Desktop only. Not in the owner reference (decision 9 notes this).

### 4.5 Bot teams page, team creation and add-bot (D-AG-13, D-AG-33)

**Purpose.** Bot Teams setup: the list of teams, creating a team, and adding bots to a team.

**Layout.** Opened from the All teams row in the Sidebar. Title "Bot teams" in `heading`, New team primary button. A grid of team cards (Card component, min width 280 px, 3 columns at 880 px and more): name with the team mark, lead bot, number of bots, host chip ("Runs on VPS"), a line "Workspaces in this team: off", and actions Open (Conference room), Add bot, More (Archive, Delete).

**New team dialog** (Dialog, 440 px, one-field form pattern extended): Name (required, up to 40 characters), Purpose (one line, optional), Lead bot (Select, from the bots added in this dialog or existing ones), Host (Select, default This PC when only one host exists), and a Switch "Turn on Workspaces in this team" (off by default; "A Development team can use Workspaces depth. You can change this later." in `body-sm`). Buttons: Cancel, Create team. Errors: "Name is already used in this host." (`status.danger.fg`).

**Add-bot dialog** (D-AG-33, 560 px): Name (required, up to 40), Role (one line, "What this bot is responsible for"), Agent (Select: Claude, Codex, Gemini, and any agent the user installed), Host (picker, default the team's host), Skills (chips with a picker; none by default). "Advanced options" disclosure, collapsed by default: Model and Effort Selects, using the model picker's values (X-AG-02). Owner to validate the collapsed default (pv96 note). Buttons: Cancel, Add bot.

**States.** No teams: EmptyState with "No teams yet", a sentence and New team. Saving: spinner in the primary button after 200 ms; fields read-only. Host unreachable: Banner "Can't reach VPS. Nothing can be created there until it is back." and the primary disabled; fields stay editable.

**Keyboard.** Tab order follows the visual order. Escape cancels without confirm (no secret is typed here).

**Desktop and Android.** Desktop only.

### 4.6 Routines page (D-AG-25)

**Purpose.** The routines of one team and the web addresses that start a task in the team. Opened from the team's Projects section (Routines).

**Layout.** Header 56 px: "Routines" in `heading`, the team pill (team character, "Desarrollo", chevron to switch team; borderless, hover fill; A59), and New routine (toolbar button: no border or fill at rest, hover fill; A12). Page at 1120 px maximum, `space.6` gutters. Intro line in `body`, `text.secondary`: "A routine starts a task on a schedule. Each run is kept in its history." Section "Routines" with a card per routine (list state, see below). Section "Web addresses" with its cards. Footer note in `caption`: "Runs on VPS, the host Desarrollo uses."

**Empty states (artboard 3).** Each section has an EmptyState card (`surface.raised`, centred, 24 px padding): a plate icon, the title, one sentence, and one button. Routines: "No routines yet" / "A routine starts a task on a schedule, with one bot in Desarrollo. Each run is kept in its history." / New routine. Web addresses: "No web addresses yet" / "Another app can call a web address, a webhook, to start one task in Desarrollo." / New web address.

**Routine row (list state, not in the artboard).** Name in `body-strong`; the schedule in words ("Every Monday at 09:00"); the bot and the host; the next run; the last run as a status word ("Done", "Failed", "Running"); an enable Switch; a More menu with History, Edit, Pause and Delete. Run now is an assumed action (decision 4). Opening a row shows its history: one row per run with the time, the status and the summary the bot posted.

**Web address row (list state).** Name, the host, the created date, and the address shown masked as its path prefix with the token hidden ("https://vps.../hook/••••••"). Actions: Copy address (the full address is copied; never shown), Regenerate, and Delete. Reason: the address carries a token, so it is a credential and is not printed in full (decision 7).

**New web address form.** Not designed in this file (gap; the inventory says the same). It needs a name, the task text and a copy step.

**States.** Loading: skeleton rows. Error: ErrorState with Retry in each section. Host unreachable: the section shows the last known routines with their age, and New routine is disabled with "Can't reach VPS. Routines can't change until it is back."

**Keyboard.** Tab order: New routine, the Routines section (rows), Web addresses section, footer. Enter opens a routine's history.

**Desktop and Android.** Desktop only.

**Artboard.** 3 (empty states).

### 4.7 Bot team routine form (D-AG-34)

**Purpose.** Makes a routine in plain language. Routines are the simplified Bot Teams form of an automation: one core scheduling engine (A26, decision 4).

**Layout.** Page with a Back button and the title "New routine" in `heading`. Header note in `caption`. Form column 640 px (`layout.routine.form.width`, see section 11), left-aligned, `space.5` between fields. Footer 56 px with Cancel (ghost) and Create routine (primary), right-aligned.

Fields:
1. **Name** (TextField, required, up to 64 characters).
2. **What should the bot do?** (text area, 3 lines, required, up to 2,000 characters), with the help "Write it as you would brief a colleague. The result is posted in the Conference room."
3. **When**: SegmentedControl Once, Daily, Weekdays, Weekly, Monthly. The day row (Weekly: seven 30 px day buttons, Monday to Sunday; Monthly: a day-of-month field from 1 to 28). Time (TextField with a clock glyph, 24-hour, "09:00"). Help: "Time zone: this computer." Once: a date field in place of the days.
4. **Bot** (Select from the team's bots; the value shows the bot name and its agent icon, "Bruno · [Codex icon]" (A17); with the help "Bots in Desarrollo: Mara (lead), Bruno and Iris.").
5. **Runs on** (Select from the team's hosts, the value with the server glyph; a single host is shown as text with the help "The host Desarrollo uses. It must be connected when the routine runs.").

Preview card (`surface.raised`, 12 px padding): repeat glyph, "Runs every Monday at 09:00 on VPS" in `title-sm`, and "Next run: Mon 16 Nov 2026, 09:00. Every run is kept in the routine's history." in `body-sm`. The preview updates as fields change, and is a live region (polite).

**Content.** Name: "Check staging sign-up errors". Instruction: "Check the staging sign-up log for failed confirmation emails and invalid tokens. Post a short summary in the Conference room, with the count for each cause." Weekly, Monday, 09:00, bot Bruno, host VPS.

**States.** Empty (all fields empty; Create disabled). Editing. Validation: "Name is required.", "Pick at least one day.", "Enter a time from 00:00 to 23:59." (`status.danger.fg`, the field border `status.danger.border`). Saving: spinner after 200 ms, fields read-only. Saved: the page returns to Routines with a toast "Routine created: Check staging sign-up errors". Host unreachable: Banner "Can't reach VPS. The routine can't be saved until it is back. Nothing is queued." Create disabled.

**Interactions and keyboard.** Tab order follows the visual order. The SegmentedControl uses Left and Right. Enter creates when the form is valid. Escape: cancels without confirm when nothing is typed; with typed text, a Dialog asks "Discard this routine?" (X-DLG-01, the default variant).

**Desktop and Android.** Desktop only.

**Artboard.** 4.

### 4.8 Team memory (D-AG-26)

**Purpose.** What the team remembers. Opened from the team's Projects section (Memory).

**Layout.** Header with the title "Memory" and the team chip. Two tabs (Bot, Team) as a SegmentedControl. A list of memory entries (ListRow): the sentence, its author (a bot or the team), the date, and a More menu with Delete. Footer: "Only the team's bots and you can read this memory." (Who else can see it is not decided; decision 10.)

**Content.** Entries: "Oppm sign-in uses the Supabase auth endpoints." (Team, from Mara, 11 Nov); "Staging mail goes through the sandbox SMTP host." (Bot Bruno). Text only; the entries are examples.

**States.** Empty: "What the team remembers shows here. Bots add notes when they learn something that helps later work." Host unreachable: last known entries with their age; Delete disabled. Delete: confirm (X-DLG-01 default variant).

**Keyboard.** Tab order: tabs, entries, footer. Delete asks for confirmation; Enter on the confirm button only after Cancel has been reachable.

**Desktop and Android.** Desktop only. Whether bots write to memory automatically is not decided (gap G-8).

### 4.9 Team files (D-ED-13)

**Purpose.** The files of one team, opened from the team's Projects section (Files).

**Layout.** Reuses the file explorer (D-ED-11: tree and search) on the left, 288 px, and the file editor (D-ED-01) on the right in a Work panel tab. Tree top: "Team folder" and "Linked project files" sections, the linked files marked with a link glyph and the project name. Versions: a list in the file's header menu, one row per version with the time and the author (a bot or you); "Restore" for an older version.

**States.** Empty team folder: "No files yet. Files a bot creates for the team show here." Host unreachable: the tree shows the last known list with its age; editing is disabled. External change: the external change banner (D-ED-06).

**Keyboard and pointer.** As D-ED-11 and D-ED-01.

**Desktop and Android.** Desktop only. Not designed as a new layout; the owner reference shows the entry only (assumed per the inventory).

## 5. Calendar (D-AG-23)

**Purpose.** Shows routines and automations on one timeline, opened from the rail as a full page (D-SH-12). Every routine appears here, whether it came from the Routines page or from a bot in the Conference room.

**Layout (artboard 5, light).** Full page, Sidebar hidden, Title bar with Back and "Calendar". Header row (24 px padding): previous and next IconButtons (28 px), a Today button (toolbar button: no border or fill at rest, hover fill; A12), the range title "9 to 15 Nov 2026" (`title`, 16/24, `heading` weight not used), and at the right a Switch "Show done" (off), and a SegmentedControl Day, Week, Month (Week on). Filter chips row: "Team: Desarrollo" (with the team character) and "Automations" (with the workflow glyph); both selected in the artboard (chip: 24 px, no border, `accent.subtle.raised` when on, hover fill when off).

Grid: time gutter 48 px with hour labels in `caption` (08:00 to 18:00), then seven day columns (minimum 96 px, `layout.calendar.column.min`). Day header 52 px: weekday in `caption` and the date in `title`. Today: 1 px `border.strong` outline and the date in `accent.fg`. Hour rule 48 px (`layout.calendar.hour`), 1 px `border.subtle`. Entries: absolutely placed rounded rectangles (`surface.raised`, `border.default`, `radius.md`), 4 px left and right inset, with the glyph and the word ("Routine" or "Automation") in `caption` and the time at the right, then the title in one line. Routine entries use the accent for the glyph. The selected entry: `accent.subtle.raised` with a 2 px accent bar.

Week in the artboard: Mon 9, 08:00 Automation "Run end-to-end checks on perulaunch-web"; Tue 10, 09:00 Routine "Summarise open pull requests"; Wed 11, 10:00 Automation "Nightly build for massorca"; Thu 12, 09:00 Routine "Check staging sign-up errors" (selected, "Runs on VPS") and 16:00 Automation "Deploy massorca docs"; Fri 13, 17:00 Routine "Post the weekly summary to the Board". Sat 14 and Sun 15: "Nothing scheduled".

Footer note: "Routines come from the Routines page or a bot in the Conference room. Automations come from Workspaces."

**States.** Loading: skeleton blocks in the columns. Empty day: "Nothing scheduled" (no card). Empty week: EmptyState "Nothing scheduled this week" with "Routines and automations show here when they have a run time." Error: ErrorState with Retry on the page; a failed entry shows ErrorState inline. Done entries: circle-check glyph, "Done", title in `text.tertiary`, only when Show done is on. Host unreachable: an entry shows the unreachable glyph and "Last known state, 5 min ago".

**Interactions and keyboard.** Tab order: header controls, filter chips, the grid as one stop. Arrow keys move between days and entries; Enter opens the active entry: a routine opens its Routines page entry (D-AG-25), an automation opens D-AG-14 (assumed). Switching views cross-fades over `duration.base`; reduced motion: instant. Entries cannot be dragged in v1. Month view: a click on a day number switches to Day for that date (proposed).

**Accessibility.** The page is a heading named "Calendar". Column headers name the weekday and date and "today". An entry is named by its word, time and title, for example "Routine, 09:00, Summarise open pull requests".

**Desktop and Android.** Desktop only; Android is open in the inventory.

## 6. Settings (D-ST-01 to D-ST-12)

### 6.1 Page layout and navigation

Settings is a full page (Sidebar hidden). Title bar: Back and "Settings". Inside the page: a section navigation column of 232 px (`layout.settings.nav`, gap G-6) with a 1 px `border.subtle` divider, and a content column with a maximum width of 880 px (`layout.page.settings`), `space.6` gutters, and `space.8` between sections.

Navigation items (UI copy, in this order):
General and account; Agents and accounts; Appearance and layout; Input and shortcuts; Terminal and browser; Sources and integrations; Hosts and phones; Notifications and voice; Extensions; Secret vault; Privacy and advanced; Routing decisions.

Each item has a 14 px icon before its label, `space.2` apart (owner, 2026-11-13): General and account, person; Agents and accounts, bot; Appearance and layout, palette; Input and shortcuts, keyboard; Terminal and browser, terminal prompt; Sources and integrations, plug; Hosts and phones, server; Notifications and voice, bell; Extensions, puzzle piece, with a trailing arrow because it opens Marketplace > Installed; Secret vault, lock; Privacy and advanced, shield; Routing decisions, route. The icon takes the item's text colour.

The selected item: `fill.selected.canvas` with `text.primary` and weight 500. Project settings are not in this list; they open from the project's own menu (D-ST-11).

Apply model (SettingsRow): changes apply when made. Switches, selects and segmented controls apply on change; text, number and keybinding rows apply on Enter or blur; Escape reverts an edit. Nothing shows "Saved" before the store confirms.

### 6.2 Search (D-CP-04)

A SearchField at the top of the page, shown focused, with the 2 px focus ring, in the artboard. Typing filters rows and sections: matching rows stay, others hide, and the match in the label is highlighted with `match.bg`. A hit line under the field gives the count: "3 matches in this section" or "4 matches in 3 sections". Selecting a result jumps to its section and outlines the row for 2 seconds with `accent.subtle.raised` and a 2 px `accent.fg` bar. No match: EmptyState "No setting matches "xyz"." with the hint "Try a word from the label, such as theme or shortcut." Ctrl+F focuses the search field in Settings (assumed; the command palette D-CP-01 is the main route). The palette's own search belongs to section 12.

### 6.3 Sections

Each section below lists purpose, rows, states and where its artboard is. Row types come from SettingsRow.

**D-ST-01 General and account.** Rows: Conversations without a project (folder picker, default `~/MassOrca/Conversations` on each host; where "No project" sessions are saved, owner 2026-11-21); MassOrca account (signed-in state; Sign out in a confirm, D-OT-14); Updates (release channel stable or preview; update status: up to date, available, downloading, failed; Check for updates); Workspace defaults (root folder for new projects; editor defaults); Setup checklist (the same checklist as D-OB-12); Support (opens the feedback dialog, D-WS-23). States: loading; update status with its word.

**D-ST-02 Agents and accounts.** Rows: default agent and launch behaviour; agent availability (one row per agent: detected, missing or signed out, with the word); agent accounts and usage (content of D-AC-01 and D-AC-06); computer use; browser access; keep-awake; session search; delegation depth limit (a number from 1 to 5, default 3; the engine is X-DL-01; the owner asked for a limit as a setting, decision 11). States: per agent detected, missing, signed out.

**D-ST-03 Appearance and layout.** Artboard 11, light. Rows: Theme (System, Dark, Light; System follows the OS until the user chooses, A8); Import theme (Warp and Ghostty; a theme below the contrast minimum is rejected with the failing pair named; D-CU-03); Layout preset (a Select of saved presets and a Save as preset ghost button; D-CU-01); Accent colour (six presets, Lavender first and default (A15), and a custom colour; swatches 20 px; the contrast check line under the control); Density (Compact, Default, Comfortable); Text size (slider, 90% to 130%); Interface font and Monospace font (Selects); Session summary (Switch; "Shows only the cards that have information"); Status bar items (five checkboxes: Agent status, Memory, Terminals, Ports, Hosts). Applies at once; the frame and canvas follow the tokens (A11).

**Built (desktop).** The Theme row is live: System, Dark or Light, stored as `appearance.theme` through the service, the same value the rail toggle writes. System shows the operating system's appearance and follows changes to it. The row is off until the service has answered. The CLI equivalent is `massorca config set appearance.theme system` (or `dark`, `light`). The other rows stay planned.

**D-ST-04 Input and shortcuts.** Artboard 12, dark. Keybinding table: columns Action, From (Core or the extension name), Shortcut (Kbd badges, Windows form). Filter field and a SegmentedControl All, Conflicts, Extensions. Reset all (ghost). Recording: click a shortcut, press keys; Escape stops recording without change. Conflict: the row opens an inline warning block (`status.warning.bg`): "Conflicts with Quick open. Ctrl+P is taken. Reassigning moves it to Pin note, and Quick open loses the shortcut." with Reassign and Cancel. Reassign opens the Dialog (X-DLG-01, default variant) before moving the key; Cancel keeps both bindings. Quick commands row: "4 saved commands" with Manage. Footer: Windows and Linux show Ctrl; macOS shows Cmd; an extension's suggested shortcut loses to a core shortcut, and the user decides (design-system 7.1). Editing defaults (for example tab width) sit in a Settings section above the table. States: recording, conflict, filtered, no match.

**D-ST-05 Terminal and browser.** Rows: terminal font, terminal palette (imports Warp and Ghostty through D-CU-03), scrollback, shell, rendering; browser home page, search engine, default zoom, profiles, link routing (D-BP-08). Standard rows. Not drawn in an artboard.

**D-ST-06 Sources and integrations.** Rows: git providers and task providers, one row each, with the state (connected; not connected; error with its message); Linear (agent skill); git behaviour (default branch handling, fetch on focus); API budget (number with unit). Connecting a provider needs the owner's grant on the provider's own page, and this screen says so in its helper. States: connected, not connected, error, host unreachable.

**D-ST-07 Hosts and phones.** Rows: one row per host (This PC, VPS) with state and last check; add host (D-HO-02); paired phones (D-PR-01: local network and SSH only; relay pairing is not in v1). Each host row can open its server settings (D-HO-07 content). States per host: connected, unreachable (last known state with age, dimmed).

**D-ST-08 Notifications and voice.** Rows: notifications per source (agent finished, waiting for you, routine ran, extension notices) with a Switch and a sound Select; dictation on or off; speech model (states: not downloaded, downloading with a Progress bar, ready, failed); transcription key. The transcription key is a credential: it is entered through the Secret modal (X-VT-01) and stored in the vault, never shown in a field (decision 12).

**D-ST-09 Extensions.** Settings keeps only the "Extensions" item, with an arrow, that opens Marketplace > Installed (owner, 2026-11-13, A68). The extension manager (7.4), the extension sources (Marketplace and local folders, Remove for local ones) and the installed skills live there. Share skills is not here (D-EX-07 dropped). Per-extension settings sections appear as sub-sections under the list (7.5).

**D-ST-10 Privacy and advanced.** Rows: telemetry (off unless the owner turns it on; D-OT-01 previews a crash report before anything is sent); diagnostic export; network (proxy); cache timers; OS permissions (macOS prompts, D-OT-02); CLI registration (D-OT-11); experimental flags; developer tools (behind the Advanced switch, which is off by default). Nothing is sent without consent.

**D-ST-11 Project settings.** Not in the Settings list. Opens from the project's menu in the Sidebar (D-WS-02). Sections: identity (name, icon), hooks (repository hooks, trust state, D-WS-20), worktree defaults, runtime, git author, GitHub account, AI actions, and Remove project (X-DLG-01 destructive variant, naming what is lost).

**D-ST-12 Routing decisions.** Shown as "Routing decisions" in UI copy. The internal name Jev never appears in UI copy (glossary; decision 3). The section lists the routing choices the helper made for Bot Teams requests. Each row: the request in short form, "Routed to Bruno (Codex)", the reason in one line, the time, and a Change action (choose another bot). A Clear history button (X-DLG-01). Read-only except Change. Labels were not read in pv96; these are written here.

## 7. Customisation

### 7.1 Marketplace catalogue (D-EX-01)

**Purpose.** Finds and reads about extensions, agents, skills and services from their sources. Opened from the rail (D-SH-13) as a full page.

**Layout (artboard 6, dark).** Full page, Sidebar hidden. Title bar with Back and "Marketplace". The same left navigation as Marketplace > Installed (7.4, A68): Discover (selected), Installed (5), Updates (1), then Categories: Extensions, Agents, Skills, Services, Themes, each with its icon. Content with `space.6` gutters. Header: "Discover" (`heading`), one line of text, a search field (280 px, "Search the catalogue"), a Sources toolbar button (server glyph, no border at rest) and the split button "Add extension" (A63). The categories replace the old filter SegmentedControl. Card chips carry an icon for what they name (A21); third-party publishers show "Third party" (A63); an agent item shows its agent icon in the plate (A16). Grid: three columns at 880 px and more, one column under 560 px, two between; gap `space.4`. Each cell is an ExtensionCard (`surface.raised`, `border.default`, 8 px radius, 16 px padding, minimum width 280 px).

**Cards in the artboard.** Workspace board (extension, Not installed, Review and install); Quick notes (extension, Not installed); Model usage panel (installed and enabled, update 2.1.0 available); Gemini CLI agent (agent, installs on VPS); Log tail (needs MassOrca 1.6 or later, no install action); Release notes (skill, Not installed).

**States.** Loading: skeleton cards. Empty search: EmptyState "Nothing matches "xyz"." Per-source error: ErrorState inside that source's group, with Retry. Disconnected host: installed items on an unreachable host show their last known state. Incompatible: warning glyph and the requirement.

**Interactions and keyboard.** The cards are list items, not tab stops. Tab visits, in order, the name link, the primary control, Settings and More on each card. Enter on the name opens the item page (7.2). Search is the first stop.

**Desktop and Android.** Desktop only; Marketplace has no Android entry in v1 (pending-owner-review item 21).

### 7.2 Marketplace item page (D-EX-08)

**Purpose.** One page for each catalogue row: host picker, install or connect on the picked host, who can use it, advanced options, and an uninstall or disconnect action.

**Layout (behind artboard 7; the order is the owner's, 2026-11-22, see pending-owner-review.md).** Full page with the left navigation of 7.4. The page has nine parts, top to bottom:
1. Logo: a 56 px plate with the item's glyph.
2. Name in `heading`, with the state chip (for an agent with an active account: "Connected" when signed in, else the account's state; otherwise the item's state) and a Copy link button. The meta line reads publisher, Marketplace, version (`mono-sm`).
3. Host: a selector that reads "This computer, Connected". Choosing another host needs the service, so the selector shows the host without opening.
4. Banner: three example prompts tagged with the item, then "Type @Name in any chat or room to ask it directly." Shown only when the item has examples.
5. Connection, for agents only: one row per account with its state word, where it is signed in, what uses it, and Active or Make active, Check now and Remove. Add account sits under the list, with the note that sign-in tokens stay in the secret vault.
6. Information: the description, then Developer, Category, Permissions and "Found on this PC" when present. A tool found on this PC is information; it does not install anything.
7. Settings: the item's settings. An agent's default agent shows "Not available yet" until the service has `agents.default_kind`. Before install, the part says that settings appear after install.
8. Advanced Options, collapsed, for installed items: versions, source URL and who can use it ("Only you").
9. Danger Zone: Uninstall, behind a destructive confirm (X-DLG-01). Disconnect for connected services is not built yet. An item that is not installed has nothing to remove, and the zone says so.

For an item that is not installed, Review and install sits at the right of the host row. It is disabled with "Not available yet", because installing needs the extension runtime (decision 0059).

**States.** Loading (skeleton). Not installed. Installed. Connected (services). Error (fetch failed: ErrorState with Retry). Host unreachable: the install control is disabled with the reason in visible text.

**Interactions and keyboard.** Each action has its orca CLI equivalent (inventory). Tab order: header actions, sections in order, Danger zone last.

**Desktop and Android.** Desktop only.

### 7.3 Permission review before install (D-EX-02)

**Purpose.** Shows what an extension asks for before it is installed, updated or rolled back, with a plain risk notice. The user grants permissions here and nowhere else (A12). Security chrome: core components, validated tokens, no extension content.

**Layout (artboard 7, dark).** Dialog 560 px wide (`layout.dialog.width`, see section 11), `surface.overlay`, `radius.xl`, 16 px padding, scrim behind. Header: 40 px plate with the glyph, the title "Install Workspace board" (`title`), the publisher, source and version (`body-sm`, version in mono), and a close IconButton.

Body:
- Risk notice (Banner, warning): "Check. This extension can read files in your project and use the network." Level computed by the core (Low, Medium or High); the manifest cannot set it.
- "This extension asks for" rows: Files (scope "read, ~/projects/*/.massorca", reason "Reason from the extension: ...", Medium); Network access (scope "api.github.com", Medium); Notifications (Show notifications, Low). Each row: 20 px glyph, title in `body-strong`, scope in mono, the extension's reason in `body-sm` (plain text, up to 120 characters), and the risk marker (glyph, word, colour).
- "What it adds" chips: Sidebar section, 2 commands, Settings section.
- Install target: a Select "Install on: This computer" (shown when more than one host exists).

Footer (top `border.subtle`): Cancel (secondary) and Allow and install (primary). High risk: a Checkbox "I understand this extension can run commands on this computer." comes above the footer; Allow and install is disabled until it is checked. High risk is not in the artboard (state list below).

**Variants.** Update: a "New in 1.5.0" group with a "New" chip; "Unchanged (3)" collapsed; with nothing new: "No new permissions" and the primary is Update. Rollback: the older set; primary "Roll back".

**States.** Reviewing; High risk not acknowledged (primary disabled); installing (progress steps "Downloading", "Checking the package", "Installing" with a Progress bar; Escape and the scrim do not close it during the package check); installed (circle-check, "Installed", buttons Open settings and Done); failed (Banner: "Not installed: <reason>. Nothing was changed." with Retry and Copy details); loading (skeleton rows); no permissions ("This extension does not ask for any permission." with a Low notice); fetch error (ErrorState with Retry); host unreachable (Banner "Can't reach VPS. Nothing can be installed there until it is back.", primary disabled, target may change).

**Interactions and keyboard.** The dialog traps focus. Initial focus is the title (a focusable heading), so Enter does nothing until a button has focus. Tab order: expanders, Install target, acknowledgement, Cancel, primary. Escape cancels except while checking the package. Focus returns to the card that opened it; after install, to that card's Switch.

**Desktop and Android.** Desktop only.

### 7.4 Extension manager (D-EX-03)

**Purpose.** Enables, disables, configures, updates, removes and rolls back installed extensions. Lives in the Marketplace, section "Installed" (owner, 2026-11-12): extensions are centralised and managed there, next to Discover, Updates and the categories (Extensions, Agents, Skills, Services, Themes). Settings keeps only a row that links to Marketplace > Installed.

**Layout (artboard 8, light).** Marketplace page (rail Marketplace item active); left navigation Discover, Installed (selected, count 5), Updates (count 1), then Categories. Header: "Installed" (`heading`), one line ("Everything installed from the Marketplace, created in Creator mode or added from a third party. Disabling keeps the files; removing deletes them and their settings."), and the split button "Add extension" (A63). One group card with one row per extension (56 to 64 px): a 36 px plate, the name in `body-strong`, the publisher, version and contribution in `caption`, the state (glyph and word), the Enabled Switch (30 by 18 px), Settings (ghost, when the extension has a settings section) and a More IconButton.

Rows in the artboard: Workspace board (Enabled); Quick notes (Enabled, with the More menu open, each item with its glyph: Roll back to 0.3.0 (history), View log (file), separator, Remove in danger text); Model usage panel (Update available 2.1.0, with the Update action); Log tail (Failed to start, View log; Switch off); Gemini CLI agent (Gemini icon in the plate (A16); Installs on VPS; "Last known: enabled, 5 min ago"; Switch disabled because the host is unreachable).

**Actions.** Switch: applies on release and waits for the host to confirm. Disable keeps the files. Remove and roll back open the X-DLG-01 confirm: "Remove Quick notes? Its settings are deleted." (destructive variant). New extensions are enabled at once (A13).

**States.** Enabled; disabled; error (Failed to start, View log opens the log); update available; host unreachable (Switch disabled; last known state with its age; Remove and roll back wait for the host).

**Keyboard.** Each row is a list item. Tab visits the Switch, Settings, More. Enter on More opens the menu; Up and Down move in it; Escape closes it.

**Desktop and Android.** Desktop only.

### 7.5 Extension settings sections (D-EX-04, D-SL-06)

**Purpose.** An extension adds its own section to Settings, under the Extensions item.

**Layout.** A sub-section per extension, headed by its icon (16 px) and name in `caption` `text.tertiary`, then SettingsRow rows. Row types an extension can use: Toggle, Text, Number, Select, Segmented (up to 4), Keybinding, List (with Add and Remove), Secret reference (the secret's name and scope, a Manage in vault link; never a value), Action (a Button), Read-only, Link.

**Limits.** 40 rows per section; label 60 characters; helper text three lines, then an ellipsis with the full text in a tooltip. An extension cannot restyle rows, open itself, or display or store a secret value.

**States.** Shown; hidden (the extension is disabled); empty list row ("None yet" and Add); host unreachable (last known value dimmed, control disabled, helper "Can't change while VPS is unreachable").

**Keyboard.** As SettingsRow.

**Desktop and Android.** Desktop only.

### 7.6 Create an extension (D-EX-05)

**Owner decision (2026-11-12).** An extension is created in a normal session in **Creator mode**, the way the owner's reference works: there is no separate builder page with template, file and log columns.

**Layout (artboard 9, dark).** A session in Workspaces mode (the rail mode button shows Workspaces, A4): Title bar with the session title; a session header with the title, a "Creator mode" chip (`accent.subtle`, wand icon), the background-job indicator and the extension name and version on the right. Centre: the Conversation, with the user's request in plain words ("Create a floating card inside the chat of a session that shows a project's info: GitHub account, organisation / project, branch / worktree"), then the agent's steps (reads the extension API, creates `extension.json` and `views/card.json`, reloads, reads a log error and fixes it, reloads clean), its answer, and the actions "Enable in this project" and "Show files". Bottom: the Composer in the shared style, with the placement bar holding the extension ("project-info-card") and "Creator mode". Right: the Work panel with three tabs: **Preview** (owner, 2026-11-12: a skeleton of the whole app, with rail, Sidebar, Conversation, Session summary, Work panel and Project sidebar as grey blocks, and the running extension drawn in its real place and outlined in `accent.fg`; reloaded after every change. A toolbar offers Select, Note and Draw, and a Full screen button that shows the skeleton at window size. Notes (sticky style) and strokes mark what to change, and "Send to agent" sends them with the next message, each note anchored to the element it points at), **Files** (the extension folder; files open in the editor and can be edited by hand) and **Logs** (with a count of errors and fixes).

**How it starts.** "Creator mode" is chosen in any Composer's placement bar, or from "Add extension" > "Create with chat" (Marketplace > Installed). The agent works in the extension's own folder, never in a project worktree.

**Add extension (third party).** In Marketplace > Installed, a split button "Add extension": the main part opens a new session in Creator mode; the menu lists "Create with chat", "From the Marketplace", and under "Third party": "From a Git URL…", "From a folder…", "From a file (.zip)…". Every third-party source goes through the permission review (D-EX-03) before it is enabled and shows a "Third party" badge in the extension manager.

**States.** Agent working (steps stream in, Stop in the Composer); reload failed (the Logs tab shows the error count, the agent reads it and proposes a fix; the last good version keeps running); ready (Enable); edited by hand (the agent sees the change on its next turn).


### 7.6b Advanced extension example: Sticky Wall (artboard bt-extension-app)

**Purpose (owner, 2026-11-12).** Shows what Creator mode can build beyond a small card: a full app inside MassOrca, created from one request ("Create a sticky note app"). Comparable community examples built in other harnesses include a token usage tracker, a paper reader, a repo stats panel, a video editing workbench and 3D viewers.

**Contributions used.** A Sidebar entry ("Sticky Wall", with an "Extension" hint); a full-canvas page drawn as a sandboxed web view (decision 3 in pending-owner-review); stored data that syncs across hosts; and agent tools ("add_note", "search_notes") so any session can write to the wall.

**Layout (artboard, dark).** Sidebar in Workspaces mode with the entry selected. Page: heading "Sticky Wall" and a line, a search field and a count chip; a note composer drawn as a large yellow note with six colour swatches, the shortcut hint and "Add to wall"; a wall of six notes in the six colours, slightly rotated, each with a label, text, time and edit icon, one pinned; a strip that names the agent tools and offers "Change in Creator mode". The Status bar names the extension and that it runs sandboxed.

**Rules.** The page follows the theme tokens for its frame, text and controls; its own content colours (the note pastels) are the extension's. It cannot draw outside its page, take app shortcuts or reach the network without the permissions it declared.

### 7.7 In-app application (D-EX-06)

**Purpose.** A user-built application that runs inside a view or panel. Requested by the owner (J11).

**Layout.** Built from slots: a Project sidebar tab (D-SL-01, artboard 10), a full-canvas page (D-SL-04) and a sidebar section (D-SL-05). Declarative views only, as in design-system 7.1. Pending-owner-review item 1 proposes a sandboxed web view as a Work panel tab or full-canvas page on desktop, only if the owner accepts it. This file does not design the web view; decision 14 keeps it out until the owner answers.

**States.** Running (the view is live; a status line "Running" with the time since start). Error: the app stopped; the last view stays with a Restart button and the log link (the error names the view that failed). Stopped by the user: the tab shows "Stopped" with Start.

**Desktop and Android.** Desktop only.

### 7.8 Extension slots (D-SL-01 to D-SL-10)

Shared rule: an extension declares the slot, and the core draws it with core components and tokens. Limits and the "cannot change" list are in design-system 7.2 and are not repeated. What this file adds is the placement and the states the artboards show.

| Slot | Placement | Shown in | States |
|---|---|---|---|
| D-SL-01 Project sidebar tab | Tab strip after the four core tabs (Files, Sessions, Source control, Checks). Icon only; the label is the tooltip and accessible name. Count badge at the top right (`accent.subtle.raised`). Active tab: 2 px accent underline. | Artboard 10 | Open; closed; count |
| D-SL-02 Status bar item | Status bar, right group by priority. Glyph, text ("Notes: 3 open"), a state kind. | Artboard 10 | Shown; hidden when narrow |
| D-SL-03 Chat card | A turn block in the Conversation, under the agent message. Header with icon and title; up to three actions sent as a message. Up to 320 px tall, then Show more. | Specified | Shown |
| D-SL-04 Main view | A full-canvas page, like Calendar; Back in the Title bar. | Specified; the in-app application | Open; error |
| D-SL-05 Sidebar section | Under the workspace or team tree; collapsible; header with icon and up to two header buttons. Rows follow SidebarRow. | Specified | Shown; hidden; collapsed by default |
| D-SL-06 Settings section | See 7.5. | Specified | Shown |
| D-SL-07 Command and shortcut | In the command palette and the keybinding editor. A suggested shortcut that clashes with a core one loses; the conflict is shown in D-ST-04. | Artboard 12 (row "Pin note") | Registered; conflict |
| D-SL-08 Work panel tab kind | In the + menu of the Work panel, not in the empty-state four. Same rules as D-SL-01 for the tab; its view fills the tab body. | Specified (one rule; lowest priority, decision 15) | Open; closed |
| D-SL-09 Activity rail icon | In an Extensions group after a divider in the rail, under the top group. Badge: a dot or a count from 1 to 99+ (neutral, accent or danger). Opens the extension's page or panel. Excess icons go under a More button. | Artboard 10 | Shown; hidden; badge |
| D-SL-10 Model picker entry | Adds an agent or models to the model picker's agent rail and list (X-AG-02). Cannot take Ctrl+1 to 4. | Specified; the Marketplace card shows its chip | Shown; hidden |

Artboards 10 and 13 follow the Workspaces Sidebar (Prototypes, Tasks, Automations, MassOrca Mobile; A9) and show agents by icon only: the session model chip ("[Claude icon] Opus 5.5"), message avatars, "Added by [Claude icon]" and the Status bar agent item ("[Claude icon] running"), with the name as tooltip and accessible label (A16, A17). The Notes panel "Add note" is a toolbar button without border (A12).

Extension icons in the rail and the Sidebar come from the Lucide set or a single-ink SVG of at most 8 KB; core icons and agent status glyphs cannot be reused.

### 7.9 Customisation preferences (D-CU-01 to D-CU-03)

- **D-CU-01 Theme and layout presets.** A Layout preset row in Settings > Appearance and layout (artboard 11): a Select of saved presets, Save as preset, and Share as a file. A preset saves open regions and their widths (`massorca.layout/1`, not a theme). States: saved; active (the Select shows it). A theme is a pair of dark and light files (design-system 6.3).
- **D-CU-02 Keybinding preferences.** Merged into D-ST-04. The keybinding editor is artboard 12.
- **D-CU-03 Theme import.** Merged into D-ST-03. The Import theme row opens a Dialog with a preview of the frame and canvas, the contrast result per pair, and Import or Cancel. States: preview; imported; error (the failing pair is named).

### 7.10 Share skills (D-EX-07)

Dropped. Sharing needs a service and is not in the first version (inventory). The Settings section does not list it, and the Marketplace has no Share action.

## 8. Secret vault (D-VT-01, X-VT-01, D-VT-02, X-CH-07)

Owner requirement, 2026-11-12. The vault is in Settings, as its own section (A27, owner accepted default). Its rule: values never go into a chat, a prompt or a log; sharing creates a link, not a copy.

### 8.1 Secret vault screen (D-VT-01)

**Layout (artboard 14, light).** Settings page with the Secret vault item selected. Header: "Secret vault" (`heading`), one line ("Values never go into a chat, a prompt or a log. Sharing creates a link, not a copy: rotate the owner's value once and every linked scope uses it."), and Add secret (primary) at the right. Toolbar: search field (names), a SegmentedControl All, Project (folder glyph), Team (team character), Global (globe glyph), and the count "7 secrets, 4 scopes".

Groups by scope (group header in `body-strong` with the scope icon: folder for a project, the team character for a team, globe for Global; and the count), each in a card of rows. Row: a 28 px plate with the key glyph (owned) or the link glyph (linked); the name in mono (`mono` 13 px); a line with the owner chip ("Owner, Project oppm") or the linked chip ("Linked from Project oppm", accent) and "Read-only here" for linked rows; the users line ("Used by [Claude icon] in oppm, 3 sessions this week"; the agent icon replaces the name, which stays as tooltip and accessible label, A17) and the rotated date in `caption`; actions at the right: Rotate (secondary, 24 px) and Delete (IconButton in danger colour) for owned rows; Unlink (secondary) for linked rows.

Linked rows have a `surface.sunken` fill. Their value is never shown. Usage shows names and times, never values (A31).

Content in the artboard: Project oppm: OPENAI_API_KEY (owner, linked to 2 scopes), STAGING_SMTP_PASSWORD (owner). Project NexusCare: OPENAI_API_KEY (linked from oppm, read-only here), DATABASE_URL (owner). Team Desarrollo: OPENAI_API_KEY (linked from oppm). Global: GITHUB_TOKEN (owner, used by 5 projects).

**Rules.**
- Edit, rotate and delete happen only at the owning scope. Unlink is allowed locally and asks nothing (A34).
- Rotate sets one new value in the owning scope. Every linked scope uses it from its next run; a running session keeps the value it started with (A30).
- Deleting at the owner removes the secret for every linked scope, after the destructive Dialog (8.4).
- Edit is in the row's More menu (assumed; the inventory names the actions but not the placement).

**States.** Empty: EmptyState "No secrets yet" with the line "Add a secret for an agent to use. It never appears in a chat." and Add secret. Loading: skeleton rows. Listed. Host unreachable: the list shows the last known secrets with their age; Rotate, Delete and Add are disabled with "Can't reach VPS. Nothing can be saved until it is back." Error: ErrorState with Retry.

**Interactions and keyboard.** Tab order: header action, search, scope control, groups in order, rows (Rotate, Delete or Unlink). Enter on a row name opens its detail (the usage list, names and times only). Escape closes the detail.

**Desktop and Android.** Android shows the list and adds a secret through the modal only; edit, rotate, delete and unlink stay on desktop (M-VT-01, A36, owner accepted default).

### 8.2 Secret entry modal (X-VT-01)

**Layout (artboard 13, dark).** Dialog, 440 px (`layout.dialog.width`, see section 11), `surface.overlay`, `border.default`, 12 px radius, 16 px padding, scrim behind. Header: title "Add secret" (`title`) and a close IconButton. Info line (`body-sm`, `text.secondary`): "Agents get it as an environment variable when they run. It is stored on VPS and never goes into the chat."

Fields, 16 px apart:
1. **Variable name**: TextField in mono, placeholder OPENAI_API_KEY; help "Letters, digits and underscores. Cannot start with a digit."
2. **Value**: masked field in mono with a Show button (eye glyph, `aria-pressed`) at its right. Hidden by default. Shown on request with Show, and hidden again after 10 seconds, on blur and on close (proposal). Pasted multi-line values show "Multi-line value, 12 lines" when hidden.
3. **Scope**: SegmentedControl "Project oppm" (folder glyph), "Team Desarrollo" (team character), "Global" (globe glyph). Project is preselected. The Team segment shows only in Bot Teams or for a project that belongs to a team. Caption: "Only agents working in oppm can use it." (or "Every project and bot of the team can use it." or "Every project on VPS can use it.").

Link row: "Link an existing secret instead" (text button, `accent.fg`). In link mode the value field is replaced by radio rows of the owner's secrets in other scopes (name in mono, owning scope chip, rotated date). Empty list: "No secrets in other scopes. Enter a new value instead." with Back.

Footer (top `border.subtle`): Cancel (secondary) and the primary: Save secret, Rotate, or Link secret.

**Variants.** Add (from the request card or the vault); Edit (name read-only, value optional, scope read-only); Rotate (name read-only, new value only); Link (name read-only, list of secrets).

**States.** Entering (Save disabled until the name is valid and the value is not empty). Invalid name (border `status.danger.border`, message with the octagon glyph). Name in use ("OPENAI_API_KEY already exists in this scope." with "Pick another name" and "Link the existing secret"). Saving (spinner after 200 ms; fields read-only; Cancel stays enabled and says a save in flight is not cancelled). Stored (the dialog closes; the card shows Stored; announced: "OPENAI_API_KEY stored for project oppm"). Host unreachable (Banner: "Can't reach VPS. Nothing can be saved until it is back."; primary disabled; nothing queued). Failed (Banner: "Not saved: <reason>. Nothing was stored."; the typed value stays for Retry). Long content (name over 64: "Use up to 64 characters.").

**Interactions and keyboard.** Tab order follows the visual order: name, value, Show, scope, link, Cancel, primary. Enter submits when valid and focus is not in a multi-line value. Escape closes without confirm and discards the value. Left and Right change the scope. Focus is trapped; on close, focus returns to the Add button on the card or the vault row.

**Security.** The value stays in the field buffer while the dialog is open. Never in view state that can be serialised, logs, the accessibility tree (the field reads "Secret value, hidden"), the clipboard while hidden, screenshots of debug tools, or the transcript. Closing zeroes the buffer. Autofill is off. Paste is allowed. No extension can place content in this dialog or read the value; an extension may only ask the host to open it for a name.

**Desktop and Android.** Desktop here. On Android the same fields in a full-height bottom sheet, with FLAG_SECURE while open (the Android spec owns this).

### 8.3 Secret request card (X-CH-07)

**Layout (artboard 13).** A card in the conversation, 720 px column. Container: `agent.waiting.bg` fill with a `status.warning.border` (pending uses the "needs you" treatment). Header row: a 28 px plate with the key glyph, the title ("Add a secret for oppm", or "Claude needs a secret" when the agent asked), and at the right a state: a waiting glyph (question mark in a circle) and the word. Name chip in mono 12 px ("OPENAI_API_KEY"). Reason line (`body-sm`, up to 200 characters, plain text; "Needed to call the OpenAI API in the summary step. Without it, the summary is skipped."). Actions right-aligned: Add secret (primary) and Decline (secondary). Safety line with a lock glyph and "The value is stored on VPS and never goes into the chat."

**States.** Pending (full card). Entering (the modal is open: both buttons disabled, the state line reads "Entering secret"; shown in artboard 13). Stored (compact row: check circle, "Stored", "for project oppm", host name, and "Open vault" in accent). Linked (compact row: link glyph, "Linked", "from project NexusCare"). Declined (compact row: ban glyph, "Declined"; the agent is told the owner declined). Failed (octagon glyph, "Not saved: <reason>", Retry and Decline). Disconnected host: Add disabled with "Can't reach VPS. Nothing can be saved until it is back."; the card stays pending; Decline still works; nothing is queued.

**Behaviour.** The user asks ("How do I give you the OpenAI API key for the project?") or the agent reports it needs a secret. The message never carries a value. Add opens the modal with the name filled in; the card stays pending until the modal closes. Deleting a secret at its owner makes the next request for that name a new pending card; the old card stays as history.

**Keyboard.** Tab order: Add secret, Decline. Enter or Space activates. No shortcut adds or declines. The card never takes focus by itself; a polite announcement says "Secret requested: OPENAI_API_KEY". When the modal closes with a result, focus moves to the card's container; when it is cancelled, focus returns to Add.

**Accessibility.** The card is a group named "Secret request: OPENAI_API_KEY". The state is a live status line. The lock line is part of the group's description.

**Desktop, first slice.** The pending requests sit in a "Waiting for you" block at the top of the Secret vault page (8.1), one card each: "<asker> needs <NAME> in <scope>", the reason, and the answers. Store a value opens the Add dialog with the name locked and the scope set; a team scope cannot take a value yet. Use the secret in <scope> links the name to a scope that already owns it (`secret.link`). Decline sends `secret.decline`. Each answer goes to the service only, and the card leaves when the vault is read again. The transcript card above is not built yet, so a request is answered from Settings for now.

**Security.** Agent-supplied title, name and reason are untrusted: plain text, length-limited, the name validated (A29). The card holds the name, the scope label, the host label and a state enum, never a value.

**Desktop and Android.** Desktop here. Android shows the same card in the session chat (M-CH-01).

### 8.4 Delete secret confirm (D-VT-02, merged into X-DLG-01)

**Layout (artboard 14).** The destructive variant of the Dialog (X-DLG-01), 440 px. Title: "Delete secret OPENAI_API_KEY?" Body: "It is removed from the scopes that use it. Agents there lose it at their next run, and the next request for this name shows a new request card." Loss list (up to five rows, `body`, counts in tabular figures): "Project NexusCare", "linked"; "Team Desarrollo", "linked". Warning line: "This cannot be undone." Footer: Cancel (default focus, left) and the danger button "Delete secret (1 project, 1 team)" (right).

**States.** Default; shared (the linked count in the title and the button, as shown); busy (the button shows a spinner and "Deleting"; Escape ignored); error (status line "Not deleted: <reason>. The secret is still there." with Retry); host unreachable (the action disabled, "Host unreachable. Last known state, 5 min ago").

**Keyboard.** Escape and Cancel close. Focus starts on Cancel; Enter on the danger button only after Tab moves to it.

### 8.5 Secret vault and Settings

The vault has no rail entry (A27). Reached from Settings > Secret vault. Android lists and adds secrets only (8.1).

## 9. Decisions for owner review

Each decision below was taken without the owner. Ordered by impact.

| # | Decision | Alternative | What changes with the alternative |
|---|---|---|---|
| 1 | Bot Teams pages keep the Sidebar (as in the owner's Routines reference), while Calendar, Marketplace and Settings hide it as full pages. Design-system 5.1 says Bot Teams pages hide it; the reference shows it. | Hide the Sidebar on Routines and team pages too. | Routines and team pages gain a Back button and lose the team list while open. |
| 2 | The Settings navigation has a "Secret vault" item (A27). The inventory's Settings list has no vault row. | Put the vault under "Agents and accounts". | One nav item moves; the vault screen keeps its layout. |
| 3 | The Jev section is labelled "Routing decisions" in UI copy. Jev is an internal name and never appears in UI copy. | Another label, for example "Bot routing". | Label only. |
| 4 | New routine opens the plain-language form (D-AG-34, pv96), and Run now and Pause are added as routine actions. The inventory left the editor choice to the owner (A26). | Open the full automation editor (D-AG-15). | The form is replaced by the editor; Run now and Pause move to the editor. |
| 5 | Calendar shows routines and automations on one timeline; Show done starts off. | Show done starts on. | One default changes. |
| 6 | Team creation is a New team dialog on the Bot teams page (D-AG-13). Workspaces in this team starts off. The inventory has no team creation row. | Create teams only through the Conference room (X-CH-10). | Creation moves into the chat flow; the page keeps only lists. |
| 7 | Web addresses show a masked path and copy the full address; they are not printed in full, because the address carries a token. | Show the full address on the card. | The card shows the full token to anyone who sees the screen. |
| 8 | When the lead bot's host is unreachable, the Conference room keeps the typed message in the box and does not queue it. | Queue messages until the host is back. | A queue needs a delivery state and an ordering rule. |
| 9 | The team project brief and link screen (D-WS-31) and team memory (D-AG-26) are specified in text, not drawn. | Draw them as artboards. | Two more artboards (the cap is 14). |
| 10 | Team memory: the Bot and Team tabs, and only the team's bots and the owner can read it. | Another audience. | Audience note only. |
| 11 | The delegation depth limit is a number from 1 to 5, default 3, in Agents and accounts. | Another default. | Default value only. |
| 12 | The transcription key (D-ST-08) is entered through the Secret modal and stored in the vault, never in a plain field. | A plain field in Notifications and voice. | A credential would be shown in a field and could be logged. |
| 13 | Superseded (owner, 2026-11-12): the extension manager lives in the Marketplace ("Installed"), reached from the rail. | - | Settings keeps a link row only. |
| 14 | The in-app application is declarative only; the sandboxed web view (pending-owner-review item 3) is not designed. | Design the web view as a Work panel tab. | One more artboard and a security review. |
| 15 | Work panel tab kind (D-SL-08) is specified as one rule, not drawn. | Defer as the inventory says (Phase 5). | None now. |
| 16 | Sample publishers "MassOrca extras" and "Community" and sample extension names are placeholders. | The owner's own catalogue names. | Text only. |

## 10. Component gaps

- **G-1 Team mark.** The triangle team character used in the Sidebar, chips and calendar. A small component (16 and 20 px) filled with a team colour; the glossary calls it a team character.
- **G-2 Bot avatar.** The bot's team character at 16, 22 and 28 px, one colour shade per bot (A57), never an initial. Avatar.md covers people and providers; the bot form needs its own variant with the Lead chip.
- **G-3 Slider.** Text size and the volume-like settings need a Slider (track 4 px, knob 14 px, value label). SettingsRow names it but no component spec exists.
- **G-4 Colour swatch picker.** Accent presets (20 px swatches, check ring) and the custom colour entry.
- **G-5 Log view.** Extension logs: monospace lines with time, a level word and a message, with an error row background. Not the terminal surface.
- **G-6 Settings navigation.** The 232 px section list with selected and current states. SettingsRow covers rows, not the list.
- **G-7 Task detail.** Opening a Board task has no row (inventory G20). The Board needs a detail component when the owner decides.
- **G-8 Memory write rule.** Whether bots write to team memory on their own. Not a component, but the memory list depends on it.
- Code editor (D-ED-02) is not in the component specs; the extension editor uses a plain code block.

## 11. Token gaps

- `layout.settings.nav`: 232 px (proposed; was 216, widened for the item icons, owner 2026-11-13).
- `layout.routine.form.width`: 640 px (proposed).
- `layout.calendar.column.min`: 96 px and `layout.calendar.hour`: 48 px (CalendarViews, proposed).
- `layout.log.line`: 18 px (proposed, for G-5).
- `layout.slider.width`: 200 px (proposed, for G-3).

## 12. Not covered

- Android rows of these areas. The vault list and add on Android (M-VT-01) is described in 8.1 only. The Android Calendar, Marketplace and Bot Teams are open in the inventory.
- The New web address form (D-AG-25) and the web address payload.
- Team project brief detail beyond the text in 4.4, and team files versions beyond the outline in 4.9.
- The Conference room's delegation and request choice cards in full (X-DL-01 and X-CH-10 belong to the chat and delegation writers; artboard 1 shows the delegation block in its inventory form only).
- The mention menu (X-CH-09) as a drawn component.
- Routine history detail and the run log.
- Jev routing rules themselves (only the Settings section is specified).
- Sign-in and provider connection flows (D-ST-06 Connect), which need the provider's own grant.
- The headless server secret store (G22 in app-flow.md): how it is unlocked and backed up.
- Extension file location (G11 in app-flow.md) and the publisher verification concept (not in the inventory).
- The sandboxed web view (pending-owner-review item 3).
- High-risk permission review state, drawn; only its rule is written (7.3).
- Keyboard shortcuts beyond those named here; the full table is with D-ST-04 and design-system section 8.
- Contrast measurements of the artboard colours; the artboards use the tokens from the token file, and the validator runs on the theme file, not on these mockups.
