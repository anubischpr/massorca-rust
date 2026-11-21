# StatusBar

## Purpose

The strip at the bottom of the Canvas column. It shows the agent status, memory use, terminal count, port count and host count with a connection dot, plus extension status items. Usage moved to the Activity rail, so it is not here.

Inventory IDs: D-SH-05 (Desktop). Android: none; the system status bar stays, and usage sits on host cards (M-HM-02).

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Bar | `surface.canvas`, no border | Height `size.statusbar` (24) |
| Left group | Agent status item, then extension items by priority | Caption text; items separated by `space.3` (12) |
| Right group | Memory, terminals, ports, hosts, in the owner order | Caption text in `text.secondary` |
| Connection dot (hosts) | Connected: solid `status.success.solid`; disconnected: no fill with a `border.strong` ring | `size.indicator.dot` (6) |
| Visibility menu | Menu: `surface.overlay`, `border.default`, `shadow.e2`, `radius.lg` | One Checkbox row per item |

## Variants and sizes

- One desktop variant at 24 px, at every density.
- Items: agent status (AgentStatus glyph at 12 px, `size.icon.xs`, like every other item), memory ("1.22 GB"), terminals ("3 terminals"), ports ("2 ports"), hosts ("2 hosts" with the dot).
- Narrow windows: the lowest priority item hides first. Hidden items stay in the visibility menu.
- Right-click on the bar opens the visibility menu, which lists each item with a checkbox.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | `surface.canvas`, `text.secondary` | Caption text and 12 px icons. |
| Hover | none | The bar has no hover; each item does (StatusItem). |
| Pressed | none | Not interactive as a whole. |
| Focus-visible | `border.focus` | Focus moves between items inside the bar; see StatusItem. |
| Selected or active | none | Not used; the bar holds no selection. |
| Disabled | none | Not used. |
| Loading | `text.tertiary` | Last known values show; a spinner appears only after 200 ms on a remote host. |
| Empty | `text.secondary` | A zero reads as a word: "No ports", "No terminals". |
| Error | per StatusItem | An item in error shows its own state; the bar stays. |
| Long content | none | Items truncate their text and never wrap the bar. |
| Disconnected host | `text.tertiary` | The hosts item dims, and its tooltip gives the age. |
| Writing | N/A | Not an input. |

## Behaviour

- Keyboard: F6 reaches the bar last in the cycle. Left and Right move between items (roving focus); Enter or Space runs the item's command. Escape leaves the bar.
- Focus order and ring: after the Project sidebar and before the Title bar (design system 8.3). Each item shows its inset ring.
- Pointer: click runs the item's command. Right-click opens the visibility menu. Tooltips appear after `duration.tooltip-delay` (400 ms).
- Responsive: the bar keeps its height. Items drop by priority, lowest first. Values never wrap.
- Motion: none on the bar. Item hover follows StatusItem. The agent spinner keeps turning under reduced motion (design system 4.7).
- Accessibility: role `status`, named "Status bar". Each item exposes its label, its value and its state word.
- Extensions: status items only through the D-SL-02 slots (see StatusItem). Extensions cannot change the bar height or colours, and cannot place views inside the bar.

## GPUI notes

Draw the bar as a 24 px row on `surface.canvas`, placed under the Canvas column only. Items are flex children with caption text and 12 px glyphs. Use tabular figures for counts. Draw the visibility menu and tooltips in the overlay layer (`z.popover`). No shadow on the bar.

## React Native notes

Not shown on Android: the system status bar stays. Host connection and usage appear on host cards (M-HM-02). Where a host list needs taps, rows are 48 dp (`size.touch.large`).

## Desktop and Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Presence | Strip under the Canvas | None |
| Host state | Dot and count in the bar | Host cards |
| Density | Fixed 24 px | Not applicable |

## Copy

- Item labels and values: "Agent status", "1.22 GB", "3 terminals", "2 ports", "2 hosts".
- Empty values: "No ports", "No terminals".
- Visibility menu heading: "Status bar items". Hidden item note: "Hidden, window too narrow".
- Tooltips: "Agents: 1 waiting for you, 2 running"; "Disconnected. Last known state, 5 min ago."

## Token gaps and open points

Token gaps: none. The connection dot is `size.indicator.dot`; every item, the agent item included, uses 12 px icons.

Open points:
- The owner reference shows an agent "dot". This spec uses the AgentStatus glyph so the state has a shape. Confirm.
- Splitting items across the two groups (agent status on the left, the rest on the right) is a proposal. Confirm.
- Hidden items are reachable only through the visibility menu, with no overflow button. Confirm.
