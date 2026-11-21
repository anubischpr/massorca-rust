# Card

## Purpose

A raised container that groups one related set of information or actions: a session summary card, a routine or team card, a settings group, a host card, or a conversation card. It is a surface, not a link; a card that opens something is an interactive card with a single target.

Inventory IDs: D-AG-27 (session summary cards), D-AG-25 (routine cards), D-SL-03 (conversation card, extension), M-HM-02 (host card), D-SH-15 (usage card in the rail popover, through Popover), and the settings groups of D-ST-01 to D-ST-11.

## Anatomy

- Container: `surface.raised` fill, `border.default` 1 px, radius `radius.lg` (8 px), `shadow.e1`. Padding `space.3` (12 px) for summary and settings cards; `space.4` (16 px) for host and routine cards.
- Header: title `title-sm` (14 / 20 / 600) in `text.primary`, optional value on the trailing edge in `body` with tabular figures, optional IconButton overflow (compact) on the far right.
- Body: `body` (13 / 20 / 400) in `text.primary`, supporting rows in `body-sm` `text.secondary`. Up to six rows in a summary card's collapsed body.
- Divider between header and body: `border.subtle` 1 px, `space.2` (8 px) above and below.
- Footer (optional): `body-sm` status line or up to two Buttons, right-aligned, `space.2` apart.
- Nested radius: a control inside a card keeps its own radius (`radius.md`). The nested-radius rule (parent radius minus padding, never below `radius.xs`) applies only to inset shapes such as wells, which take `radius.xs` or `radius.sm` here.

## Variants and sizes

| Variant | Use | Behaviour |
|---|---|---|
| static | summary, settings group, host card with actions | not focusable as a whole; inner controls are |
| interactive | routine card, team card, workspace card that opens a view | the whole card is one focus target and one click target; inner controls stop propagation |
| summary | session summary card (D-AG-27) | appears only when it has information; animated insert |
| settings group | a set of Settings rows under an overline heading | rows separated by `border.subtle` |

Widths: fills its column. Summary cards follow the Session summary column (288 px default, `layout.summary.default`). Android: full width with 16 dp gutters.

## States

| State | Card |
|---|---|
| default | `surface.raised`, `border.default`, `shadow.e1` |
| hover (interactive) | `fill.hover.raised` fill; border unchanged |
| pressed (interactive) | `fill.pressed.raised` |
| focus-visible (interactive) | ring `border.focus` 2 px, offset 2 px |
| selected | `border.focus` is not used; selected card shows an accent bar (2 px `accent.default` leading edge) and `fill.selected.raised` |
| disabled | `text.disabled` content; no hover; `aria-disabled` on interactive cards |
| loading | Skeleton card (`fill.skeleton` blocks in the card's shape) |
| empty | never rendered. A summary card with no information is not shown; a settings group with no rows is not shown. |
| error | ErrorState (section size) inside the card; the card keeps its title |
| long content | body caps at 320 px tall with "Show more" (conversation card) or scrolls inside the card; title truncates |
| disconnected host | the card shows last known values with "Last known state, 5 min ago" in `body-sm` |

## Behaviour (handoff)

- Keyboard: interactive cards are one focus target; Enter opens. Inner actions are reached with Tab after the card. Arrow keys move between cards in a grid when the grid is a roving list.
- Focus order: card order in the column; within a card, header actions, body controls, footer actions.
- Pointer: click opens the interactive card; hover fills it; inner controls do not trigger the card.
- Resizing: fills its column; text wraps in body and truncates in the title.
- Motion: summary cards grow in over `duration.card-insert` (250 ms) with `ease.enter`, fade and move up 8 px; cards below slide with `ease.standard`. Reduced motion: appears in place instantly. Hover fill in `duration.fast`.
- Accessibility: static cards are `region` only when they have a heading; interactive cards are `button` or `link` with the title as their name. A summary card insert is announced once, politely: "Plan card added".
- Extensions: an extension may contribute a session summary card (title 24 characters, value 12 characters, up to 3 cards, collapsed body up to six rows) and a conversation card (header, view up to 320 px tall, up to three actions). It may not force itself open, show an empty placeholder, reorder core cards, or animate beyond the standard insert.

## GPUI notes

- Card is a `div` with the raised fill and a 1 px border; the shadow `shadow.e1` is a box shadow on the element (confirm the blur convention in the pinned GPUI version).
- Interactive cards use a focus handle on the whole element; inner controls stop click propagation.
- Summary insert animates the height from 0 to measured height; anchor the scroll so nothing jumps.

## React Native notes

- `View` with `borderRadius` and a border; `shadow` via elevation 1 dp on Android. Interactive cards are `Pressable` with `accessibilityRole="button"`.
- Summary card insert uses layout animation (`LayoutAnimation` or Reanimated layout) over `duration.card-insert`.

## Desktop vs Android

- Desktop: summary cards in a column beside the conversation; Android: summary cards in the session screen's detail view, full width.
- Elevation 1 dp on Android against `shadow.e1` on desktop; same surface and border.

## Copy

- Title is a noun: "Plan", "Checklist", "Context", "Results", "Child sessions", "Subagents", "Connectors", "Background tasks", "QA servers".
- Values are short numbers or words, tabular for numbers.
- Body rows are facts: "2 of 5 done", "Last known state, 5 min ago".

## Token gaps and open points

- No gaps: `space.3`, `space.4`, `radius.lg`, `shadow.e1` and `border.default` cover the card.
- Open: whether settings groups should carry `shadow.e1` as well. Spec says yes, to match the summary cards (the design system lists settings groups at e1).
