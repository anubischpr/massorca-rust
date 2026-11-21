# Browser chrome

## Purpose

The toolbar of a Browser tab in the Work panel: back, forward, reload, the address bar, find, grab and annotate, and a few page actions. The page itself is drawn by the embedded web engine and is not designed here. The chrome keeps navigation honest about where a page lives (this PC or a remote host) and who is driving it (the user or an agent).

Inventory ID: D-BP-01 (browser pane). Related: D-BP-02 (grab and annotate), D-BP-05 (agent browser access), D-FD-01 (find bar). Tabs are Work panel tabs (D-SH-17), not part of this chrome.

## Anatomy

- Toolbar: height `size.toolbar` (36 px; controls `size.control.default` with `space.1` above and below), `surface.canvas`, a `border.subtle` rule below. Items, left to right, `space.1` apart:
  - Back, Forward (arrow-left, arrow-right icon buttons).
  - Reload (refresh-cw), which becomes Stop (x) while the page loads.
  - Address bar (flexible, minimum 160 px): height `size.control.default`, `radius.md`, `surface.sunken`, 1 px `border.strong`, padding `space.2`. Left: a site indicator (see below). Text: `body`. Unfocused it shows the host in `text.primary` and the rest of the URL in `text.tertiary`, truncated at the end; focused it shows the full URL, all selected.
  - Find (search icon), Grab (crosshair icon), and a More button (ellipsis). Toolbar buttons are borderless ghost IconButtons: no border and no fill at rest, `fill.hover.canvas` on hover (A12).
  - Grab and annotate (D-BP-02) writes notes as a floating note anchored to the element with a caret (A44): an element selector chip with a close x, a focused text field, the hint "Enter to send · Esc to close" and a round Send. No card across the page, no thumbnail, no Cancel or Send buttons. Several elements can be noted before sending. A zoom chip ("125%", `label`, `surface.sunken`, `radius.sm`) appears between the address bar and Find when the zoom is not 100 percent.
- Site indicator (left in the address bar), always with a word available as the tooltip and accessible name:
  - Secure: lock icon 16 px, `text.secondary`.
  - Not secure (http, except localhost): triangle-alert icon in `status.warning.fg` and the visible word "Not secure" in `label` `status.warning.fg`.
  - Certificate problem: octagon-alert in `status.danger.fg` and the word "Certificate error".
  - Local page (localhost or an address on this PC): no indicator.
- Host chip: when the page lives on a remote host the address bar starts with a chip "on VPS" (Badge, `radius.sm`, server icon 12 px). A page served from this PC while the session runs elsewhere shows "this PC".
- Agent chip (right of the address bar, only while an agent drives this tab): a bot icon, "Claude is controlling this tab" in `label`, and a "Take over" button (`size.control.compact`). Fill `accent.subtle.canvas`, 1 px `border.default`, `radius.sm`.
- Progress line: a 2 px line along the bottom of the toolbar, `accent.default`, travelling left to right while loading (determinate when the engine reports progress).
- Content frame: below the toolbar, filled by the engine. Error and empty pages of the chrome (see States) are drawn in this frame by the app.

## Variants and sizes

| Variant | Difference |
|---|---|
| Browser tab | Full toolbar. |
| Document preview | Address bar replaced by the document name; no back and forward; an "Open externally" button that goes through the external-link confirm (X-DLG-01). |
| Compact (tab narrower than 400 px) | Find and Grab move into More; zoom chip hidden; the address bar keeps its minimum width. |

Android: not designed (index platform D). The phone opens links in the system or an in-app browser chosen in settings (M-ST-04).

## States

| State | Visual | Tokens |
|---|---|---|
| default | Toolbar as above | `surface.canvas`, `text.secondary` icons |
| hover (button) | Fill | `fill.hover.canvas` |
| pressed | Fill | `fill.pressed.canvas` |
| focus-visible | Ring 2 px, offset 2 px on the focused control; address bar border becomes `border.focus` while focused | `border.focus` |
| selected | Find or Grab toggled on: `fill.selected.canvas` and a 2 px `accent.fg` underline | `fill.selected.canvas`, `accent.fg` |
| disabled | Back or Forward with no history; Reload with no page | `text.disabled`; `aria-disabled` |
| loading | Reload becomes Stop; progress line; the favicon slot shows a spinner after 200 ms | `accent.default`, `agent.running` |
| empty | A new tab: the address bar is empty and focused with the placeholder "Search or enter an address" | `text.tertiary` |
| error | The frame shows ErrorState: "Can't reach localhost:3000" with the reason line (refused, timed out, name not found) and "Retry" | `status.danger.fg` |
| client-hosted page | Host chip "this PC" with the caption in the frame when the session runs elsewhere | `text.secondary` |
| long content | A long URL truncates at the end unfocused; focused it scrolls horizontally. Tab titles are not in the chrome | `text.tertiary` |
| disconnected host | A page on an unreachable host cannot load: the frame says "Can't reach VPS" with the unreachable glyph and the last loaded page's title and time; Reload stays enabled and retries. A loaded page stays visible and frozen, with a Banner "VPS can't be reached. The page may be out of date." | `agent.unreachable` |

