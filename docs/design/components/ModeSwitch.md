# ModeSwitch

## Purpose

The single button in the top slot of the Activity rail. It shows the current mode (grid for Workspaces, bot for Bot Teams) and switches the Sidebar between the two modes with an icon morph.

Inventory IDs: D-SH-10 (Desktop). It changes the content of D-WS-01 (Workspaces tree) and D-WS-29 (Bot Teams sidebar).

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Button, a RailItem in toggle form | `radius.md`, fill from state | `size.rail.button` (36) |
| Icon box with two stacked outline icons: `layout-grid` (Workspaces), `bot` (Bot Teams) | `text.primary` when selected | `size.icon.md` (20) |
| Active bar on the rail's left edge | `accent.fg` | `border.width.thick` (2 px) |
| Tooltip | `surface.overlay`, `border.default`, `shadow.e2`, `radius.lg`, `body-sm` | delay `duration.tooltip-delay` |

No text label.

## Variants and sizes

One variant, two modes. The button always shows the current mode.

## States

| State | Tokens | Rule |
|---|---|---|
| Default (current mode) | fill `fill.selected.frame`; icon `text.primary`; bar `accent.fg` | Always selected, because it shows the mode. |
| Hover | keeps `fill.selected.frame`; tooltip after `duration.tooltip-delay` | No second fill on a selected item. |
| Pressed | fill `fill.pressed.frame` | While the pointer is down. |
| Focus-visible | `border.focus` ring, 2 px, offset 2 px | Keyboard only. |
| Selected or active | as default | |
| Disabled | N/A | Never disabled. |
| Loading | Sidebar rows show `fill.skeleton` | The button does not wait for the Sidebar. |
| Empty | N/A | |
| Error | no morph; Sidebar shows its own error and retry | The mode does not change. |
| Long content | N/A | |
| Disconnected host | N/A | |

## Behaviour

- Keyboard: Enter or Space switches the mode and focus stays on the button. No shortcut is assigned. Do not use `Mod+K` or any key in the app-flow keyboard table.
- Focus order: first item of the ActivityRail.
- Pointer: on desktop, from any other page the first press returns to the mode's home (pending-owner-review item 6); pressing it on the home switches the mode. On Android one press always switches. The tooltip names the target: "Switch to Bot Teams" or "Switch to Workspaces".
- Morph (desktop): `duration.moderate` (200 ms) with `ease.standard`. The outgoing icon scales 1 to `MORPH_SCALE` (0.6), rotates 0 to -`MORPH_ANGLE_DEG` (30) degrees and fades 1 to 0 over the first 60 percent. The incoming icon scales `MORPH_SCALE` to 1, rotates +`MORPH_ANGLE_DEG` to 0 degrees and fades 0 to 1 over the last 60 percent. The two overlap in the middle `MORPH_OVERLAP_PERCENT` (20) percent. The constants are behaviour constants, not tokens (design system 4.8). The Sidebar content cross-fades over `duration.fast` at the same moment. Reduced motion: instant swap of both icons and of the Sidebar.
- Responsive: fixed size.
- Accessibility: role `button`. Name: the current mode, "Workspaces mode" or "Bot Teams mode". Description: the tooltip with the target. After a switch, announce the new mode name as a polite status.
- Extensions: none. An extension cannot add or replace a mode.

## GPUI notes

Stack two SVG icons in one 20 px box. Animate opacity and scale per frame with `ease.standard`. Rotation needs an SVG transform; if the pinned GPUI version lacks it, keep the scale and cross-fade and drop the rotation (open point). Do not draw a path morph.

## React Native notes

No rail on Android. The mode control is a 48 dp icon button in the Home top bar, with the same two icons and a cross-fade only. TalkBack: role button, name as desktop, with the current mode in the label.

## Desktop and Android differences

Desktop: rail icon, morph, tooltip. Android: top-bar icon button, cross-fade, no tooltip.

## Copy

- Names: "Workspaces mode", "Bot Teams mode".
- Tooltips: "Switch to Bot Teams", "Switch to Workspaces".
- Announcement: "Bot Teams mode" or "Workspaces mode".

## Token gaps and open points

Token gaps: none. The morph scale, angle and overlap are the behaviour constants `MORPH_SCALE`, `MORPH_ANGLE_DEG` and `MORPH_OVERLAP_PERCENT` (design system 4.8); timing uses `duration.moderate` and `ease.standard`.

Open points:
- The owner reference shows a fill only. Design-system section 2.6 requires a second cue on selected items, so this spec adds the 2 px accent bar. Confirm.
- The owner said one click switches the mode. pv96 needs a second press on pages other than home (screen-inventory open question 15).
- GPUI rotation support depends on the pinned version (open question 1 spike).
