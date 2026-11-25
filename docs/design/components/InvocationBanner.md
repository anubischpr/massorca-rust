# InvocationBanner

## Purpose

Part 4 of the Marketplace item page, between the header and Connection. It shows the example prompts an item answers to. Each prompt is a pill that opens a new chat with that prompt. Below the pills, one line says how to ask the item directly in any chat or room.

Code: `crates/ui/src/components/invocation_banner.rs` (the component), `crates/desktop/src/shell/marketplace/item.rs` (`banner`, `banner_colours`, `start_chat_with_prompt`), `crates/desktop/src/model/marketplace/item_page.rs` (`banner_tag`, `banner_palette_slot`).

Reference: the owner's approved item page (screenshot of the Engram item). No artboard in `docs/design/artboards/` shows this banner (`pending-owner-review.md`, "Marketplace item page order"), so the screenshot is the source until an artboard exists.

## Anatomy

- Panel: `radius.2xl` (16 px), no border. A gradient at 135 degrees runs from the first listed colour (top left) to the second (bottom right). Padding 24 px at the sides and 20 px at the top and bottom, 12 px between the pills and the footer.
- Pills: one centred column of equal width, 560 px at most. Each pill has `radius.full`, the `surface.raised` fill (the composer's surface), and the `fill.hover.raised` fill on hover. Inside, from the left: the tag chip, the prompt without its leading mention in `text.primary`, and a right arrow in `text.secondary`.
- Tag chip: `badge.neutral.bg` fill, `badge.neutral.fg` text and icon, `radius.full`. It holds the item's mark (its agent's own mark, else its kind icon) and its name, such as "Engram".
- Footer: `body-sm` in `text.secondary`, centred: "Type @Name in any chat or room to ask it directly."
- Keyboard: each pill is a Tab stop with the focus ring. Space presses it (tested); Enter presses it through GPUI's keyboard click, as for `Button`.
- No example: no panel. The part shows one sentence instead: "This item lists no example prompts, so there is nothing to invoke."

## Colours

- Listed: `listing.banner.colors`, one or two `#RRGGBB` values. Each gradient stop is the listed colour blended towards `surface.canvas`, as far as the footer text (`text.secondary`) keeps 4.5:1 on it. With one listed colour, the end fades halfway into the Canvas.
- Not listed: the item takes `usage.ring.1` to `usage.ring.6`, six distinct hues in each theme. The slot is a stable FNV-1a hash of the package id (or of the item name), so an item keeps its colour from run to run. No token is named categorical. These six provider slots are the closest set the design system has, so the design session should confirm them.

## Behaviour

- Pressing a pill opens a new chat in the Canvas, the selected workspace's start page or, with no workspace, the start page with none, and puts the full prompt in its composer. The prompt is not sent.
- The footer names the item's mention from `listing.mention`, or else its name.
