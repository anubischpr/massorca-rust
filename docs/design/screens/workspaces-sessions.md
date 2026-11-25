# Screens: Workspaces and sessions

Status: draft for owner review, 2026-11-12. Desktop only (Windows, macOS, Linux); the Android rows of these areas are listed under "Not covered".

Scope: inventory sections 5 (Workspaces and sidebar, Workspaces mode), 6 (Agents and sessions, without the Bot Teams and Calendar rows), 8 (Terminal), 9 (Editor and files), 10 (Diff, review and source control), 11 (Browser pane) and 12 (Command palette and search), plus the shell rows these areas live in: Project sidebar (D-SH-04), Work panel (D-SH-17) and Mobile emulator tab (D-SH-18). Delegation in Workspaces (X-DL-01) is in section 6.

Read with: `glossary.md` (region names), `design-system.md` (tokens, layout, motion), `components/*.md` (every component used here has a spec; the few that do not are under "Component gaps"), `screen-inventory.md` (IDs), `app-flow.md` (journeys J3 to J7).

## How to read this file

Each screen has: purpose, layout (regions and components, with sizes), content and copy, states, interactions and keyboard, desktop and Android notes, and the artboard that shows it. Token names are written as in the design system (`surface.canvas`, `space.3`). Sizes are logical pixels at 100 percent zoom. Rows that are merged, dropped or become extensions get one line in the table at the end of their section.

Common frame, used by every session artboard: Activity rail 48 px on `surface.frame`; Sidebar 256 px (200 to 400) on the same fill; Title bar 36 px (Windows and Linux; macOS 40 with the traffic lights at the left); the Canvas on `surface.canvas` with `radius.canvas` (12 px) on its top-left corner only; Status bar 24 px at the bottom of the Canvas column. Regions on the Canvas, left to right: Conversation, Session summary, Work panel, Project sidebar, separated by 1 px `border.subtle` lines (the splitter hit area is 6 px).

## Artboard index

Folder: `scratchpad/screens/project/`. All artboards are static comps (no interaction), 1440 by 900 unless stated.

| File | Size | Theme | Shows |
|---|---|---|---|
| `ws-start.dc.html` | 1440 x 900 | dark | Start page ("+ Create") with the composer and the Model picker open (flipped below the composer) |
| `ws-session.dc.html` | 1600 x 900 | dark | A running session with all six regions in order: Sidebar, Conversation with minimap, Session summary (only cards with data), Work panel with a terminal tab, Project sidebar on Files |
| `ws-diff.dc.html` | 1440 x 900 | dark | The same session after the run: Work panel on a diff file tab, Project sidebar on Source control, Sidebar and Session summary hidden (owner reference) |
| `ws-summary-cards.dc.html` | 1440 x 900 | dark | Session summary card insert, before and after, and the Results card expanded |
| `ws-delegation.dc.html` | 1440 x 900 | dark | Delegation tree: a coordinator agent with child agents (parallel, ordered, waiting, done, unreachable) |
| `ws-workpanel-empty.dc.html` | 1440 x 900 | dark | Work panel empty state with the four New actions; Project sidebar on Sessions |
| `ws-browser.dc.html` | 1440 x 900 | light | Browser tab with a dev server, and Grab and annotate with its confirm sheet |
| `ws-palette.dc.html` | 1440 x 900 | light | Command palette over a session |
| `ws-create-workspace.dc.html` | 1440 x 900 | dark | New workspace composer for a folder without Git, with the No Git popover in the Sidebar |
| `ws-tasks.dc.html` | 1440 x 900 | light | Tasks, full page |
| `ws-automations.dc.html` | 1440 x 900 | dark | Automations, full page: list, detail, runs |
| `ws-prototypes.dc.html` | 1440 x 900 | light | Prototypes, full page: directions, viewer, element inspector |
| `ws-narrow.dc.html` | 1440 x 900 | dark | Narrow window collapse: four windows at 1300, 1100, 880 and 880 px (overlay), drawn at 52 percent |
| `ws-light.dc.html` | 1440 x 900 | light | Light theme: editor tab over a split terminal pane, Project sidebar on Checks |

Light theme appears in five artboards (browser, palette, Tasks, Prototypes, editor and Checks), so every area has at least one light artboard: 5 (Tasks), 6 (Prototypes), 8, 9 and 10 (`ws-light`), 11 (browser), 12 (palette).

---

## 5. Workspaces and sidebar (Workspaces mode)

### D-WS-01 Sidebar (workspace tree)

