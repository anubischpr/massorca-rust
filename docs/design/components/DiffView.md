# Diff view

## Purpose

Shows the changes in one file, opened as a file tab in the Work panel, with inline change markers, a toolbar to move between changes and switch views, and line comments that go to the agent. It is the single diff surface: chat snippets, Source control and compare all open it.

Inventory IDs: D-DF-01 (desktop), M-DF-01 (Android diff review). Review notes: X-RV-02. Owner requirement 2026-11-12: a diff opens as a file tab in the Work panel, like a file, with a toolbar of rendered or source, split, previous and next change, edit and more.

## Anatomy

A Work panel tab body, on `surface.canvas`, top to bottom:

- **Toolbar** (height `size.toolbar`, 36 px: controls `size.control.default` with `space.1` above and below), left to right:
  - File chip: status letter (M, A, D, R) in a 16 px Badge, the file-type icon 16 px (A46 to A48; generic files a grey document), path in `mono-sm` (directory `text.tertiary`, file name `text.primary`), "vs origin/main" in `caption` `text.tertiary`.
  - Counts: `+334` in `diff.add.fg` and `-15` in `diff.remove.fg`, `mono-sm`, tabular; each count carries its sign.
  - Right group (icon buttons, `size.control.default`): Rendered or Source (file-text and code icons, a two-state toggle, `aria-pressed`), Split (columns-2, toggle), Previous change (arrow-up), Next change (arrow-down), Edit (pencil), More (ellipsis). Tooltips carry the name and shortcut.
- **Review bar** (only when notes exist): "3 draft notes" and a "Send to agent" button (primary, `size.control.compact`), on `surface.raised` with a `border.subtle` rule.
- **Body**:
  - Gutter: old line number, new line number (inline) or one per side (split), `mono` 13 / 20, `text.secondary`, tabular, `size.diff.gutter` (40 px) each; then a `size.diff.marker` (20 px) marker column with `+` or `-` in `diff.add.fg` or `diff.remove.fg`.
  - Added line: background `diff.add.bg`, gutter `diff.add.gutter`. Removed line: `diff.remove.bg`, `diff.remove.gutter`. Intra-line changes: `diff.add.word` or `diff.remove.word` behind the changed characters. Code text on changed lines is `text.primary`. Context lines (three around each hunk) sit on `surface.sunken`, with syntax colours.
  - Hunk header row: `@@ -90,7 +90,8 @@ function name` in `mono-sm` `text.tertiary` on `surface.sunken`, 24 px high.
  - Collapsed unchanged block: a 24 px row "Show 42 unchanged lines" (`caption`, `text.secondary`, chevrons-up-down icon), `fill.hover.canvas` on hover; each click reveals 20 lines.
  - Comment marker: a message-square icon (12 px, `accent.fg`) in the gutter of a line with a draft or sent note; hovering a changed line shows a comment button (message-square plus) in the gutter.
  - Inline note: under its line, a `surface.raised` card with `border.default`, `radius.md`: author (you), text in `body-sm`, status "Draft" or "Sent" in `caption`, and a text field when editing.
- **Change ruler** (8 px strip at the right edge of the scrollbar): one mark per change block in `diff.add.fg`, `diff.remove.fg`, or both for a modified block (split vertically), the current change outlined with `text.primary`.
- **Rendered mode** (Markdown and similar): the rendered document with changed blocks flagged by a 2 px bar in the gutter (`diff.add.fg`, `diff.remove.fg`) and a "+" or "-" glyph; clicking a flagged block shows its source diff in place.

## Variants and sizes

| Variant | Difference |
|---|---|
| Inline (default) | One column, removed line above added line. |
| Split | Two panes with aligned pairs, synchronised scroll. Available when the tab is at least 640 px wide; below that the button is disabled with the tooltip "Widen the Work panel to use split view". |
| Rendered | Markdown and other renderable types only; the toggle is hidden for code. |
| Read-only compare | Used by the external-change banner (D-ED-06): no Edit, no comments. |
| Android | Inline only, one new-line-number column, 13 / 18 sp mono, tap a line to comment (sheet), previous and next change in a bottom bar. |

## States

| State | Visual | Tokens |
|---|---|---|
| default | As anatomy | `surface.sunken`, `diff.*`, `text.primary` |
| hover (line) | Comment button appears in the gutter; line gets no fill | `text.secondary` |
| hover (toolbar button) | Fill | `fill.hover.canvas` |
| pressed | Fill | `fill.pressed.canvas` |
| focus-visible | Toolbar buttons: ring 2 px offset 2 px. A diff line: inset 2 px ring on the line | `border.focus` |
| selected | Toggle on (Split, Source): `fill.selected.canvas` plus a 2 px `accent.fg` underline. Current change: ruler mark outlined, line block marked by an inset 2 px `accent.fg` bar | `fill.selected.canvas`, `accent.fg` |
| disabled | Previous at the first change, Next at the last, Edit when the host is unreachable, Split when narrow | `text.disabled` |
| loading | Skeleton: toolbar and 14 lines of two lengths after 100 ms; a spinner appears after 200 ms | `fill.skeleton`, `agent.running` |
| empty | "No changes in this file." with a ghost button "Show file" | `text.secondary` |
| error | ErrorState: "Could not load this diff", Retry | `status.danger.fg` |
| large | Prompt: "This diff is large (12,431 lines). Show it anyway?" with "Show diff" and "Open as source" | `status.info.bg` Banner |
| conflicts skipped | Info Banner: "Conflicted hunks are skipped. Resolve them in the merge view." | `status.info.fg` |
| binary or image | "Binary file changed" with size before and after; images open the viewer's image diff | `text.secondary` |
| long content | Lines never wrap by default (horizontal scroll with sticky gutters). "Wrap lines" in More wraps at the panel width. A line over 2,000 characters is cut with "Show full line" | `text.secondary` |
| disconnected host | The loaded diff stays. A Banner: "Showing the last loaded diff. VPS can't be reached; changes made since may be missing." Edit and Send to agent are disabled; navigation still works | `agent.unreachable` |

