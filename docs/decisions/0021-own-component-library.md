# 0021: Our own component library on plain GPUI

Status: accepted, 2026-11-20.

## Context

The design system (`docs/design/design-system.md`, 66 component specs, 94
binding agreements) is specific: an L-shaped frame, solid precomputed state
fills, agent icons everywhere, one focus ring, exact sizes. GPUI ships only
primitives (`div`, text, `svg`, `img`, `uniform_list`, `list`, `anchored`,
`deferred`, animations, an input handler trait). The main third-party
component crate is `gpui-component` (longbridge, crates.io 0.10.0,
2026-11-16), about 60 components with its own theme model.

## Decision

- Build our components in `crates/ui` directly on GPUI. Do not depend on
  `gpui-component`.
- Each component reads the theme through one accessor (`cx.theme()`), never a
  literal colour or size. Agreements that apply to a component are coded in
  the component, so screens inherit them.
- Component state that has rules (selection, disabled reasons, roving focus,
  open and closed overlays, toast stack order) lives in plain Rust structs that
  are tested without a window. The GPUI element is a thin view of that state.
- Hard primitives (single-line text input with IME, virtual lists, anchored
  overlays) are written on GPUI's own APIs (`EntityInputHandler`,
  `uniform_list`, `anchored` and `deferred`), using Zed's examples as the
  reference for how the APIs work. No code is copied from MassOrca.

## Alternatives

- **`gpui-component`.** Faster start, but it depends on `gpui-pre`, a weekly
  third-party republish of Zed's GPUI (`=0.3.12` in 0.10.0), so our GPUI pin
  would follow its release train. Its theme is a different model (its own
  colour names and its own radius and shadow scale); restyling 60 components
  to our agreements costs about as much as writing the 25 we need now, and
  every upgrade would reopen the restyle.
- **Fork `gpui-component`.** Same theme mismatch, plus a fork to maintain.

## Consequences

- More code in `crates/ui`, but every pixel follows our tokens and the
  agreements, and an upgrade of GPUI touches only our code.
- The text input with IME (spike S7) is ours to build; it is the riskiest
  primitive and gets its own pull request.
