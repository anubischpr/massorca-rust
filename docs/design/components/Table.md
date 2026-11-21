# Table

## Purpose

The data table for dense lists of comparable rows: header labels, sortable columns, numbers aligned right, and loading, empty and error states. It serves Checks, usage detail, the workspace space page and GitHub project views.

Inventory IDs: none of its own. Used by D-DF-04 (Checks), D-AC-06 (usage detail), D-WS-18 (workspace space page, which absorbs D-WS-19) and D-WS-26 (GitHub Projects, an extension). Android: ListRow lists below 600 dp.

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Header row | `label` type style, `text.secondary`; bottom rule `border.subtle` | Height `size.control.default` (28) |
| Body row | `body` type style, `text.primary`; bottom rule `border.subtle` | Height `size.control.default` (28), or `size.control.comfortable` (32) |
| Sort indicator | chevron-up or chevron-down (Lucide), `text.secondary` | `size.icon.xs` (12) |
| Numeric cell | `body` with tabular figures, right aligned | Cell padding `space.2` |
| Selection checkbox | Checkbox spec | 28 px row |
| Row hover | `fill.hover.canvas` | Full row |
| Selected row | `fill.selected.canvas`, `accent.fg` bar | 2 px at the left edge |
| Skeleton row | `fill.skeleton` | Body row height |

## Variants and sizes

- Default density (28 px rows) for Checks; comfortable (32 px) where the view sets it.
- Cell types: text, numeric (right aligned), status (AgentStatus glyph with its word), agent, and action (an IconButton at the row end, with a 24 px target).
- Agent column (A20): header "Agent"; the cell shows only the agent's icon, 16 px, full colour, with the agent name as tooltip and accessible label. Never a letter, never the name as text.
- A text cell that names a kind of content carries its icon on the left (A21); a file name carries its file-type icon (A46). Column headers, numbers and values have no icon.
- An optional selection column holds a Checkbox.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | `text.primary` | Rows at rest; rules in `border.subtle`. |
| Hover | `fill.hover.canvas` | Full-row hover. |
| Pressed | `fill.pressed.canvas` | While pressed on a row. |
| Focus-visible | `border.focus`, inset | On the focused cell or row; inset so the ScrollArea does not clip it. |
| Selected or active | `fill.selected.canvas`, `accent.fg` bar | The bar is the second cue. |
| Disabled | `text.disabled` | Row actions that do not apply are disabled, not hidden. |
| Loading | `fill.skeleton` | Five skeleton rows (proposed); static, with no shimmer. |
| Empty | none | EmptyState in the body, with one action. |
| Error | `status.danger.fg` | ErrorState with Retry replaces the body; the headers stay. |
| Long content | `text.primary` | Cells truncate with an ellipsis and never wrap; the full value is in the tooltip. |
| Writing | N/A | Cells are not edited in this spec. |

## Behaviour

- Keyboard: Tab enters the table. Arrow keys move between cells; Home and End go to the first and last cell of the row. Enter opens the row. Space toggles the row's checkbox. Enter or Space on a sortable header sorts by it.
- Focus order: sortable headers first, then the rows.
- Pointer: click a row to select it (single selection in v1). Click a sortable header to cycle ascending, descending and none. Click a checkbox to toggle it.
- Responsive: columns keep a minimum width. Wide tables scroll sideways inside a ScrollArea. Numeric columns stay right aligned. Below 600 dp on Android, the table becomes ListRow lists (proposed).
- Motion: sorting does not animate, and skeletons do not shimmer. Reduced motion: no change.
- Accessibility: role `table` or `grid`, named by the view heading. Sortable headers carry `aria-sort` with ascending, descending or none. Selected rows carry `aria-selected`. A sort change is announced politely, for example "Sorted by Updated, descending."
- Extensions: extension views may use Table; D-WS-26 is an extension. The host provides the scrolling, because an extension cannot place a ScrollArea.

## GPUI notes

Draw rows as fixed-height flex rows, virtualised when the list is long. Right-align numeric columns and use tabular figures. Draw the 1 px rules as border quads in `border.subtle`. Truncate cells with the text layout's ellipsis. Use solid fills per state and no shadows.

## React Native notes

Below 600 dp, render the same rows as ListRow lists, with `size.row.two-line.touch` (56 dp) rows for two-line rows, as in ListRow. Sorting moves to a sheet of sort options (proposed). Each row reads as one TalkBack item, with its cells in order.

## Desktop and Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Layout | Dense table with sortable headers | Lists below 600 dp |
| Pointer | Hover fill; sort on click | Touch; sort in a sheet |
| Scroll | Horizontal ScrollArea | Vertical list |

## Copy

- Column labels in sentence case, for example "Name", "Status", "Updated", "Usage".
- Checks empty state: "No check runs for this branch."
- Error: "Could not load this table.", with "Retry".
- Sort announcement: "Sorted by Updated, descending."

## Token gaps and open points

Token gaps: none. Every value uses an existing token.

Open points:
- Single-column sort only in v1. Confirm for Checks and usage detail.
- Row selection is single in v1; bulk actions need a decision.
- The inventory marks D-WS-19 as merged into D-WS-18, so the space table depends on that merge. Confirm.
