# Button

## Purpose

Runs one action in a region, a dialog, a sheet or a form. The shared text button for every surface.

Inventory IDs: none (shared primitive). Used across the inventory wherever a verb phrase is the action.

## Anatomy

- Container: height `size.control.default` (28 px); radius `radius.md`; border `border.width.default` (1 px) only in the variants that have one.
- Label: style `label` (12 / 16 / 500, desktop) or 14 / 20 / 500 (Android sp).
- Leading icon: Lucide, `size.icon.sm` (16 px), same colour as the label. A label that names a kind of content carries its icon on the left (A21); only a label no icon shows better than the word goes without.
- Trailing icon (optional): as leading. Icon to label gap `space.1` (4 px).
- Padding: `space.3` (12 px) horizontal at default; see Sizes.

## Variants and sizes

| Variant | Rest fill | Label and icon | Border | Use |
|---|---|---|---|---|
| primary | `accent.default` | `text.on-accent` | none | The one main action of a view or dialog. One per view. |
| secondary | `surface.raised` | `text.primary` | `border.strong` | Other actions next to a primary in dialogs, sheets and forms. Never in a toolbar, a placement bar or a composer. |
| ghost | none (sits on its surface) | `text.secondary` | none | Low-emphasis actions in rows, panels and toolbars. Always used for placement buttons (host, project, worktree), toolbar buttons and composer chips (A12): no border and no fill at rest, `fill.hover.<surface>` with a `text.primary` label on hover, `fill.selected.<surface>` while its menu is open, the focus ring on keyboard focus. |
| danger | `status.danger.solid` | `status.danger.on` | none | The confirming button inside a destructive Dialog (X-DLG-01): "Delete project", "Remove host". Only inside the confirm (ruling P5). |
| danger-ghost | none (sits on its surface) | `status.danger.fg` | none | A quiet trigger for a destructive or irreversible action outside a confirm: "Delete project", "Delete routine", "Remove host", "Remove account", "Delete secret", "Uninstall" in the item page Danger zone. It always opens the destructive Dialog, where the solid `danger` button confirms. At most one per view, never the view's main action, and placed last (bottom of a page or section, right end of a row). Ruling P5, 2026-11-25. |

| Size | Height | Horizontal padding | Icon | Label style |
|---|---|---|---|---|
| compact | `size.control.compact` (24) | `space.2` (8) | 16 | `label` |
| default | `size.control.default` (28) | `space.3` (12) | 16 | `label` |
| comfortable | `size.control.comfortable` (32) | `space.4` (16) | 20 | `body` (13 / 20 / 400) |

Android: default height `size.touch.default` (44 dp) with a 48 dp hit target (`size.hit.touch`); the screen's single bottom primary action uses `size.touch.large` (48 dp).

danger-ghost (ruling P5): same sizes as the other variants (desktop 28 px default, Android 44 dp with a 48 dp hit target); optional leading icon 16 px (`trash-2` to delete, `x` or `unplug` to remove); the focus ring is the standard `border.focus` ring, never red; on Android it is a text button in `status.danger.fg` with `status.danger.bg` as its ripple and pressed fill, no border. It uses only existing tokens.

Icon placement: none, leading or trailing. An icon-only button is an IconButton, never a Button with an empty label.

## States

| State | primary | secondary | ghost | danger | danger-ghost |
|---|---|---|---|---|---|
| default | `accent.default` | `surface.raised`, `border.strong` | transparent, `text.secondary` | `status.danger.solid` | transparent, label and icon `status.danger.fg` |
| hover | `accent.hover` | `fill.hover.raised` | `fill.hover.<surface>`, `text.primary` | `status.danger.hover` | fill `status.danger.bg`, label `status.danger.fg` |
| pressed | `accent.pressed` | `fill.pressed.raised` | `fill.pressed.<surface>` | `status.danger.pressed` | fill `status.danger.bg`, 1 px border `status.danger.border` |
| focus-visible | ring `border.focus` 2 px, offset `border.focus.offset` (2 px); on accent fills the inset ring uses `text.on-accent` when the ring would be clipped | same | same | same |
| selected / active | not applicable (a toggling control is a Switch or SegmentedControl) | | | |
| disabled | `fill.disabled`, label `text.disabled` | `border.disabled`, label `text.disabled` | label `text.disabled` | `fill.disabled`, label `text.disabled` | label and icon `text.disabled`, no fill, no hover |
| loading | label stays; icon slot shows a spinner (`size.icon.sm`, `duration.spinner`, `ease.linear`) after 200 ms; width fixed | same | same | same |
| empty | not applicable | | | |
| error | never shown on the button; the failure goes to the region (ErrorState, Banner or Toast). The button returns to rest. | | | |
| long content | the label truncates with an ellipsis and never wraps; the full text goes in a Tooltip | | | |
| disconnected host | disabled when the action needs an unreachable host; the reason is visible text next to the button (for example "Host unreachable. Last known state, 5 min ago") linked with `aria-describedby` | | | |

