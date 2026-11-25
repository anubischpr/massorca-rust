# InvocationBanner

## Purpose

The fourth part of a Marketplace item page (the owner's nine-part order, decision 0059), under the host line. It shows example prompts for the item. Pressing a prompt opens a chat with it. A last line says how to call the item directly.

Approved by the owner on 2026-11-25 from a screenshot (agreement A99, artboard `bt-invocation-banner`). This replaces the bordered info box that was built first; that box is rejected.

The owner's words, summarised:
- The banner is a panel with a gradient background taken from the item's icon colour. It has no border and much rounder corners.
- The example prompts are centred pills in the chat composer's surface colour. Each pill has the item's tag, the text and an arrow at the end.
- The footer line is centred.
- An item with no colour of its own takes its gradient from a palette of several colours, so the pages do not all look the same.

Inventory: Marketplace item page (D-MK item page, section 7.2 order), desktop and Android.

Code: `crates/ui/src/components/invocation_banner.rs` (the component), `crates/desktop/src/shell/marketplace/item.rs` (`banner`, `banner_colours`, `start_chat_with_prompt`), `crates/desktop/src/model/marketplace/item_page.rs` (`banner_tag`, `banner_palette_slot`).

## Anatomy

| Part | Spec | Tokens |
|---|---|---|
| Panel | Full width of the page column. No border and no shadow. Padding 24 px on every side. Content is centred in one column. | `radius.2xl` (16), `space.6` |
| Gradient | Linear, 135 degrees, from the top-left corner to the bottom-right corner. The first stop is at 0 and the second at 1. See Colour. | listed colours, `banner.palette.*`, `banner.end` |
| Pill column | Centred. Each pill is as wide as the column allows, up to 560 px. The gap between pills is 8 px. | `space.2` |
| Pill | 44 px high, with 8 px corners. Fill `surface.raised`, the chat composer's surface, with no border. Padding is 8 px on the left and 16 px on the right. The gap between its parts is 12 px. | `surface.raised`, `radius.lg`, `space.2`, `space.4`, `space.3` |
| Tag chip | First in the pill. It shows the item's mark at 14 px (an agent uses its AgentIcon) and the item's name. Height 24, padding 0 8 px, gap 4 px. Fill `badge.neutral.bg`, `radius.full`, text in `body-sm` 500 with `badge.neutral.fg`. | `badge.neutral.bg`, `badge.neutral.fg`, `radius.full` |
| Prompt text | Fills the rest of the pill on one line and ends with an ellipsis. The full prompt is the accessible name. The leading `@Tag` is not repeated, because the chip shows it. | `body`, `text.primary` |
| Arrow | Lucide `arrow-right`, 16 px, at the end of the pill. | `text.secondary` |
| Footer line | 12 px under the last pill, centred: "Type @Tag in any chat or room to ask it directly." | `body-sm`, `text.primary`, `space.3` |

The footer uses `text.primary`, not `text.secondary`. In the approved screenshot it is near white (#fafafa). `text.secondary` cannot reach 4.5:1 on any gradient that still reads as colour (see Colour).

## Colour

The gradient takes its colours from one of three sources, in this order:

1. **The listing has two colours** (`listing.banner.colors`, the same values the reasoning slider uses, A98). The gradient runs from the first colour to the second.
2. **The listing has one colour.** The gradient runs from that colour to `banner.end`.
3. **The listing has no colour.** The first stop is `banner.palette.N` and the second is `banner.end`.
   - N is chosen per item: FNV-1a of the extension ID, modulo 8, plus 1. An item with no extension ID uses its name.
   - The same item keeps the same colour on every run and on every device.

**Readability.** Listed colours can be any colour, so each listed stop is mixed toward `surface.canvas`. It moves in steps of 5 percent until `text.primary` reaches at least 4.5:1 on it; if no step reaches that, the stop is `surface.canvas`. The palette and `banner.end` already meet 4.5:1, so they are used as they are. Examples:

| Listed colours | Dark stops | Light stops |
|---|---|---|
| Claude #D97757 to #B5532F | #945541 to #a54d2d | #d97757 to #c06c4e |

## Ruling R-IB1: the palette for items without colours (2026-11-25)

**Question.** The build uses the six `usage.ring.*` colours for items with no listed colour. Are they right, or should there be a separate palette?

**Ruling: a separate, brand-neutral palette.** Do not use `usage.ring.*` for banners. There are three reasons:

1. **Identity.** A usage ring colour stands for a provider slot in the Activity rail (design-system 2.12). If an item's banner used the same hue, it would read as linked to that provider.
2. **Contrast.** The ring colours are bright, because they are drawn as 2 px strokes. Under a text line they fail: for example `usage.ring.1` (#5aa2ff) gives `text.primary` only 2.2:1 (`usage.ring.3`, 1.6:1). The build then mixes them toward the Canvas until the text passes, so every page loses its hue. The owner's approved green (#558d69) also gives `text.primary` only 3.3:1.
3. **Coupling and count.** Six colours repeat sooner than eight. A change to the rings for legibility in the rail would also repaint every banner.

**The new tokens.** They are in `tokens.json` (added by the banner build, PR #372). Each hue is the owner's approved green turned around the colour wheel at the same saturation and lightness. Its lightness is then lowered just enough that `text.primary` reaches 4.5:1 or more in dark. The light values are pale tints that give `text.primary` 13:1 or more.

| Token | Dark | Light | Hue |
|---|---|---|---|
| `banner.palette.1` | `#457255` | `#cfe5d6` | green (the approved reference, darkened for AA) |
| `banner.palette.2` | `#407071` | `#cde4e4` | teal |
| `banner.palette.3` | `#4c6b97` | `#d3deef` | blue |
| `banner.palette.4` | `#6462a4` | `#dcdbf0` | indigo |
| `banner.palette.5` | `#7d5d95` | `#e5d9ee` | violet |
| `banner.palette.6` | `#935870` | `#efd8e1` | rose |
| `banner.palette.7` | `#7d653d` | `#ecdfc8` | amber |
| `banner.palette.8` | `#666c42` | `#e2e5c9` | olive |
| `banner.end` | `#686a69` | `#e4e5e4` | the neutral grey the approved gradient fades into |

These colours carry no meaning. They only tell pages apart; the item is always named by its mark and name.

## Variants and sizes

- One size on desktop. The panel follows the page column, and the pills shrink below 560 px in a narrow window.
- Android: the same panel with 16 dp padding. Pills are 48 dp high, the full width of the panel, with `radius.lg`. The footer is `body-sm` (13 sp).

## States

| State | Look |
|---|---|
| Default | As above. |
| Pill hover | The pill fill changes to `fill.hover.raised`. |
| Pill pressed | `fill.pressed.overlay` over the pill. |
| Pill focus | The standard focus ring outside the pill, at `radius.lg`. |
| No example prompts | There is no banner. The page goes straight from the host line to Connection. |
| One or two prompts | The same panel with fewer pills. Its height follows the pills. |
| Long prompt | One line with an ellipsis. The full text is in the tooltip and in the accessible name. |

## Behaviour

- Pressing a pill (by click, Enter or Space) opens a new chat in the Canvas: the selected workspace's start page or, with no workspace, the start page with none. The pill's full prompt, including its `@Tag`, goes into the composer and is not sent.
- The footer names the item's mention from `listing.mention`, or else its name.
- Each pill is a button and a tab stop, in order from top to bottom. The panel itself is not a tab stop.
- Reduced motion has no effect, because nothing moves.

## Accessibility

- The panel has `role="group"` and is named by the footer line. Each pill is a button whose name is the full prompt.
- The colours are decorative; nothing depends on them.
- Contrast: `text.primary` on every palette stop and on `banner.end` is at least 4.5:1 in both themes. Listed colours are adjusted to the same level (see Colour).

## GPUI notes

- `linear_gradient(135, stop(start, 0), stop(end, 1))` on the panel; `rounded(RADIUS_2XL)`; no border.
- Pills: `rounded(RADIUS_LG)`, `h(px(44))`, `bg(surface.raised)`; the chip stays `RADIUS_FULL`.

## React Native notes (Android)

- `react-native-linear-gradient` (or Skia) with `start {x:0,y:0}`, `end {x:1,y:1}`; `borderRadius` 16. Pills are `Pressable` with a ripple in `fill.ripple`.

## Copy

- Footer: "Type @Tag in any chat or room to ask it directly."
- Examples come from the item's listing. The tag chip shows the item's display name, not the `@` handle.

## Token gaps and open points

- Tokens: `banner.palette.1` to `banner.palette.8` and `banner.end` (Ruling R-IB1) are in `tokens.json`.
- Sizes use existing tokens. The 44 px pill height is a fixed measure from the approved screenshot, not a new token.
