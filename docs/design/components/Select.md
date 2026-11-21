# Select

## Purpose

Chooses one value from a known list of up to about twelve options, with a trigger that shows the current value and a Menu that opens below it. For longer or searchable lists use Picker; for two to five options that should be seen at once use Radio.

Inventory IDs: none of its own. Used by D-WS-14 (base options), settings rows and extension settings sections.

## Anatomy

- Trigger: a button shaped as a field. Height `size.control.default` (28). Fill `surface.sunken`, outline `border.strong` 1 px, radius `radius.md`, padding `space.2` (8 px) horizontal.
- Value: `body`, `text.primary`. Placeholder "Choose a branch" in `text.tertiary`.
- Chevron: Lucide `chevrons-up-down`, 16 px, `text.secondary`, at the trailing edge.
- Menu: the Menu component, anchored under the trigger, `surface.overlay`, `border.default`, `radius.lg`, `shadow.e2`. Options are Menu items. The selected option shows a check (`accent.fg`) and `fill.selected.overlay`.
- Search (only above twelve options): a SearchField at the top of the Menu.

## Variants and sizes

| Variant | Use |
|---|---|
| default | single choice from a list |
| with description | options carry a second line in `body-sm` (for example agent or host) |

Sizes follow the density of the surface: 28 px default, 32 px comfortable in forms. Android: 44 dp trigger, options in a Sheet.

## States

| State | Trigger |
|---|---|
| rest | `surface.sunken`, `border.strong` |
| hover | `fill.hover.raised` fill |
| pressed | `fill.pressed.raised` fill |
| focus-visible | ring `border.focus` 2 px, offset 2 px |
| open | `border.focus` 2 px inside the trigger; chevron `accent.fg`; `aria-expanded="true"` |
| selected | the value shows in `text.primary`; the option shows a check |
| disabled | `fill.disabled`, `text.disabled`, `aria-disabled` |
| loading | the Menu shows Skeleton rows (`fill.skeleton`) instead of options; the trigger shows the last value |
| empty options | the Menu shows one line in `text.secondary` that says why, for example "No branches in this repo", and no action |
| error | `status.danger.fg` outline; helper line with `octagon-alert` and the words |
| long content | the value truncates with an ellipsis; the full option text is in the option's tooltip |
| disconnected host | options come from the last known list with "Last known state, 5 min ago"; the trigger stays usable |

## Behaviour (handoff)

- Keyboard: Enter, Space or Down arrow opens. Up and Down move the active option; Home and End jump to the ends; a letter jumps to the next option starting with it. Enter selects and closes; Escape closes and returns focus to the trigger.
- Focus order: the trigger takes the place of a field in its form.
- Pointer: click opens; click on an option selects and closes; click outside closes without changing the value.
- Resizing: the trigger fills its container; the Menu is as wide as the trigger or the longest option, within `layout.menu.min-width` (180) and `layout.menu.max-width` (280).
- Motion: Menu opens with opacity and a 4 px move, `duration.base`, `ease.enter`; closes with `ease.exit`. Reduced motion: instant.
- Accessibility: role `combobox` (select-only) with `aria-expanded`, `aria-haspopup="listbox"`; options are `option` with `aria-selected`. Name from the field label.
- Extensions: may supply options (up to 40, labels up to 40 characters). Cannot change the trigger or the Menu styling.

## GPUI notes

- Trigger is a `div` with focus handling; the Menu is an overlay element on the overlay layer, anchored to the trigger bounds.
- Typeahead uses the element's key handler; no timers beyond the reset window.

## React Native notes

- Trigger is a `Pressable` with `accessibilityRole="combobox"`. Options open in a Sheet (see Sheet) with radio-style rows at 48 dp.
- Value text uses 16 sp so it is readable without zoom; scales to 200 percent.

## Desktop vs Android

- Anchored Menu on desktop; bottom Sheet on Android.
- Search inside the Menu on desktop above twelve options; inside the Sheet on Android.

## Copy

- Placeholder names the choice: "Choose a branch", "Choose a host".
- Empty options say why: "No branches in this repo".
- Option labels use the names the user knows, not the internal id.

## Token gaps and open points

- No gaps: the chevron uses `size.icon.sm` and the trigger hover uses `fill.hover.raised`.
- Open: the twelve-option threshold is a proposed rule, not a token. Confirm with the owner when the lists are known.