## Behaviour (handoff)

- **Keyboard.** Focus enters the body with Tab; Up and Down move a line cursor; Page Up and Page Down scroll; Home and End jump to the file's first and last line. `F7` and `Shift+F7` move to the next and previous change (the same as the toolbar buttons; keep the current app's keys if they differ, see open points). Enter on a collapsed block expands it. `Mod+Alt+M` opens a note on the focused line (changed lines only). `Mod+F` opens the Find bar over the diff. Toolbar: Tab moves through it; Left and Right move between buttons (one tab stop).
- **Focus order.** Toolbar, review bar, body, ruler (not focusable).
- **Pointer.** Click the gutter comment button to open an inline note. Click a ruler mark to jump to that change; the ruler also shows a tooltip of the line range. Selecting text in the code and the Copy action copies without gutters and markers. Right click: the diff line context menu (copy, copy path with line, comment, extension items).
- **Resizing.** The tab follows the Work panel width; below 640 px Split is unavailable and an open split falls back to inline. Gutters stay sticky on horizontal scroll.
- **Comments.** A draft note is stored locally and listed in the review bar. "Send to agent" posts all drafts as one message with `file:line` references; sent notes become "Sent" and arrive in the chat as a Review notes block (MessageBlocks). If the agent is busy, they join the queue (A10).
- **Edit.** Switches the tab to the editor (D-ED-01) at the same position; the diff view returns when editing ends. A dirty marker follows the tab.
- **Motion.** None except smooth jump to a change: scroll over `duration.moderate`, `ease.standard`. Reduced motion: instant. Expanding unchanged lines is instant.
- **Accessibility.** The body is a document of lines; each line is read as "Added line", "Removed line" or "Unchanged line" followed by its text and line numbers. The toolbar is a toolbar with named buttons; toggles use `aria-pressed`; counts read "334 added, 15 removed". Moving to a change announces "Change 3 of 12".
- **Extension limits.** The diff view is core. Extensions may add context menu items on a diff line (contribution point 13, trailing group, up to 5, one submenu level) and nothing else. They cannot recolour the diff or add toolbar buttons.

## GPUI notes

- Virtualise the line list with fixed 20 px rows (`uniform_list`). Backgrounds are quads per visible row; intra-line highlights are quads behind the shaped text; gutters are painted separately from the code so horizontal scroll can leave them in place.
- Compute the diff and the word-level changes in the core on a worker thread; the view receives hunks and rows. Do not diff in the render path.
- Cache shaped lines per (row id, theme). Split view shares one scroll offset between two lists.
- The change ruler is one custom element painting quads from the hunk table.
- Syntax highlighting: tree-sitter captures per context row. On changed lines the design uses `text.primary` because the syntax tokens are validated on `surface.sunken` and `surface.canvas` only (decided, pending-owner-review item 34).

## React Native notes (Android)

- A FlatList of rows with fixed height, horizontal scroll inside a wrapper; the line gutter is pinned. Tap a changed line to open a comment sheet; long-press selects. Previous and next change in a 48 dp bottom bar; hunk headers are sticky. TalkBack reads each line as on desktop.

## Desktop vs Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Container | Work panel file tab | Full screen (M-DF-01) |
| Layouts | Inline, split, rendered | Inline |
| Gutters | Old and new line numbers | New line number only |
| Comment | Gutter button, `Mod+Alt+M` | Tap a line, sheet |
| Navigation | Toolbar, `F7`, ruler | Bottom bar |

## Copy

- Toolbar tooltips: "Source", "Rendered", "Split view", "Previous change", "Next change", "Edit", "More".
- Messages: "No changes in this file.", "This diff is large (12,431 lines). Show it anyway?", "Show diff", "Open as source", "Binary file changed", "Show 42 unchanged lines", "Widen the Work panel to use split view".
- Notes: "3 draft notes", "Send to agent", "Draft", "Sent".
- Disconnected: "Showing the last loaded diff. VPS can't be reached; changes made since may be missing."

## Token gaps and open points

Token gaps: none. Gutter and marker widths are `size.diff.gutter` and `size.diff.marker`; the toolbar is `size.toolbar`.

Open points:
- Syntax colours on changed lines: `text.primary` on `diff.add.bg` and `diff.remove.bg` (and their word colours), where contrast is already validated (V7). Syntax colours stay off changed lines (pending-owner-review item 34); they colour unchanged lines only.
- `F7` and `Shift+F7` move to the next and previous change (decided; pending-owner-review item 27).
- How many files or all changes are listed is not designed (the Source control panel opens one file at a time).
- The "more" menu items are not specified; proposed: copy path, wrap lines, ignore whitespace, expand all, reveal in Project sidebar.
