# Message minimap

## Purpose

A thin vertical timeline on the right edge of the Conversation. Each message is one tick, the visible part of the transcript is marked, and a click or drag jumps there. It helps on long sessions where scrolling a thumb gives no sense of where the questions and the problems are.

Inventory ID: D-AG-28. The owner's reference shows the minimap only (dense horizontal ticks, one longer tick at the bottom); behaviour here is designed from that picture.

## Anatomy

- Track: a column on the right edge of the Conversation scroll area, `size.minimap.track` (24 px) wide, full height of the scroll area, on `surface.canvas` (no fill of its own). It overlays the right gutter and never changes the width of the message column.
- Ticks, right-aligned inside the track, one per message:
  - Height `border.width.thick` (2 px), radius `radius.xs`, gap `space.half` (2 px) while all ticks fit.
  - User message: `size.minimap.tick.md` (12 px) wide, `text.secondary`.
  - Agent message: `size.minimap.tick.sm` (8 px) wide, `border.strong`.
  - Needs-you message (approval, question, secret request): `size.minimap.tick.lg` (16 px) wide, `agent.waiting`.
  - Error message: `size.minimap.tick.lg` (16 px) wide, `agent.error`.
- Viewport window: a rectangle behind the ticks covering the range now visible in the list, `fill.selected.canvas`, `radius.xs`, with a 2 px `accent.fg` bar on its right edge as the second cue.
- Hover card (tooltip): appears after `duration.tooltip-delay` beside the pointer: the message's first line (one line, 60 characters), author and time. Surface `surface.overlay`, `border.default`, `shadow.e2`, `radius.lg`, text `body-sm`. The tick under the pointer turns `text.primary`.

## Variants and sizes

One variant. When the transcript has more messages than the track has rows (more than one tick per 4 px), ticks become buckets: each 4 px row stands for a range of messages and takes the strongest kind in it (error, then needs-you, then user, then agent).

The minimap is shown only when the transcript is taller than 1.5 viewports. It is hidden on Android and in windows where the Conversation is narrower than 480 px.

## States

| State | Visual | Tokens |
|---|---|---|
| default | Ticks and viewport window | `text.secondary`, `border.strong`, `fill.selected.canvas`, `accent.fg` |
| hover | Tick under the pointer `text.primary`; hover card shows | `text.primary`, `surface.overlay` |
| pressed (scrubbing) | The viewport window follows the pointer; ticks do not change | `fill.pressed.canvas` |
| focus-visible | Not a tab stop. Keyboard users use the list (`Alt+Up`, `Alt+Down`, Home, End) | not applicable |
| selected / active | The viewport window is the active marker | `accent.fg` |
| disabled | Not applicable | not applicable |
| loading | Hidden until the first page of messages is known | not applicable |
| empty | Hidden (fewer than 1.5 viewports of content) | not applicable |
| error | Hidden with the list's error state | not applicable |
| long content | Sampled into buckets, as above; a single very long message is still one tick | not applicable |
| disconnected host | Ticks stay as they are. The last tick of a running turn becomes the unreachable mark (`agent.unreachable`, `size.minimap.tick.lg`). Hover card says "Last known: running, 5 min ago" | `agent.unreachable` |

## Behaviour (handoff)

- **Pointer.** Click a tick: scroll so that message starts 24 px below the top of the list, over `duration.moderate` with `ease.standard` (instant with reduced motion). Drag on the track: scrub, the list follows the pointer with no easing. Wheel over the track scrolls the list. The cursor is the default arrow; there is no resize behaviour.
- **Keyboard.** The minimap is not in the tab order: everything it does is available from the list (`Alt+Up` and `Alt+Down` to the previous and next user message, Home and End, Page Up and Page Down). This keeps one tab stop per region.
- **Follows streaming.** While the list is stuck to the bottom, new ticks append at the bottom and the whole map re-scales smoothly only when it overflows; otherwise ticks keep their positions.
- **Resizing.** Ticks recompute when the Conversation height changes, debounced to one pass per frame.
- **Motion.** None except the scroll jump. The viewport window follows the list instantly.
- **Accessibility.** `aria-hidden="true"` and `role="presentation"`. It duplicates information the list already exposes (messages, their kinds and their order). Needs-you and error marks are also reachable through the "Waiting for you" jump button and the list.
- **Extension limits.** None. Extensions cannot add ticks or markers. A conversation card counts as one agent tick.

## GPUI notes

- Draw the whole track in one custom element that paints quads (ticks, viewport window, edge bar). No child element per tick.
- Compute the tick array (kind, bucket) when messages are appended or the height changes, not per frame; store it as a flat vector.
- Pointer hit test on the track maps y to a message index by binary search on the cumulative bucket table.
- The hover card is a deferred tooltip layer; it must not capture the pointer.

## React Native notes (Android)

Not designed for Android. The phone has no minimap; scrolling and the "Latest" button are enough at that size.

## Desktop vs Android differences

Desktop only (index platform D). On Android the Conversation message list carries no timeline.

## Copy

- Hover card: author, time, first line. Needs-you marks add "Waiting for you"; errors add "Error". Unreachable adds "Last known: running, 5 min ago".

## Token gaps and open points

Token gaps: none. The track and tick widths are `size.minimap.track` and `size.minimap.tick.sm`, `md` and `lg`.

Open points:
- The owner has not said whether the minimap marks user messages only or all messages. The design marks all, with user messages wider.
- Whether the minimap should also mark search matches while the Find bar is open (same marks as `match.bg`).
