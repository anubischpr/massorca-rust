# Avatar

## Purpose

A round mark for an agent, a bot, a team or a person: the supplied provider or agent mark, a team icon, or initials. Where it sits next to an agent, it carries the agent status glyph so the state has a shape as well as a colour.

Inventory IDs: X-AG-01 (agent status indicators), D-AC-06 (usage and subscriptions), D-SH-15 (rail usage rings, which use the Avatar mark inside the ring). Also the Bot Teams team icon and the Conference room header.

## Anatomy

- Container: a circle, radius `radius.full`. Sizes `size.avatar.sm` (20 px), `size.avatar.md` (28 px), `size.avatar.lg` (40 px).
- Fill: `surface.raised` with `border.default` 1 px for a mark; `accent.subtle.raised` for initials on a team; `surface.sunken` for a fallback.
- Mark: a supplied single-ink SVG drawn in `text.primary`, or a full-colour asset (see open points); scaled to 70 percent of the circle.
- Initials: `label` style (12 / 16 / 500) or `body-strong` at lg, colour `accent.fg` on team fills and `text.primary` on people fills.
- Status glyph (optional): 12 px badge at the bottom right, on a 2 px ring of `surface.frame` or `surface.canvas` (the surface it sits on). Uses the X-AG-01 glyphs: running (spinner arc, `agent.running`), waiting (circle with question mark, `agent.waiting`), idle (hollow ring, `agent.idle`), error (triangle, `agent.error`), unreachable (dashed ring, `agent.unreachable`).

## Variants and sizes

| Variant | Use |
|---|---|
| agent | the agent's own icon from AgentIcon (components/AgentIcon.md; the app's real asset, A94) in full colour (A16, A18); the generic agent mark only when the agent registers none; never a letter (C, X, G, O, K) |
| team | team icon or initials (Bot Teams) |
| person | initials, for a person in a shared surface |
| status | agent mark with the status glyph (X-AG-01) |

Sizes: sm 20 (lists, status bar), md 28 (rows, cards), lg 40 (headers, Conference room). Android: 40 dp in rows, 28 dp in chips.

## States

| State | Avatar |
|---|---|
| default | mark or initials on its fill |
| hover | not interactive by itself; a row or button around it owns hover |
| selected | a 2 px `accent.default` ring outside the circle |
| focus | not focusable on its own |
| disabled | mark in `text.disabled`, no status glyph |
| loading | Skeleton circle (`fill.skeleton`) |
| empty | initials of the name; never a blank circle |
| error | the status glyph `agent.error` with the word "Error" in the accessible name |
| long content | initials use at most two letters |
| disconnected host | the dashed ring glyph (`agent.unreachable`) and the name "Unreachable, last known 5 min ago" |

## Behaviour (handoff)

- Keyboard: not focusable on its own.
- Pointer: none on the Avatar; hover tooltips belong to the parent button or row.
- Resizing: fixed circle; the glyph size scales with the circle (12 px at md and lg, 10 px at sm).
- Motion: spinner arc in the running glyph turns over `duration.spinner`; reduced motion: the glyph stays a static arc, text carries the state.
- Accessibility: the accessible name is the agent or person name, with the status word when a glyph is shown ("Claude, Running"). Decorative marks are `aria-hidden` when the name is in the row.
- Extensions: an extension may supply an icon only as a Lucide name or a single-ink 24-unit SVG. It cannot use a core status glyph as its own icon, and it cannot choose the mark colour beyond `text.primary`.

## GPUI notes

- Circle `div` with `rounded_full`; mark as an SVG element tinted with the ink colour; initials as text.
- Status glyph is a second small element positioned over the bottom-right edge; its ring is a border in the frame or canvas colour.
- Spinner arc is an SVG with rotation.

## React Native notes

- `View` with `borderRadius` at half the size; mark through `react-native-svg` or `Image` for full-colour assets.
- Status glyph 12 dp; accessible name through `accessibilityLabel`.

## Desktop vs Android

- Desktop uses the rail rings and the sidebar rows; Android uses 40 dp in the host card and rows. Same marks, same glyphs.

## Copy

- Names are the agent or team names: "Claude", "Codex", "Conference room" lead bot, "NexusCare team".
- The status word follows X-AG-01: "Running", "Waiting for you", "Idle", "Error", "Unreachable, last known 5 min ago".

## Token gaps and open points

- No gaps: `border.width.thick` (2 px) and `size.icon.xs` (12 px) cover the status ring and glyph.
- Open: whether rail marks stay full colour inside the usage ring (pending-owner-review item 32). Spec follows the pending decision: full colour, with ring colours carrying identity.
