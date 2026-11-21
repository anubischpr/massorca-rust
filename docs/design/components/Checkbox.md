# Checkbox

## Purpose

Selects one or more independent options in a list or form, or marks a row in a bulk action. Shows checked, unchecked, and indeterminate (some children selected).

Inventory IDs: none of its own. Used in forms (X-VT-01 options, D-WS-14 options), bulk selection in lists and extension settings.

## Anatomy

- Box: 16 by 16 px (`size.icon.sm`), radius `radius.md`, border `border.width.default` (1 px).
- Tick or dash: Lucide `check` or a horizontal bar, 12 px (`size.icon.xs`), stroke per iconography rule.
- Label: style `body`, colour `text.primary`, `space.2` (8 px) to the right of the box. Helper text in `body-sm`, `text.secondary`.
- Hit area: row at least 24 px high on desktop (`size.hit.pointer`); 48 dp on Android (`size.hit.touch`).

## Variants and sizes

| Variant | Use |
|---|---|
| default | single option |
| group | a set of checkboxes under an overline label (`overline` style, capitals) |
| indeterminate | parent of a partly selected set |

One size on desktop. Android: box 20 dp inside a 48 dp row.

## States

| State | Box fill | Outline | Mark |
|---|---|---|---|
| unchecked, rest | `surface.sunken` | `border.strong` | none |
| unchecked, hover | `fill.hover.raised` | `accent.fg` | none |
| checked, rest | `accent.default` | `accent.default` | `text.on-accent` tick |
| checked, hover | `accent.hover` | `accent.hover` | `text.on-accent` tick |
| indeterminate | `accent.default` | `accent.default` | `text.on-accent` dash |
| focus-visible | ring `border.focus`, 2 px, offset 2 px around the box and the label | | |
| disabled | `fill.disabled` | `border.disabled` | `text.disabled` |
| loading | not applicable on the box; a bulk action shows its own Progress | | |
| error | `surface.sunken` | `status.danger.fg` 1 px, plus helper text in `status.danger.fg` with `octagon-alert` and the words | none |
| empty | not applicable | | |
| long content | the label wraps to a second line inside forms; truncates with an ellipsis in lists | | |
| disconnected host | disabled, with the reason in the helper line | | |

## Behaviour (handoff)

- Keyboard: Space toggles. Tab moves to the next checkbox; arrows are not used for a checkbox group.
- Focus order: top to bottom in the group.
- Pointer: click on the box or the label toggles it.
- Indeterminate parent: click clears all children if any is checked; otherwise selects all.
- Resizing: the box keeps its size; the label wraps in forms and truncates in lists.
- Motion: tick draws in `duration.fast` with `ease.standard`. Reduced motion: instant.
- Accessibility: role `checkbox`, `aria-checked` true, false or `mixed`. Name is the label.
- Extensions: may use checkboxes in extension views and settings sections. Cannot restyle the box or the mark.

## GPUI notes

- A 16 px `div` with a 1 px border and a rounded corner, filled per state.
- Tick and dash are SVG paths in the mark colour.
- The focus ring is drawn around the row, not only the box, so the label is part of the target.

## React Native notes

- `Pressable` with `accessibilityRole="checkbox"` and `accessibilityState={{ checked: 'mixed' | boolean, disabled }}`.
- Row 48 dp; box 20 dp; label 14 sp / 20 / 400, font scaling allowed.
- Ripple on the row.

## Desktop vs Android

- 16 px box with hover against 20 dp box with ripple.
- Long forms on Android stack labels full width.

## Copy

- Label names the option: "Include archived projects".
- Group label names the set in capitals: "SHOW".
- Error helper: "Choose at least one option."

## Token gaps and open points

Token gaps: none.

Open points:
- The hover outline uses `accent.fg`, which may read as a selected state. Decided acceptable because it is on the outline only and the fill stays unchecked.
