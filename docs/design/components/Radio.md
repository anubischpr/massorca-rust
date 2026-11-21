# Radio

## Purpose

Chooses exactly one option from a short visible set (two to five), where all options should be seen at once. Above five options, or for a long or searchable set, use Select or Picker.

Inventory IDs: none of its own. Used in forms and settings (for example a single agent choice in a setup form, D-AG-13).

## Anatomy

- Circle: 16 by 16 px (`size.icon.sm`), radius `radius.full`, border `border.width.default` (1 px).
- Dot: 8 px circle centred inside the selected circle, `accent.default`.
- Label: `body`, `text.primary`, `space.2` (8 px) to the right. Optional description in `body-sm`, `text.secondary`, below the label.
- Hit area: row at least 24 px high on desktop; 48 dp on Android.

## Variants and sizes

| Variant | Use |
|---|---|
| default | label only |
| with description | a label plus one line of description, for option choices that need an explanation |

One size on desktop. Android: circle 20 dp inside a 48 dp row.

## States

| State | Circle fill | Outline | Dot |
|---|---|---|---|
| unselected, rest | `surface.sunken` | `border.strong` | none |
| unselected, hover | `fill.hover.raised` | `accent.fg` | none |
| selected, rest | `surface.sunken` | `accent.default` | `accent.default` |
| selected, hover | `fill.hover.raised` | `accent.hover` | `accent.hover` |
| focus-visible | ring `border.focus`, 2 px, offset 2 px around the row | | |
| disabled | `fill.disabled` | `border.disabled` | `text.disabled` |
| loading | not applicable; a choice that loads data shows the content's Skeleton | | |
| error | `surface.sunken`, outline `status.danger.fg`, helper text in `status.danger.fg` with `octagon-alert` and the words | | |
| empty | not applicable | | |
| long content | label wraps inside forms; truncates in lists | | |
| disconnected host | disabled with the reason as helper text | | |

## Behaviour (handoff)

- Keyboard: the group is one tab stop. Arrow keys move focus and select the next or previous option (selection follows focus). Tab leaves the group.
- Focus order: top to bottom, or left to right for a horizontal group.
- Pointer: click on the circle or the label selects.
- Resizing: circle keeps its size; labels wrap in forms.
- Motion: dot appears in `duration.fast` with `ease.standard`. Reduced motion: instant.
- Accessibility: role `radio` inside `radiogroup`, `aria-checked`. The group has a visible name.
- Extensions: may use radios in extension views (allowed). Cannot change the dot colour.

## GPUI notes

- Circle as a rounded `div` with border; dot as a child circle in the accent colour.
- Keyboard handled by the group element.

## React Native notes

- `Pressable` with `accessibilityRole="radio"` and `accessibilityState={{ checked, disabled }}`, inside a container with `accessibilityRole="radiogroup"`.
- Row 48 dp; circle 20 dp; label 14 sp / 20 / 400.

## Desktop vs Android

- Desktop radios sit in forms and settings; Android uses the same radios in a full-width row list, or a Sheet with radio rows for sets of more than three.

## Copy

- Label names the option: "Local", "VPS".
- Group label is a noun: "Run on".
- Do not start labels with "Use" or "Choose".

## Token gaps and open points

Token gaps: none.

Open points:
- The hover outline uses `accent.fg`; same decision as Checkbox.
