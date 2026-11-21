# Splitter

## Purpose

The draggable divider between two side-by-side regions (Sidebar, Conversation, Session summary, Work panel, Project sidebar). It draws as a 1 px line with a 6 px hit area, and it resizes the region on one side.

Inventory IDs: none; this is a design-system component. Android: none, because the master pane has a fixed width.

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Line at rest | `border.subtle` | `border.width.default` (1) |
| Line on hover | `border.strong` | 1 px |
| Drag line | `accent.default` | `border.width.thick` (2) |
| Hit area, centred on the line | No fill | `border.splitter.hit` (6) |
| Keyboard focus line | `border.focus`, inset | `border.focus.width` (2), inside the hit area |

## Variants and sizes

- Vertical only in v1: the regions sit side by side.
- One size. Limits come from the region's layout tokens: `layout.sidebar.min` and `layout.sidebar.max`, `layout.summary.min` and `layout.summary.max`, `layout.workpanel.min`, and `layout.project.min` and `layout.project.max`.
- The Conversation minimum (`layout.conversation.min`, 360) stops any resize that would narrow it.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | `border.subtle` | 1 px line. |
| Hover | `border.strong` | The horizontal resize cursor shows over the hit area. |
| Pressed | `accent.default`, `border.width.thick` | While dragging; the line follows the pointer. |
| Focus-visible | `border.focus` | The line becomes a 2 px focus line inside the hit area. |
| Selected or active | none | Not a selection; see Pressed. |
| Disabled | none | Not used in v1. A region at its limit still shows the line, and dragging stops at the limit. |
| Loading | none | Not applicable. |
| Empty | none | Not applicable. |
| Error | none | Not applicable. |
| Long content | none | Not applicable. |
| Writing | N/A | Not an input. |

## Behaviour

- Keyboard: a focusable separator. Arrow keys move the edge by `space.2` (8 px); Shift with an arrow key moves it by `space.6` (24 px). Home and End go to the limits (design-system 8.3).
- Focus order and ring: Tab reaches the splitter after the controls of the region on its left. The 2 px focus line shows for keyboard focus only.
- Pointer: drag resizes. Double-click resets the region to its default width. Dragging below the minimum collapses the region at the minimum less `SPLITTER_SNAP_PX` (40 px; behaviour constant, design system 4.8); dragging back out reopens it at the minimum.
- Responsive: the Conversation never narrows below 360 px. Widths are remembered per region and per window.
- Motion: no animation while dragging; the line and the region follow the pointer. Collapse and reopen use the region width animation (`duration.moderate`, `ease.standard`). Reduced motion: instant.
- Accessibility: role `separator`, vertical, with the current, minimum and maximum width in pixels. Name: "Resize" followed by the region name, for example "Resize Work panel".
- Extensions: extensions cannot use a Splitter. Extension views fill the width their host region gives them.

## GPUI notes

Draw the 1 px line inside a 6 px hit element with the horizontal resize cursor. While dragging, draw the 2 px accent line in the overlay layer so regions do not reflow each frame, and apply the new width once per frame. Do not animate the drag.

## React Native notes

Not used on Android. The master pane is a fixed 360 dp (`layout.android.master`), and touch does not drag dividers in v1.

## Desktop and Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Resizing | Drag, arrow keys, double-click reset | Fixed master width |
| Cursor | Horizontal resize on hover | Not applicable |

## Copy

- Accessible names: "Resize Sidebar", "Resize Work panel".
- No visible text and no tooltip.

## Token gaps and open points

Token gaps: none. The collapse distance is the behaviour constant `SPLITTER_SNAP_PX` (design system 4.8).

Open points:
- Home and End jump to the limits; design-system 8.3 lists the splitter keys.
- Keyboard collapse has no shortcut in v1. Confirm.
