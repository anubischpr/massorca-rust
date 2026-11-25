# Design agreements register

What this is: every design agreement the owner made while reviewing the MassOrca screens (screens artifact versions 1 to 69) and the design system artifact, one row per agreement.
Sources: comment threads on the screens artifact (`2cb722f0-…`) and the design system artifact (`dff12697-…`), owner chat messages in the design sessions, `pending-owner-review.md` (owner-marked items only) and the "owner, 2026-11-1x" notes in `screens/*.md`, `design-system.md` and `components/*.md`.
Date: 2026-11-13.

**How to use:** check each screen against every General row in its area (and in Icons and labels, Theme and colour, and Copy, which reach every area); One-screen exception rows apply only to the screen they name. A row marked "Superseded by An" is history: check against An instead.

Notes on the columns:
- **Version** is the screens artifact version where the change landed. "DS vN" is a design system artifact version (it has its own numbering). "chat" is an owner chat message, "doc" an owner note in the docs with no version.
- **Source** is a comment thread id (screens artifact unless marked DS) or "chat" with the owner message time (UTC).
- Decisions in `pending-owner-review.md` that Claude took on the owner's behalf and the owner has not confirmed are not agreements and are not listed, except items 17 and 32, which carry an owner mark.

## Frame and navigation

| ID | Version | Rule | Applies to | Scope | Source | Supersedes |
|---|---|---|---|---|---|---|
| A1 | chat | The Activity rail, Title bar and Sidebar form one L-shaped frame surface (`surface.frame`, #1b1b1c in dark) with no seam or border between them; the canvas (`surface.canvas`, #151517 in dark) sits inside it with a rounded top-left corner. | Desktop shell, every desktop screen | General | chat 2026-11-12 09:52 and 11:05 | |
| A2 | chat | The Activity rail is always visible; the Sidebar toggle never hides it. | Activity rail, every desktop screen | General | chat 2026-11-12 11:09 | |
| A3 | chat | The Activity rail holds, top to bottom: the mode button, Tasks, Calendar, Marketplace; at the bottom the theme toggle, one usage ring per agent and Settings. Agent usage lives in the rail rings, not the Status bar. | Activity rail | General | chat 2026-11-12 10:55 | |
| A4 | chat | The mode button is a single button whose icon shows the current mode (grid for Workspaces, bot for Bot Teams) and morphs between the two on press (cross-fade; instant with reduced motion). | Activity rail mode button | General | chat 2026-11-12 10:57 | |
| A5 | chat | Desktop regions run left to right: Activity rail, Sidebar, Conversation, Session summary, Work panel, Project sidebar, and are called by these glossary names. | Desktop session layout, all docs and UI copy | General | chat 2026-11-12 11:55 and 11:59 | |
| A6 | chat | The Work panel holds terminal, browser, Markdown/file and mobile emulator as tabs; the Project sidebar holds files, sessions, source control and checks. | Work panel, Project sidebar | General | chat 2026-11-12 11:53; screens/shell-setup-hosts.md D-SH-17 | |
| A7 | chat | A file or diff opened from the Project sidebar opens as a tab in the Work panel. | Project sidebar and Work panel, Workspaces | General | chat 2026-11-12 12:14 | |
| A8 | chat | Activity rail destinations (Tasks, Calendar, Marketplace, Settings) open as full canvas pages. | Activity rail destinations | General | chat 2026-11-12 12:14 | |
| A9 | chat | Automations has its entry in the Workspaces Sidebar (next to Prototypes, Tasks and MassOrca Mobile, as in the owner screenshot) and opens as a full page; the rail has no Automations icon. | Workspaces Sidebar, Activity rail | General | chat 2026-11-12 12:35 | |
| A10 | 8 | The main grouping is by project, and a project can live on more than one host: the Sidebar's top level is the project, with one host row per host under it and that host's workspaces under the host row; an unreachable host dims only its own row. | Sidebar tree and every list that groups workspaces (desktop and Android) | General | 89e2c7a0-5747-4688-9a72-bbc3e2eae09e | Partly superseded by A97: the project and host levels stay; "that host's workspaces under the host row" is replaced by session rows |
| A11 | 6 | Window controls and other Title bar buttons have no fill of their own; they show the Title bar background (`surface.frame`). | Title bar buttons, desktop and Android bars | General | ba015a7c-a06e-43f3-bb94-3c8a3d452ae6 | |
| A12 | 9 | Placement and toolbar buttons have no border and no fill at rest; on hover the fill lights up (`fill.hover.canvas`) and the label turns primary; open shows the selected fill; keyboard focus shows the focus ring. | Placement buttons (host, project, worktree) and other ghost buttons in rows | General | 057c2cd3-bccc-47cf-9ed9-da9a8f4ca643 | |
| A13 | 25 | The command palette has a "Sessions" group; groups run Workspaces, Sessions, Tabs, Commands, and each session row starts with the icon of its agent, then title, "project / workspace" and state or last activity. | Command palette (desktop) | General | 8d7dbf98-69e0-4c2d-8707-202851accdc8 | |

## Theme and colour

| ID | Version | Rule | Applies to | Scope | Source | Supersedes |
|---|---|---|---|---|---|---|
| A14 | chat | There is a light theme with the same structure as dark: the L-shaped frame in a slightly darker neutral and the canvas as the lightest surface, with the same rounded corner. | Every screen, light variants | General | chat 2026-11-12 10:00 | |
| A15 | 5 (DS v9) | The accent is lavender, replacing blue #2c63dc everywhere (fill #7461d8 dark / #6447c9 light; text and focus #b8a8f8 dark / #5b3fc4 light); terminal, syntax and text-selection blues are not the accent and stay. The default Appearance preset is lavender. | Every accent use: primary buttons, switches, checkboxes, progress, active icons, links, focus ring | General | chat 2026-11-12 18:57 | |

## Icons and labels

| ID | Version | Rule | Applies to | Scope | Source | Supersedes |
|---|---|---|---|---|---|---|
| A16 | 7 | Every place that shows an agent uses its own icon, never a letter (C, X, G, O, K). | Usage rings, account rows, session chips, Android marks, any agent mark | General | f83fab06-ca93-48ad-9c93-143134267c00; 36f2077c-773c-45a8-b915-0505eaadc65d; pending-owner-review item 32 | |
| A17 | 18 | Always use agent icons; where a text starts with an agent name, the icon (14 px) replaces the name ("[Codex icon] Feed endpoints"), and the name stays as tooltip and accessible label. | Every label, row, title, tab, chip or table cell that names an agent | General | 0a47fe08-f141-4a55-9b47-ad540a1219cb; pending-owner-review item 32 | |
| A18 | DS v6 | The Model picker shows each agent by its full-colour icon, never a letter, in the agent column, the model rows and the composer chip; an agent without its own icon uses the generic agent mark. | ModelPicker | General | DS 807f1538-f868-4d16-8d19-ea5d631468f8 | |
| A19 | 14 | Session import sources (Claude Code, Codex, OpenCode, Gemini CLI) show their agent icon next to the source name, after the status indicator. | Session import, detect and map | One-screen exception | 0c3b8732-9c8d-4e05-a339-3571816b68ca | |
| A20 | 15 | In tables, the agent column is titled "Agent" and shows only the agent icon (16 px), with the name as tooltip and accessible label. | Every table with an agent column | General | 2f3423b0-4c59-46a7-9e88-e41796eb8aed | A19 (table part only) |
| A21 | 13 | Every label that names a kind of content carries an icon on its left that represents that content (fields, buttons, menu items, chips, tabs, rows); exceptions are table headers, prose, numbers and values. | Every label that names a content type | General | 6bafc176-eb7d-43d0-ab50-2b0c750136da; design-system.md 4.6 | |
| A22 | 30 | The Environment card's data rows carry icons on the left: account `person`, repository `book`, branch `git-branch`, pipeline `workflow`, host `server` beside its status dot; each row has its name as tooltip. | Environment card (Session summary, host-unreachable screens, Android Summary) | General | ff36c41e-b452-4211-ab1e-f48b65447937 | A21 icon for Repo (`folder` became `book`) |
| A23 | 36 | Session summary cards have icons on the left at two levels: card titles (Environment `server`, Context gauge, Results wand, Plan list, Checklist list-checks) and data (account person, repository book, branch, pipeline workflow, URLs globe, files document), each with a tooltip. | Session summary cards, desktop and Android | General | 5a526535-a0cb-40cb-8d1b-75ce636c46aa | |
| A24 | 40 | "Memory" uses the Lucide brain icon, not a book. | Every Memory entry (Board, Routines, Conference room, routine form, Settings, Android Bot Teams) | General | 3c16e8fc-4be5-4cd1-97f7-e64b3cda3ea5 | |
| A25 | 56 | The Board is shown with a kanban icon (columns of different heights), labelled "Board" / "Open the Board". | Every Board entry | General | c5ce852a-e808-43fc-bd22-001d4bff60ae | |
| A26 | 12 | The MassOrca logo is the pink rose (#FF2E88) from pv96, kept as a reusable asset `massorca-logo.svg`. | MassOrca logo wherever it appears | General | 91203630-7fa8-4914-b784-ae2f054ba62c | |
| A94 | 76 (DS v13) | Agent icons are the real assets of the MassOrca app, copied exactly from the app's catalogue (`agent-catalog.tsx`), never letters, generic glyphs or approximations: Claude, Codex (OpenAI mark), Antigravity, Gentle Shell (the rose, never "Pi" or a Pi mark), and Gemini, OpenCode and Cursor; one shared AgentIcon component in the design system feeds every screen. | AgentIcon component; every agent mark on every screen, desktop and Android, light and dark | General | chat 2026-11-20 (owner request relayed by the parent session) | A16 (stand-in marks) |
| A95 | 77 | Every project menu and project pill offers "No project", with a struck-through folder icon; such conversations are saved in a folder set in Settings (General), default `~/MassOrca/Conversations`, and run with no worktree. | Project menu (desktop placement bar), project pill (Android New session), Settings General, Setup checklist (default path, changeable; v78) | General | ff51a69f-9c69-4568-bcf8-59c0f4e48df4 | |
| A96 | 80 | Child sessions and subagents are different things and never share a name: child sessions are separate delegated sessions (Sidebar row, own worktree, any agent) shown in a "Child sessions" card and the DelegationTree; subagents are helpers an agent runs inside its own session, shown only in a "Subagents" card, and every session (child sessions included) has its own Subagents card (v83). | Session summary cards (desktop and Android), DelegationTree, glossary, copy | General | e486da39-00b6-44d5-95c3-32a6b4967d03 | |

## Composer

| ID | Version | Rule | Applies to | Scope | Source | Supersedes |
|---|---|---|---|---|---|---|
| A27 | chat | One Model picker component is the single selector for agent, model, effort and access in every composer ("+ Create", Tasks and sessions): a chip with agent icon, model and effort opens a popover with an agent column, a searchable model list with shortcuts, an effort column, and a footer with Standard / Fast and access. | ModelPicker in every composer | General | chat 2026-11-12 10:46 | |
| A28 | doc | In a session the Model picker opens above the Composer (owner reference screenshot). | ModelPicker placement in session composers | General | screens/workspaces-sessions.md, Model picker placement | |
| A29 | DS v8 | Composer chips (attachments, mentions, suggestions and the model chip) are pills (`radius.full`), not boxes. | Composer (all composers, desktop and Android) | General | DS 95adc331-9835-41b6-bf2b-3295721f7cea | |
| A30 | 33 | The Chat / Terminal selector has no border; it sits on a sunken fill (`surface.sunken`) and the active option shows the selected fill. | Chat / Terminal selector wherever it appears | General | a3fce08a-fc21-406b-a270-2236ec5e968b | |
| A31 | 38 | The Tasks composer is centred at 760 px: a centred "Workspaces / Bot Teams" switch above; a placement bar attached to the Composer's top edge (same border, rounded top corners) with borderless host, project and worktree buttons; the Composer below with access, Chat / Terminal, "Start now", the model picker with agent icon and Send in its bottom row. In Bot Teams the bar shows team and project. | Tasks composer | General | aa7b8094-85eb-4b31-9e51-515c9146104d | |
| A32 | 42 | Every composer that needs placement (start page, empty start page, Tasks) uses the Tasks layout: placement bar attached to the Composer's top edge, Composer below with 12 px corners, and the Chat / Terminal switch in the Composer's bottom row, never in the placement bar. Session composers keep the same body without the bar. | Every desktop composer | General | 96068fb7-2627-4f99-9f68-dfbcd2969ebd; components/Composer.md | |
| A33 | 47 | Every desktop session composer uses the approved body: placeholder "Ask anything, @ to mention files, / for commands"; bottom row +, access chip ▾ (for example "Workspace write"), the model picker with the agent icon, microphone and a round Send; 12 px corners and a light shadow. No "Message Claude" field or rectangular Send. | Composer (all desktop session composers: sessions, extension slots, secret request, Creator mode) | General | 3651165b-f564-4571-84b8-23df52137f7a | |
| A34 | 47 | The Conference room composer uses the same session composer body, but its selector shows the lead bot (for example Mara) instead of a model. | Conference room composer, desktop | General | 3651165b-f564-4571-84b8-23df52137f7a | |
| A98 | slider mock-up (approved 2026-11-25) | The Model picker's Reasoning column is a vertical slider that snaps to one stop per reasoning level the model declares (2 to 5; a model with no levels, or one, shows no slider). The fill is a gradient in the selected agent's brand colours. Below the top level the fill is only the gradient; at the top level particles, a glow and a shimmer appear. Reduced motion holds that frame still. Up and Down (also Right and Left, Home and End) change the level. Dragging or clicking the bar or a level changes the level and keeps the popover open; it closes only on an outside click or Escape. After a change the thumb and fill move in one 220 ms transition, and below the top level nothing loops. Stop spacing is computed from the track height. The top-level glyph is Lucide `sparkles`. | Model picker Reasoning column (X-AG-02), desktop and Android | General | owner approval of artboard ws-reasoning-slider, relayed 2026-11-25 | the list of effort rows in ModelPicker.md |

## Sessions and Sidebar

| ID | Version | Rule | Applies to | Scope | Source | Supersedes |
|---|---|---|---|---|---|---|
| A35 | chat | Session summary cards appear only when they hold information; each card appears when its first record arrives (short animation, none with reduced motion); an empty card is never shown. | Session summary, desktop and Android | General | chat 2026-11-12 11:53 | |
| A36 | chat | Artifacts have no page of their own: they appear in the Results card of the Session summary and open in a Work panel tab; each URL has Open and Copy URL. | Results card, Work panel | General | chat 2026-11-12 12:35 | |
| A37 | 10 | With no projects yet, the project button reads "Add a project" with a folder-plus icon and opens the add-project dialog directly. | Start page project button | One-screen exception | aad73d1c-7233-4dfe-b2f3-6b40c6f643fc | Superseded by A38 |
| A38 | 11 | Adding a project is done from the "Choose a project" menu: it lists the host's projects and always ends with "Add a project…" ("No projects on This PC yet" when empty); there is no separate "No project yet" block. | Project button and project menus on start pages | General | 3277f477-db94-460b-8848-08feb6b74397 | A37 |
| A39 | 12 | The MassOrca logo sits to the left of "Start a session" on the empty start page. | Empty start page | One-screen exception | 91203630-7fa8-4914-b784-ae2f054ba62c | Superseded by A40 |
| A40 | 32 | The start page shows the MassOrca logo (48 px) centred above a centred heading, "What should we work on?" or, with a project chosen, "What should we work on in <project>?" with the project name dotted-underlined and clickable to change project; placement buttons are centred. Both start pages (with and without workspace) match. | Start page and empty start page | General | a4af097b-d285-4c2b-aaee-e027b5ddf90c | A39 |
| A41 | 19 | A project can have its own icon: a 40 px icon button (pencil) opens "Project icon" with colour (usage colours plus neutral), icon grid, Upload image, Use GitHub avatar (only with a GitHub account) and Reset to folder; the chosen icon replaces the folder glyph in the Sidebar, tabs and pickers, saved per project and synced across hosts. | Project icon in Sidebar, tabs, pickers, project settings, new workspace | General | 05bfb946-d0e2-4654-8ec0-9457649d3144 | |
| A42 | 16 | A job that always runs in the background has no "Run in background" button; one quiet line says it runs in the background and the Title bar Back leaves the page. | Session import and other always-background progress pages | General | cefe1888-24ae-4518-a661-630505609890 | |
| A43 | 41 | The Work panel empty state is a centred column of four buttons, each as wide as its label with its icon on the left: New terminal, New browser tab, New Markdown, New mobile emulator; no panel, title or explanation. | Work panel empty state | General | 2aa00f5a-7078-4c8d-a0d5-9be8198af277 | |
| A44 | 17 | Notes on a page element are a floating note anchored to the element with a caret (selector with close X, focused text field, "Enter to send · Esc to close", round send), as in Claude Code; no card across the page, no thumbnail, no Cancel / Send buttons; several elements can be noted before sending. | Element notes in the browser tab and every annotate-an-element flow | General | 4e4dd096-9750-407b-9fa8-e1e7acc59c7f | |
| A97 | doc (2026-11-24) | The desktop Sidebar has no workspace level: project, then one host row per host the project lives on (also when it is only one), then the session rows of that host, then child sessions nested under their parent. A session row is the AgentStatus glyph, the agent icon and the session title (the branch name until the agent sets a title); the worktree, branch or folder is part of the session and shows in the row tooltip and in the Session summary Environment card, never as a row. | Sidebar tree and session rows (desktop); Android lists follow A82 | General | ruling from artboards ws-session, ws-delegation, ws-narrow, ws-light, ws-workpanel-empty, sh-add-host (v68 to v83) | A10 (its "workspaces under the host row") |

## Approvals and review

| ID | Version | Rule | Applies to | Scope | Source | Supersedes |
|---|---|---|---|---|---|---|
| A45 | 39 | An approval row has a single "Review" button (clicking the row does the same); Review opens the task's session in the floating workspace over the current page, with agent icon, title, "project / workspace", "Go to session", minimise and close, and the approval card (exact command, access warning, Deny / Approve) inside. | Approval rows (Tasks "Needs your approval" and similar lists) | General | 02d5e466-335c-4788-95fe-2dad3ea2f817 | |

## Files and diffs

| ID | Version | Rule | Applies to | Scope | Source | Supersedes |
|---|---|---|---|---|---|---|
| A46 | 20 | Every file name carries a file-type icon on its left: a badge with the type's short name (TS, TSX, JS, RS, PY, CSS, JSON); `CLAUDE.md` uses the Claude icon and `AGENTS.md` the Codex icon; folders keep the folder glyph. | File names in trees, lists, tabs, diff headers, palette and search results | General | 73f63e95-ca40-4d6b-a3fb-b8c0af2360f3; design-system.md 4.6 | |
| A47 | 22 | Each file type uses its representative colour (PY #3776ab with #ffd43b, TSX/JSX #61dafb, TS #3178c6, JS #f7df1e, RS #dea584, CSS #663399, JSON #cbcb41); the generic file is a blue document (#4f8df5). | File-type icons | General | 1641eea5-dcdb-4c05-8802-4f0a621af5e6 | Superseded by A48 (generic colour only) |
| A48 | 24 | Generic files (.md and any type without its own colour) use a grey filled document (`text.tertiary`), not blue; typed files keep their representative colour. | File-type icons | General | 1d9ffde3-52c8-4cb4-81d6-ed01cfab9304 | A47 (generic colour) |
| A49 | 21 | The Commit message field and Commit button take the Composer's soft edges: 8 px radius, `border.default` instead of `border.strong`, `surface.raised`, `shadow.e1` on the field; focus switches to the focus border. | Commit box in Source control (desktop and Android) | General | 59d5ddb1-158b-41a6-a472-83dd9716d041 | |
| A50 | 23 | Each check is one 28 px line (status glyph, name, status word, chevron); workflow, duration and actions (View log, Re-run) live in an expandable detail under the row; running or failed checks open expanded. | Checks list, desktop and Android | General | 99f817c6-7663-4dc2-b69a-14779555a6a8 | |

## Prototypes and design system

| ID | Version | Rule | Applies to | Scope | Source | Supersedes |
|---|---|---|---|---|---|---|
| A51 | 26 | A design system belongs to one project and can be shared with another project as a link, not a copy: only the source edits; a linked project shows a link chip naming the source and can Unlink or Make a copy; the source shows a link count; deleting the source asks for confirmation naming linked projects. | Design system rows in Prototypes, desktop and Android | General | f599bc61-b6cf-43c3-aa13-8378409704c9 | |
| A52 | 27 | The Design system row expands into sections (Overview, Colors, Typography, Spacing, Corner radius, Shadows, Motion, Components, Assets), each with icon and count, plus "Add section…"; own sections can be renamed, reordered and deleted, base sections only hidden; linked projects see them read-only. | Design system in Prototypes, desktop and Android | General | 0ae5b96c-57fd-4435-8d4b-f48aaaa5175b | |
| A53 | 29 | The Prototypes centre is an infinite canvas (dot grid, pan, zoom −/%/+/Fit) with every screen and direction as a named artboard and flow arrows; Play and a floating bar step between screens; floating sticky notes can anchor to elements and go to the agent with "Send to agent"; the fine detail panel shows only when the user opens the right side panel. | Prototypes canvas, desktop and Android | General | dc4618b0-f6a4-4d05-a4ed-6046fb5c8fb7 | |
| A54 | 69 | Android has Prototypes and per-project design system screens that follow the desktop rules (A51 to A53): Prototypes list with the design system first per project, prototype canvas with Directions / Design system / Versions, Play, design system sections, linked read-only design system. | Android Prototypes and design systems | General | chat 2026-11-13 09:12 | |

## Bot Teams

| ID | Version | Rule | Applies to | Scope | Source | Supersedes |
|---|---|---|---|---|---|---|
| A55 | chat | Bot Teams tasks are born in the Conference room when a requirement is detected and appear on the Board; a composite requirement goes to the lead bot, which delegates parts to other bots by competence. The Bot Teams Sidebar holds Conference room, Board, and Projects with Files, Memory and Routines. | Bot Teams navigation, Conference room, Board | General | chat 2026-11-12 11:28 | |
| A56 | chat | Routines are created from the Routines page or by a bot in the Conference room when asked to schedule work; every routine appears in the Calendar. Routines are a simplified version of Automations for non-technical users. | Routines, Calendar, Conference room | General | chat 2026-11-12 11:29 and 12:08 | |
| A57 | DS v7 | In Bot Teams, each bot is drawn as its team character (shape, colour, eyes from pv96) with animations: a working bot leans and shows its effect (only a halo under 24 px), an idle bot gestures every 8 to 30 s, an offline bot stays still, and everything stops with reduced motion or animations off; small and still inside Session summary cards. | DelegationTree Bot Teams variant and every bot/team character | General | DS 87aee0c7-2211-4532-98a2-af0d5c166471 | |
| A58 | 54 | The Bot Teams home is a carousel of team characters (running ring while a bot works, waiting badge, name and state, "New team" at the end) with, below, the tasks of all teams: filter pills, "Needs your review" first, then "Working now". Tapping a team opens its Conference room; long press edits. No search, "All teams" row or team list. | Android Bot Teams home | One-screen exception | abce7d62-232c-4554-8c89-24266351369c | |
| A59 | 55 | The team pill on a team page equals the Conference room pill: team character, team name and ▾ to switch team, with no "Runs on" text (the host is shown once, elsewhere). | Every team selector pill | General | 4ef16913-84e8-45d8-bb0d-5674c645ff88 | |
| A60 | 56 | The Board shortcut is a kanban icon button in the Conference room bar. | Conference room top bar | One-screen exception | c5ce852a-e808-43fc-bd22-001d4bff60ae | Superseded by A61 |
| A61 | 57 | The Conference room top bar keeps only the "…" button; its menu holds, each with its icon: Board (task count), Routines (count), Calendar (filtered to the team), Memory, Files, Team settings. | Conference room overflow (Android; see doubts for desktop) | General | c5ce852a-e808-43fc-bd22-001d4bff60ae | A60 |

## Extensions and Marketplace

| ID | Version | Rule | Applies to | Scope | Source | Supersedes |
|---|---|---|---|---|---|---|
| A62 | chat | The Workspace board (D-WS-08) is an extension, not core. | Workspace board | General | chat 2026-11-12 12:21 | |
| A63 | 43 | Extensions can be created through the chat composer, and third-party extensions can be added: a split button "+ Add extension ▾" offers Create with chat, From the Marketplace and, under "Third party", From a Git URL…, From a folder…, From a file (.zip)…; every third-party extension passes the permission review and carries a "Third party" badge. | Add extension entry points (Marketplace, extension manager) | General | abb062df-ad4b-4baf-9d32-380fe78a34c0 | |
| A64 | 43 | The extension builder screen has a composer at the bottom with a bar showing the extension and "Creator mode ▾". | Create an extension screen | One-screen exception | abb062df-ad4b-4baf-9d32-380fe78a34c0 | Superseded by A65 |
| A65 | 44 | An extension is created in a normal session in Creator mode, not a builder page with template, file, editor and log columns: a "Creator mode" chip in the session header, the agent's steps in the Conversation, the shared composer with "Creator mode ▾", and a Work panel with Preview, Files and Logs. | Create an extension (Creator mode session) | General | chat 2026-11-13 05:32 | A64 |
| A66 | 45 | The Creator mode Preview shows the skeleton of the whole app with the extension in its real place (accent outline and name), a fullscreen option, and Select, Note and Draw tools; "Send to agent" sends notes (anchored to elements) and strokes with the next message. | Creator mode Preview tab | General | bc765a2c-f674-442d-b705-765701cff738 | |
| A67 | 46 | The extension screens include an advanced full-page extension app example (Sticky Wall) built in Creator mode, with its own Sidebar entry and agent tools. | Extension app example artboard | One-screen exception | chat 2026-11-13 05:54 | |
| A68 | 48 | Extensions are centralised and managed in the Marketplace (Discover, Installed, Updates, categories), section Installed; Settings keeps only a row that links to Marketplace > Installed. | Extension manager, Marketplace, Settings | General | fa7a297e-d85d-487c-9290-705ebbbaa660; pending-owner-review item 17 || Superseded by owner decision 2026-11-24 (Settings has no Extensions section; extensions are managed only in Marketplace > Installed) |
| A99 | screenshot (approved 2026-11-25) | The Marketplace item page's invocation banner is a panel with no border and 16 px corners. Its gradient comes from the item's listed colours; an item with no colour takes one from an eight-colour brand-neutral palette chosen per item, never the usage ring colours (Ruling R-IB1). The example prompts are centred pills in the composer's surface colour, each with the item's tag, the text and a trailing arrow. The footer line is centred. | Marketplace item page, part 4 (desktop and Android) | General | owner approval of the screenshot, relayed 2026-11-25; components/InvocationBanner.md; artboard bt-invocation-banner | the bordered info box first built for the banner |

## Settings

| ID | Version | Rule | Applies to | Scope | Source | Supersedes |
|---|---|---|---|---|---|---|
| A69 | 49 | Every Settings menu item has an icon on the left (person, bot, palette, keyboard, terminal, plug, server, bell, puzzle, lock, shield, route); "Extensions" ends with an arrow because it opens Marketplace > Installed; the nav column is 232 px. | Settings navigation, all Settings screens | General | 3d1e54c3-ffdb-4072-9a94-d3cbfec81950 | |
| A70 | chat | A secret vault lets the user give agents credentials without pasting them in chat: a secret-request card in the conversation opens a modal for variable name and value; the value never enters chat, model context or logs; the Vault screen views, edits, rotates and deletes secrets. | Secret request card, secret modal, Vault | General | chat 2026-11-12 11:43 | |
| A71 | chat | Secrets have three scopes, project, team and global, with project preselected in the modal. | Secret modal, Vault | General | chat 2026-11-12 11:44 | |
| A72 | chat | A secret can be shared internally as a link, not a copy: updating it updates every user of it, and when its owner deletes it, it is deleted for all. | Secret modal, Vault | General | chat 2026-11-12 11:46 | |

## Delegation and worktrees

| ID | Version | Rule | Applies to | Scope | Source | Supersedes |
|---|---|---|---|---|---|---|
| A73 | chat | Agent orchestration is in the first version: one delegation engine for both modes, and a delegation tree with each child's state inside the parent session (Workspaces) or the Conference room (Bot Teams). | DelegationTree, sessions, Conference room | General | chat 2026-11-12 11:31 | |
| A74 | DS v7 | The DelegationTree Workspaces variant shows each agent with its own icon (the one in the Model picker) instead of a terminal icon. | DelegationTree (Workspaces) | General | DS 248919f8-f173-4bd4-9e30-4db38f138544 | |
| A75 | 37 | Child sessions follow four worktree rules: (1) by default a child gets its own worktree; (2) "Use the current worktree" only when the child only reads, the workspace is a plain folder without git, or the user picks it on the delegation card; (3) two sessions never write in the same worktree at once, so a child writing in the current worktree makes the parent wait; (4) parallel launches always use an own worktree per child. The card preselects "Own worktree", disables "Current worktree" with a reason in parallel launches, and the parent shows a waiting strip. | Delegation card, DelegationTree, parent session | General | chat 2026-11-13 04:29 (owner decision of 2026-11-12) | |

## Android layout

| ID | Version | Rule | Applies to | Scope | Source | Supersedes |
|---|---|---|---|---|---|---|
| A76 | 50 | Android phone screens use floating controls instead of bars: 48 dp round floating buttons (back or mode, overflow), a centred floating title pill, and a floating group for two to four actions. The session screen has the view group Chat, Summary, Files, Terminal and a floating session pill under it; the composer is a floating card with +, model chip with agent icon, reasoning bars, access lock, microphone and Send, the desktop controls in the same order. The team composer is one row. | Every Android phone screen | General | chat 2026-11-13 07:35; screens/android.md 1.1 | |
| A77 | 51 | In Android "New session" the options are one line of pills that scrolls sideways; there is no "Claude · Opus 5.5 · Medium" pill, because the composer already shows agent, model and effort. | Android New session (and any Android option row above a composer) | General | d9ef29b0-da22-4ecd-a43a-116af4f93588 | |
| A78 | 52 | Android New session: a close button, a centred host pill ("● This PC ▾") that opens a "Run on" sheet, and "…"; the prompt is written from the top under "What should Claude work on?"; above the composer a one-line pill carousel starting with an icon-only attach-image pill, then project, worktree, base ref ("From main") and "Chat view". | Android New session | One-screen exception | chat 2026-11-13 07:57 | |
| A79 | 53 | The Android mode change is a visible floating segmented pill "Workspaces / Bot Teams" (each with icon, active on lavender) in the centre of Home; Settings is the leading round button and "…" (holding the theme toggle) the trailing one. | Android Home in both modes | General | chat 2026-11-13 08:06 and 08:07 | pending-owner-review item 21 (theme in the Home bar) |
| A80 | 58 | Home shows a Projects section first, each project with one row per host. | Android Home | One-screen exception | 22b718ad-bea4-4646-85bf-e91ef6000fa1 | Superseded by A81 |
| A81 | 59 | Android Home lists only Hosts, as status cards (connection, usage, Pair again, Retry) that open the host detail, not sessions; a "Projects" row in the top grouped card opens the Projects screen (M-PJ-01). | Android Home, host cards | General | 22b718ad-bea4-4646-85bf-e91ef6000fa1 | A80 |
| A82 | 60 | The Android Projects screen is a clean list with no cards, matching the desktop Sidebar: each project is a header, a small host label separates groups when a project lives on several hosts, and child sessions are nested under their parent session. | Android Projects list and every Android workspace list | General | chat 2026-11-13 08:43 | |
| A83 | 61 | Child sessions are shown by indentation under the parent only, with no connecting lines. | Nested child sessions (Android Projects; desktop Sidebar if nested) | General | 5778b32f-2b7d-446d-a3c3-39b5f2a16693 | A82 (connecting lines) |
| A84 | 62 | Every session row, parent and child, carries its agent icon between the status icon and the name. | Session rows in Projects and workspace lists | General | 9bcd38aa-b114-4b1f-8b8e-87e7a0bb0e3c | |
| A85 | 64 | A child session's indent places its agent icon in line with the first letter of the parent session's name (80 dp on Android). | Nested child session rows | General | 2e05ca41-b97b-4ff5-82a7-20f63259f6e8 | |
| A86 | 63 | The "Projects" row comes first in the top card of every Android Home; with no projects, a "No projects yet" screen offers "+ New workspace"; the New session project pill lists the host's projects and ends with "No project" (folder session, no worktree or branch pills) and "New project…". | Android Home, Projects empty state, New session project pill | General | 98aee4c5-8c6d-4e6a-b4a0-03bda79aacbd | |
| A87 | 65 | On Android, the clipboard icon in the session view group is the session Summary (Plan, Checklist, Environment and the other cards live inside it), and the Summary screen uses the same view group with the session pill under it. | Android session view group, Summary screen | General | 0c2ae92d-cfb0-49d5-a509-f8e5bf53b4c5 | |
| A88 | 66 | The Android workspaces section follows the Projects agreements: title pill "Projects", project headers, host labels, agent icons and nested children; empty and error copy speak of projects; the filter is "Filter projects" with a Hosts group; "New workspace" opens New session (the old form stays only as reference). | Android Projects and workspaces artboards | General | bf64c1d6-6a2d-4baf-8e98-6201b7e3fa64 | |
| A89 | 67 | Any "New workspace" form starts with a Host field (host icon, name, connection dot) above Project, listing only the hosts where the project lives. | New workspace form (reference) and host choice before project | General | de2d95f4-7196-4a16-94ed-49376dee25bd | |
| A90 | 68 | The Android Projects "…" menu holds Prototypes, Design systems, Archived workspaces and Sort by, each with its icon; inside a session, Prototypes stays in the session "…" menu. | Android Projects overflow, Android session overflow | General | defcf13c-8f93-4836-8e77-64dd140a2fb4 | |

## Copy

| ID | Version | Rule | Applies to | Scope | Source | Supersedes |
|---|---|---|---|---|---|---|
| A91 | chat | UI copy and docs use the glossary names (Activity rail, Sidebar, Conversation, Session summary, Work panel, Project sidebar) from now on. | All copy and docs | General | chat 2026-11-12 11:59 | |
| A92 | 31 | The start page has no helper text under the heading ("Choose where it runs, then describe the task…" is removed); the placement buttons follow the heading directly. | Start pages (desktop) | General | 09d604d9-ebb5-4f26-b178-5447218bb45f | |
| A93 | 16 | Background job copy: "Runs in the background. Use Back to leave; a notification tells you when it ends." | Session import progress (and A42 pages) | General | cefe1888-24ae-4518-a661-630505609890 | |

## Doubts

- A12: the owner said "remove the border from the buttons" on the placement row; it is unclear whether this reaches every secondary button or only placement and toolbar buttons. Read here as ghost buttons in rows.
- A21 and A22: the Repo icon changed from `folder` (v13, design-system.md 4.6) to `book` (v30, v36); design-system.md still says `folder`.
- A27 and A33: the chat agreement puts access inside the Model picker footer, while the approved session composer (v47) and the Android composer (v50) show access as its own chip or lock.
- A34: whether the lead-bot selector also applies to the Android Conference room composer, which v50 drew as one row without a selector.
- A58 and A61: both were asked on Android screens; whether the desktop Bot Teams home and desktop Conference room bar should follow is not stated.
- A82 to A85: the owner asked for nested child sessions on Android; whether the desktop Sidebar should also nest them (today a counter) was asked by Claude in v60 and not answered.
- A87: Claude offered to remove the redundant "Open chat" button from the Android Summary; no answer.
- A88 and A89: v66 marks the "New workspace" form superseded, v67 then adds a Host field to it; read as the form kept for reference with Host first.
- Term: Claude used "extension" for the owner's "plugins" (glossary term) and offered to switch to "plugin"; no answer.
