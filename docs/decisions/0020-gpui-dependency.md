# 0020: Which GPUI, how it is pinned, how upgrades are budgeted

Status: accepted, 2026-11-20.

## Context

GPUI is pre-1.0. State checked on 2026-11-20 against primary sources:

- crates.io `gpui` is 0.2.2, published 2025-10-22. Zed has not published it
  since; Zed's own `crates/gpui/Cargo.toml` still says 0.2.2.
- Upstream moved on: GPUI is now split into `gpui` plus platform crates
  (`gpui_platform`, `gpui_windows`, `gpui_macos`, `gpui_linux`, `gpui_wgpu`,
  `gpui_web`), gained a cross-platform `HeadlessAppContext` with real text
  shaping (`crates/gpui/src/app/headless_app_context.rs`) and AccessKit
  support. None of this is on crates.io as `gpui`.
- `gpui-pre` on crates.io is a weekly snapshot of Zed's GPUI republished by
  the `gpui-component` maintainer (0.3.13, "snapshot of zed@125ca86",
  2026-11-16).
- Zed main requires Rust 1.98.1 (`rust-toolchain.toml`, bumped 2026-09-11).
  The last Zed commit that builds on Rust 1.92 is from 2026-01-22.
- Spike, 2026-11-20: `gpui` and `gpui_platform` from Zed commit
  `2f808b3c2de049ecfdbad0fdb26763cebfcaa3b4` (main, 2026-11-21) build on
  Windows with Rust 1.98.1 in about five minutes from a clean target.

## Decision

- Depend on `gpui` and `gpui_platform` as git dependencies of
  `https://github.com/zed-industries/zed` at one exact `rev`, declared once in
  the workspace `Cargo.toml` (`[workspace.dependencies]`). `Cargo.lock` pins
  every transitive crate. No branch or tag references.
- Raise the workspace toolchain to Rust 1.98.1, the version Zed builds with,
  in its own pull request before any GPUI code. Using another compiler than
  Zed's is the main source of GPUI build breaks.
- Only `crates/ui` and `crates/desktop` depend on GPUI. Our code reaches GPUI
  through ordinary imports, but components never use GPUI's own `ui` or
  `theme` crates (they are Zed's, not GPUI's).
- **Upgrades.** One upgrade pull request per quarter, or earlier when a fix
  we need lands upstream: bump the `rev` (and the toolchain if Zed's moved),
  fix compile errors, run the gallery and the shell by hand in both themes,
  and record the new `rev` here. Budget: one working day per upgrade. An
  upgrade never mixes with feature work.

## Alternatives

- **crates.io `gpui` 0.2.2.** Builds on our old toolchain but is a year old,
  with no headless text shaping and none of the 2026 fixes; the eventual
  upgrade would be the largest.
- **`gpui-pre`.** Small download from crates.io, but a third party decides
  which commit we get and when, and it needs the same new toolchain.
- **Zed commit from 2026-01-22 on Rust 1.92.** Avoids the toolchain change
  but starts ten months behind upstream, without the headless renderer.

## Consequences

- The first build clones the Zed repository (cached by Cargo and by CI).
- Linux CI needs GPUI's system packages (X11, Wayland, xkbcommon, fontconfig,
  Vulkan loader); the workflow installs them.
- Pinned commit history:

| Date | Zed rev | Rust | Notes |
|---|---|---|---|
| 2026-11-20 | `2f808b3c2de049ecfdbad0fdb26763cebfcaa3b4` | 1.98.1 | first pin |
