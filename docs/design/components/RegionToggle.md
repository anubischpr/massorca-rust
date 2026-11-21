# RegionToggle

## Purpose

The icon buttons on the right of the Title bar that show or hide the Canvas regions and open the session tools. Each one is a 28 px icon button with a tooltip, and the toggles that switch a region on or off report their state.

Inventory IDs: D-SH-03 (Desktop). Region toggles act on D-SH-04 (Project sidebar), D-AG-27 (Session summary), D-SH-17 (Work panel) and D-TM-01 (terminal).

## Anatomy

| Toggle | Lucide icon (proposed) | Action | Kind |
|---|---|---|---|
| New chat | `square-pen` | Starts a new session | action |
| Terminal | `terminal` | Shows or hides the terminal tab in the Work panel | toggle |
| Split | `columns-2` | Splits the Work panel (see open points) | toggle |
| Checklist | `list-checks` | Shows or hides the Session summary | toggle |
| Open in Work panel | `square-arrow-out-up-right` | Opens the current file or diff in the Work panel | action |
| Project sidebar | `panel-right` | Shows or hides the Project sidebar | toggle |

Button: 28 px (`size.control.default`), `radius.md`, icon `size.icon.sm` (16). Groups are separated by `space.1`; the right group ends at `space.2` from the window controls.

## Variants and sizes

- One size. Density does not change the title bar.
- Toggle (`aria-pressed`) or action (no pressed state).

## States

| State | Tokens | Rule |
|---|---|---|
| Off | icon `text.secondary`; no fill | |
| On | fill `fill.selected.frame`; icon `accent.fg` | The fill and the icon colour both change; `aria-pressed` carries the state. |
| Hover | fill `fill.hover.frame`; icon `text.primary` | `duration.fast`. |
| Pressed | fill `fill.pressed.frame` | |
| Focus-visible | `border.focus`, 2 px, offset 2 px | |
| Disabled | icon `text.disabled`; `aria-disabled` | For example Project sidebar on full pages, where the region is hidden instead. |
| Loading | none | |
| Empty | none | |
| Error | none on the button | A failed region reports its own error. |
| Long content | N/A | |
| Disconnected host | N/A | |

## Behaviour

- Keyboard: part of the Title bar toolbar (roving focus). Enter or Space activates. There is no default shortcut. Every toggle is reachable from the command palette (`Mod+K`).
- Focus order: after the Title bar's session title, left to right.
- Pointer: click toggles. Tooltip after `duration.tooltip-delay`, with the name and the shortcut when one exists.
- Responsive: toggles stay visible down to the 800 px window minimum. Region overlays open when a region is hidden for width (screen-inventory section 5.1).
- Motion: the region change animates with the region (`duration.moderate`, `ease.standard`). The button itself uses `duration.fast`. Reduced motion: instant.
- Accessibility: role `button` with `aria-pressed` for toggles. Names: "Toggle terminal", "Toggle Session summary", "Toggle Project sidebar", "New chat", "Open in Work panel", "Split Work panel".
- Extensions: not a contribution point. Extensions do not add title-bar toggles in v1.

## GPUI notes

Draw each button as a 28 px rounded rectangle with precomputed solid fills. Icons are SVG at 16 px with an absolute stroke of 1.5 px (4.6). Show the tooltip in the overlay layer. Do not put a shadow on the button.

## React Native notes

Not on Android. The equivalents are the overflow menu and the top bar (see TitleBar). Touch targets are 48 dp.

## Desktop and Android differences

Desktop: toggles in the title bar, with a tooltip and a pressed state. Android: none in the bar; the same actions are in the overflow menu.

## Copy

- Tooltips: "New chat", "Terminal", "Split", "Session summary", "Open in Work panel", "Project sidebar".
- Pressed state text for screen readers: "pressed" or "not pressed".

## Token gaps and open points

Token gaps: none. Every value uses an existing token.

Open points:
- The inventory does not say what the "split" toggle splits. This spec assumes the Work panel. Confirm.
- The owner reference shows a "checklist" icon. This spec maps it to the Session summary toggle, which the foundations name. Confirm.
- "Open in Work panel" is an action, not a toggle. Confirm the behaviour when no file is open (the button disables).
- The owner's Bot Teams and Calendar references show a floating panel button at the bottom right of the Canvas. The inventory does not name it. Confirm whether it is a second region toggle.
