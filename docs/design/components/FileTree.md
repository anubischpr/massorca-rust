# FileTree

## Purpose

The tree of workspace files in the File explorer of the Project sidebar (Files tab). It has a search field with a Names or Content toggle, git status letters, and dimmed ignored files. Team files use the same tree.

Inventory IDs: D-ED-11 (Desktop); D-ED-13 (team files). The Android counterpart is M-FI-01, a separate component.

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Search field | SearchField spec: `surface.sunken`, `border.strong`; `border.focus` when focused | Height `size.control.default` (28) |
| Names or Content toggle | SegmentedControl spec | Height 28 |
| Row | `radius.md`, no fill at rest | Height `size.control.default` (28); compact `size.control.compact` (24) |
| Indent | Per level | `space.3` (12) |
| Chevron | `text.secondary` | `size.icon.xs` (12) |
| Folder or file icon | folders: outline `folder` in `text.secondary`; files: the file-type icon (A46 to A48): a rounded badge with the type's short name in its representative colour (TS #3178c6, TSX and JSX #61dafb, JS #f7df1e, RS #dea584, PY #3776ab with #ffd43b, CSS #663399, JSON `{}` #cbcb41); `CLAUDE.md` the Claude icon, `AGENTS.md` the Codex icon; any other file (Markdown included) a filled document in grey `text.tertiary` | `size.icon.sm` (16) |
| Name | `text.primary`; ignored `text.tertiary` | `body` type style |
| Git letter | M `status.warning.fg`; A `diff.add.fg`; D `diff.remove.fg`; ? `status.info.fg` | `label` type style, right edge |
| Filter match | `match.bg` | Behind matched characters |
| Selected row | `fill.selected.canvas` and a 2 px `accent.fg` bar | Left edge |

## Variants and sizes

- Two densities: default rows (28 px) and compact rows (24 px). Density changes height only.
- Search has two scopes. Names filters the tree. Content lists matching lines, each with its path in `mono-sm`.
- Deep trees scroll sideways inside a ScrollArea. Names never wrap.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | `text.primary` | Rows at rest, no fill. |
| Hover | `fill.hover.canvas` | Row hover on the Canvas. |
| Pressed | `fill.pressed.canvas` | While the pointer is down. |
| Focus-visible | `border.focus`, inset | Ring inside the row, because the ScrollArea would clip an outer ring. |
| Selected or active | `fill.selected.canvas`, `accent.fg` bar | Marks the file open in the Work panel. |
| Disabled | `text.disabled` | Not used in v1. |
| Loading | `fill.skeleton` | Tree-shaped skeleton rows, replaced by rows; no spinner. |
| Empty | `text.secondary` | EmptyState: "This folder is empty." |
| Error | per ErrorState | ErrorState with Retry. The last list stays visible when one exists. |
| Filtered | `match.bg` | Matched characters are highlighted; ancestors of a match stay expanded. |
| Long content | `text.primary` | Names truncate with an ellipsis; the full path is in the tooltip. |
| Writing | `border.focus` | The search field shows its focus border while typing; results update after the SearchField debounce. |

## Behaviour

- Keyboard: tree pattern with roving focus. Up and Down move between rows; Right expands a folder or moves into its first child; Left collapses a folder or moves to its parent; Home and End go to the first and last visible row. Enter opens a file in the Work panel or toggles a folder. Typing letters jumps to the next name that starts with them.
- Focus order and ring: search field, then the Names or Content toggle, then the tree as one stop. Mod+Shift+F opens search in files (D-CP-03). F6 reaches the tree.
- Pointer: click on a file opens it in the Work panel; click on a folder toggles it. Right-click opens the file-row menu.
- Responsive: fills the Project sidebar width. Rows truncate rather than wrap.
- Motion: no animation on expand or collapse in v1. Hover and pressed fills use `duration.fast` with `ease.standard`. Reduced motion: instant.
- Accessibility: role `tree`, named "Files" (or "Team files"). Rows are `treeitem`s with an expanded state. The name includes git status, for example "README.md, modified".
- Extensions: extensions add items to the file-row menu only, in its trailing group, five at most. They cannot add row icons, badges or tree rows.

## GPUI notes

Render rows in a virtualised list with a fixed row height. Indent by 12 px per level. Right-align the git letter in a fixed slot. Draw matched runs with a solid `match.bg` behind the text. Use no shadows.

## React Native notes

Not used on Android; the Android file list is M-FI-01, built with ListRow. If the tree is reused on tablets, rows become 48 dp touch targets (`size.touch.large`) with TalkBack names and no hover.

## Desktop and Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Navigation | Keyboard tree, hover, git letters | Touch list (M-FI-01) |
| Ignored files | Dimmed text | Dimmed text |

## Copy

- Placeholder: "Search files". Toggle: "Names", "Content".
- Empty: "This folder is empty."
- No match: "No matches for "{query}"" with the action "Clear search" (the EmptyState filtered rule).
- Error: "Could not read the file list.", with "Retry".
- Git words: "Modified", "Added", "Deleted", "Untracked", "Ignored".

## Token gaps and open points

Token gaps: none. Every value uses an existing token.

Open points:
- Only M, A, D and ? are specified. R (renamed) and U (conflict) need a letter and a token. Confirm before Git status ships.
- A single click opens a file. This is proposed; the owner did not say. Confirm.
