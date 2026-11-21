# TitleBar

## Purpose

The strip across the top of the Frame, to the right of the Activity rail. It holds the Sidebar toggle, back and forward, the session title, the region toggles and, on Windows and Linux, the window controls.

Inventory IDs: D-SH-03 (Desktop). Part of the Frame (D-SH-01).

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Bar, one fill with the Frame; no border, no shadow | `surface.frame` | height `size.titlebar.windows` (36) or `size.titlebar.macos` (40) |
| Sidebar toggle `panel-left` | icon `text.secondary`; button 28 px | `size.control.default` (28); icon `size.icon.sm` (16) |
| Back and forward `arrow-left`, `arrow-right` | as above; disabled `text.disabled` | 28 px buttons |
| Session title | `title-sm` in `text.primary`, left aligned, truncated | flexible width |
| Region toggles, right group (see RegionToggle) | RegionToggle | 28 px |
| Window controls, Windows and Linux: minimise, maximise, close | icons `text.secondary`; close on hover `status.danger.solid` with `status.danger.on` | `size.titlebar.control.width` by `size.titlebar.windows` (46 by 36 px) each, drawn by the app |
| macOS traffic lights | native, inside the Frame | 16 px from the left edge |

Order: Sidebar toggle, back, forward, title, flexible gap (drag area), region toggles, window controls.

## Variants and sizes

- Windows and Linux: 36 px high, window controls at the right.
- macOS: 40 px high, traffic lights at the left, with an 80 px gutter when the Sidebar is hidden or narrower than the lights.
- Full page (Tasks, Calendar, Marketplace, Settings, Bot Teams pages): a Back button with the label "Back" replaces back and forward, and the page title replaces the session title. Region toggles are hidden on full pages.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | `surface.frame` | |
| Hover (button) | `fill.hover.frame`; icon `text.primary` | `duration.fast`. |
| Pressed | `fill.pressed.frame` | |
| Focus-visible | `border.focus`, 2 px, offset 2 px | Keyboard only. |
| Selected (toggle on) | see RegionToggle | |
| Disabled (back or forward with no history) | `text.disabled`; `aria-disabled` | |
| Loading | session title shows `fill.skeleton` | Only while the session loads. |
| Empty (no session) | title reads "No session open" in `text.tertiary` | Landing view. |
| Error | none on the bar | A failed region is reported in its own region. |
| Long content | title truncated with an ellipsis; full title in tooltip | Never wraps. |
| Disconnected host | session title suffix "(host unreachable)" in `status.warning.fg` with the word | Honest state, not colour alone. |

## Behaviour

- Keyboard: the bar is a toolbar with roving focus. Left and Right move between buttons; Enter or Space activates. F6 and Shift+F6 cycle the regions. Window close and minimise use the OS window keys, never `Mod+W` (pending-owner-review item 12).
- Focus order: first region after the Title bar (F6 order: Title bar, Activity rail, Sidebar, Conversation, Session summary, Work panel, Project sidebar, Status bar).
- Pointer: the empty part of the bar drags the window. Double-click follows the OS (maximise or zoom).
- Responsive: the session title takes the flexible space and truncates first. At the 800 px minimum window width, region toggles stay visible.
- Motion: none on the bar. Reduced motion: no change.
- Accessibility: landmark `banner` or `toolbar`, name "Title bar". Each button has a tooltip and an accessible name.
- Extensions: none. The Title bar is not a contribution point; extensions cannot add buttons to it in v1.

## GPUI notes

Draw the bar as one rectangle filled with `surface.frame`, with no border or shadow. Use a drag region over the empty part for window moves. Window controls are drawn by the app on Windows and Linux; on macOS, leave the native traffic lights in place and reserve the gutter. Do not draw a divider between the bar and the Sidebar.

## React Native notes

Not on Android. Its job moves to the Home top bar (`surface.frame`, `size.appbar.android`, 56 dp high) with the title, Back and overflow actions. Touch targets are 48 dp.

## Desktop and Android differences

Desktop: a frame bar with drag region, window controls and region toggles. Android: a system-styled top app bar with 48 dp actions and no window controls.

## Copy

- Tooltips: "Hide sidebar" and "Show sidebar", "Back", "Forward", "New chat", "Terminal", "Split", "Session summary", "Open in Work panel", "Project sidebar", "Minimise", "Maximise", "Close".
- Empty title: "No session open".
- Host suffix: "(host unreachable)".

## Token gaps and open points

Token gaps: none. The Android app bar is `size.appbar.android` and the window control width is `size.titlebar.control.width`.

Open points:
- Keep the Title bar inside the Frame? The pv96 build removed it and the owner decides (screen-inventory open question 14). This spec keeps the owner's frame.
- Native traffic lights on macOS: confirm the 80 px gutter on a Mac.
- The inventory does not say what the "split" toggle splits. This spec assumes the Work panel. Confirm.
