# SidebarTree

## Purpose

The Sidebar: the tree of projects, groups, workspaces and hosts in Workspaces mode, and the list of teams in Bot Teams mode. It sits on the Frame beside the Activity rail, below the Title bar, and the sidebar toggle hides it.

Inventory IDs: D-WS-01 (Workspaces tree), D-WS-29 (Bot Teams sidebar), D-HO-01 (hosts under each project). Desktop only.

## Anatomy

Workspaces mode, top to bottom:

| Part | Tokens | Size |
|---|---|---|
| "+ Create" primary button (opens the start page) | fill `accent.default`, label `text.on-accent`, `label` type, `radius.md` | height `size.control.default` (28) |
| Search field | fill `surface.sunken`, border `border.strong`, placeholder `text.tertiary` | height 28; icon `size.icon.sm` (16) |
| Entries (SidebarRow, nav), each with its icon on the left (A21): Prototypes (`shapes`) with count "2" and chevron, Tasks (`list-checks`), Automations (`workflow`), MassOrca Mobile (`smartphone`) | `body` text, `text.secondary` icons | row 28 |
| "Projects" header with actions: bell (notifications), list-filter (filter), folder-plus (add folder), plus (new project) | `overline` text in `text.secondary` | header 24 (`size.control.compact`); actions 16 px icons in 24 px boxes |
| Project rows (top level, A10): the project icon (A41: the folder glyph or the user's chosen icon, 16 px), name, owner or org in secondary text | SidebarRow project | row 28 |
| Host rows under each project, one per host where the project lives (D-HO-01): `server` icon, host name, connection dot, state word | SidebarRow host | row 28 |
| Workspaces under their host row: `git-branch` icon (worktree) or `folder` (plain folder), name | SidebarRow workspace | row 28 |
| Sessions under their workspace: AgentStatus glyph 12 px, the agent icon 16 px (A84), title | SidebarRow session | row 28 |
| Child sessions nested under their parent session, by indentation only, no connector lines (A83); the indent places the child's agent icon in line with the first letter of the parent's title (A85) | SidebarRow session, child | row 28 |
| Group "Unknown" for projects with no repository | SidebarRow group | row 28 |
| Footer: help, locate, panel toggle | icons `text.secondary` | 28 |

Bot Teams mode (D-WS-29): search field; "All teams" row; one team row with its team character, then child rows "Conference room" (`messages-square`, hint "Nothing open") and "Board" (the kanban icon, A25; hint "No tasks"); "Projects" header with filter and plus; rows "Files" (`folder`), "Memory" (Lucide `brain`, A24), "Routines" (`calendar-clock`); the same footer.

The grouping is by project, never by host (A10): a project that lives on two hosts shows two host rows under it, and an unreachable host dims only its own row and the workspaces under it.

## Variants and sizes

- Two contents, one chrome: the Workspaces tree and the Bot Teams list. Both sit on `surface.frame` with no border.
- Width: `layout.sidebar.default` (256), min `layout.sidebar.min` (200), max `layout.sidebar.max` (400). Remembered per window.
- Row height follows density: `size.control.compact` (24), `size.control.default` (28), `size.control.comfortable` (32).
- Hidden: width 0. The rail and the Canvas corner stay visible.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | `surface.frame` | No border or shadow to the rail or Title bar. |
| Focus within | row focus ring | One tab stop for the tree. |
| Loading | rows as `fill.skeleton` bars, 3 to 5 | On first load and after a mode switch. No spinner. |
| Empty, Workspaces | `text.tertiary` "No projects yet" | The "+ Create" button stays. |
| Empty, Bot Teams | `text.tertiary` "No teams yet" | Proposed copy. |
| Filtered, no match | `text.tertiary` "No match for \"name\"" | Clears with the search field. |
| Error | `status.warning.fg` `triangle-alert` with "Check", and a text button "Retry" | Other rows still work. |
| Long content | names truncate with an ellipsis; full name in tooltip | Never wraps. |
| Disconnected host | host row and its workspaces dimmed in `text.tertiary`; glyph `agent.unreachable`; "Last known state, 5 min ago" | Dimmed, with the age. Never "stopped". |

## Behaviour

- Keyboard: the tree has one tab stop. Up and Down move between rows. Right expands a group or moves to its first child; Left collapses or moves to the parent. Enter opens. Home and End jump to the ends. Typing moves to the next row whose name starts with that text. Tab leaves the Sidebar. F6 cycles regions.
- Focus order: after the Activity rail, before the Conversation.
- Pointer: click selects and opens. Right-click or Shift+F10 opens the row menu (D-WS-02). The right edge is a splitter (`border.splitter.hit`, 6 px); drag resizes, double-click resets to 256.
- Responsive: dragging below 200 collapses the Sidebar at 160 (min minus 40); dragging back out reopens it at 200.
- Motion: width changes use `duration.moderate` with `ease.standard`. Content keeps its final width and is clipped. Reduced motion: instant.
- Accessibility: landmark `navigation`, name "Sidebar". Rows expose level, expanded and selected state. Count badges are part of the name ("Prototypes, 2").
- Extensions: a sidebar section (D-SL-05) below the core tree, collapsed by default. Header up to 24 characters, up to 2 header icon buttons, rows up to 80 characters, up to 3 trailing actions per row. An extension cannot restyle core rows, add rows outside its section, expand itself, or appear in the other mode unless it declares it.

## GPUI notes

Render the tree as a virtualised list inside a ScrollArea. Row fills are precomputed solid colours. Draw row menus in the overlay layer, not inside the Sidebar clip. The "+ Create" button is a flat accent fill with no shadow.

## React Native notes

Not on Android. Workspaces are the list M-WS-01 (a master pane on wide layouts), and teams have their own screens. Do not port this tree.

## Desktop and Android differences

Desktop: one tree with keyboard navigation, hover actions and a splitter. Android: separate list screens; no tree, no splitter. The Android Projects list follows the same structure (A82, A88): project headers, a small host label per host, agent icons on session rows and nested child sessions.

## Copy

- Workspaces: "Create" (shown as "+ Create"), "Search", "Prototypes", "Tasks", "Automations", "MassOrca Mobile", "Projects", "Unknown".
- Bot Teams: "Search", "All teams", "Conference room", "Board", "Projects", "Files", "Memory", "Routines".
- Hints: "Nothing open", "No tasks".
- Empty: "No projects yet", "No teams yet".
- Host state: "Last known state, 5 min ago".
- Accessible names: "Workspaces", "Teams".

## Token gaps and open points

Token gaps: none. Count badges use the neutral badge (`badge.neutral.bg`, `badge.neutral.fg`).

Open points:
- The reference shows "+ Create" in a neutral light fill. This spec uses `accent.default` because the owner called it the primary button, and the foundations give one primary accent action per view. Confirm.
- Prototypes is a core entry (pending-owner-review item 1); its Sidebar row comes from D-SL-05.
- The footer icons (help, locate, panel toggle) have no action in the inventory. Confirm their actions.