## Behaviour (handoff)

- **Keyboard.** `Mod+L` focuses the address bar and selects it. Enter navigates; Escape reverts the text and returns focus to the page; typing text that is not an address searches with the configured engine (D-ST-05). `Mod+R` reloads; `Mod+Shift+R` reloads without the cache; Escape stops loading. `Alt+Left` and `Alt+Right` go back and forward (`Cmd+[` and `Cmd+]` on macOS). `Mod+F` opens the Find bar. `Mod+=`, `Mod+-` and `Mod+0` zoom. These keys reach the app before the page does (the app-flow keyboard rule for browser panes).
- **Focus order.** Back, Forward, Reload, address bar, zoom chip, Find, Grab, agent chip, More, then the page. Arrow keys move between the first three buttons (roving). `F6` leaves the Work panel; the page keeps its own focus ring.
- **Pointer.** Long-press or right-click on Back and Forward opens the history list. Click the site indicator to see the connection details. Click the zoom chip to reset to 100 percent. Middle-click and `Mod+click` on links are handled by the engine and the link-routing preference (D-BP-08).
- **Resizing.** The address bar flexes; buttons never shrink. Compact rule above.
- **External links.** Opening a page in the system browser or a document externally asks first (X-DLG-01), as in the old preview.
- **Agent control.** When an agent drives the tab, the chip shows and clicks and typing by the user interrupt the agent ("Take over" is implicit on user input). The permission itself lives in Settings (D-BP-05).
- **Motion.** The progress line moves with `ease.linear`; with reduced motion it becomes a static partial bar that steps with progress. Hover fills step over `duration.fast`.
- **Accessibility.** The toolbar is `role="toolbar"` named "Browser toolbar". Buttons have names ("Back", "Forward", "Reload", "Stop"). The address bar is a text box named "Address". The site indicator's word is read before the address. Loading and errors are announced politely ("Loading localhost:3000", "Can't reach localhost:3000"). The agent chip is a status region.
- **Extension limits.** The Browser chrome is not an allowed component in extension views and extensions cannot add toolbar buttons. They may add context menu items on page links through the engine's menu (contribution point 13).

## GPUI notes

- The page is a native web view (WebView2, WKWebView, WebKitGTK) or an off-screen CEF texture. A native web view sits above everything GPUI paints, so popovers, menus, tooltips and the Find bar that must overlap the page cannot be drawn over it. Options, decided with the browser-engine spike (Phase 0): render the engine to a texture drawn as a GPUI image (overlays work, input and accessibility need work), or hide or shrink the native view while an overlay is open, or keep toolbar menus above the page area only. Until decided, the spec assumes overlays from this toolbar open inside the toolbar's own 36 px and the Find bar is an overlay at the pane's top right (FindBar.md), drawn over the page once the engine decision above is made.
- The address bar uses the shared text input. History and suggestions are not designed (open).
- Progress line: a quad animated by the engine's progress events.

## React Native notes (Android)

Not part of the Android app in v1. If an in-app browser is built later it would use the platform's own browser chrome.

## Desktop vs Android differences

Desktop only. On Android links open in the system browser or an in-app browser set in Settings (M-ST-04).

## Copy

- Placeholder: "Search or enter an address".
- Indicators: "Not secure", "Certificate error", "on VPS", "this PC".
- Agent: "Claude is controlling this tab", "Take over".
- Errors: "Can't reach localhost:3000", "Can't reach VPS", "Retry".
- Tooltips: "Back", "Forward", "Reload", "Stop", "Find in page", "Grab and annotate", "More".
- Banner: "VPS can't be reached. The page may be out of date."

## Token gaps and open points

Token gaps: none. The toolbar height is `size.toolbar`.

Open points:
- Web view layering over GPUI (above) depends on the browser-engine decision.
- The agent chip and "Take over" are a proposal so agent browsing is never silent; D-BP-05 only defines a permission switch.
- Address suggestions, history and bookmarks are not in the inventory and are not designed.
- The Find bar placement inside the page area (FindBar spec) versus the toolbar.
