# Skeleton

## Purpose

A grey placeholder shaped like the content that is loading, so a list, card or page keeps its layout while data arrives. A Skeleton is never the final state: it is replaced by content, an EmptyState or an ErrorState.

Inventory IDs: none of its own. Used by D-AG-14 (automations skeleton), D-AG-23 (calendar loading), D-WS-24 (tasks loading), M-WS-01 (workspace list loading), the Select and Picker loading rows, and every list or card that loads.

## Anatomy

- Block: fill `fill.skeleton`, radius matching the shape it stands for: `radius.xs` for text lines, `radius.md` for rows and fields, `radius.lg` for cards, `radius.full` for avatars.
- Text line: height `size.skeleton.line` (12 px) at `body` line rhythm, widths vary (100, 80, 60 percent) so the block reads as text.
- Row: height `size.control.default` (28) on desktop density, containing one circle (`size.avatar.sm`) and two lines.
- Card: `surface.raised` with `border.default` and the card's padding, containing blocks.
- Avatar: a circle of `size.avatar.md`.

## Variants and sizes

| Variant | Shape |
|---|---|
| line | one text line |
| row | a list row: leading shape, two lines |
| card | a card with a title line and three lines |
| avatar | circle |
| block | rectangle for a chart, editor or image area |

One size per shape; sized by its container.

## States

| State | Rendering |
|---|---|
| loading | blocks in `fill.skeleton`, shown after `SKELETON_DELAY_MS` (100 ms) of loading (nothing for the first 100 ms) |
| shimmer (default off) | no moving highlight in v1. Static fill only. |
| resolved | blocks removed and content shown in the same place, without a flash |
| empty | replaced by EmptyState; never left in place |
| error | replaced by ErrorState; never left in place |
| disabled | not applicable |
| long content | not applicable; blocks match the expected length, not the content |
| disconnected host | replaced by "Last known state, 5 min ago" with the last content, or by ErrorState when there is no last content |

## Behaviour (handoff)

- Keyboard: not focusable. The region is the focus target.
- Pointer: none.
- Resizing: blocks follow the container width; line widths are percentages.
- Motion: static by default. A moving highlight is not specified (no token). Reduced motion: static always.
- Accessibility: blocks are `aria-hidden`. The region carries `aria-busy="true"` and a single polite announcement "Loading routines" once, not per block.
- Extensions: may use Skeleton in extension views (allowed list). Its shape must match the extension's own content.

## GPUI notes

- Plain rounded `div`s with `fill.skeleton`; precomputed colour per theme.
- No gradient and no opacity animation for the placeholder.

## React Native notes

- `View`s with `backgroundColor` = `fill.skeleton` and `borderRadius` from the table; `accessibilityElementsHidden` on the placeholder group.
- No native shimmer effect unless a shimmer token is added.

## Desktop vs Android

- Same shapes. Android rows are 48 dp high; desktop rows 28 px.

## Copy

- No words inside a skeleton. The announcement, when needed, is "Loading {name}".

## Token gaps and open points

Token gaps: none. The line height is `size.skeleton.line`. Shimmer is off in v1, so no highlight colour is needed.

Open points:
- Whether shimmer should be added later. If so, it needs a highlight token for each surface; v1 keeps one motion model.
