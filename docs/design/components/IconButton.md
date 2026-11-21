# IconButton

## Purpose

A square button that shows only an icon, for toolbars, rows, panel headers, region toggles and composer actions. Every IconButton has an accessible name, which is its tooltip text. A build fails without one.

Inventory IDs: none of its own. Used by D-SH-03 (title bar region toggles), D-SH-17 (add tab), D-WS-02 (row menu), D-SH-06 (retry), and the composer actions of the Composer.

## Anatomy

- Container: square, `size.control.default` (28 px). Radius `radius.md`. No padding; the icon is centred.
- Icon: Lucide outline, `size.icon.sm` (16 px) at compact and default, `size.icon.md` (20 px) at comfortable. Stroke per the iconography rule (1.5 px at 16).
- Hit area: at least 24 px on desktop (`size.hit.pointer`). On Android the visual is 44 dp (`size.touch.default`) inside a 48 dp hit target (`size.hit.touch`).
- Tooltip: attached on hover and on keyboard focus (see Tooltip). The tooltip text is the accessible name.
- Optional status dot: 6 px (`size.indicator.dot`), top right, filled with a status token and always paired with the state word in the tooltip.

## Variants and sizes

| Variant | Rest fill | Icon | Border | Use |
|---|---|---|---|---|
| ghost (default) | none | `text.secondary` | none | Toolbars, row actions, region toggles, header actions, composer buttons, Title bar buttons (no fill of their own, A11). Never outlined in these places (A12). |
| outline | `surface.raised` | `text.secondary` | `border.strong` | An icon action next to a field or a card header |
| toggle (selected) | `fill.selected.<surface>` | `accent.fg` | none | An on or off control shown as an icon. Sets `aria-pressed` |

| Size | Square | Icon |
|---|---|---|
| compact | `size.control.compact` (24) | 16 |
| default | `size.control.default` (28) | 16 |
| comfortable | `size.control.comfortable` (32) | 20 |

Android: 44 dp visual, 48 dp hit, 24 dp icon in toolbars.

## States

| State | Desktop rendering |
|---|---|
| default | ghost: icon `text.secondary` on its surface. Outline: `border.strong` outline. |
| hover | `fill.hover.<surface>` (`canvas`, `frame`, `raised` or `overlay`), icon `text.primary` |
| pressed | `fill.pressed.<surface>` |
| focus-visible | ring `border.focus`, 2 px, offset 2 px. Inset by 2 px where a scroll area or the Frame edge would clip it. |
| selected / active | toggle: `fill.selected.<surface>` and icon `accent.fg`; `aria-pressed="true"`. The icon colour and the fill both change, so it is not colour alone. |
| disabled | icon `text.disabled`; no fill; `aria-disabled="true"`. The tooltip stays and gives the reason. |
| loading | spinner (`size.icon.sm`, `duration.spinner`, `ease.linear`) replaces the icon after 200 ms |
| empty | not applicable |
| error | not applicable on the button. A status icon may use `status.danger.fg` only when the icon itself reports the failure, and then also carries its word in the tooltip. |
| long content | not applicable. The tooltip wraps up to 240 px wide. |
| disconnected host | disabled; the tooltip says "Host unreachable. Last known state, 5 min ago." |

## Behaviour (handoff)

- Keyboard: Enter or Space activates. In a toolbar or a row group the whole group is one tab stop with roving focus: Left and Right (or Up and Down in a vertical toolbar) move between icon buttons, Home and End jump to the ends.
- Focus order: left to right in the toolbar. Overflow ("More actions") is last.
- Pointer: activates on release. Tooltip shows after `duration.tooltip-delay` (400 ms) on hover and at once on keyboard focus. Moving from one icon button to the next shows the next tooltip with no delay.
- Resizing: fixed square. In a narrow toolbar, the lowest-priority icon buttons move into a More overflow button, in the order the spec lists them.
- Motion: fill colour step in `duration.fast` with `ease.standard`; spinner `duration.spinner`. Reduced motion: fills instant, spinner still turns.
- Accessibility: role `button`; toggles use `aria-pressed`. Name is the tooltip text. Decorative icons are `aria-hidden`. The icon is never the only cue for a state: the toggle also changes its fill, and the tooltip carries the word.
- Extensions: may add IconButtons in the composer actions, the Work panel `+` menu and extension toolbars, using a Lucide name or a single-ink 24-unit SVG. They cannot use core status glyphs as their icon, cannot set colour, and their tooltip must start with the extension's name (identity rule).

## GPUI notes

- Interactive `div`, square, with a focus handle. Icon is an SVG path tinted with the state colour (precomputed per theme).
- Tooltip is an overlay element on the overlay layer, not a child of the button, so it is never clipped by the toolbar.
- Roving focus is owned by the toolbar element; the button only reports its focus state.
- No shadow and no opacity transition on the button.

## React Native notes

- `Pressable` with `accessibilityRole="button"`; toggles use `accessibilityRole="togglebutton"` with `accessibilityState.checked`.
- `accessibilityLabel` is the name. Hit area 48 dp through `hitSlop`.
- No hover on Android. The name is the accessibility label only. No long-press popup: an icon button is a button, so the Button rule applies.
- Icon through `react-native-svg`, 24 dp in toolbars.

## Desktop vs Android

- 28 px square with hover and tooltip against a 48 dp hit target with long-press name.
- Overflow menu (More) opens an anchored Menu on desktop and a Sheet on Android.

## Copy

- Tooltip is a verb phrase, the same words as the matching menu item: "New terminal", "Close tab", "Copy path", "More actions".
- Toggle names state the on state as a noun or verb: "Show project sidebar", "Hide project sidebar".
- No names that repeat the icon shape ("Plus icon").

## Token gaps and open points

Token gaps: none beyond Button, which now has the danger hover and pressed tokens.

Open points:
- Region toggles in the Title bar use the toggle variant with `fill.selected.frame`. Whether they also need a 2 px accent bar was not decided; the spec relies on the fill plus `accent.fg` and `aria-pressed`.
