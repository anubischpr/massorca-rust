# Badge

## Purpose

A small label, count or dot that shows a status, a category or a number on another element: a status chip, a count on a tab or a row, an unread dot, or an extension badge on a rail item. A badge is never the only cue for a status: it always carries a glyph and a word, except a count or an unread dot that sits next to a labelled item.

Inventory IDs: none of its own. Used by D-SH-15 (rail usage state), extension rail badges (D-SL-09: neutral, accent or danger; dot or count 1 to 99+), D-WS-06 (status appearance), X-AG-01 (child counts), M-HM-02 (host card status badge), tab and row counts.

## Anatomy

- Container: height `size.badge.height` (16 px), padding `space.1` (4 px) horizontal for text badges, radius `radius.sm` (4 px). Count badges use radius `radius.full`.
- Text: `caption` (11 / 16 / 400) for status and counts, `label` (12 / 16 / 500) for category chips. Counts use tabular figures.
- Glyph (status variants only): Lucide, 12 px (`size.icon.xs`), stroke per the iconography rule, gap `space.1` to the text.
- Dot: `size.indicator.dot` (6 px) circle, `radius.full`, no text.

## Variants and sizes

| Variant | Background | Text and glyph | Border | Use |
|---|---|---|---|---|
| neutral | `badge.neutral.bg` | `badge.neutral.fg` | none | category, extension badge kind neutral; the count badge on rails, Sidebar rows and tabs |
| accent | `accent.subtle.raised` | `accent.fg` | none | selected or new, extension badge kind accent |
| success | `status.success.bg` | `status.success.fg`, glyph `circle-check` | `status.success.border` | completed, connected, passed |
| warning | `status.warning.bg` | `status.warning.fg`, glyph `triangle-alert` | `status.warning.border` | needs attention |
| danger | `status.danger.bg` | `status.danger.fg`, glyph `octagon-alert` | `status.danger.border` | failed, extension badge kind danger |
| info | `status.info.bg` | `status.info.fg`, glyph `info` | `status.info.border` | notice |
| solid (status) | `status.<kind>.solid` | `status.<kind>.on` | none | high-emphasis status on a dark row only |
| count | `badge.neutral.bg` or the kind's `bg` | `badge.neutral.fg` | none (the kind's `border` on a kind `bg`) | numbers; max "99+" |
| dot | the kind's `solid` | none | none | unread, rail badge dot; always beside a labelled item |
| tag (neutral) | `surface.sunken` | `text.secondary`, optional 12 px icon (`size.icon.xs`) before the text | `border.subtle`, 1 px | metadata chips on cards and rows: assignee ("Unassigned"), "What it adds", scope, source. Height `size.tag.height` (20 px), padding 6 px horizontal (`space.1` + `space.half`), gap `space.1`, radius `radius.sm`, text `caption` (11 / 16 / 400). Ruling B9, 2026-11-25 |
| tag (accent) | `accent.subtle.raised` | `accent.fg` | none | the selected or highlighted tag, same metrics as tag (neutral) |

Sizes: 16 px high on desktop (`caption` text inside), except the tag variant, which is 20 px high (`size.tag.height`) so it sits on the 18 to 20 px lines of cards and rows. Android: text 12 sp, height 20 dp, may grow with font scale to 130 percent (cap for chips and badges).

## States

| State | Badge |
|---|---|
| default | as the variant |
| hover | not interactive; a badge that opens something is a button and uses its own states |
| selected | not applicable; selection is shown on the row, not on the badge |
| focus | not focusable |
| disabled | `text.disabled`, no background |
| loading | not applicable; a count shows a skeleton block when its number is not known yet |
| empty | a count of zero is not shown (the badge is removed) |
| error | the danger variant with the glyph and the word ("Failed") |
| long content | text truncates with an ellipsis after 16 characters; full text in a tooltip |
| disconnected host | the agent.unreachable dashed ring glyph and "Unreachable" text in neutral; never "Stopped" |

## Behaviour (handoff)

- Keyboard: not focusable unless it is wrapped in a button.
- Pointer: no action. A badge with a tooltip shows it on hover.
- Resizing: fixed height; text truncates before the badge wraps.
- Motion: none. A count changing in place fades its digits over `duration.fast`; reduced motion: instant.
- Accessibility: the badge text is read as part of its row or tab ("Checks, 3 failing"). A dot is decorative only when the labelled item says the same thing; otherwise it has a name.
- Extensions: the rail badge supports neutral, accent and danger kinds, dot or count only. An extension cannot set its own colour, and cannot use the success or warning kinds (those are core status meanings).

## GPUI notes

- Container `div` with a fill and a 1 px border from the variant table; text run with the caption or label metrics.
- Count uses tabular figures through the font feature set.
- Dot is a circle element; no shadow.

## React Native notes

- `View` with `accessibilityLabel` on the wrapping row; decorative dots use `accessible={false}`.
- Glyph through `react-native-svg` at 12 dp.

## Desktop vs Android

- Caption or label text on desktop, 12 sp on Android, same variants and copy.

## Copy

- Words: "Running", "Waiting for you", "Failed", "Connected", "Unreachable", "New".
- Counts are numbers only; the label is on the row ("Checks 3").
- No exclamation marks; no emoji.

## Token gaps and open points

Token gaps: none. Height is `size.badge.height` (16 px), the dot is `size.indicator.dot` (6 px) and the neutral fill is `badge.neutral.bg` with `badge.neutral.fg`.

Open points:
- Whether the count pill should use `radius.full` or `radius.sm` on desktop. Spec: pill for counts, `radius.sm` for status chips.
