# UsageRing

## Purpose

A 32 px circular meter that shows one agent provider's usage, with the provider's mark inside it. On desktop it sits in the Activity rail and opens a usage popover. On Android it sits on the host card as a read-only meter.

Inventory IDs: D-SH-15 (rail, Desktop), D-AC-06 (full usage page, Desktop), M-HM-02 (host card, Android). Platform: both.

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Track, full circle | `usage.ring.track` | ring 32 px (`size.usage.ring`), stroke `border.width.thick` (2 px) |
| Arc, from 12 o'clock, clockwise | `usage.ring.1` to `usage.ring.6` by provider slot; `usage.ring.warn` from 80 percent; `usage.ring.limit` from 95 percent | same stroke |
| Provider mark, centred, full colour asset | the asset itself (see open points) | `size.icon.sm` (16 px) |
| Warning dot, top-right of the mark | `usage.ring.warn` | `size.indicator.dot` (6 px) |
| Limit dot, top-right of the mark | `usage.ring.limit` | `size.indicator.dot` (6 px) |
| Usage popover, desktop only | `surface.overlay`, `border.default`, `shadow.e2`, `radius.lg`, `body` text | `layout.popover.width.sm` (280 px) |

## Variants and sizes

- Single arc: one window reported (for example weekly).
- Double arc: two windows reported (inner arc 5-hour, outer arc weekly). This comes from the pv96 build and is an open point.
- One size: `size.usage.ring` (32 px) in the rail and on the host card.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | arc on `usage.ring.track` | Arc length equals the usage percentage. |
| Hover (desktop) | fill `fill.hover.frame` behind the rail button; ring unchanged | Popover opens after `duration.tooltip-delay`. |
| Pressed | fill `fill.pressed.frame` | While the pointer is down. |
| Focus-visible | `border.focus` ring, 2 px, offset 2 px, around the 36 px rail button | Keyboard only. |
| Selected (popover open) | fill `fill.selected.frame` | Stays while the popover is open. |
| Disabled or unavailable | track only; mark `text.disabled` | Name: "Usage unavailable". |
| Loading | 90 degree arc in `text.tertiary`, turning at `duration.spinner` with `ease.linear` | No value yet. |
| Empty | track only | Name: "Usage not reported". |
| Error | track, plus a `size.indicator.dot` (6 px) dot in `status.danger.solid`; mark `text.secondary` | Name keeps the last value: "Usage could not be read, last value 62 percent". |
| Long content | N/A | Percentages above 100 clamp to a full arc in the limit colour. |
| Disconnected host | arc and mark `text.tertiary` (dimmed) | Name adds the age: "last known 5 min ago". Android host card. |

## Behaviour

- Keyboard: focusable as a RailItem. Enter or Space opens the popover. Esc closes it and returns focus to the ring.
- Pointer: hover opens the popover after `duration.tooltip-delay`. Moving onto the popover keeps it open. Leaving closes it after `duration.base`. A click pins it open.
- Motion: a changed value sweeps the arc over `duration.base` with `ease.standard`. Loading turns at `duration.spinner` with `ease.linear`. Reduced motion: the value changes instantly; the loading spinner keeps turning (4.7).
- Accessibility: role `progressbar`, value 0 to 100, with the name carrying the numbers: "Claude usage 62 percent, resets in 2 days". Announce only when a threshold (80 or 95 percent) is crossed, as a polite status.
- Responsive: fixed size.
- Extensions: an extension may ask for a usage slot through the model picker (contribution point 12). It cannot choose a colour; the core assigns slots 1 to 6 in order of first use.

## GPUI notes

Draw the track as a stroked circle and the arc as a stroked path from 12 o'clock. Precompute the arc path per whole percentage, or draw it in a canvas element. Do not use a conic gradient or blur. The provider mark is a 16 px asset. The dots are 6 px solid circles drawn in the overlay layer at the mark's top-right corner.

## React Native notes

On the Android host card, draw the ring with `react-native-svg` (`Circle` with `strokeDasharray`), 32 dp with a 2 dp stroke. The ring is display-only: it is not focusable, and the host card is the 48 dp target. Give the card `accessibilityRole` "progressbar" or a text value with the same name as desktop. Do not use gradients.

## Desktop and Android differences

Desktop: an interactive rail item with a popover, and the full view at D-AC-06. Android: a read-only meter on the host card, with detail at M-AC-01.

## Copy

- Name: "Claude usage 62 percent, resets in 2 days".
- Popover title: "Usage". Rows: "Resets in 2 days", "Plan" with the plan name.
- Unavailable: "Usage unavailable". Empty: "Usage not reported". Error: "Usage could not be read. Last value 62 percent".
- Threshold announcements: "Usage at 80 percent", "Usage at 95 percent".

## Token gaps and open points

Token gaps: none. The ring is `size.usage.ring` on both platforms, the dots are `size.indicator.dot`, and the popover is `layout.popover.width.sm`.

Open points:
- Double arc (5-hour inside, weekly outside) comes from the pv96 build only. The owner decides whether v1 shows one arc or two.
- Provider marks drawn in full colour inside the ring (pending-owner-review item 32). Single-ink marks would make the ring colour the only identifier.
- The thresholds (80 and 95 percent) follow design-system section 2.12. Confirm they match the provider limits shown on D-AC-06.
