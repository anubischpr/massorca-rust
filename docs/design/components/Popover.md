# Popover

## Purpose

An anchored overlay that holds secondary content next to its trigger: a usage detail, a short form, an info panel, or an extension's composer action view. Desktop only; on Android the same content opens in a Sheet.

Inventory IDs: X-DLG-02 (container). Used by D-SH-15 (usage detail in the rail), D-WS-02 hover card, the Picker panel, composer action popovers (up to 320 px).

## Anatomy

- Container: `surface.overlay`, `border.default` 1 px, radius `radius.lg` (8 px), `shadow.e2`, padding `space.3` (12 px). Width is the content width, capped at `layout.popover.width.md` (320 px) for composer actions, `layout.popover.width.sm` (280 px) for the usage popover and `layout.popover.width.lg` (360 px) otherwise.
- Anchor offset: `space.2` (8 px) from the trigger edge.
- Header (optional): `title-sm` in `text.primary`, with an IconButton close (compact) on the trailing edge.
- Body: `body` and `body-sm` content; lists use ListRow rows.
- Footer (optional): right-aligned Buttons, `space.2` apart, primary last.

## Variants and sizes

| Variant | Use |
|---|---|
| info | read-only detail (usage windows, status). No footer. |
| form | short input with Apply and Cancel. Focus moves into the first field. |
| picker | hosts the Picker panel |
| action | composer or context action with one view (up to 320 px wide) |

## States

| State | Popover |
|---|---|
| closed | not rendered |
| open | container with `shadow.e2` |
| loading | body shows Skeleton blocks (`fill.skeleton`); the header stays |
| empty | one `body-sm` line in `text.secondary` with the reason; no placeholder card |
| error | ErrorState (section size) inside the body with Retry |
| focus | focus moves into the popover only for form and picker variants; info popovers do not take focus |
| disabled | not applicable |
| long content | body scrolls inside the popover after 360 px tall; header stays fixed |
| disconnected host | the body shows the last known values with "Last known state, 5 min ago" |

## Behaviour (handoff)

- Keyboard: Enter or Space on the trigger opens. Escape closes and returns focus to the trigger. Tab moves through the popover, then out to the next control; Tab out of a form closes it only when nothing is unsaved.
- Focus order: trigger, then the popover's first control (form and picker only).
- Pointer: hover opens the usage detail popover after a short pause (rail rings only); click opens everywhere else. Click outside closes an info popover; a form asks before it discards edits (use Dialog for the question).
- Resizing: the popover moves to stay inside the window and flips when needed.
- Motion: opacity 0 to 1 with a 4 px move, `duration.base`, `ease.enter`; exit with `ease.exit`. Reduced motion: instant.
- Accessibility: role `dialog` (non-modal) with the header as name. `aria-expanded` on the trigger. Closing returns focus to the trigger.
- Extensions: an extension may supply an action popover view (up to 320 px) for a composer action. It cannot take focus on its own and cannot be opened without a user action.

## GPUI notes

- Overlay element anchored to the trigger; drawn above the Canvas on the overlay layer.
- Shadow `shadow.e2` as a box shadow on the overlay element; confirm the blur convention in the pinned GPUI version.
- Focus is moved explicitly with the element's focus handle on open.

## React Native notes

- No Popover on Android. The same content renders inside a Sheet (see Sheet). Do not port anchored positioning.

## Desktop vs Android

- Anchored Popover with `shadow.e2` on desktop; Sheet on Android.
- Hover-opened usage popovers exist only on desktop.

## Copy

- Header names the thing: "Claude usage", "Activity".
- Body states facts in plain words: "Resets in 2 days".
- Form footer uses the verb of the action: "Apply", "Cancel".

## Token gaps and open points

Token gaps: none. Width caps are `layout.popover.width.sm`, `md` and `lg`.

Open points:
- Should the usage rings popover stay open while the pointer is inside it. Spec says yes.
