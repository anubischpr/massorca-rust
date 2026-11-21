# 0026: Web engine for the browser pane

Status: accepted as a direction, 2026-11-20. Nothing is built yet.

## Context

The Work panel has a browser tab (A6; components/BrowserChrome.md): an
address bar drawn by the app and a page drawn by a web engine. GPUI has no
webview. Spike S5 asks how a native web view and GPUI overlays (menus,
popovers, toasts, the palette) share the screen. Research done 2026-11-20
against primary sources:

| Option | Platforms | Overlays over the page | Size | Licence | State |
|---|---|---|---|---|---|
| `wry` child web view (WebView2, WKWebView, WebKitGTK) | Windows, macOS, Linux on X11; Wayland needs a GTK container that GPUI does not have | No: a native child always draws above GPUI | Small (OS engine; WebKitGTK on Linux) | MIT or Apache-2.0 | 0.57, mature (Tauri) |
| WebView2 visual hosting | Windows only | Yes, the app composes it; the app forwards input and scaling | Small | OS runtime | Stable Microsoft API, not exposed by `wry` |
| CEF off-screen (`cef` crate, tauri-apps/cef-rs) | Windows, macOS, Linux incl. Wayland | Yes: frames import into a `wgpu` texture (D3D11, IOSurface, DMA-BUF) | Over 100 MB | BSD (CEF), MIT or Apache-2.0 (bindings) | Tracks Chromium releases |
| Servo (`servo` crate) | All three | Yes (texture) | Medium | MPL-2.0 | Embedding API still breaking; web compatibility behind |
| Ultralight | All three | Yes | Small | Proprietary; free tier has no JS JIT or media | Built for game UI |

Sources: github.com/tauri-apps/wry and docs.rs/wry (`WebViewBuilder::build_as_child`
is X11-only on Linux); learn.microsoft.com/microsoft-edge/webview2/concepts/windowed-vs-visual-hosting;
github.com/tauri-apps/cef-rs and docs.rs/crate/cef (`osr_texture_import`);
servo.org blog 2026; ultralig.ht/pricing. `gpui-component` ships an
experimental `wry` web view that syncs bounds on every prepaint and does not
handle z-order.

## Decision

1. **First version: a `wry` child web view** on Windows and macOS, and on
   Linux under X11 (XWayland on Wayland sessions). The pane syncs the child's
   bounds and visibility on every prepaint and hands focus back to GPUI on a
   click outside it.
2. **Overlay rule while the child exists:** no GPUI overlay may cover the page
   rectangle. Menus and popovers of the browser chrome open over the chrome
   row, not the page. When a modal overlay (dialog, command palette) opens, the
   web view is hidden and a static placeholder in `surface.sunken` takes its
   place until the overlay closes. Toasts move to the side of the window away
   from the browser tab when they would overlap it.
3. **Later, if the overlay rule is too limiting or Wayland needs native
   support:** CEF off-screen into a texture behind a build feature. It is the
   only option that works on all three systems with free z-order; the price is
   the binary size and Chromium updates.

## Consequences

- The browser pane is a separate piece of work with its own pull requests and
  its own spike: hiding the child on overlay open, focus hand-back, DPI.
- Native accessibility of the page comes from the OS engine in step 1.
- Extension web views ([P3]) reuse the same element and the same rule.