- **Purpose.** The tree of projects, workspaces, child sessions and hosts in Workspaces mode. Bot Teams mode shows teams instead (D-WS-29, another pass).
- **Layout.** On the Frame, width 256 (min 200, max 400), to the right of the rail, no border or seam. Top to bottom, with `space.2` side padding: "+ Create" primary button (28 px, `accent.default`, opens the start page); Search field (28 px, `surface.sunken`, `border.strong`); four navigation rows (28 px): Prototypes with a count badge and a chevron, Tasks, Automations, MassOrca Mobile; the "Projects" header (`overline`, 24 px) with four 24 px icon buttons: notifications (bell), filter and sort, add folder, new project; then the tree; footer (36 px): help, locate (selects the open session's row), panel toggle. Rows are 28 px (24 compact, 32 comfortable).
- **Tree rows.** Project: folder icon, name (`body-strong`), owner or org (`body-sm`, `text.tertiary`), a 12 px fork icon for a repository that is a fork. Host: one row per host the project lives on, under the project (server icon and host name, indent 16; A10). Workspace: under its host row, one line, indent 32, a `git-branch` icon (16 px) for a worktree or a `folder` icon for a plain folder, then the name (`body`); the open one has `fill.selected.frame` and a 2 px `accent.fg` bar. Child sessions nest under their parent by indentation only (no connecting lines, no "Child" badge): status glyph, agent icon and title, with the indent placing the child's agent icon in line with the first letter of the parent's name (owner, A83 to A85, applied to the desktop Sidebar). After the title a child names where it runs (`body-sm`): "Current worktree", or the branch of its own worktree with a 12 px `git-branch` icon before it. A child in its own worktree is listed once, under its parent, and that worktree's own workspace row stays listed (WS-35). The CLI equivalent is `massorca agent tree`. A project whose folder has no Git shows the "No Git" pill (warning tint, glyph and word) at the row's trailing edge. The "Unknown" group holds projects with no repository.
- **Built (desktop): folding (WS-33).** A workspace lists its first five top-level sessions, each with its children. A session that is working, waiting for you, failed or unread stays listed past five, and so does the open session. Unread means the agent's latest waiting or done report has not been marked seen, and only a service that offers seen state (`agent-seen/1`) has unread sessions; an unread row shows an accent dot at its trailing edge and its title in `body-strong`. Opening a session, from the command palette or by clicking its Sidebar row, tells the service it was seen (`agent.mark_seen`); the dot goes when the service's change arrives, and a refused call leaves the session unread. Monitoring has no state on the desktop yet. A top session with one of these below it stays listed too. The rest fold behind "Show N more", which counts the folded top-level sessions and not their children; its trailing status glyph shows the worst state among the hidden sessions and their children: Error (red) first, then Waiting for you (orange). Idle, Done and Unreachable show nothing, since Unreachable is a lost link, not a failure. No hidden session can be waiting or failed today, because those stay listed, so the glyph does not show on the desktop yet. "Show less" folds them back. The unfolded state lasts for the app run and is not saved. To list the same sessions from a terminal, use `massorca agent list` and `massorca agent tree <SESSION>`; to mark one seen, use `massorca agent seen <SESSION>`.
- **Content and copy.** "Create", "Search", "Prototypes", "Tasks", "Automations", "MassOrca Mobile", "Projects", "Unknown". Tooltips: "Notifications", "Filter and sort", "Add folder", "New project", "Help", "Locate current session", "Hide sidebar". Empty: "No projects yet" in `text.tertiary`, with no button. No filter match: "No workspaces match" with "Clear filters".
- **States.** Empty (no projects); loading (3 `fill.skeleton` rows of project height, shimmer replaced by a static fill with reduced motion); collapsed project (chevron right); filtered (the filter icon shows a dot); selected; sleeping workspace (row dimmed, hint "Sleeping", wakes on open; this absorbs the hibernation gate D-AG-10); host unreachable (the host's rows are dimmed, glyph `agent.unreachable`, hint "Last known state, 5 min ago"; no status is claimed); external worktree found (an inline row under the project: "Worktree created outside MassOrca" with "Add" and "Dismiss"; absorbs D-WS-22).
- **Interactions.** Click opens the workspace in the Conversation; click on a project toggles it. Arrow keys move, Right and Left expand and collapse, Enter opens, F2 renames, Delete asks to remove (through X-DLG-01). A project group is renamed inline; deleting one confirms. The sidebar toggle in the Title bar hides only the Sidebar; the rail stays. Width is set by dragging the right edge; double-click resets to 256. Hidden below 928 px (design system 5.1) and opened as an overlay.
- **Desktop and Android.** Desktop only. Android: M-WS-01 workspace list, a separate pass.
- **Artboard.** `ws-session`, `ws-start`, `ws-create-workspace` (selected row with No Git).

### D-WS-02 Workspace row and menu (absorbs D-WS-03)

- **Purpose.** One workspace row and its actions.
- **Layout.** The row of D-WS-01. On hover or focus two 24 px buttons appear at the trailing edge: new session (plus) and more (ellipsis). Right-click opens the same Menu (`surface.overlay`, `radius.lg`, `shadow.e2`).
- **Menu items.** "New session" (built: opens the workspace's start page with no session open), "Open in" (submenu: file manager, terminal, editor, other tools the host reports), "Rename", "Edit workspace" (D-WS-04), "Open in Source control", "Developer" (submenu: copy path, copy branch name, reveal in Work panel), a divider, "Delete workspace" (danger text, opens D-WS-16). Items that need Git are disabled for a No Git workspace, with the reason in the tooltip.
- **Hover card.** After 400 ms a Popover shows Environment: host, user, repo, branch, last status and its age. It never opens for keyboard focus; the tooltip does.
- **States.** Idle, working (running glyph), waiting for you (row tinted `agent.waiting.bg`), error (`agent.error.bg`), unread (a 6 px `accent.fg` dot and `body-strong`), selected, sleeping, unreachable (dimmed).
- **Desktop and Android.** Android: M-WS-02 long-press sheet.
- **Artboard.** `ws-session` (rows with running, idle and waiting glyphs and child rows).

### D-WS-04 Workspace edit and D-WS-06 Status appearance

- **Purpose.** Rename a workspace and choose its icon and label colour.
- **Layout.** A Dialog (width 440): Name (TextField), Icon (a grid of 24 Lucide glyphs plus an emoji field), Label colour (six swatches). Buttons "Cancel" and "Save".
- **Decision.** The label colour comes from the six `usage.ring.*` slot colours, never from `status.*` or `agent.*`, so a user's own tag cannot look like an agent state or a failure. The icon replaces the AgentStatus glyph only when the workspace is idle.
- **States.** Idle; saving; name already used ("A workspace named oppm already exists."). Absorbs emoji suggestions and icon pickers (D-OT-10).

### D-WS-09 Add project

- **Purpose.** One entry for adding a project from any source.
- **Layout.** A Dialog (width 560, two steps). Step 1 "Add a project": a list of sources as ListRow items with icon and one-line description: "Open a folder", "Clone a repository", "Create a new project", "Import nested projects", "Connect a remote host", "Start from a server", "Add from GitHub". Step 2 depends on the source: folder or URL field with a host picker; clone shows a progress bar and the log; nested import shows a checklist with the scan limit notice.
- **Non-Git folder (absorbs D-WS-11).** Step 2 shows a Banner (warning): "This folder is not a Git repository. You can add it as a plain folder, or initialize Git." with "Initialize Git" and "Add as plain folder". The project then shows the No Git pill. After adding, a toast says what happened (X-NT-01); there is no "Project added" dialog (D-WS-12 is dropped).
- **Nested scan (absorbs D-WS-13).** Inline notice in step 2: "Stopped after 500 folders. Narrow the folder to scan the rest." with "Narrow folder".
- **States.** Source choice; per-source step; progress; failed ("Could not clone. Nothing was added." with "Retry"); host unreachable (the source is disabled: "VPS can't be reached").
- **Keyboard.** Up and Down choose a source, Enter continues, Escape closes.

### D-WS-14 New workspace composer

- **Purpose.** The one place that creates a workspace: location, type, base ref, parent, dropped files and the setup script offer.
- **Layout.** A page in the Canvas (the Sidebar stays; no region chrome). A 640 px column, top padding 56: heading "New workspace" (`heading`) with "in perulaunch-web, on This PC" (`text.secondary`); a Banner when the project needs attention; a Host field first (host icon, name, connection dot; it lists only the hosts where the project lives, with the help line "perulaunch-web lives only on This PC."; owner, A89); a Type SegmentedControl ("Git worktree", "Plain folder"); Location (the path field; the host is chosen above); a two-column row "Base ref" and "Parent workspace" (Selects, X-PK-01); Name; the setup script offer as a card with a checkbox; footer buttons "Cancel" (ghost) and "Create workspace" (primary). Files dropped on the page are listed as chips above the footer.
- **Project icon (owner, 2026-11-12).** A 40 px project icon button sits left of the heading (and in Project settings); the same icon replaces the folder glyph in the Sidebar project row, tabs and pickers. Clicking it opens a 296 px "Project icon" popover: a colour row (usage colours 1 to 6 and neutral, never status colours), an icon grid (folder by default, a short Lucide set, "More" opens the full icon search), "Upload image…" (PNG or SVG, up to 1 MB, cropped square), "Use GitHub avatar" (the repository owner's avatar, shown only when a GitHub account is connected), and "Reset to folder". Purpose: tell apart projects that come from the same repository. The choice is stored per project and syncs across hosts; images are drawn in a rounded square, glyphs in the chosen colour.
- **No Git state (artboard).** The project is a plain folder. A warning Banner (glyph, word and tint): "This folder is not a Git repository" with the line "Worktrees, branches and Source control need Git. You can work in the folder as it is, or initialize Git first." and an "Initialize Git" button. The Type control selects "Plain folder"; "Git worktree" is disabled with the word "Needs Git"; Base ref and Parent workspace are disabled with the hint "Needs Git". In the Sidebar the project row shows the No Git pill, and its popover repeats the explanation with "Initialize Git" and "Dismiss". After Initialize Git succeeds the Banner and the pill go, and Type returns to "Git worktree".
- **Other states.** Idle; validating (the field shows a spinner after 200 ms); base ref missing ("main was not found. Use origin/main?" with the default offered); creating (the button reads "Creating", disabled, then D-WS-15 opens in the Work panel); setup offered; remote host unreachable (Banner "VPS can't be reached", creation disabled); name already used.
- **Interactions.** Enter in a field moves on; Mod+Enter creates. Escape returns to the previous page.
- **Desktop and Android.** Android: M-WS-04 button and form sheet, same fields.
- **Artboard.** `ws-create-workspace`.

### D-WS-15 Workspace creation progress

- **Purpose.** Shows progress while a workspace is created, in a Work panel tab, in place of a terminal.
- **Layout.** A tab "Creating oppm / fix-period" with a step list (Progress bar and rows with status glyphs: "Create worktree", "Copy local files", "Run hooks", "Run setup script") and a log well below (`terminal.bg`, `mono`).
- **States.** Pending, done (tab turns into a normal terminal or closes, by setting), failed (the log stays; actions "Retry" and "Remove partial workspace", which confirms through X-DLG-01).

### D-WS-16 Delete workspace

- **Purpose.** Confirms deleting a workspace, with the warnings that matter.
- **Layout.** Destructive Dialog (X-DLG-01), width 480. Title "Delete workspace fix-run-period?". Body lists, each with a count and a glyph: unsaved changes ("3 files with unsaved changes"), unpushed work ("2 commits are not pushed"), nested children ("1 child workspace and 2 child sessions will be removed"). A checkbox "Keep the branch". Buttons "Cancel" and "Delete workspace (3 files, 2 commits)" (danger fill). Clean workspace: no list, one line "This removes the worktree folder from This PC."
- **States.** Clean; unsaved; unpushed; nested children; host unreachable ("Can't reach VPS. The workspace cannot be deleted until it is back.", button disabled). The result is never announced before it is true.

### D-WS-18 Workspace cleanup (absorbs D-WS-17 and D-WS-19)

- **Purpose.** Bulk cleanup of finished workspaces, with disk use and the branches that were kept after earlier deletes.
- **Layout.** A Dialog (width 760, height up to 80 percent). Header with the total ("14 workspaces, 6.2 GB") and a SegmentedControl "Workspaces | Kept branches". Table (columns: checkbox, Workspace, Last activity, Disk, State; numbers right aligned, tabular). State words: "Merged", "Unpushed work", "Has agent running" (the row is disabled with the reason). Footer: "Delete selected (3, 1.8 GB)" (danger) and "Close".
- **Kept branches tab.** Rows with branch name, workspace it came from, date, and actions "Review" (opens the branch in Source control) and "Delete branch".
- **States.** Loading (skeleton rows and "Measuring disk use"); blocked items; empty ("Nothing to clean up."); error ("Could not measure disk use on VPS.").
- **Entry.** The filter and sort menu in the Projects header: "Clean up workspaces".

### D-WS-20 Repo hooks trust

- **Purpose.** An explicit trust step before running a repository's hook scripts.
- **Layout.** Dialog (X-DLG-01 error and decision variant), width 520: "Run hook scripts from this repository?" and the list of scripts in `mono` wells (event, command). Text: "These commands run on your computer with your access." Buttons "Don't run" and "Trust and run". A checkbox "Ask again when the hooks change" is on and cannot be turned off for hooks that run commands outside the repository.
- **States.** Untrusted; trusted (no dialog); changed ("The hook scripts changed since you trusted them" with a diff of the commands).

### D-WS-24 Tasks page

- **Purpose.** Issues and PRs from connected task providers, and the Bot Teams tasks from team Boards, in one list. Opened from the rail (Tasks) or from the Sidebar entry.
- **Layout.** A full page: the Sidebar and the regions give way, the rail keeps the Tasks item active, the Title bar shows "Back" and the title. Content column maximum 1120 (`layout.page.default`), padding `space.6`. Order: heading "Tasks"; a New task card (raised: text area, then a row with a Workspaces or Bot Team SegmentedControl, a project or team Select, a "Start now" or "Later" Select, the Model picker chip and a primary "Create task"); "Needs your approval" with a count and one tinted card per request (`agent.waiting.bg`, waiting glyph, title, origin badge "Workspaces" or "Bot Team", one-line detail, "Open" and "Review"); "Open tasks" with a search field, a provider SegmentedControl ("All sources", GitHub, Linear, Jira, "Board: Desarrollo") and a Table (Title, Reference, Labels, Assignee, Updated, Source).
- **Task composer (owner, 2026-11-12).** Centred, 760 px wide. From top to bottom: a "Workspaces | Bot Teams" switch (SegmentedControl, centred); a placement bar attached to the top of the Composer (same border, rounded top corners, inset 12 px) with borderless buttons for host ("This PC"), project and worktree ("New worktree"); then the Composer: text, and a bottom row with the access chip ("Ask for approval"), the "Chat | Terminal" switch, "Start now" (or "Later"), the Model picker chip with the agent icon, and the send button. In Bot Teams the placement bar shows the team and project instead.
- **Content and copy.** Composer placeholder: "Describe the task. Start it now, or keep it for later." Buttons: "Create task" and one "Review" per approval row (owner, 2026-11-12). "Review" opens the task's session in the Floating workspace (D-TM-19), over the Tasks page: header with the agent icon, task title, "project / workspace", "Go to session" (opens the session where the task runs), minimise and close; body with the last agent message and the approval card (command, access level, "Deny" and "Approve"). The user can keep working on Tasks while it is open. Clicking a row outside its button does the same as "Review". Rows use real references (`oppm #313`, `NEX-212`).
- **States.** Per provider: loading (skeleton rows); not connected (the provider row in the filter shows "Connect", and an EmptyState "Connect GitHub to see your issues and pull requests" with "Connect GitHub"); error ("Could not load Linear. Showing the last list from 5 min ago." with "Retry"); empty ("No open tasks"); nothing needs approval (the section is not shown).
- **Interactions.** Click a row opens D-WS-25. Mod+Enter in the composer creates the task. The Model picker works as in a session.
- **Desktop and Android.** Android: M-TK-01.
- **Artboard.** `ws-tasks` (light).

### D-WS-25 Task item detail

- **Purpose.** Issue or PR detail, and the way to create a workspace from it.
- **Layout.** A 480 px panel on the right of the Tasks page (the list narrows; below 1000 px the panel replaces the list with a Back button). Header: reference, title, status badge; a provider section (labels, assignee, milestone, linked PR); body (Markdown); a Reviewers picker and a Duplicates list when the provider supports them; footer: "Create workspace" (primary, opens D-WS-14 with the base ref and name filled), "Open in browser".
- **States.** Loading; saving; error ("Could not save. The task is unchanged." with "Retry").

### X-WS-01 Workspace filter (desktop popover)

- **Purpose.** Chooses which workspaces and projects the Sidebar shows. Absorbs the visibility dialog (D-WS-28) and its help.
- **Layout.** A Popover (240 wide) from the filter button: "Show" (All, Active, Archived), "Status" (checkboxes: Waiting for you, Running, Idle, Error), "Hosts" (a checkbox per host with its connection dot), "Sort" (SegmentedControl Name or Recent). Footer: "Reset".
- **States.** All; filtered (the Sidebar header icon shows a dot, and a one-line "Filtered, 4 of 12 shown" row sits under the header); no match.

### Section 5 rows that are not designed as screens

| ID | Disposition |
|---|---|
| D-WS-03 Open-in and developer submenus | Merged into D-WS-02 (submenus of its Menu). |
| D-WS-05 Project groups | Merged into D-WS-01: inline rename; delete through X-DLG-01. |
| D-WS-07 Sidebar header menu | Merged into D-WS-01: the Projects header actions and X-WS-01. |
| D-WS-08 Workspace board | Extension (owner accepted): shown through the extension view slot D-SL-04, not designed. |
| D-WS-10 Add-project source steps | Merged into D-WS-09. |
| D-WS-11 Non-git folder and add-from-folder confirms | Merged into D-WS-09 (non-Git state) and D-WS-14 (No Git state). |
| D-WS-12 Project added confirmation | Dropped: a toast says it (X-NT-01). |
| D-WS-13 Create-from picker and nested scan notice | Merged into D-WS-09 (inline notice). |
| D-WS-17 Preserved-branch review | Merged into D-WS-18 (Kept branches tab). |
| D-WS-19 Workspace space page | Merged into D-WS-18. |
| D-WS-21 Local git toolchain banner | Merged into D-DF-07 (Settings, Git): the banner is a Banner (warning) in the Source control tab with "Open Git settings". |
| D-WS-22 External workspace inbox | Merged into D-WS-01 (inline row). |
| D-WS-23 Sidebar feedback dialog | Merged into D-ST-01 (Support section), another pass. |
| D-WS-26 GitHub Projects views | Extension (owner accepted). Not designed. |
| D-WS-27 Sparse directory picker | Dropped (niche; extension candidate). |
| D-WS-28 Workspace visibility dialog | Merged into X-WS-01. |
| D-WS-29, D-WS-30, D-WS-31 | Bot Teams, not in this pass. |

---

## 6. Agents and sessions

### D-AG-01 Start page

- **Purpose.** Starts an agent session with or without a workspace. It replaces the landing page and the create-chat page (D-AG-02). Opened by "+ Create" and by the mode switch from Workspaces mode's home.
- **Layout.** A page in the Canvas with the Sidebar visible. A 720 px column, centred horizontally: the rose logo (`rose_logo`) centred above a centred heading, "What should we work on?" with no project, or "What should we work on in <project>?" once a project is chosen, with the project name dashed-underlined and clickable to change it (owner, 2026-11-12). Under the heading, the Composer, start variant, with the placement card joined to its top edge: the card holds the host, project and branch or new worktree pills (see the placement note below), inset 12 px from the Composer's sides. When joined, the Composer spans the column without side gutters, so the card's inset lines up with its edges, and it keeps its 12 px corners. Its bottom row holds `+`, the access chip, the Chat | Terminal switch, the model chip, the microphone and a round Send. Under the Composer, left-aligned, a row of key hints (Enter, Shift+Enter, @, /). The text area starts at the Composer's standard minimum height, 44 px; the 66 px start minimum (three lines) is not built.
- **Placement buttons.** Host, project and branch or new worktree are borderless buttons, 28 px high, `radius.md`, no fill at rest; on hover the fill becomes `fill.hover.canvas` and the label `text.primary`; open shows `fill.selected.canvas`; focus shows the focus ring. No border in any state (owner, 2026-11-12). The project button opens a menu of projects on the chosen host, ending with "Add a project…", which opens the Add project dialog. The menu always offers "No project" (owner, 2026-11-21): a folder icon struck through and the label, as a plain 28 px menu row with no path caption (owner, 2026-11-21); the folder where conversations without a project are kept is set in Settings, General and account, and in Setup. A "No project" session runs in that folder, with no worktree or branch, so the worktree button is disabled. With no projects yet the button still reads "Choose a project"; its menu says "No projects on This PC yet", then "No project", then "Add a project…". The start page has no separate empty-state block (owner, 2026-11-12).
- **Model picker open.** The Model picker (X-AG-02) opens from the chip "Opus 5.5 Medium". On the start page it flips below the Composer, because the Composer sits mid-screen (ModelPicker spec). In a session it opens above, as in the owner's reference.
- **States.** Empty; workspace chosen (the placement row shows the project's branch); starting (Send becomes disabled and a status line "Starting Claude on This PC"); agent signed out (Banner above the Composer: "Sign in to Claude to continue", button "Sign in", X-AC-01); no projects yet (the project button reads "Choose a project" and its menu offers only "Add a project…"; no separate empty-state block); host unreachable (the host chip shows the unreachable glyph and "Can't reach VPS", Send disabled).
- **Interactions.** Enter sends, Shift+Enter adds a line, `@` opens the mention menu (X-CH-09), `/` commands. The Chat or Terminal switch decides whether the agent opens as a Conversation or as a terminal in the Work panel (pv96). Escape closes the picker first, then does nothing.
- **Built (desktop).** The Model picker's agent rail lists only the agents that are installed and turned on: the kinds `agent.kinds` reports as enabled and detected here, with no problems. A kind that is off or not installed does not appear. The service declares no models yet, so each agent's only model is the kind itself, and the first agent is chosen. The chosen agent stays across a refresh of the kinds, and `agent.start` sends no model or effort until PR #288 adds the fields (a marker comment marks the spot). With no agent installed the rail is empty and Send stays off with "No agents installed."; the ui empty state with its Marketplace link is not wired yet. Send starts a session of the chosen kind in this workspace, with the typed text as its first prompt. Send stays off with its reason while no kind is chosen ("Choose an agent to start a session."), while the service is not connected ("The service is not connected."), and while a start is in flight ("Starting Claude on This PC"). A refusal keeps the prompt in the Composer and shows the service's sentence above it; a data folder that is not secured also shows the "Secure the data folder" banner.
- **Built (desktop): permission mode.** When the chosen kind declares permission modes, the access chip opens one menu with a row per mode: the kind's declared default first, tagged "Default" and named after its mode, then the other selectable modes in the kind's order (for Claude: Auto (Default), Manual, Accept edits, Plan, Bypass permissions). Don't ask is never offered, though a session that runs it still shows it. Each row shows the kind's declared description under its name and its own icon; a bypass mode takes the warning icon in the warning colour. There is no separate "The CLI's default" row (owner, 2026-11-25): the Default row is the declared default itself, and it sends no mode, so the vendor CLI starts as it would on its own (decision 0083 D3). Any other row sends its flag. The chip shows the picked row's name, and its tooltip shows the description. Accept edits takes that name from the owner's table, whatever the kind calls it. A picked mode is sent with `agent.start` and stays while the kind stays chosen. The artboard and the approved screenshot show "Workspace write", which the service does not declare; showing a concrete mode there would make the start send it, which is a decision for the owner. Bypass modes are offered but never selected by default, and the chip shows a warning glyph in place of the shield while one is selected. A kind that declares no modes shows no chip. The CLI equivalent is `massorca agent start <KIND> --mode <MODE>`.
- **Built (desktop): model and effort.** The start sends the picked model and effort only when the service offers `agent-options/1` and the row is a real model; the effort labels map to `low`, `medium`, `high`, `xhigh` and `max`, and no kind declares models yet, so today neither is sent.
- **Built (desktop): Chat / Terminal.** The switch in the composer's bottom row starts on Chat, and the choice is kept per workspace. In Chat the started session opens in the Canvas. In Terminal the started session also opens in the Canvas, with its terminal in the session view, and the keyboard moves into that terminal, so the start composer never keeps it after Send (owner ruling 2026-11-25, from the owner's 2026-11-24 testing: the session they started must be what they see and type into). Opening the agent's terminal in the Work panel is an explicit action, not the default. A note under the composer says that some features are limited there. The CLI equivalent is `massorca agent start` followed by `massorca agent attach <SESSION>`. Chat is on while the service offers `agent-chat/1` and the picked agent can chat. Until the service offers it, every start page opens on Terminal and Chat says "Chat sessions need a newer service."; when the picked agent cannot chat, Chat is off with that agent's reason (for example "Claude was not found on this computer.") and the page opens on Terminal.
- **Built (desktop): + Create.** "+ Create" opens this page with no workspace: the question, the placement card (host, project and branch pills, see the placement note below) and the start Composer with its agent pick, access chip and Chat / Terminal switch. The project pill reads the chosen project, the first one until another is picked (or "No project" once it is chosen), and its menu lists the projects, "No project" and "Add a project�"; a project added there becomes the page's project. "No project" is off, with "Needs a newer service.", until the service offers `agent-no-project/1`; then it is chosen like a project, and Send starts the agent with no workspace through `agent.start` alone, in the conversations folder, with no `workspace.create`. With a project, Send creates a workspace in it through `workspace.create`, named from the prompt's first words ("Fix the login bug" becomes `fix-the-login-bug`; a prompt with no usable words lets the service generate the name), then starts the chosen kind in it through `agent.start` with the prompt and the picked mode. A refusal of either call keeps the prompt and shows the service's sentence above the Composer; a refused create starts nothing. With no project listed and no "No project" choice, Send stays off with "Add a project to start a session.". When the service refuses a start, a create or an added project because the data folder is not secured (`profile.unsafe_location`, a new data folder on Windows), both start pages show a warning banner, "Secure the data folder", that says the service refuses every call and names the command to run, `massorca profile secure --yes`; the desktop has no call that secures it. The banner goes once a call goes through. The New workspace form is no longer opened from "+ Create" (A88). The CLI equivalent is `massorca workspace create` followed by `massorca agent start`.
- **Built (desktop): placement card and branch menu.** The placement row is one raised card joined to the top of the Composer, with three pills. Host: "This PC", off with "Needs a newer service." because `workspace.create` takes no host yet. Project: the project menu, which reads "Choose a project" while there is none, and "No project" once that is chosen. Branch: "New worktree" with "from <base>", where the base is the one the menu names, else the project's base branch, else the default branch; with "Use an existing branch" on, the pill shows the branch to reuse, or "Existing branch" until one is named. With "No project" chosen, the branch pill is off, reading "No worktree" with "conversations folder" beside it, and Send ignores the branch menu. The branch pill opens a menu with the fields Send passes to `workspace.create`: Name (blank takes the prompt's first words, or the service's generated name when the prompt has none), Branch (a new branch's name, or the branch to reuse), "Use an existing branch", and Base. Name sits there because it names the worktree the pill creates. Reusing a branch needs its name: until one is named, Send keeps the prompt and shows "Name the branch to reuse." above the Composer. Left untouched, the menu sends the same call as before. The workspace start page uses the same card: its project pill opens the project menu, and its branch pill shows the workspace's branch, off with "Git operations come later." The title names the project as a dashed-underlined link that opens the project menu, and reads "What should we work on?" while there is none or "No project" is chosen. The pills are Tab stops with the focus ring. Enter or Space opens a pill's menu, and an off pill's reason also shows as a keyboard tip.
- **Built (desktop): differences from the artboard.** The card is inset 12 px on each side from the Composer, as the artboard and the approved screenshot draw it. Its top corners are 8 px, and its pills use 8 px side padding and a 4 px gap (the artboard: 10 px, 10 px and 6 px, which have no tokens, so they are kept as they are). The project name's underline is dashed, because GPUI has no dotted border style. The pills take a focus ring and hover on `fill.hover.canvas`, but their open state (`fill.selected.canvas`) is not drawn yet. The status bar shows "N running" with the running glyph, "N terminals" and "N hosts" with the green dot, as the artboard does. Memory ("1.22 GB" in the artboard, "Memory 1.22 GB" on desktop) and ports ("1 port") have no data source from the service, so those items stay hidden. The Chat | Terminal switch sits in the Composer's bottom row, as the artboard draws it, in the sunken segmented style. The key hint reads "@ mention files", the wording A33 gives the approved composer body; the artboard reads "@ mention". The rose logo stands where the artboard draws the MassOrca logo. The text area starts at 44 px, not the 66 px start minimum.
- **Desktop and Android.** Android: M-SE-01 new session sheet.
- **Artboard.** `ws-start`.

### D-AG-03 Conversation

- **Built (desktop), terminal arm.** Source: owner request 2026-11-24, "in terminal mode, why does the chat composer appear? shouldn't I type in the terminal itself?" A terminal-arm session shows its header, its live terminal and no Composer. The header shows the session's title (or its agent's name when it has none) with its status glyph, the kind, the arm ("Terminal"), the mode chip and the Delegate button; a live run keeps a Stop there, which stops it at once through `agent.stop` with no confirm. The mode chip is a select that shows the session's mode as the service reports it: "The CLI's default" for a session in the vendor's default, never the declared default, with a caption "Requested" when nothing confirmed the agent took a chosen mode, or "Next resume: Plan" for a switch waiting for the next resume. Picking another mode calls `agent.set_mode` (not forced). A refused attach shows a banner with Retry in place of the terminal. Every session is terminal-arm until `AgentSession` carries `run: terminal|chat`, which the desktop reads in one place (`is_terminal_arm`).

- **Ended (terminal arm).** The exit row of the terminal shows "Process finished" with no Restart: a plain Restart would start a plain shell, not the agent. One bar under the terminal gives the reason and the one action the state offers. A resumable kind offers "Resume" (`agent.resume`, which continues the saved conversation; the canvas then follows the new run's terminal). A kind that cannot resume offers only "Start a new session here" (`agent.start` in the same workspace with the same agent, without the saved conversation; the new session opens in this view). When the service refuses the resume with `agent.resume_not_found` or `agent.resume_unavailable`, or the resumed terminal prints "No conversation found", the bar says "The saved conversation could not be found." and offers only "Start a new session here". Any other refused resume keeps Resume and shows the service's sentence above the terminal. The `resume` field of `AgentSession` (available, code, reason) is not in the protocol yet, so the desktop reads the refusal codes only.

- **Built (desktop), chat arm (not drawn yet).** No session runs in the chat arm yet, so the Composer is built and kept but not drawn. The Composer under the terminal is the shared one. Send types the message into the session through `agent.prompt` (not forced): the draft stays read-only until the service accepts it, and a refusal keeps it above the Composer with the service's sentence and Retry. While the agent works or waits, the primary button is Stop, which stops the run at once through `agent.stop` with no confirm. When the service keeps a prompt queue (capability `agent-queue/1`, decision 0084), a draft sent while the agent works or waits queues through `agent.queue` instead: it stays read-only until the service accepts it and is then cleared, and a refusal keeps it with the service's sentence. The session's queue is drawn in the queued-message dock above the Composer (X-CH-03), from the store, which reads every session's queue on connect and again on each `agent.queue_changed` event. Each row shows why it waits (busy, asking, paused by a stop or a restart, behind a failed prompt) or why it failed, and offers Edit (`agent.queue.edit`), Remove (`agent.queue.remove`) and Send now or Retry (`agent.queue.send_now`); a paused queue offers Resume (`agent.queue.resume`). A refused action is kept above the Composer with the service's sentence, and the dock changes only when the service's queue does. Remove has no Undo yet. A service without the queue shows no dock and a busy agent can only be stopped, as before. The CLI equivalents are `massorca agent queue add`, `list`, `edit`, `remove`, `send-now` and `resume`. An ended session offers Resume beside its reason when its kind can resume (`agent.resume`), and the canvas follows the new run's terminal; otherwise one line says the session cannot be resumed. The model picker, attach button and voice input are not offered (the service declares no models; voice is disabled as on the start page). When the session's kind declares permission modes, the access chip shows the session's mode as the service reports it: "The CLI's default" for a session in the vendor's default, never the declared default, and a caption "Requested" when nothing confirmed the agent took a chosen mode. Picking another mode calls `agent.set_mode` (not forced). A live switch shows as requested, and a switch for the next resume leaves the running mode on the chip with the caption "Next resume: Plan". A refusal shows above the Composer with the service's sentence. The service carries a bypass mode into a run only when the resume names it, so Resume names a pending bypass mode; any other pending mode the service applies by itself. The CLI equivalent is `massorca agent mode <SESSION> <MODE>`. The transcript, Find and the message minimap come with the conversation work.

- **Purpose.** The transcript and composer of one session, with the message minimap, find, thread goal banner and file drop.
- **Layout.** Region fills the Canvas left of the Session summary, minimum 360. Message column and Composer have a 720 px maximum width, centred, `space.4` gutters. The minimap (24 px, on the right edge) shows only when the transcript is taller than 1.5 viewports and the Conversation is at least 480 px wide. In the artboard: user message right-aligned in a raised bubble; agent prose on the Canvas in `chat` text; a collapsed tool group ("4 steps") with status glyphs; a diff snippet card with "Open in Work panel"; the turn footer with the working status; the Composer with the approved session body (owner, A33: "Ask anything, @ to mention files, / for commands"; bottom row +, the "Workspace write" access chip, the Model picker chip with the agent icon, microphone and a round Send, here a round Stop because the agent is running; 12 px corners and `shadow.e1`). Where the Conversation is too narrow for both chips (the light editor artboard), the access chip shows only its shield icon, with "Access: Workspace write" as tooltip and accessible name, and the model chip drops the effort and a queued message docked above it (X-CH-03: "Queued", the text, "Send now", remove).
- **Thread goal banner.** When a goal is set, a Banner (info, 36 px) sits under the Title bar: target icon, "Goal: Ship the re-run warning", "Edit" and a dismiss button. It never shows when no goal is set.
- **Find.** Mod+F opens the FindBar at the top right of the Conversation: "Find in chat", "3 of 12" (D-FD-01).
- **States.** Loading (three skeleton turns); session gate (a card "Starting Claude on This PC" with the host and the agent, until the first event); empty ("Describe what you want done." and "Use @ to mention files and / for commands."); ready; error ("Could not load this conversation." with "Retry"); host unreachable (a Banner "VPS can't be reached. Showing the last known state from 5 min ago. Work that was running there may still be running."; the status line reads "Last known: running, 5 min ago"; Send queues messages, X-CH-03).
- **Interactions.** Drop files on the Conversation to attach them (the Composer shows "Drop to attach"). Hover a message for copy, fork from here, retry. The jump pill ("Latest", "3 new", "Waiting for you") appears when scrolled up.
- **Desktop and Android.** Android: M-CH-01.
- **Artboard.** `ws-session`, `ws-diff`, `ws-delegation`, `ws-workpanel-empty`, `ws-light`.

### D-AG-28 Conversation message minimap

- **Layout and behaviour.** As in the MessageMinimap spec: 2 px ticks (12 px user, 8 px agent, 16 px `agent.waiting` for "needs you", 16 px `agent.error` for errors), the viewport window as `fill.selected.canvas` with a 2 px `accent.fg` bar, click or drag to jump, hover card with the first line, author and time.
- **Artboard.** `ws-session` (about 90 ticks; one orange needs-you tick and one red error tick are visible).

### D-AG-05 Continue or fork a session

- **Purpose.** One dialog for continuing an earlier session and for forking it into a new pane.
- **Layout.** Dialog, width 480. Title "Continue this session?". Two Radio rows: "Continue here" ("Keeps the same worktree and history.") and "Fork into a new pane" ("Starts a copy from this point. The original stays as it is."). Below: the Model picker chip (the fork may use another agent), and the message text area (optional). Buttons "Cancel" and "Continue" or "Fork" (primary).
- **States.** Choosing; running ("Starting"); failed ("Could not fork. The original session is unchanged.").

### D-AG-06 Resume after restart

- **Purpose.** Lists the runs that survived a restart so the user can resume them.
- **Layout.** Dialog, width 560, shown once after a restart when at least one run exists. Title "Resume your sessions". Rows: AgentStatus glyph, session title, workspace, host, status text ("Still running", "Finished while closed", "Could not restart: <reason>"), a "Resume" button. Footer: "Resume all" (primary) and "Not now".
- **States.** Loading (skeleton rows); per run ok or failed; none (the dialog does not open).

### D-AG-07 Session history search

- **Purpose.** Finds text inside past sessions, per computer.
- **Layout.** In the Project sidebar, Sessions tab (D-SH-04): a search field "Search sessions", a SegmentedControl "This workspace | All", then the list. While the field has text the list shows results: session title, a one-line match with the matched characters on `match.bg`, host chip, age. Child sessions nest under their parent; automated runs are hidden unless "Show automated runs" is on.
- **States.** Indexing (a Banner "Indexing sessions on VPS, 40 percent" with the results so far); results; none ("No sessions match "xyz""); host unreachable ("Results from This PC only. VPS can't be reached.").
- **Artboard.** `ws-workpanel-empty` (Sessions tab, no query).

### D-AG-14 Automations page and D-AG-15 Automation editor

- **Purpose.** Scheduled or triggered runs in their Workspaces form: list, detail and run history in one page.
- **Layout (page).** Full page; the Sidebar is hidden and Back returns to the session. Page header (56 px): heading "Automations", a count badge, "New automation" (primary). Two panes: a 400 px list (search field, an "All hosts" borderless toolbar button, rows) and a detail pane. Row: name (`body-strong`), schedule in words and host, last run glyph and word with its time, next run. The paused row carries a "Paused" badge. Detail: name (`heading`) and description; actions "Run now", "Edit", "Pause", more, all borderless toolbar buttons that fill on hover (owner, A12); a card with Schedule, Next run, Host, Agent, Workspace, Access and the Prompt in a `surface.sunken` well; "Runs" with a count, a segmented "All | Failed" and a Table (Started, Status with glyph and word, Duration, Host, "Open output").
- **Editor (D-AG-15).** Dialog, width 640: Name, Prompt (text area), Schedule (Select: Once, Hourly, Daily, Weekdays, Weekly, Monthly, then time fields; an "Advanced" toggle reveals a cron field), Agent (Model picker chip), Workspace (existing workspace or "New worktree each run"), Host, Access. Buttons "Cancel" and "Save automation". Delete goes through X-DLG-01: "Delete automation Nightly dependency audit? Its 14 runs stay in the history." Absorbs the delete dialogs (D-AG-16).
- **States.** Empty (EmptyState "No automations yet", "Automations run a task on a schedule or when something happens.", "New automation"); skeleton; owner conflict (a Banner on the row: "Another account owns this automation. You can read it but not change it."); host filter; host unreachable (rows keep their last known state, dimmed, with "Last known state, 5 min ago"; "Run now" is disabled with the reason).
- **Interactions.** Up and Down move through the list, Enter opens the detail focus, Mod+Enter runs now. The same engine feeds the Calendar (another pass); opening a run from the Calendar lands here.
- **Desktop and Android.** Desktop only for the page. Not designed for Android.
- **Artboard.** `ws-automations`.

### D-AG-17 Prototypes page, D-AG-29 to D-AG-32

- **Purpose.** A core page for building and comparing UI prototypes with their own design system, directions and merge.
- **Layout (page).** Full page, Back in the Title bar. Left list (264 px): search, projects, each with its own "Design system" row (palette icon, version) followed by its prototype sets and counts. Centre: a toolbar (SegmentedControl "Directions | Design system | Versions", each segment with its icon: layout, palette, history; then "Note", "Play", "Send to agent", "Merge direction B" as borderless toolbar buttons that fill on hover, owner A12 and A21), the viewer on `surface.sunken` (the prototype itself is the user's design and is drawn in its own colours, not with MassOrca tokens), and a strip of direction thumbnails (A, B, C; "Main" badge on the one in the main set; the open one has a 2 px `accent.default` outline). Right aside (288 px): the element inspector.
- **Shared design system (owner, 2026-11-12).** A design system belongs to one project, and that project can share it with another project as a link, not a copy, so two projects of the same company keep one visual language. The linked project shows "Design system" with a link chip naming the source project ("NexusCare", `accent.fg`); the source shows a link count. Edits happen only in the source project and reach every linked project; a linked project can "Unlink" (keeps nothing, falls back to its own design system) or "Make a copy" (becomes its own, independent). Sharing is set from the design system menu: "Share with project…" lists the projects the user can open. Deleting the source asks first and names the linked projects.
- **Design system sections (owner, 2026-11-12).** The "Design system" row expands (chevron) into a second level of sections, as complete as the Claude design-system artifact: Overview, Colors, Typography, Spacing, Corner radius, Shadows, Motion, Components, Assets, each with its icon and a count where it has items (24 colours, 12 type styles, 66 components, 8 assets). "Add section…" at the end adds a custom section (name, icon, and a kind: tokens, components, assets or free page); custom sections can be renamed, reordered by drag and removed. Built-in sections can be hidden but not deleted. A linked project sees the same sections read-only. Selecting a section opens it in the centre pane under the "Design system" tab.
- **Prototype canvas (owner, 2026-11-12).** The centre is an infinite canvas, as in the Claude design canvas: pan with drag or scroll, zoom with Mod+scroll or the zoom control (bottom right: minus, percent, plus, "Fit"), a dot grid on `surface.sunken`. Every screen of the set and every direction is an artboard on the canvas with its name above it; arrows show the flow between screens; the selected artboard has the accent outline. "Play" (toolbar, or the play bar at the bottom centre) runs the prototype full size; the play bar steps between screens ("Patients, 1 / 4" with previous and next, also Left and Right arrow keys). "Note" drops a floating note (sticky style, author and time) that can be pinned to an element with a dashed leader line; notes go to the agent with "Send to agent". The fine detail panel (Element and Components: fill, border, radius, padding, value type) is hidden by default and appears only when the user opens the right panel with the toolbar button.
- **Element inspector (D-AG-31).** Tabs "Element | Components". The selected element's name and selector, rows of property and token chip (`surface.raised`, `border.default`, `radius.lg`, `space.3`), a draft note ("Draft: Value type display to heading. Edits go to Claude as one message when you send."), "Send draft" and "Discard". Draft edits are never applied until sent.
- **Design system view (D-AG-29).** The centre switches to tokens (grouped colour, type, spacing swatches and values), components and images, with a selection panel at the right; a version banner "Version 7, draft changes 2" with "Archive"; empty ("No design system yet", "Ask Claude to extract one from this project.").
- **Merge review (D-AG-30).** A page step: preview document of the main set with the direction applied, the merge plan as a checklist, progress, then review. States preview, merging, review, error ("Could not merge. The main set is unchanged.").
- **Versions (D-AG-32).** The centre shows a list of versions of the set or of the design system (date, author agent, a one-line note) and a "Restore" action that confirms through X-DLG-01 ("Restore version 5? Version 7 stays in the history.").
- **States.** List; viewer; unopenable (EmptyState "This prototype cannot be opened" with the reason and "Open file"); editing; loading.
- **Desktop and Android.** Android: M-PT-01 and M-PT-02.
- **Artboard.** `ws-prototypes` (light). Merge, versions and design system sub-views are described here and not drawn.

### D-AG-18 Prototype links in chat

Chat cards in the transcript (MessageBlocks card frame): "Prototype ready" with the set name, a thumbnail, and "Open"; a count card ("3 directions"); a proposal card ("Merge direction B into the main set?" with "Review merge"); a system card ("Design system updated, version 7"). "Open" goes to D-AG-17.

### D-AG-22 Agents overview

- **Purpose.** Running agents and their latest step, at a glance.
- **Decision.** No separate page. The overview is the Project sidebar Sessions tab, grouped "Needs you", "Running", "Recent", plus a Popover from the Status bar agent item ("Agents: 1 waiting for you, 2 running") with the top rows and "Open Sessions". The kanban view and the terminal popout are not carried over.
- **Layout.** Session rows are 40 px: AgentStatus glyph (16), the agent icon (16, name as tooltip), title, a second line with model and age. Child sessions nest under their parent by indentation only, with their agent icon in line with the first letter of the parent's title (owner, A84 and A85).
- **States.** Empty ("No sessions yet"); running; waiting (tint); done; host unreachable (dimmed with the age of the last known state).
- **Artboard.** `ws-workpanel-empty` (Sessions tab).

### D-AG-27 Session summary cards

- **Purpose.** The column of cards that summarise a session. Only cards that hold information are rendered; a card appears when its first record arrives.
- **Layout.** Region 288 px (240 to 400), `space.3` padding, cards stacked with `space.3`, scrolling as a whole. Card: `surface.raised`, `border.default`, `radius.lg`, `space.3` padding; header (28 px) with the title in `body-strong`, the value in `caption`, and a chevron. Fixed order: Environment, Plan, Checklist, Context, Results, Subagents, Connectors, Background tasks, QA servers, then extension cards. The column title "Session summary" is region chrome, not a card.
- **Cards in the artboards.** Environment ("VPS", host dot, user, repo, branch, pipeline), Plan ("3 of 7", current and next step), Checklist ("2 of 5", glyphs per item), Context ("35%", a 4 px Progress), Results ("1 reported" then "2 reported", the first item), Subagents ("1 running", one line per child with its glyph and word). Connectors, Background tasks and QA servers have no record in this session, so there is no card: no empty placeholder is ever drawn.
- **Insert.** Over 250 ms with `ease.enter`: height from 0 to content, opacity 0 to 1, up 8 px; cards below slide down. If the column is scrolled and the card lands above the view it appears without animation and the scroll position is anchored. Announcement: "Results card added". Reduced motion: instant. See the before and after panels in the artboard.
- **Results card, expanded.** Subtitle "URLs of the environment, file changes and visuals recorded in this chat." URL item: globe, the URL in `mono-sm` wrapping at any character, "Open" and "Copy URL" (24 px secondary buttons). When the URL is on a remote host "Open" is disabled and the note reads "This URL is on the remote host. Forward its port to open it on this PC." File item: the path as an underlined `accent.fg` link that opens a Work panel tab. Artifact item (X-AR-01): icon, name, kind, opens a Work panel tab.
- **Visibility.** The title bar toggle "Session summary" shows or hides the region. It is never hidden automatically because a file tab is open (decision 5). It is the first region to give way when the window narrows.
- **States.** Empty (the region shows only its title and the text "Nothing to summarise yet." in `text.tertiary`; the region itself can be hidden); one card; many; host unreachable (the Environment card shows "Last known state, 5 min ago", dimmed; other cards keep their last values).
- **Desktop and Android.** Desktop only. Android shows plan, background work and status through sheets and a pill.
- **Artboard.** `ws-session`, `ws-summary-cards`, `ws-delegation` (Child sessions card and a Subagents card: "Explore", "code-reviewer", each with its status and task, no agent icon; every session, child sessions included, shows its own subagents).

### X-AG-01 Agent status indicators

AgentStatus glyphs as in the design system 2.8, in this set of artboards: running (arc, 12 or 16 px), waiting for you (filled circle with a question mark; the row or card is tinted `agent.waiting.bg`), idle (hollow ring), error (triangle), unreachable (dashed ring with a slash; dimmed row; the age of the last known state in words), done (circle-check in `status.success.fg`), not started (idle ring). Where they appear: Sidebar rows (12 px), session rows (16), the tool call rows and turn footer (16), Delegation tree rows (16), the Status bar item (16), Subagents card lines (12). Every glyph has its word available as the accessible name.

### X-AG-02 Model picker

- **Purpose.** The one selector for agent, model, reasoning effort, speed and access in every composer.
- **Layout.** Trigger chip in the Composer's bottom row: agent mark, "Opus 5.5" (`label`, `text.primary`), "Medium" (`label`, `text.secondary`), chevron; height 24. Popover 560 wide, `radius.xl`, `shadow.e2`: agent rail 48 (Favourites star, a divider, one mark per agent; the active one has a 2 px `accent.fg` bar on the rail's right edge), model column 296 (search field with a 2 px `border.focus` underline when focused; rows of 32 px with provider mark, name, a hover star and a Ctrl+1 to Ctrl+4 badge; the current model has a left bar and `body-strong`; "Show more"), reasoning column 216 (header "Reasoning" and the model name; efforts Low, Medium, High, Extra High, Max with a check on the selected one). Footer 44 px: "Standard | Fast" only, as a borderless sunken switch; access is not in the picker but in the composer's own "Workspace write ▾" chip (agreement A27 read as C3, 2026-11-13).
- **Placement.** Opens above the trigger in a session (owner reference); flips below on the start page. Right edge aligned to the trigger.
- **States.** Closed; open; searching; no match ("No models match "xyz""); favourites empty ("No favourites yet", "Star a model to keep it here"); agent signed out (its rail mark is dimmed and the tooltip says why); loading models; error ("Could not load models from Claude"); host unreachable ("Last known models, 5 min ago"); model without Fast ("This model has no Fast mode", the segment disabled).
- **Interactions.** Mod+1 to Mod+4 choose the first four rows only while the picker is open. Up and Down move in the list, Tab moves between columns, Escape closes and returns focus to the trigger.
- **Desktop and Android.** Android: the same parts as a bottom sheet, not designed here.
- **Artboard.** `ws-start`. The marks in the artboards are neutral placeholders, not provider artwork.

### X-CH-02 Interactive cards, X-CH-03 Queued messages, X-CH-05 Plan view

- **Approval and question cards (X-CH-02).** In the transcript, `agent.waiting.bg` fill while pending, "Waiting for you" with the waiting glyph, title "Run command?", the command in a `surface.sunken` well, "Deny" and "Approve". Answered cards collapse to one line ("Approved" with a check, the title, the time). Question cards: radio rows, checkboxes or a text field; "Send answer" stays disabled until there is an answer. The row in the Sidebar and the minimap tick turn orange. Host unreachable: "Can't reach VPS. Your answer would not reach the agent." and the buttons are disabled.
- **Queued messages (X-CH-03).** A strip docked above the Composer: "Queued", the text, "Send now" and remove. "Send now" asks inline: "Send now? The agent will stop what it is doing and read this." (artboard `ws-session`).
- **Plan view (X-CH-05).** The Plan summary card expands in place (current and next step). "Open plan" opens the plan document as a Markdown tab in the Work panel. States: shown, scrolled, empty ("The agent has not shared a plan yet."), error ("Could not load the plan." with "Retry").

### X-CH-06 Delegation card and X-DL-01 Delegation tree

- **Purpose.** One engine for delegation. In Workspaces a coordinator coding agent starts child agents in sequence or in parallel; where each child runs follows the four rules below. The tree lives inside the parent session; there is no floating orchestration panel.
- **Layout.** A card in the transcript (`surface.raised`, `border.default`, `radius.lg`, `space.3` padding), and the card form inside the Subagents summary card. Header: users icon, "Delegation", the summary ("5 agents, 1 running, 1 waiting, 1 done"), a chevron that collapses the tree. Rows (at least 32 px, two lines): expander (12 px), AgentStatus glyph (16), agent icon (16; it replaces the agent name, which stays as tooltip and accessible label, owner A17), a role badge ("Coordinator") on the root, the model and effort in `caption`, the branch in `mono` `caption`, then on the right the status word in its state colour and the duration. Second line: the task given to the child, which is also why it was delegated. Group captions: "Run in parallel" and "Run in order" (ordered children carry a step number). Connectors are decorative. Indent 16 px per level. More than four levels show "+N deeper".
- **Row states.** Working; waiting for you (tint `agent.waiting.bg`, "Open to answer"); done (check, a chip "2 commits ready" and "Review", which opens the bring-back card X-CH-08); error (tint `agent.error.bg`, the reason on the second line, "Open"); unreachable (dimmed, "Can't reach VPS", "Last known: running, 5 min ago"); not started.
- **Where a child runs (owner, 2026-11-12).** Where a child runs (owner, 2026-11-12): (1) by default each child gets its own worktree; (2) "Use the current worktree" is allowed only when the child only reads (research, review, explain), when the workspace is a plain folder without git, or when the user picks it by hand on the delegation card; (3) two sessions never write in the same worktree at once: if a child writes in the current worktree, the parent waits until it finishes (sequential, not parallel); (4) parallel launches always give each child its own worktree, with no option.
- **Placement on the card.** Each child on the delegation card has a "Runs in" control with two choices: "Own worktree" (preselected, shows the new branch name) and "Current worktree". The choice is set by the rules, and the card says why when it cannot be changed:
  - Parallel launch: "Current worktree" is disabled with the reason "Children that run in parallel each get their own worktree."
  - Read-only child (research, review, explain): "Current worktree" is preselected, with the note "Only reads. No worktree needed."; the user can switch to "Own worktree".
  - Plain-folder workspace (no git): "Current folder" is the only choice, with the note "This workspace has no Git, so the child works in this folder."; "Own worktree" is disabled and offers "Initialize Git".
  - Manual pick: the user may choose "Current worktree" for a sequential child that writes; the card then warns "This session waits until <agent> finishes."
- **Parent waiting (rule 3).** While a child writes in the current worktree, the parent session shows a waiting strip above its Composer: the child's agent icon, "Waiting for Codex to finish in this worktree", the elapsed time and "Open child". The parent's Composer stays usable for messages, which queue until the child ends; the parent's own edits and tool runs pause. The child's row in the tree carries a "Current worktree" chip; children in their own worktree show the branch name.
- **Sidebar.** Child sessions nest under the parent workspace by indentation only: status glyph, agent icon, title; the child's agent icon lines up with the parent's name (owner, A83 to A85). The tree rows show the agent icon only, with the agent name as tooltip and accessible label (owner, A17).
- **Interactions.** Right and Left expand and collapse; Enter on a row opens that child's session. The depth limit is a setting (D-ST-02).
- **Desktop and Android.** Android shares the card (X-CH-06) and shows a flat list in a sheet.
- **Artboard.** `ws-delegation`.

### X-CH-07 Secret request card, X-CH-08 Bring back, X-CH-09 Mention menu, X-CH-11 Checklist

- **Secret request card (X-CH-07).** A transcript card per the SecretRequestCard spec: "Claude needs a secret", the variable name in `mono`, "Waiting for you", "Add secret" and "Decline", and the line "The value is stored on VPS and never goes into the chat." States: stored, linked, declined, "Not saved: <reason>". The value never appears in the card.
- **Bring back (X-CH-08).** From a child's "Review", a card in the parent transcript: the child's branch, the commit list (hash, message), the predicted result ("Merges cleanly into catalog-delta-feed" or "2 files will conflict"), and "Bring back". It runs a merge without fast-forward on the host; nothing is pushed. Blocked states each say why: "The child has uncommitted work. Commit or discard it first.", "catalog-delta-feed moved ahead. Update it first.", "Already brought back."
- **Mention menu (X-CH-09).** `@` in a Composer opens a Menu above it with groups Files, Sessions, Apps and Projects; the picked item becomes a chip with its icon. "No match for "xyz"" when nothing matches. Teams and bots appear only in Bot Teams.
- **Checklist (X-CH-11).** The Checklist summary card ("2 of 5", glyph per item, line-through for done). A transcript row when the agent changes it. Add and edit are inline in the expanded card. It shows only when there are items.
- **Artboard.** Checklist: `ws-session`. The others are described here and not drawn.

### X-AR-01 Artifacts

Artifacts are items of the Results card (item 7 in `pending-owner-review.md`): icon, name, kind. Opening one shows it in a Work panel tab (a Browser-style document preview). There is no page and no Sidebar entry. States: loading, error ("Could not open this artifact." with "Retry"), preview. Android keeps M-AR-01. Artboard: `ws-summary-cards`.

### Section 6 rows that are not designed as screens

| ID | Disposition |
|---|---|
| D-AG-02 Create-chat page | Merged into D-AG-01. |
| D-AG-04 Access mode picker | Merged into the Composer access chip "Workspace write ▾" (C3, 2026-11-13; earlier the X-AG-02 footer dropdown). |
| D-AG-08 Agent launch notice | Merged into D-TM-02 (pane state). |
| D-AG-09 Codex warnings and restart chip | Dropped (workarounds for the old Codex integration). |
| D-AG-10 Agent hibernation gate | Merged into D-WS-01 (sleeping state). |
| D-AG-11 Agent settings dialog | Merged into D-ST-02 (Settings, another pass). |
| D-AG-12 Activity page | Dropped (no stated purpose). |
| D-AG-16 Automation delete dialogs | Merged into X-DLG-01 (see D-AG-14). |
| D-AG-19 Usage cards in chat | Merged into D-AC-06 (usage home, another pass). |
| D-AG-20 Inline diff card | Merged into D-DF-01 (the diff snippet card in the transcript opens it). |
| D-AG-21 Visual panel and recorded file viewer | Merged into D-ED-09 (read-only viewers). |
| X-CH-10 Team request choice card | Bot Teams, not in this pass. |
| D-AG-13, D-AG-23, D-AG-24, D-AG-25, D-AG-26, D-AG-33, D-AG-34 | Bot Teams and Calendar rows, not in this pass. |

---

## 8. Terminal

### D-SH-17 Work panel and D-TM-01 Terminal tabs

- **Purpose.** The region between the Session summary and the Project sidebar: tabs for Terminal, Browser, Markdown or file, and Mobile emulator. Files and diffs open here as tabs.
- **Layout.** Default 480 px (min 320). Tab strip 36 px (`border.subtle` underline): tabs 32 px high with a 16 px icon, a label (`label`, truncated, maximum 200 px), and a close x (replaced by a 6 px `accent.fg` dot when the file is unsaved); the active tab has `fill.selected.canvas` and a 2 px `accent.default` underline; a plus button and a More menu follow. The body is the active tab's content. A pane tree can split the body: each pane carries its own tab strip and panes are separated by a 1 px `border.default` line with a 6 px hit area (see `ws-light`: an editor pane over a terminal pane).
- **Empty state (owner, 2026-11-12).** No tabs. Centred in the Work panel, a single column of four buttons, each as wide as its label, 10 px apart, with its icon on the left: "New terminal", "New browser tab", "New Markdown", "New mobile emulator" (secondary style, 32 px, `radius.lg`). No panel, title or explanation around them.
- **Tab behaviour.** Drag to reorder, move between panes or split by dropping on an edge (a drop overlay shows where the pane lands). Middle-click or Mod+W closes (Mod+W never closes a session or the window; with no tab open it does nothing). Tab context menu: "Close", "Close others", "Split right", "Split down", "Rename", "Pin".
- **Keyboard.** Mod+T new terminal tab, Mod+Shift+B new browser tab, Mod+W close the focused tab, Ctrl+Tab recent tab switcher (D-TM-11). F6 reaches the Work panel in the region cycle.
- **States.** Empty; one tab; split; dragging; error in a tab (the tab body shows its own ErrorState; other tabs stay usable); host unreachable (tabs of that host show the unreachable glyph).
- **Artboard.** `ws-workpanel-empty`, `ws-session`, `ws-light`.

### D-TM-02 Terminal pane

- **Purpose.** One terminal or agent TUI, with header, paste, file drop and context menu.
- **Layout.** Body filled with `terminal.bg`, padding `space.2`, text style `terminal` (13 / 16, JetBrains Mono). Optional header (28 px): shell icon, title ("zsh, oppm"), host chip when the terminal runs elsewhere, state glyph and word, a More button. Cursor is a filled block when focused and a hollow block when not. The artboards show a pytest run in ANSI colours (green passed, red failed, blue branch).
- **Overlays (drawn by the app).** Exit row at the bottom: "Process exited with code 1" with "Restart". Top banners: "Restored from earlier. This is history, not a running process.", "Reconnecting to VPS", "Can't reach VPS. Last output from 5 min ago. The process may still be running." Jump chip "Jump to bottom". Size badge "120 x 32" for 800 ms after a resize. Paste confirm for many lines: "Paste 12 lines?" with "Paste" and "Cancel".
- **States.** Starting ("Starting shell"); restoring; running; exited; reconnecting; unreachable (last output kept, no exit claimed).
- **Interactions.** Mod+Shift+C and Mod+Shift+V copy and paste on Windows and Linux (Cmd+C and Cmd+V on macOS). Mod+click on a path or URL opens it. File drop pastes the path. Mod+F opens the FindBar "Find in terminal" (absorbs D-TM-10).
- **Desktop and Android.** Android: M-TM-01.
- **Artboard.** `ws-session`, `ws-light`.

### D-TM-07 Clipboard write from terminal

A program that writes to the clipboard (OSC 52) shows a one-line Banner at the top of the pane: "A program in this terminal wrote to your clipboard." with "Allow once", "Always allow in this terminal" and "Block". The default is Block; the setting lives in Settings, Terminal. There are no silent writes.

### D-TM-08 Terminal context menu

A Menu: "Copy", "Paste", "Select all", "Find", "Clear", a divider, "Split right", "Split down", "Rename tab", and, for an agent TUI, "Open as Conversation".

### X-QC-01 Quick commands (desktop: D-TM-09)

A "Quick commands" submenu in the strip's plus menu lists saved commands (name, the command in `mono-sm`, scope badge "Project" or "Global"). Choosing one types and runs it in the active terminal (with a confirm for commands marked "Ask first"). "Manage quick commands" opens a Dialog (width 560) with a list and an editor (Name, Command, Scope, "Ask before running"). Empty: "No quick commands yet" with "New quick command".

### D-TM-11 Recent tab switcher

Holding Ctrl and pressing Tab opens a centred overlay (`surface.overlay`, `radius.xl`, `shadow.e3`, no scrim) listing the Work panel tabs in recent order, 28 px rows with icon and name, the next tab highlighted; releasing Ctrl opens it, Escape cancels. Shift+Tab goes the other way.

### D-TM-13 Floating terminal panel and D-TM-19 Floating panel tabs

- **Purpose.** A quick terminal and a small workspace that float above the Canvas.
- **Layout.** A floating frame (`surface.overlay`, `border.default`, `shadow.e2`, `radius.lg`), 480 by 320, remembered position, toggled from a title bar or palette command. Header 32 px: tabs ("Terminal", "Checklist", "New session", "Sessions"), maximise, minimise, close. Terminal tab: a TerminalSurface without header. Checklist tab: the open session's checklist, or "No checklist yet". New session tab: a host Select and the Composer (start variant). Sessions tab: the session picker (search and rows) to open an existing session.
- **States.** Hidden; shown; maximised (fills the Canvas); minimised (a 32 px pill at the bottom right); empty.

### D-TM-18 Driver overlay (terminal and browser)

While another device drives a pane, a strip at the top of the pane (`accent.subtle.canvas`, `border.default`): "Pixel 9 is controlling this pane" with a "Take over" button. Input from this PC is ignored until released. One overlay for terminal and browser panes.

### Section 8 rows that are not designed as screens

| ID | Disposition |
|---|---|
| D-TM-03 SSH reconnect overlay | Merged into X-CN-01 (shared connection Banner). |
| D-TM-04 Terminal close confirms | Merged into X-DLG-01 ("Process still running" text kept): "Stop the process or keep it running?" with "Keep running" and "Stop process". |
| D-TM-05 Layout save failure dialog | Merged into X-DLG-01 (error variant). |
| D-TM-06 Terminal error toast | Merged into X-NT-01. |
| D-TM-09 Quick commands editor and tab menu | Merged into X-QC-01 (see above). |
| D-TM-10 Terminal search bar | Merged into D-FD-01. |
| D-TM-12 Tab hover card | Dropped (the recent switcher and the tab name cover it). |
| D-TM-14 Floating orchestration dialog | Merged into X-DL-01 (the tree lives in the session). |
| D-TM-15 Pet overlay | Dropped. |
| D-TM-16 Native chat cover on terminal pane | Dropped (a pane holds one kind of tab). |
| D-TM-17 Legacy terminal surfaces | Dropped. |

---

## 9. Editor and files

### D-ED-01 Editor tab, D-ED-02 Code editor

- **Purpose.** A file as a Work panel tab.
- **Layout.** Tab: file icon, name, dirty dot. Under the strip a 28 px header: the path in `mono-sm` (directories `text.tertiary`, file `text.primary`), "Unsaved" in `status.warning.fg` with the word, and icon buttons Save, Find, Word wrap (and Preview for Markdown, D-ED-03). Body on `surface.sunken`: gutter 40 px (line numbers `text.secondary`, tabular, the current line's number in `text.primary`), code in `mono` 13 / 20, the current line on `fill.selected.canvas`, syntax colours from `syntax.*`. A gutter menu (breakpoints are not in v1) offers "Copy line link" and "Add review note".
- **States.** Empty (no file: EmptyState "No file open"); loading (skeleton lines); load error ("Could not open the file." with "Retry"); dirty; saved; read-only ("Read-only. This file is opened from a remote host that is not connected." or a recorded file); large file (over 2 MB: "This file is large. Editing is limited." with "Open anyway"); unreachable host (the last loaded text stays, read-only, with a Banner).
- **Interactions.** Mod+S saves, Mod+F finds, Mod+click on a path in chat or terminal opens it, Mod+Alt+click opens a recorded file in its own tab. Plain Mod+click keeps its dialog behaviour.
- **Desktop and Android.** Android: M-FI-02 read-only preview.
- **Artboard.** `ws-light`.

### D-ED-03 Markdown editor and preview

The same tab with a Source or Preview toggle in the header (two icon buttons, `aria-pressed`) and a table of contents button that opens a 200 px outline on the left of the preview. Mermaid diagrams render in the preview with a "View as source" button. Files over the size gate show "This document is large (3.2 MB). Preview is off." with "Preview anyway". The New Markdown action creates an untitled file; naming happens on save through the one-field Dialog (D-ED-05 merged).

### D-ED-06 External change banner and compare

A Banner under the tab header: "This file changed on disk." with "Reload", "Compare" (opens the read-only diff, D-DF-01 without Edit) and "Keep mine". While dirty, Reload asks first through X-DLG-01.

### D-ED-07 Merge conflict review

A tab per conflicted file: three panes (Current, Incoming, Result) and a hunk list at the left; each conflicted hunk has "Accept current", "Accept incoming", "Accept both" and the Result is editable. A progress line "2 of 5 conflicts resolved"; "Mark resolved" is disabled until none is left. States: conflicted; resolved ("Resolved. Stage the file in Source control.").

### D-ED-09 Read-only viewers

One viewer host with type-specific views: image (fit, 100 percent, zoom with Mod+scroll, a checkerboard in `surface.sunken`), audio and video (play, scrub, time), PDF (pages), paged CSV (Table with "Show next 1,000 rows"), recorded files, image diff (side by side with "Swipe" and "Difference"). States: loading, too large ("Showing the first 1,000 rows"), unsupported ("This file type has no viewer." with "Open externally"). PDF and media depend on the browser-engine decision.

### D-ED-11 File explorer (Project sidebar, Files tab)

- **Purpose.** The tree of workspace files with search.
- **Layout.** Project sidebar (288, min 240): the tab strip (36 px, icon-only, 28 px tabs, active one with `fill.selected.canvas` and a 2 px underline; Files, Sessions, Source control, Checks); the project name with three 24 px buttons (collapse all, refresh, more); the search field "Search files"; the "Names | Content" SegmentedControl; the tree. Rows 28 px, indent 12, chevron 12, folder or file icon 16, name, git letter at the right edge (M `status.warning.fg`, A `diff.add.fg`, D `diff.remove.fg`, ? `status.info.fg`). Ignored files are italic in `text.tertiary`.
- **Search.** Names filters the tree and highlights matches on `match.bg`. Content lists matching lines with their path in `mono-sm` (D-CP-03).
- **States.** Loading; empty ("This folder is empty."); filtered; no match ("No files match your search."); error ("Could not read the file list." with "Retry"); host unreachable (the last tree, dimmed, with "Last known state, 5 min ago").
- **Interactions.** Click opens a file as a Work panel tab (single click previews in an italic tab, double click keeps it); right-click: "Open", "Reveal in file manager", "Copy path", "Add to chat". Arrow keys, Enter, F2 rename.
- **Artboard.** `ws-session`.

### Section 9 rows that are not designed as screens

| ID | Disposition |
|---|---|
| D-ED-04 Rich markdown editor | Dropped (duplicates D-ED-03; reconsider after v1). |
| D-ED-05 New file dialogs | Merged into X-DLG-01 (one-field variant). |
| D-ED-08 Diff surfaces | Merged into D-DF-01. |
| D-ED-10 Notebook viewer and kernel setup | Extension (owner accepted). |
| D-ED-12 Notes send menus | Merged into X-RV-02. |
| D-ED-13 Team files | Bot Teams (reuses D-ED-11 and D-ED-01), not in this pass. |

---

## 10. Diff, review and source control

### D-DF-01 Diff view

- **Purpose.** The changes in one file, opened as a file tab in the Work panel with inline change markers.
- **Layout.** Toolbar 36 px: status letter badge (M, A, D, R), the path in `mono-sm`, "vs origin/main", counts `+3` in `diff.add.fg` and `-1` in `diff.remove.fg` (each with its sign), then icon buttons Source, Rendered (a two-state toggle, hidden for code), Split, Previous change, Next change, Edit, More. A review bar (36 px) appears when notes exist: "1 draft note" and "Send to agent". Body on `surface.sunken`: two number gutters (40 px each) then a 20 px marker column with `+` or `-`; added lines `diff.add.bg` with `diff.add.gutter`, removed lines `diff.remove.bg` with `diff.remove.gutter`, intra-line changes on `diff.add.word` and `diff.remove.word`; hunk header rows (24 px, `mono-sm`, `text.tertiary`); a "Show 42 unchanged lines" row (24 px). The change ruler (8 px) sits at the right edge with one mark per change and the current one outlined.
- **Content and copy.** "Source", "Rendered", "Split view", "Previous change", "Next change", "Edit", "More". "No changes in this file." "This diff is large (12,431 lines). Show it anyway?" with "Show diff" and "Open as source". "Binary file changed". "Widen the Work panel to use split view" (Split needs 640 px).
- **States.** Loading; empty; large (prompt); conflicts skipped (Banner "Conflicted hunks are skipped. Resolve them in the merge view."); disconnected ("Showing the last loaded diff. VPS can't be reached; changes made since may be missing."); read-only compare (no Edit, no notes).
- **Interactions.** The current app has change navigation keys, but app-flow.md does not list them; Alt+Down and Alt+Up are proposed and not checked against OS shortcuts. Hover a changed line for the comment button; click it to write a note (X-RV-02).
- **Desktop and Android.** Android: M-DF-01.
- **Artboard.** `ws-diff`.

### X-RV-02 Review notes

A note is an inline card under its line (`surface.raised`, `border.default`, `radius.md`): "You", "Draft" or "Sent", the text, remove. The gutter shows a message icon (12 px, `accent.fg`). The review bar counts drafts; "Send to agent" turns them into one message in the Conversation ("Review notes sent", expandable list of `file:line` and comment). When the agent is busy the message joins the queue (X-CH-03). States: draft, sent, none.

### D-DF-02 Source control (Project sidebar tab)

- **Purpose.** Changes, the branch, the commit box and the commits of the branch.
- **Commit box style (owner, 2026-11-12).** The Message field and the Commit split button take the Composer's softer edges: `radius.lg` (8 px), `border.default` instead of `border.strong`, `surface.raised` fill and `shadow.e1` on the field; focus swaps the border to `border.focus`.
- **Layout.** Project sidebar, third tab. Top block (`space.2` padding): PR chip ("PR #313", `accent.subtle.canvas`) with Search and More; the branch name in `mono-sm` with `+334 -15` counts; `origin/main` with a right arrow and ahead and behind counts (`status.warning.fg`, with arrows and numbers) and an external-link icon; the message box (72 px, `surface.sunken`, placeholder "Message", a wand button for AI assist); a split button "Commit" with a menu chevron beside it. Then "Commits in branch" (`overline`, count badge, "See all"); rows (28 px): file icon, name, directory in `text.tertiary`, `+n -n` counts and the status letter. The row of the open diff has the selected fill and bar.
- **States.** Clean ("Nothing to commit."); changes (file groups "Staged" and "Changes" when the commit box has focus); detached HEAD (a warning badge "Detached HEAD" before the Commit button, which reads "Commit anyway"); conflicts (Banner "2 files have conflicts" with "Open merge view"); no PR (the chip reads "Create PR"); git missing (Banner "Git is not installed" with "Open Git settings"); host unreachable.
- **Interactions.** Click a file opens its diff tab (D-DF-01). Mod+Enter in the message box commits. "Ask agent to commit" and "Ask agent to open a PR" are items of the Commit menu (D-DF-03 merged); their progress shows in the Conversation. Repo hooks ask for trust first (D-WS-20).
- **Artboard.** `ws-diff`.

### D-DF-04 Checks (Project sidebar tab)

- **Purpose.** CI check runs for the branch or PR.
- **Layout.** Fourth tab. Header: "Checks", count badges "1 failed" (danger tint, glyph and word) and "1 running", a refresh button; the branch and PR in `mono-sm`. Rows (owner, A50): each check is one 28 px line: status glyph (failed: octagon-alert; passed: circle-check; running: arc; skipped: minus), name, the status word in its colour, chevron. Workflow, duration and actions live in the expandable detail under the row (`surface.sunken`); running and failed checks open expanded. A failed check's detail adds the first failing test and its message, and its actions are "View log", "Re-run" and "Ask Claude to fix" (sends the failure to the session).
- **One line per check (owner, 2026-11-12).** Each check is a single 28 px row: status glyph, name (truncated), status word, chevron. Workflow, trigger, duration and actions ("View log", "Re-run") live in an expandable detail under the row (`surface.sunken`, `radius.md`, indented to the name); click or Enter expands it, and a running or failed check opens expanded by default.
- **States.** Loading; none ("No check runs for this branch."); failing; passing; error ("Could not load check runs." with "Retry").
- **Artboard.** `ws-light`.

### X-PR-01 Pull request view

Desktop: a Work panel tab "PR #313" (details, description, comments, checks, reviewers) and the chip in Source control. Not linked: "Link a pull request" and "Create pull request" (the base is chosen in X-PK-01). Creating: a progress line; error: Banner "Could not create the pull request" with the provider's reason and "Retry". Branch not pushed: "Push the branch first" with "Push".

### Section 10 rows that are not designed as screens

| ID | Disposition |
|---|---|
| D-DF-03 Agent commit or PR action | Merged into D-DF-02 (items of the Commit menu). |
| D-DF-05 Pull request page | Merged into X-PR-01. |
| D-DF-06 Base branch picker | Merged into X-PK-01. |
| D-DF-07 Git settings | Merged into D-ST-06 (Settings, another pass). |

---

## 11. Browser pane

### D-BP-01 Browser pane (and D-BP-05 agent access)

- **Purpose.** Browser tabs in the Work panel, with the chrome that keeps navigation honest about where a page lives and who drives it.
- **Layout.** Tab "localhost:3000" with a globe icon. Toolbar 36 px: Back and Forward (Forward disabled with no history), Reload (becomes Stop while loading), the address bar (28 px, `surface.sunken`, `border.strong`; left: a host chip "this PC"; text: host in `text.primary`, the rest in `text.tertiary`), a zoom chip ("125%") when not 100 percent, Find, Grab and annotate (crosshair, pressed in the artboard), More. A 2 px `accent.default` progress line runs along the toolbar's bottom while loading. The page fills the rest.
- **Indicators.** Not secure: warning glyph and the word "Not secure" (except localhost). Certificate error: danger glyph and "Certificate error" (fails closed; no trust decision in v1). A page on a remote host starts with the chip "on VPS". While an agent drives the tab an agent chip appears right of the address bar: "Claude is controlling this tab" with "Take over". Compact (tab under 400 px): Find and Grab move into More.
- **Content and copy.** "Search or enter an address". Errors in the content frame: "Can't reach localhost:3000" with "Try again"; "Can't reach VPS" and a Banner "VPS can't be reached. The page may be out of date."
- **States.** Loading; error; client-hosted page; document preview (address bar replaced by the document name, "Open externally" through X-DLG-01); driven by another device (D-TM-18 overlay).
- **Interactions.** Mod+L focuses the address bar, Mod+R reloads, Mod+F opens "Find in browser", Mod+Shift+B opens a new browser tab. Keys reach the app first (Mod+K still opens the palette over a page).
- **Artboard.** `ws-browser` (light): a terminal tab and the browser tab on a dev server; the page is the user's own app, drawn in its own colours.

### D-BP-02 Grab and annotate

Grab (crosshair) lets the user hover a page and outline an element (2 px `accent.default` outline, offset 2). A click opens a floating note anchored to the element (owner, 2026-11-12), not a card across the tab: 300 px wide, `surface.overlay`, `border.default`, `radius.lg`, `shadow.e2`, with a caret pointing at the element; it sits below the element, or above when there is no room. It holds the selector in `mono-sm` with a close button, a text field with focus ("Add a note for Claude"), and a footer "Enter to send · Esc to close" with a round send button (`accent.default`). Sending puts the note, the HTML, computed styles and a crop in the next message; the outline stays until the message is sent. Several elements can carry notes before sending. Markup (arrows, boxes on the crop) is a second step. States: grab, note open, note sent.

### D-BP-03 Cookie import

Off by default, explicit and per profile. A section in Settings (D-ST-05) and a one-time disclosure Dialog from the Browser menu: "Import sign-ins from Chrome?", the profile list with checkboxes, "Cookies are copied to this PC only and never leave it.", buttons "Import" and "Cancel". A machine notice for remote hosts: "Cookies are not copied to VPS."

### D-BP-06 WebAuthn account picker

A Popover anchored to the address bar: "Choose an account for nexuscare.example", rows with name and last used, "Cancel". Depends on the engine.

### Section 11 rows that are not designed as screens

| ID | Disposition |
|---|---|
| D-BP-04 Browser profiles | Merged into D-ST-05. |
| D-BP-07 Emulator pane | Merged into D-SH-18 (below). |
| D-BP-08 Link routing preference | Merged into D-ST-05. |

### D-SH-18 Mobile emulator tab

A Work panel tab "Phone preview" with a device frame centred in the body: a Select for the device above it (default "Pixel 9"), a URL field ("localhost:3000" or the workspace's dev server) and Reload. Empty: "Enter an address or start the dev server" with "Run dev server". Error (assumed, A37): "Could not start the preview" with the reason and "Retry". The action appears only when the setting `mobileEmulatorEnabled` is on. Details are assumed (A37) and not drawn.

---

## 12. Command palette and search

### D-CP-01 Command palette, D-CP-02 Quick open, D-CP-03 Search in files, D-CP-04 Settings search

- **Purpose.** One palette to jump to workspaces, sessions, tabs and commands (Mod+K), with three modes: "Go to file" (Mod+P), "Search in files" (Mod+Shift+F) and "Settings" (Mod+,).
- **Result groups (owner, 2026-11-12).** Workspaces, Sessions, Tabs, Commands, in that order. A session row starts with the icon of the agent in charge of it, then the session title, then "project / workspace" in `body-sm` `text.tertiary`, and on the right its state ("Waiting for you" in `agent.waiting`) or its last activity.
- **Layout.** A modal: `scrim` over the whole window (Frame included); the panel is 600 wide, 96 px from the top, `surface.overlay`, `border.default`, `shadow.e3`, `radius.xl`. Input row 44 px: search icon, the query in 14 px, a caret, an "Esc" Kbd. Results (maximum 480 px, scrolls): group headers (`overline`, `text.tertiary`: "Recent", "Workspaces", "Tabs", "Commands", "Files", "Settings"), rows of 28 px with a 16 px icon, the label (matched characters on `match.bg`), an optional hint, and the shortcut as Kbd chips at the right. The active row has `fill.selected.overlay` and a 2 px `accent.fg` bar. A footer (32 px, `surface.sunken`) lists the other modes with their keys.
- **Content and copy.** Placeholder "Search workspaces, tabs and commands". Mode labels at the input's left: "Go to file", "Search in files", "Settings". The artboard shows the query "term": a workspace ("oppm / terminal-reconnect", hint "waiting for you"), a tab ("zsh, oppm"), and commands ("New terminal tab Ctrl T", "Terminal: split pane", "Terminal: clear scrollback").
- **States.** Empty (recent items); filtered; no match ("No matches for term"); loading (rows in Search in files show a spinner after 200 ms); ripgrep missing in Go to file and Search in files ("Search in files needs ripgrep installed." with "Install ripgrep"); host unreachable (rows from that host are dimmed).
- **Interactions.** Up and Down move, Enter runs, Mod+Enter opens in a new Work panel tab, Escape closes and returns focus. Mode keys switch the mode with the current text kept. Extensions register commands here (D-SL-07).
- **Desktop and Android.** Desktop only.
- **Artboard.** `ws-palette` (light).

### D-FD-01 Find bar

A strip at the top right of the active pane (terminal, Conversation, editor, browser, Settings): field "Find", the count "3 of 12", previous and next, close ("Close find bar (Esc)"). The scope is in the field's accessible name ("Find in terminal"). Matches on `match.bg`, the active one on `match.active.bg`. "No results" when empty. Mod+F opens it; a full-screen program in a terminal may need the key (G16).

### Section 12 rows that are not designed as screens

| ID | Disposition |
|---|---|
| D-CP-05 Palette tip dialog | Dropped (tips are removed, D-OB-13). |

---

## Shell rows used above

### D-SH-04 Project sidebar

Rightmost region (288, min 240, max 480). A 36 px strip of icon-only tabs (28 px each, 16 px icons): Files, Sessions, Source control, Checks, then extension tabs. Active tab: `fill.selected.canvas` and a 2 px `accent.default` underline. Tooltips carry the labels; the accessible names include counts ("Checks, 1 failing"). Title bar toggle "Project sidebar". Artboards: Files (`ws-session`), Source control (`ws-diff`), Sessions (`ws-workpanel-empty`), Checks (`ws-light`).

### Narrow window collapse

The artboard `ws-narrow` shows the order in which regions give way (design system 5.1): Session summary below 1408 px, Project sidebar below 1168 px, Sidebar below 928 px, Work panel below 728 px (reachable only with UI zoom, because the window minimum is 800 px). A hidden region keeps its Title bar toggle; clicking it opens the region as an overlay (`z.region-overlay`, `shadow.e2`, `border.default`) over the right part of the Canvas; Escape, a click outside or losing focus closes it; only one overlay is open at a time. Regions the user had open come back when the window grows. The Conversation never drops below 360 px. The rail always stays.

---

## Decisions taken in this pass

The owner did not make these. Each follows the owner's notes as closely as possible.

| # | Decision | Alternative | What changes |
|---|---|---|---|
| 1 | `ws-session` is 1600 by 900, not 1440. (Derived reasoning; the measured ladder in `ws-narrow` is 1408, 1168, 928 and 728 px.) All six regions open with a minimap need 48 + 200 + 480 + 240 + 320 + 240 = 1528 px, because the minimap needs a 480 px Conversation. | Keep 1440 and lower the minimap threshold to 360 px. | One number in the MessageMinimap spec; at 1440 the six-region view would show a minimap on a 392 px Conversation. |
| 2 | The Model picker opens above the Composer in a session (owner reference) and flips below it on the start page (ModelPicker spec). | Always above. | On the start page the picker would cover the heading and the placement row. |
| 3 | The agents overview (D-AG-22) has no page: the Sessions tab and a Status bar popover do the job. Kanban view and terminal popout are not carried over. | A full-canvas overview page. | One more Sidebar or rail entry and a page. |
| 4 | A workspace's label colour comes from the six usage slot colours, not from status colours (D-WS-04, D-WS-06). | Free colour choice. | A user tag could look like an error or a waiting state. |
| 5 | The Session summary is not hidden automatically when a file tab opens; the owner reference only shows it hidden. | Hide it whenever a file or diff tab is active. | Fewer toggles to press, but the cards disappear without a user action. |
| 6 | The Work panel empty state is a raised panel with a 2 by 2 grid (EmptyState spec). | The loose stack of four buttons in the owner's screenshot. | Layout of one empty state only. |
| 7 | Tasks: composer on top, approvals, then the list with a provider filter; the item detail is a side panel. Follows pv96's Tasks home and the rule that Tasks covers both modes. | List first, composer in a dialog. | Order of three blocks. |
| 8 | Automations is a list and detail page with the run table; create and edit open a dialog (D-AG-15). | Edit in place in the detail pane. | The dialog goes away; the detail pane becomes a form. |
| 9 | Prototypes: tabs Directions, Design system, Versions; the element inspector is a fixed right aside. | Inspector opens only on selection. | The viewer gets 288 px more width when nothing is selected. |
| 10 | The floating terminal panel is 480 by 320 with four tabs (Terminal, Checklist, New session, Sessions). No source gives size or position. | A different default size, or no floating panel. | Setting D-ST-03 defaults. |
| 11 | Terminal clipboard writes (OSC 52) are blocked by default with a one-line choice (D-TM-07). | Allow with a visible notice. | Some programs copy without a click; they would need the user's choice each time. |
| 12 | Agent marks in the artboards are neutral placeholders, not provider artwork. | Real provider marks. | Assets only. |
| 13 | The Project sidebar tab strip is 36 px high (28 px tabs with 4 px padding) so its rule lines up with the Work panel tab strip, as in the owner's reference. The ProjectSidebarTabs spec now says the same (measured in the artboards, 2026-11-21). | Keep 28 px. | The two strips would not share one baseline. |
| 14 | The No Git popover on the Sidebar row repeats the explanation of the composer Banner and offers "Initialize Git" in both places. | The popover only, or the Banner only. | One place fewer to learn. |

## Component gaps

Components used here that have no spec yet:

- **WorkPanelTabs**: the Work panel tab strip (icon, label, dirty dot, close, drag, split drop overlay, plus menu). `ProjectSidebarTabs` covers only the icon strip.
- **CodeEditor** surface (gutter, current line, selection, find, read-only state). `DiffView` and `TerminalSurface` exist; the editor does not.
- **SplitButton** ("Commit" with a menu chevron).
- **PaneTree** (nested splits with their own tab strips) if `Splitter` is not meant to cover it.
- **FloatingPanel** frame (D-TM-13): header with tabs and window-style buttons.
- **FilterChips** for the provider filter on Tasks (the artboard uses SegmentedControl).
- **PrototypeViewer** canvas with element picking.
- A **step list with log** for D-WS-15 (Progress rows plus a terminal well); composed from existing parts, but unnamed.

## Not covered

- Android rows of these areas: M-WS-01 to M-WS-07, M-TK-01, M-CH-01 to M-CH-07, M-SE-01 to M-SE-07, M-AR-01, M-PT-01, M-PT-02, M-TM-01 to M-TM-06, M-FI-01, M-FI-02, M-DF-01 to M-DF-04, M-EX-01. Only their desktop counterparts are designed.
- Bot Teams and Calendar rows: D-WS-29, D-WS-30, D-WS-31, D-AG-13, D-AG-23 to D-AG-26, D-AG-33, D-AG-34, X-CH-10, D-ED-13.
- Not drawn, only described: the Add project dialog, Delete workspace, Workspace cleanup, Repo hooks trust, Task item detail, Automation editor, Prototype design system, merge and versions views, Continue or fork, Resume after restart, Session history search results, Mobile emulator tab, Floating terminal panel, Recent tab switcher, Merge conflict review, Read-only viewers, Cookie import, PR view, Secret request card, Bring back, Mention menu, Quick commands editor, the host-unreachable and error variants of every screen.
- The macOS title bar (traffic lights, 40 px) is specified but not drawn; every artboard shows the Windows title bar.
- High-contrast and colour-blind themes (pending-owner-review item 35).
- Densities other than default (28 px rows) are not drawn.
- Real keyboard assignments beyond `app-flow.md`: the change navigation keys in the diff, the Sidebar toggle key and the palette footer wording are proposals and have not been checked against OS shortcuts.
- Motion is described, not shown: card insert, region collapse and overlays are static comps.
- Provider artwork: marks are placeholders.
