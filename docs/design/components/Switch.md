# Switch

## Purpose

A binary on or off setting that applies at once, shown as a track with a sliding knob. Use it for preferences that take effect when toggled. For a choice that needs a confirm, use a Button in a Dialog.

Inventory IDs: none of its own. Used in Settings rows (D-ST-01 to D-ST-11, SettingsRow) and extension settings sections.

## Anatomy

- Track: `size.switch.track.width` by `size.switch.track.height` (32 by 18 px on desktop), radius `radius.full`, border `border.width.default` (1 px) when off.
- Knob: `size.switch.knob` (12 px) circle, radius `radius.full`, inset 3 px, slides from one end to the other.
- Label: style `body` (13 / 20 / 400), colour `text.primary`, on the left of the control in a Settings row. Supporting line `body-sm` in `text.secondary` below it when needed.
- Row: height `size.row.settings` (40) on desktop, the minimum for a label alone (4.2); a Settings row uses its own padding.

## Variants and sizes

| Variant | Use |
|---|---|
| default | Settings and inline preferences |
| with state text | The state word ("On", "Off") is shown only when the setting has a host or sync state; the switch itself never carries the word |

One size on desktop. Android: the track is `size.switch.touch.track.width` by `size.switch.touch.track.height` (36 by 20 dp) with a `size.switch.touch.knob` (14 dp) knob, inside a 48 dp row.

## States

| State | Track | Knob |
|---|---|---|
| off, rest | `surface.sunken` fill, `border.strong` outline | `switch.knob.off` |
| off, hover | `fill.hover.raised` fill, `border.strong` outline | `text.primary` |
| on, rest | `accent.default` | `text.on-accent` |
| on, hover | `accent.hover` | `text.on-accent` |
| on, pressed | `accent.pressed` | `text.on-accent` |
| focus-visible | ring `border.focus`, 2 px, offset 2 px, around the track | unchanged |
| disabled | `fill.disabled` with `border.disabled` outline; label `text.disabled` | `text.disabled` |
| loading | track keeps its state; the knob is replaced by a spinner (`size.icon.xs`, `duration.spinner`) while the change is saved on the host | spinner |
| error | the switch returns to its previous state; a helper line below in `status.danger.fg` with the glyph `octagon-alert` and the words | unchanged |
| empty | not applicable | |
| long content | the label truncates with an ellipsis; the full text goes in the row's tooltip | |
| disconnected host | disabled; the helper line says "Host unreachable. Last known state, 5 min ago" | |

## Behaviour (handoff)

- Keyboard: Space toggles. Tab and Shift+Tab move between controls. Enter does nothing on a switch.
- Focus order: the row's order in the Settings page.
- Pointer: click or tap on the track or the label (the whole row is the hit area on Android; on desktop the label also toggles).
- Resizing: the switch keeps its size; the label truncates first.
- Motion: knob slides and track colour changes in `duration.fast` with `ease.standard`. Reduced motion: instant.
- Accessibility: role `switch`, `aria-checked` true or false. Name is the label. A state change is announced by the checked state, not by a live region.
- Extensions: may add switches in an extension settings section (toggle row). Cannot change track size, colours or motion, and cannot apply a change to another extension's setting.

## GPUI notes

- Track is a rounded `div` with a 1 px border when off; knob is a circular `div` positioned by an animated offset over `duration.fast`.
- Colour changes are precomputed solid fills; no shadow on the knob.
- Spinner in the knob is an SVG arc.

## React Native notes

- `Switch` built on `Pressable` with `accessibilityRole="switch"` and `accessibilityState={{ checked, disabled, busy }}`.
- Row `minHeight` 48 dp. Track 36 by 20 dp (`size.switch.touch.*`). Knob uses `Animated` over `duration.fast`; respect "Remove animations".
- Label 14 sp / 20 / 400.

## Desktop vs Android

- Desktop switch at the right edge of a Settings row. Android row at 48 dp, switch at the end with a 48 dp hit area.
- Hover fills on desktop; ripple on Android.

## Copy

- Label is the noun or the on-state phrase: "Show hidden files", "Run on login".
- Helper text says the consequence when it is not obvious: "Applies to new terminals".
- Error helper: "Could not save. Setting went back to off."
- No "Enable" or "Disable" pairs that flip the meaning of the switch.

## Token gaps and open points

Token gaps: none. Track and knob sizes are `size.switch.*`; the off knob is `switch.knob.off` and the disabled outline `border.disabled`.

Open points:
- Whether a Settings page with a single switch should show the state word as well. Decided no.
- The Android knob (14 dp) is derived from the 3 dp inset on a 20 dp track; the earlier spec gave only the track.
