# 0025: UI tests never open a visible window

Status: accepted, 2026-11-20.

## Context

Automated runs must not steal focus or open windows on the owner's desktop.
GPUI offers three test tools (checked against Zed's source at the pinned
commit, see 0020):

- `#[gpui::test]` with `TestAppContext` and `VisualTestContext`: a fake
  platform with deterministic scheduling; windows are fake, nothing is shown.
  Text is measured by a fake text system.
- `HeadlessAppContext` (`gpui/src/app/headless_app_context.rs`): the same fake
  platform with a real text system (DirectWrite on Windows, CoreText on macOS,
  cosmic-text on Linux), so layout and text widths are real. With a headless
  renderer it can also capture screenshots.
- Headless renderers exist for macOS (Metal) and Linux (wgpu). On Windows
  `gpui_platform::current_headless_renderer()` returns `None`: no screenshots
  without a window.

## Decision

Three layers, cheapest first:

1. **State tests** (every component with rules): plain Rust tests of the
   component's state struct (selection, roving focus, disabled reasons,
   collapse order, palette ranking). No GPUI at all. Run everywhere.
2. **Element tests**: `#[gpui::test]` with a fake window to check that the
   element renders, dispatches its actions and handles keys and clicks
   (`simulate_keystrokes`, `simulate_click`). Run everywhere.
3. **Layout and screenshot checks**: `HeadlessAppContext` with the platform's
   real text system to assert sizes (row heights, region widths, text that
   must not wrap). Screenshots of the gallery are captured on Linux with the
   wgpu headless renderer (CI and WSL, software Vulkan when no GPU) and stored
   as artifacts for comparison against the design; they are not pixel-exact
   gates, because font rasterisation differs per OS.

The gallery and the shell binaries are opened only by a person. Automated
runs never start them with a window. A `--headless-check` flag builds every
gallery page in a headless context and exits, so CI proves they render.

## Alternatives

- **Off-screen windows on Windows.** Still a real top-level window that can
  flash, take focus or appear on another monitor.
- **Pixel-exact screenshot gates.** Brittle across OS font rendering; useful
  later on one pinned Linux image.

## Consequences

- Windows has no automated screenshots; visual comparison there is manual
  through the gallery.
- Linux screenshots depend on a working wgpu adapter in CI; if none is
  available the step is skipped and says so.
