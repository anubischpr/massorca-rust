# FrameCanvas

## Purpose

The window structure of the desktop app: the Frame (Activity rail, Sidebar and Title bar as one surface) and the inset Canvas that holds the regions and the Status bar. On Android it is the adaptive layout for compact, medium and expanded windows.

Inventory IDs: D-SH-01 (Desktop). Android counterpart: M-SH-01 (compact stack; master-detail from 600 dp).

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Frame: Activity rail, Sidebar and Title bar, one fill with no seam | `surface.frame`, no border, no shadow | Rail `size.rail.width` (48); Sidebar `layout.sidebar.default` (256); Title bar `size.titlebar.windows` (36) or `size.titlebar.macos` (40) |
| Canvas, inset in the Frame | `surface.canvas`, `radius.canvas` on the top-left corner only | Fills the rest; the other corners meet the window edge |
| Regions on the Canvas | `surface.canvas`; splitters `border.splitter.hit` | Conversation min `layout.conversation.min` (360); Session summary `layout.summary.default` (288); Work panel `layout.workpanel.default` (480, min 320); Project sidebar `layout.project.default` (288) |
| Status bar, under the Canvas column | `surface.canvas`, caption text | `size.statusbar` (24) |
| Window | none | Minimum `layout.window.min-width` (800) by `layout.window.min-height` (520) |
| Layers | `z.base` 0, `z.sticky` 10, `z.region-overlay` 30, `z.popover` 40, `z.dialog` 50, `z.toast` 60 | Stacking order only |

## Variants and sizes

- Desktop: any region can be open or hidden. With the Sidebar hidden, the Canvas corner sits next to the rail.
- Narrow desktop: regions give way in this order. Session summary below 1408 px, Project sidebar below 1168, Sidebar below 928, Work panel below 728. These are the steps drawn on artboard `ws-narrow` (design system 5.1). A hidden region opens as an overlay.
- Android compact (under 600 dp): one pane, a 56 dp top app bar (`size.appbar.android`) on `surface.frame`, square content on `surface.canvas`.
- Android medium (600 to 839 dp) and expanded (840 dp and up): master pane 360 dp (`layout.android.master`) on `surface.frame`; detail on `surface.canvas` with the rounded corner. Expanded caps the detail column at 720 dp.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | `surface.frame`, `surface.canvas` | No seam, border or shadow between the Frame and the Canvas. |
| Hover | none | The Frame and Canvas have no hover; controls inside regions do. |
| Pressed | none | Not interactive. |
| Focus-visible | `border.focus` | The Frame takes no ring. F6 moves focus into a region, and its first control shows the ring. |
| Selected or active | none | Not a selection; open regions are remembered per window. |
| Disabled | none | Not used. |
| Loading | `fill.skeleton` | The Frame paints at once; each region shows its own Skeleton. |
| Empty | `surface.canvas` | No active workspace: the Canvas shows the start page, with no region chrome. |
| Error | per ErrorState | A failed region shows ErrorState in its own area; other regions stay usable. |
| Long content | `text.primary` | Region titles truncate with an ellipsis. The window never scrolls sideways. |
| Overlay open | `surface.overlay`, `border.default`, `shadow.e2` | A hidden region opened over the Canvas. Closes on Escape, a click outside or focus leaving it. |
| Dialog open | `scrim`, `shadow.e3` | The scrim covers the whole window, Frame included, under `z.dialog`. |
| Writing | N/A | Not an input. |

## Behaviour

- Keyboard: F6 and Shift+F6 cycle the regions in this order: Title bar, Activity rail, Sidebar, Conversation, Session summary, Work panel, Project sidebar, Status bar. Hidden regions are skipped; an open overlay joins the order. Escape closes the topmost overlay.
- Focus order and ring: Tab and Shift+Tab move inside a region. When an overlay closes, focus returns to the control that opened it.
- Pointer: Title bar toggles show or hide a region. Splitters resize neighbouring regions (see Splitter). Double-click on the empty Title bar follows the OS.
- Responsive: regions collapse in the order under Variants and remember their widths per window. The Conversation never drops below 360 px.
- Motion: a region's width changes over `duration.moderate` with `ease.standard`; inner content keeps its width. Reduced motion: `duration.instant`.
- Accessibility: landmarks named "Activity rail", "Sidebar", "Title bar", "Conversation", "Session summary", "Work panel", "Project sidebar" and "Status bar".
- Extensions: extensions cannot use FrameCanvas or change the Frame, the Title bar or the Canvas. They add content only at contribution points: activity rail items, Sidebar sections, Work panel tab types, Project sidebar tabs, status bar items and full-canvas pages.

## GPUI notes

Paint the window with `surface.frame`, and the Canvas as one rectangle with a per-corner radius (top-left `radius.canvas`, others 0). Lay regions out as flex columns. Draw overlays, menus and toasts in the overlay layer. No border, shadow, blur or gradient.

## React Native notes

Compact: a stack under a 56 dp top app bar (`size.appbar.android`) on `surface.frame`, below the status bar inset. Medium and expanded: master pane 360 dp on `surface.frame`, detail on `surface.canvas` with `radius.canvas`. Draw edge to edge and apply safe-area insets. Keep `space.4` (16 dp) side padding. Back follows the system gesture.

## Desktop and Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Structure | Frame with rail, Sidebar and Title bar; Canvas of regions | Stack, then master-detail |
| Narrow windows | Overlays in collapse order | Screen-by-screen navigation |
| Chrome | Custom Title bar with region toggles; Status bar | System status bar; 56 dp top app bar |

## Copy

- Region names as listed under Behaviour. The Frame has no visible labels.
- Title bar Back button on full pages: "Back".
- Overlay regions have no label.

## Token gaps and open points

Token gaps: none. The Android top app bar height is `size.appbar.android`.

Open points:
- An open overlay region gets no scrim, so the Canvas stays lit. Overlays are not modal. Confirm.
- The owner reference runs the Sidebar footer (help, locate, panel icons) to the window bottom, while the Status bar stays under the Canvas. Confirm the footer belongs to SidebarTree.