## Behaviour (handoff)

- Keyboard: Enter or Space activates. Tab and Shift+Tab move between controls in a region. A keyboard shortcut is shown in the Tooltip with Kbd, using `Mod` (Ctrl on Windows and Linux, Cmd on macOS). Never bind an OS-reserved combination (Alt+F4, Ctrl+Alt+Del, Win+L, Cmd+Q, Cmd+H, Cmd+Tab).
- Focus order: left to right, then top to bottom. In a Dialog footer the order is Cancel, then the primary or danger action; a destructive dialog starts focus on Cancel.
- Focus ring: as the focus ring rule in the design system (2 px `border.focus`, offset 2 px, radius = element radius + 2).
- Pointer: activates on release inside the control. Hover and pressed fills change in `duration.fast` with `ease.standard`.
- Resizing: content-sized (padding, icon, label). Min width is not tokenised; the label truncates before the button drops its icon.
- Motion: colour step only. Spinner keeps turning under reduced motion; the fill change becomes instant.
- Accessibility: role button. Accessible name is the label text. `aria-busy` while loading. Disabled buttons stay focusable with `aria-disabled` so a screen reader reaches the reason text.
- Extensions: may use primary, secondary, ghost and danger inside extension views. Danger only triggers a destructive confirm through the host's Dialog API. Extensions cannot set colours, sizes or new variants.

## GPUI notes

- Draw as an interactive `div` with a focus handle. Use precomputed solid colours for each state per theme; do not blend at runtime.
- `border_1` in `border.strong` for secondary; the focus ring is a 2 px border drawn outside the bounds (offset 2 px), or inset by 2 px where the parent clips.
- Radius from `radius.md`. No box shadow on buttons; shadows belong to floating layers only.
- Label is one text run with the `label` metrics, truncated at the element edge. Spinner is an SVG arc rotated over `duration.spinner`.
- Avoid gradients and opacity transitions for hover.

## React Native notes

- `Pressable` with `accessibilityRole="button"` and `accessibilityState={{ disabled, busy }}`.
- `minHeight` 44 dp (`size.touch.default`) with `hitSlop` to 48 dp. Use `minHeight`, never a fixed height, so the label can scale to 200 percent.
- Label 14 sp / 20 / 500. Leading icon 20 dp through `react-native-svg`.
- `android_ripple` colour: `fill.ripple` on secondary and ghost buttons; primary and danger buttons ripple with `accent.pressed` and `status.danger.pressed`.
- TalkBack reads the label, the role and the state (disabled, busy).

## Desktop vs Android

- Height 28 px (default) against 44 dp visual and 48 dp hit.
- Hover fill and Tooltip against ripple and no tooltip. A long-press label is not used for buttons.
- Primary action of a screen is large (48 dp) on Android, pinned to the bottom bar.

## Copy

- Sentence case verb phrases that name the object: "New routine", "Add secret", "New terminal".
- Save, Cancel, Retry and Close are the only generic verbs.
- Destructive buttons name what is lost: "Delete secret (3 projects use it)".
- No "OK", "Yes", "Submit" or exclamation marks.
- Loading words are progressive ("Saving"); never a result word before the result ("Saved").

## Token gaps and open points

Token gaps: none. `status.danger.hover`, `status.danger.pressed`, `border.disabled` and `fill.ripple` now exist.

Open points:
- Whether the Android primary action should always use 48 dp. Spec says yes for the single bottom action only.
