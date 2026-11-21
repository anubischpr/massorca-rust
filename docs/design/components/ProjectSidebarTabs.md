# ProjectSidebarTabs

## Purpose

The icon-only tab strip at the top of the Project sidebar. It holds the four core tabs (Files, Sessions, Source control, Checks) and any extension tabs after them. Each tab's label is its tooltip and accessible name.

Inventory IDs: D-SH-04 (Desktop); extension tabs are slots D-SL-01. Android: none in v1.

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Strip | `surface.canvas`, bottom rule `border.subtle` | Height `size.control.default` (28) |
| Tab | Icon in `text.secondary`; active `text.primary` on `fill.selected.canvas` | 28 by 28 px; icon `size.icon.sm` (16) |
| Active underline | `accent.default`, `border.width.thick` | 2 px at the bottom edge |
| Count badge (optional) | Badge spec; `status.danger.solid` with `status.danger.on` for failing runs | Top right of the icon |
| Tooltip | Tooltip spec | Label text |

Icons (Lucide, proposed): Files folder-tree, Sessions history, Source control git-branch, Checks badge-check. Extension tabs use the extension's own icon.

## Variants and sizes

- One size, icon only, at every width; each tab stays 28 px.
- Four core tabs in fixed order, then extension tabs. Each extension may add up to two (design system 7.2).
- The strip does not scroll: the four core tabs and two extension tabs fit within the 240 px minimum width of the Project sidebar (six tabs of 28 px).
- Count badges are a proposal: Source control shows changed files; Checks shows failing runs.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | `text.secondary` | Icon only on the strip. |
| Hover | `fill.hover.canvas`, `text.primary` | The canvas fill, not the frame fill. |
| Pressed | `fill.pressed.canvas` | While the pointer is down. |
| Focus-visible | `border.focus`, inset | Ring drawn inside the 28 px tab. |
| Selected or active | `text.primary`, `fill.selected.canvas`, underline | Two cues: the fill and the underline. |
| Disabled | `text.disabled` | Not used in v1. |
| Loading | none | Extension tabs appear when their extension loads; core tabs never load. |
| Empty | none | A tab with nothing to show uses its EmptyState in the panel, not on the strip. |
| Error | none on the strip | A failed extension view shows ErrorState in the panel. |
| Long content | none | Icon only; extension labels are capped at 24 characters. |
| Writing | N/A | Not an input. |

## Behaviour

- Keyboard: the strip is one tab stop. Left and Right move between tabs; Enter or Space selects. F6 reaches the Project sidebar.
- Focus order and ring: after the Work panel in the F6 cycle; the strip comes before the panel content. The ring is inset.
- Pointer: click selects. Hover shows the tooltip after 400 ms (`duration.tooltip-delay`); keyboard focus shows it at once.
- Responsive: tab size and order stay fixed at every Project sidebar width, 240 to 480 px.
- Motion: fill and underline change with `duration.fast` and `ease.standard`. Reduced motion: instant.
- Accessibility: role `tablist`, named "Project sidebar". Each tab is a `tab` named by its label. Any count joins the name, for example "Checks, 2 failing".
- Extensions: up to two tabs per extension, each with an icon (a Lucide name or a single-ink SVG), a label of 24 characters and an optional count badge. Extensions cannot reorder tabs before the core tabs, hide them, or change the region width.

## GPUI notes

Draw each tab as a 28 px square with a solid hover fill. The underline is a 2 px rectangle at the bottom edge. Icons are 16 px SVG paths with a 1.5 px absolute stroke. Draw tooltips in the overlay layer.

## React Native notes

Not on Android. Source control and checks are reached from item detail screens; that flow is not designed in this spec.

## Desktop and Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Presence | Icon strip in the Project sidebar | None in v1 |
| Labels | Tooltip and accessible name | Not applicable |

## Copy

- Labels: "Files", "Sessions", "Source control", "Checks".
- Tooltips repeat the label. Badge names read "Checks, 2 failing runs".
- No other copy; the views carry their own text.

## Token gaps and open points

Token gaps: none. Every value uses an existing token.

Open points:
- The Lucide names folder-tree, history, git-branch and badge-check are proposals. Confirm.
- The owner note says up to two extension tabs; the contribution rule says two per extension. This spec follows the rule. Confirm the total cap.
- The count badges on Source control and Checks are not in the owner notes. Confirm.
