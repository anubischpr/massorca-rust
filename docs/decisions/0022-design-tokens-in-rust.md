# 0022: Design tokens become Rust by code generation

Status: accepted, 2026-11-20.

## Context

`docs/design/tokens.json` is the token contract (handoff 2.1): 133 colour
tokens with a dark and a light value, type styles, spacing, radius, shadows,
sizes, layout widths, borders, durations, easings, z-index and contrast
floors. Token names are the contract across GPUI, React Native and extension
themes. A value copied by hand drifts.

## Decision

- A generator turns `tokens.json` into `crates/ui/src/tokens/generated.rs`.
  The generated file is committed (in its own commit when it changes), so a
  reviewer sees the values and the build needs no extra step.
- The generator is a test: `cargo test -p massorca-ui tokens_are_generated`
  regenerates in memory and fails when the committed file differs. With
  `MASSORCA_UPDATE_TOKENS=1` the same test rewrites the file. CI runs it, so
  the JSON and the Rust cannot drift.
- Shape of the output:
  - `Palette`, one struct per theme (`Palette::DARK`, `Palette::LIGHT`) with
    one group per first name segment and the rest of the name as a snake-case
    field: `surface.frame` -> `palette.surface.frame`, `status.danger.fg` ->
    `palette.status.danger_fg`, `fill.hover.canvas` ->
    `palette.fill.hover_canvas`. Two levels, because some names are both a
    token and a prefix of another (`agent.waiting`, `agent.waiting.bg`,
    `agent.waiting.bg.overlay`). `Palette::get("status.danger.fg")` looks a
    colour up by its exact token name for theme files.
  - Aliases (`{status.success.fg}`) are resolved at generation time; the
    generator fails on an unknown alias.
  - Colours are `Rgba` with alpha (`#ffffff1f`, `rgba(...)`).
  - Scales without themes are constants named after the whole token in
    capitals: `SPACE_2`, `RADIUS_CANVAS`, `SIZE_RAIL_WIDTH`,
    `LAYOUT_SIDEBAR_DEFAULT`, `DURATION_MODERATE`, `EASE_ENTER`, `Z_DIALOG`.
    Lengths are `f32` logical pixels (the components turn them into `px()`);
    percent and `ch` values are a `Measure`.
  - Type styles become `TYPE_BODY: TextStyleToken { family, size,
    line_height, weight, letter_spacing_em }`.
  - Shadows become a list of `ShadowToken { x, y, blur, spread, colour }` per
    theme; `shadow.focus` keeps its two rings.
- The theme at run time is `Theme { appearance, palette, accent }`. The
  accent seed and derived accent colours (design-system 6.4) are computed in
  `crates/ui`, not generated. Theme JSON files from users and extensions and
  the validator V1 to V12 come later and reuse the same `Palette` type.

## Alternatives

- **`build.rs` reading the JSON at build time.** No committed file to review,
  and the crate would read a file outside its directory on every build.
- **Hand-written struct.** Drifts.
- **A JSON loaded at run time only.** Loses compile-time names; a typo becomes
  a missing colour at run time.

## Consequences

- Renaming a token in the design is a compile error in every component that
  uses it, which is the intended contract.
- The React Native object can come from the same generator later.
