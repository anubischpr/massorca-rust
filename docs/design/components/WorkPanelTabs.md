# WorkPanelTabs

## Purpose

The tab strip of the Work panel: one tab per terminal, browser page, Markdown or file, or mobile emulator, a "+" that adds tabs, and an empty state with four actions. Files and diffs open here as file tabs.

Inventory IDs: D-SH-17 (Desktop); tab groups and split panes are D-TM-01. Android: none in v1.

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Strip | `surface.canvas`, bottom rule `border.subtle` | Height `size.control.default` (28) |
| Tab | `text.secondary` at rest; active `text.primary` on `fill.selected.canvas` | Height 28; icon `size.icon.sm` (16); label in the `label` type style; max width `layout.tab.max-width` (200) |
| Active underline | `accent.default`, `border.width.thick` | 2 px at the tab's bottom edge |
| Dirty dot | `text.secondary` | `size.indicator.dot` (6) |
| Close mark | `text.secondary`; `text.primary` on hover | 12 px glyph (`size.icon.xs`) inside a 24 px target (`size.hit.pointer`) |
| Drop indicator | `accent.default`, `border.width.thick` | 2 px line between tabs |
| Add button | IconButton with a plus glyph, `text.secondary` | 28 px (`size.control.default`) |
| Empty state | A centred column of four buttons, each as wide as its label with its icon on the left (New terminal, New browser tab, New Markdown, New mobile emulator); no panel, title or explanation (A43) | Centred in the panel |

## Variants and sizes

- Tab kinds, with Lucide icons (proposed): terminal (square-terminal), browser (globe), Markdown or file (the file-type icon of the open file, A46 to A48: generic files and Markdown a grey filled document, typed files their coloured badge; a diff tab the same icon), mobile emulator (smartphone).
- Extension tab types join the "+" menu after the four core types, with titles up to 24 characters.
- The strip scrolls sideways when tabs overflow. Labels never wrap.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | `text.secondary` | Inactive tab, no fill. |
| Hover | `fill.hover.canvas` | Shows the close mark. |
| Pressed | `fill.pressed.canvas` | While the pointer is down. |
| Focus-visible | `border.focus`, inset | Ring drawn inside the tab (offset -2). |
| Selected or active | `text.primary`, `fill.selected.canvas`, underline | The underline is the second cue. |
| Disabled | `text.disabled` | Not used in v1. |
| Loading | `text.tertiary` | The label shows at once; the body shows a Skeleton. |
| Empty | `text.secondary` | No tabs: EmptyState with the four actions. |
| Error | none on the tab | A failed body shows ErrorState; the tab keeps its colour. |
| Long content | `text.primary` | Labels truncate with an ellipsis at the max width; the full path is in the tooltip. |
| Writing | N/A | Not an input. |

## Behaviour

- Keyboard: the strip is one tab stop. Left and Right move between tabs; Enter or Space activates. Proposed, not in the app-flow keyboard table: Mod+T adds a terminal tab, Mod+Shift+B adds a browser tab, and Mod+W closes the active tab, never the window (pending-owner-review item 12).
- Focus order: the Work panel sits in the F6 cycle after Session summary.
- Pointer: click activates. The close mark closes. Drag reorders, with a drop indicator. The "+" opens a menu of the four core types, then extension types. Right-click opens the tab menu; extension items go in its trailing group, five per menu.
- Responsive: tabs keep their height. The strip scrolls sideways when tabs overflow, and the active tab scrolls into view.
- Motion: hover fills use `duration.fast` with `ease.standard`. The underline appears without animation. Reorder follows the pointer. Reduced motion: instant.
- Accessibility: role `tablist` with `tab` items. Each tab is named by its label, plus "unsaved changes" when dirty. The "+" button is named "New tab".
- Extensions: tab types only through the D-SL-08 slot, after the core types. Extensions cannot replace or hide core types.

## GPUI notes

Draw each tab as a flex child with a solid hover fill. The underline is a 2 px rectangle at the bottom edge. Draw the drop indicator as a 2 px rectangle in the overlay layer during a drag. Overflow uses a horizontal scroll container; do not fade the edges with a gradient.

## React Native notes

Not on Android in v1. If the four tab kinds become screens on Android, use a 48 dp target (`size.touch.large`) with the role "tab" and `accessibilityState` selected. Not designed yet.

## Desktop and Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Presence | Strip at the top of the Work panel | No Work panel in v1 |
| Pointer | Hover, close mark, drag to reorder | Touch; reorder not specified |
| Empty state | Four actions on the panel | Not designed |

## Copy

- Empty state labels: "New terminal", "New browser tab", "New Markdown", "New mobile emulator". No title or sentence (A43).
- Actions: "New terminal", "New browser tab", "New Markdown", "New mobile emulator".
- Tooltips: the full path for files; "Close tab" on the close mark; "New tab" on the "+".

## Token gaps and open points

Token gaps: none. The tab maximum width is `layout.tab.max-width` and the dirty dot `size.indicator.dot`.

Open points:
- Tab height is 28 px (`size.control.default`), decided; the `size.control.comfortable` usage no longer names Work panel tabs (pending-owner-review item 31).
- Overflow scrolls sideways in v1; a tab-list menu is not specified. Confirm.
- The Lucide names (square-terminal, globe, file-text, smartphone) are proposals.
