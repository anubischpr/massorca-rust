# ListRow

## Purpose

One row in a list or a tree: a workspace, a session, a file, a team, a host, a setting item. The list row is the single pattern behind the Sidebar rows, the mobile workspace list and most menus-with-content.

Inventory IDs: D-WS-02 (workspace row, desktop), M-WS-01 (workspace list, Android), D-WS-29 (team rows in Bot Teams sidebar), D-ED-11 (file rows), D-SL-05 (sidebar section rows). The Sidebar row for workspaces uses this spec; the tree wrapper is SidebarTree.

## Anatomy

- Container: min height `size.control.default` (28 px) on desktop default density (`size.control.compact` 24, `size.control.comfortable` 32). Two-line rows use `size.row.two-line` (44 px). Padding `space.2` (8 px) horizontal, `space.1` (4 px) vertical in the compact density. Radius `radius.md`.
- Leading slot (optional): 16 px icon, or a status glyph 16 px (X-AG-01), or an Avatar at `size.avatar.sm`. Gap `space.2` to the text.
- Title: `body` (13 / 20 / 400) in `text.primary`; `body-strong` (13 / 20 / 600) for the selected row or a title that names a workspace.
- Secondary line: `body-sm` (12 / 18 / 400) in `text.secondary`. Max one line on desktop.
- Trailing slot (optional, one of): timestamp in `caption` `text.tertiary` tabular; Badge (count or state); status word; ChevronRight 12 px; a Switch; a trailing IconButton (overflow), shown on hover or focus only on desktop.
- Separator: none between rows; sections use an overline header.

## Variants and sizes

| Variant | Use |
|---|---|
| one-line | file rows, session rows, menus with content. A session row, parent or child, carries its agent icon between the status glyph and the title (A84); a child session is nested under its parent by indentation only, with no connector lines, and its agent icon lines up with the first letter of the parent's title (A83, A85; 80 dp on Android). A file row carries its file-type icon (A46 to A48). |
| two-line | workspace rows (title and branch or status), team rows, host rows |
| selectable | rows that open a view: the current row shows the selected state |
| toggle | a row with a Switch trailing (settings) |
| tree | a row with a disclosure chevron and an indent of `space.3` (12 px) per level |

Android: row min height 48 dp; two-line rows 56 dp (`size.row.two-line.touch`; `size.touch.large` is 48 for the primary action, the row uses the 56 dp rule). Desktop density changes row height only.

## States

| State | Row |
|---|---|
| default | transparent on its surface |
| hover | `fill.hover.canvas` on the canvas, `fill.hover.frame` in the Frame (sidebar), `fill.hover.overlay` in a menu |
| pressed | `fill.pressed.<surface>` |
| focus-visible | ring `border.focus` 2 px inset by 2 px so it stays inside the row |
| selected | `fill.selected.<surface>`, a 2 px `accent.fg` bar on the leading edge (the thick border), and title in `body-strong` |
| disabled | title and icon `text.disabled`; not selectable |
| loading | Skeleton row (`fill.skeleton` blocks) |
| empty | not a row state; a list with no rows shows EmptyState |
| error | the status glyph `octagon-alert` in `status.danger.fg` and the secondary line gives the reason; never a red row |
| long content | title truncates with an ellipsis; secondary line truncates; full text in the row's tooltip (desktop) or the detail (Android); never wraps |
| disconnected host | the leading glyph `agent.unreachable` (dashed ring) and the secondary line "Last known state, 5 min ago" |

## Behaviour (handoff)

- Keyboard: Up and Down move between rows (roving focus; the list is one tab stop). Enter opens. Right expands a tree row, Left collapses it or goes to its parent. Space selects in multi-select lists. Menu key or Shift+F10 opens the row's context menu.
- Focus order: top to bottom in the list.
- Pointer: click selects and opens; double-click renames where renaming is supported (Sidebar); right-click opens the context menu; the overflow IconButton appears on hover and focus.
- Resizing: the title truncates first, then the secondary line; the trailing slot stays visible and is the last to go.
- Motion: fill colour step in `duration.fast` with `ease.standard`. Tree expand uses `duration.moderate` with `ease.standard` for the height. Reduced motion: instant.
- Accessibility: role `listitem` in a `list`, or `option` in a listbox, or `treeitem` in a tree. Name is the title, followed by the secondary line and the state word ("oppm-api, main, Running"). `aria-selected` and `aria-expanded` as needed.
- Extensions: an extension may contribute rows to its own sidebar section (up to three trailing actions per row, row label 80 characters). It may not restyle rows, add rows outside its section, or use a core status glyph as its icon.

## GPUI notes

- Row is a `div` with focus handling; the list is virtualised with fixed row heights for each density.
- Selected accent bar is a 2 px child element on the leading edge, not a border of the row (keeps the row width stable).
- Trailing hover actions are drawn only when hovered, to keep the list fast.

## React Native notes

- `Pressable` in a `FlatList` or `FlashList`; `getItemLayout` with the fixed height. Two-line rows are 56 dp.
- `accessibilityRole="button"` for tappable rows; list semantics from the list. Long-press opens the action sheet (M-WS-02) with `accessibilityActions` mirroring it.
- Leading glyph and avatar through `react-native-svg`.

## Desktop vs Android

- Desktop: hover reveals trailing actions; right click opens a menu; double click renames. Android: no hover; long press opens a Sheet; swipe actions also exist as accessibility actions.
- Desktop density changes row height; Android uses one density (touch).

## Copy

- Titles are the names the user knows: workspace name, branch, file name, team name.
- Secondary line states facts in plain words: "main, 2 changes", "Last known state, 5 min ago".
- Status words follow X-AG-01: "Running", "Waiting for you", "Idle", "Error", "Done", "Unreachable".

## Token gaps and open points

Token gaps: none. Two-line heights are `size.row.two-line` (desktop, 44) and `size.row.two-line.touch` (Android, 56 dp).

Open points:
- Two-line row height: 44 px on desktop and 56 dp on Android (`size.row.two-line` and `size.row.two-line.touch`), so two lines fit at 200 percent font scale (4.2).
