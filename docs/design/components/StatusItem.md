# StatusItem

## Purpose

One item in the status bar: an optional 12 px glyph, a short caption value, an optional state (info, success, warning or danger) and a click command, with a tooltip. Core items and extension status items (D-SL-02) share this component.

Inventory IDs: D-SL-02 (extension slot) and D-SH-05 (Desktop). Android: none.

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Item | No fill at rest; `radius.sm` on hover | Height `size.statusbar` (24) |
| Glyph (optional) | `text.secondary`, or the state colour | `size.icon.xs` (12) |
| Caption value | `caption` style in `text.secondary`, tabular figures | 11 px on a 16 px line |
| State glyph and word | Success `status.success.fg` (circle-check, "Done"); warning `status.warning.fg` (triangle-alert, "Check"); danger `status.danger.fg` (octagon-alert, "Failed"); info `status.info.fg` (info, "Note") | 12 px |
| Tooltip | Tooltip spec: `surface.overlay`, `border.default`, `shadow.e2`, `radius.lg` | Width per the Tooltip spec |

## Variants and sizes

- Text only, or glyph and text. State kinds: info, success, warning, danger.
- One size: the bar's 24 px height. Glyphs stay 12 px.
- Extension items: text up to 24 characters, priority 1 to 100, two items per extension.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | `text.secondary` | Caption text on the bar, no fill. |
| Hover | `fill.hover.canvas`, `text.primary` | The bar sits on the Canvas, so hover uses the canvas fill, not the frame fill. |
| Pressed | `fill.pressed.canvas` | While the pointer is down. |
| Focus-visible | `border.focus`, inset | A 2 px ring drawn inside the item (offset -2), because the window edge would clip an outer ring. |
| Selected or active | none | Items are not toggles. An open menu keeps `fill.pressed.canvas`. |
| Disabled | `text.disabled` | Not used in v1. |
| Loading | `text.tertiary` | The last known value, or a dash; no spinner before 200 ms. |
| Empty | `text.secondary` | A zero reads as a word: "No ports". |
| Error | `status.danger.fg` | octagon-alert glyph; "Failed" in the tooltip and the name. |
| Long content | `text.secondary` | Text truncates with an ellipsis; the full value is in the tooltip. Extension text is capped at 24 characters. |
| Disconnected host | `text.tertiary` | Text dims; the `size.indicator.dot` (6 px) connection dot becomes a hollow `border.strong` ring; the tooltip gives the age. |
| Writing | N/A | Not an input. |

## Behaviour

- Keyboard: when the bar has focus, Left and Right move between items, and Enter or Space runs the command. Keyboard focus shows the tooltip at once.
- Focus order and ring: the bar's place in the F6 cycle. Each item shows its inset ring.
- Pointer: click runs one command. Hover shows the tooltip after 400 ms (`duration.tooltip-delay`).
- Responsive: the height never changes. Text truncates before the bar hides the item.
- Motion: the hover fill uses `duration.fast` with `ease.standard`. Nothing blinks or pulses. Reduced motion: instant.
- Accessibility: role `button` when it runs a command, otherwise plain text. The name holds the label, the value and the state word, for example "Hosts, 2 hosts, one disconnected". The tooltip is the description.
- Extensions: own text (24 characters), a 12 px icon, a state kind and a tooltip view. Extensions cannot change height, colours or animation, and cannot open views inside the bar.

## GPUI notes

Draw each item as a flex child with a caption text run and a 12 px glyph. Hover is a solid `fill.hover.canvas` rectangle with `radius.sm`, with no alpha. Focus is an inset 2 px rectangle in `border.focus`. Draw tooltips in the overlay layer. Use tabular figures for counts.

## React Native notes

Not on Android. The same values appear on host cards and session rows as text, an AgentStatus glyph or a badge, with TalkBack labels that carry the state word.

## Desktop and Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Container | Status bar | None; host cards and lists |
| Pointer | Hover fill and tooltip | Touch; long press shows the label |
| State cue | Glyph; word in the tooltip and name | Glyph and word as text |

## Copy

- State words, as the design system pairs them: "Done" (success), "Check" (warning), "Failed" (danger), "Note" (info).
- Values: "1.22 GB", "3 terminals", "2 ports", "2 hosts", "No ports".
- Tooltips: "Hosts: 2 connected, 1 disconnected"; "Last known state, 5 min ago".

## Token gaps and open points

Token gaps: none. The hosts connection dot is `size.indicator.dot`.

Open points:
- Warning and success words show only in the tooltip and accessible name, not in the bar text. Confirm the bar does not need them visible.
- Click targets for each item are proposals (see StatusBar). Confirm.
