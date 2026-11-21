# RailItem

## Purpose

One 36 px button in the Activity rail. It shows one icon, an optional badge and an active bar, and it carries a tooltip with its name and shortcut. It is used for navigation entries, the mode switch, action buttons and extension icons.

Inventory IDs: D-SH-11 to D-SH-16 and D-SL-09 (Desktop). The mode switch is D-SH-10 (ModeSwitch).

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Button | `radius.md`, no border, fill from state | `size.rail.button` (36) |
| Icon | `text.secondary` at rest; `text.primary` on hover and active | `size.icon.md` (20), Lucide outline |
| Active bar | `accent.fg`, on the rail's left edge | `border.width.thick` (2 px), inset `space.2` at top and bottom |
| Badge | see Variants | height `size.badge.height` (16), at the top-right corner |
| Tooltip | `surface.overlay`, `border.default`, `shadow.e2`, `radius.lg`, `body-sm` in `text.primary` | to the right of the rail, delay `duration.tooltip-delay` |

## Variants and sizes

- Navigation: opens a page or view (Tasks, Calendar, Marketplace, an extension icon).
- Toggle: the mode switch (see ModeSwitch). Its pressed state is the selected style.
- Action: runs a command in place (theme toggle, Settings).
- Badge, extension only: none; dot (`size.indicator.dot`, 6 px); count from 1, shown as `99+` above 99. Kinds:
  - Neutral: `badge.neutral.bg` with `badge.neutral.fg`.
  - Accent: `accent.default` with `text.on-accent`.
  - Danger: `status.danger.solid` with `status.danger.on`, and the `octagon-alert` glyph when the count is not shown.
- One size. Density does not change rail items.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | icon `text.secondary`; no fill | Rest state. |
| Hover | fill `fill.hover.frame`; icon `text.primary` | `duration.fast`, `ease.standard`. An active item keeps `fill.selected.frame` on hover. |
| Pressed | fill `fill.pressed.frame` | While the pointer is down. |
| Focus-visible | `border.focus` ring, 2 px, offset 2 px, radius `radius.lg` | Keyboard focus only. |
| Selected or active | fill `fill.selected.frame`; bar `accent.fg`; icon `accent.fg` | The bar is the second cue besides the fill. |
| Disabled | icon `text.disabled`; no fill; `aria-disabled` | Not used in v1. |
| Loading | none | Extension icons appear when loaded. |
| Empty | no badge | No badge is shown rather than a zero. |
| Error | badge kind danger | Glyph `octagon-alert` plus the word "Needs attention" in the tooltip. |
| Long content | label cut at 32 characters with an ellipsis | Full label in the tooltip. |
| Disconnected host | N/A | Not used on items. |

## Behaviour

- Keyboard: Enter or Space activates. Focus moves with the rail's roving focus (Up and Down). Settings is `Mod+,`. `Mod` is Ctrl on Windows and Linux and Cmd on macOS. No item takes an OS-reserved key.
- Focus order and ring: as ActivityRail. Keyboard focus shows the tooltip at once; pointer hover waits `duration.tooltip-delay`.
- Pointer: click activates. Hover shows the tooltip.
- Responsive: fixed 36 px at every window width.
- Motion: fill changes use `duration.fast` with `ease.standard`. The active bar appears without animation. Reduced motion: instant.
- Accessibility: role `button`. The accessible name is the label, with the badge in words, for example "Calendar, 3 new". The mode switch changes its name to the current mode. The tooltip is the description.
- Extensions (D-SL-09): own icon (a Lucide name, or a single-ink 24-unit SVG of 8 KB at most), a label of 32 characters, and an optional badge: a count or a dot, kind neutral, accent or danger. An extension cannot set a badge colour, restyle the item, animate it or take a core position.

## GPUI notes

Draw the button as a 36 px rounded rectangle with a precomputed solid fill for each state. Draw the badge as a small rounded rectangle in the overlay layer so the rail does not clip it. Draw the active bar as a 2 px rectangle on the rail's left edge. Icons are SVG paths at 20 px with an absolute stroke of 1.75 px. Do not add shadows to items.

## React Native notes

Not on Android. Where an entry moves (see ActivityRail), it becomes a 48 dp touch target (`size.touch.large`) with `accessibilityRole` "button" and the label as its TalkBack name. Use `accessibilityState` selected for the active entry. Ripple colour: `fill.ripple`, the only translucent state fill (pending-owner-review item 33).

## Desktop and Android differences

Desktop has a hover tooltip, an active bar and keyboard roving focus. Android has no rail item: the same entries are top-bar and overflow actions, with no hover or tooltip, and a long press shows the label.

## Copy

- Labels: "Tasks", "Calendar", "Marketplace", "Settings", "Switch to light theme", "Switch to dark theme".
- Mode switch tooltip: "Switch to Bot Teams" or "Switch to Workspaces".
- Badge text: "3", "99+". Danger tooltip: "Needs attention".
- Extension tooltip: the extension name, a colon, then the label.

## Token gaps and open points

Token gaps: none. The dot is `size.indicator.dot`, the badge height `size.badge.height`, and the neutral badge `badge.neutral.bg` with `badge.neutral.fg`.

Open points:
- Design-system section 4.6 says active icons use `accent.fg`. This spec keeps the icon at `text.primary`, so the item has one accent cue (the bar). Confirm.
- Badges on core entries (Tasks, Calendar) are not defined. Only extension badges are specified.
