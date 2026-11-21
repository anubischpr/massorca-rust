# BoardColumn

## Purpose

One column of the team Board (D-WS-30): a header with a title and a count, the team's task cards with their agent status, and a "No tasks" line when the column is empty. Cards move by pointer drag or by keyboard.

Inventory IDs: D-WS-30 (Desktop). Android: the phone Board (pending-owner-review item 4).

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Column | `surface.sunken`, no border | Width `layout.board.column.width` (280); fills the Board height |
| Header | `title-sm` title in `text.primary`; count in `caption`, tabular, `text.secondary` | Height `size.control.comfortable` (32) |
| Card | `surface.raised`, `border.default`, `shadow.e1`, `radius.lg` | Column width less `space.2` gutters |
| Card title | `body-strong`, `text.primary` | Up to two lines, then an ellipsis |
| Team or bot name | `body-sm`, `text.secondary` | One line |
| Status | AgentStatus glyph at 12 px, with its word in `caption` | `size.icon.xs` (12) |
| Time | `caption`, `text.tertiary` | One line |
| Drop indicator | `accent.default`, `border.width.thick` | 2 px line between cards |
| Empty text | "No tasks" in `text.tertiary` | Centred in the column |

## Variants and sizes

- Four columns proposed: Open, Running, Waiting for you, Done. The owner did not set the columns (see open points).
- One card form: title, team or bot name, status and time.
- Columns keep their width; the Board scrolls sideways when they overflow.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | `surface.raised`, `border.default` | Resting card. |
| Hover | `fill.hover.raised` | Pointer over a card. |
| Pressed | `fill.pressed.raised` | While a card is pressed; a movement after pressing starts the drag. |
| Focus-visible | `border.focus`, 2 px, offset 2 px | Keyboard focus on a card; drawn inset at the column's top and bottom edges, where the scroll area clips it. |
| Selected or active | `surface.overlay`, `shadow.e2`; source slot dashed `border.strong` | The card being moved; the lifted card follows the pointer or the keys. |
| Disabled | `text.disabled` | Not used in v1. |
| Loading | `fill.skeleton` | Card-shaped skeletons while tasks load. |
| Empty | `text.tertiary` | "No tasks" in the column. |
| Error | `status.danger.fg` | ErrorState inside the column; other columns keep working. |
| Long content | `text.primary` | Titles wrap to two lines, then truncate. This is a card, not a row. |
| Disconnected host | `agent.unreachable`, `text.tertiary` | The card shows the unreachable glyph and "Last known state, 5 min ago". |
| Writing | N/A | Card titles are not edited in v1. |

## Behaviour

- Keyboard: Space picks up the focused card. Left and Right move it to the next column; Up and Down move it within a column. Space drops it; Escape cancels and returns it to its original slot. Each move is announced, for example "Moved to Running, position 2 of 3."
- Focus order: column by column, top to bottom. Column headers are not stops.
- Pointer: drag a card; a 2 px drop indicator shows between cards; release drops it. A move changes the column only; it does not start or stop an agent (proposed).
- Responsive: columns keep their width; the Board scrolls sideways in a ScrollArea.
- Motion: a dropped card slides into its slot over `duration.fast` with `ease.standard`. The lifted card follows the pointer without easing. Reduced motion: instant.
- Accessibility: each column is a list named by its title and count, for example "Running, 2 tasks". Each card is a list item named by its title, team or bot, status word and time. Moves are announced politely.
- Extensions: extensions cannot add columns or cards in v1; the Board has no contribution point.

## GPUI notes

Columns are fixed-width flex children with a solid `surface.sunken` fill. Cards are raised rounded rectangles with a 1 px border and the e1 shadow. While dragging, draw the card in the overlay layer and the insertion line as a 2 px rectangle. Do not blur, and do not ease the lifted card.

## React Native notes

Not on Android in v1. If the Board appears on Android, group cards into status sections in a list (proposed), and offer "Move" as an accessibility action in the card menu instead of drag.

## Desktop and Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Movement | Pointer drag; keyboard Space and arrows | Move action from the card menu (proposed) |
| Layout | Columns side by side, scrolling sideways | Sections in a list (proposed) |

## Copy

- Column titles: "Open", "Running", "Waiting for you", "Done" (proposed).
- Empty column: "No tasks".
- Announcement: "Moved to Running, position 2 of 3."
- Status words: "Running", "Waiting for you", "Idle", "Error".

## Token gaps and open points

Token gaps: none. Column width is `layout.board.column.width`.

Open points:
- The four columns are a proposal; the owner did not define the Board columns. Confirm.
- Whether a finished agent moves its card to Done automatically is not defined. Confirm.
- Enter on a card has no target yet: opening a task has no detail row, because the inventory covers issues and PRs only. Confirm.
