# AgentStatus

## Purpose

The one status glyph for an agent or session: running, waiting for you, idle, done, error or unreachable. Each state has its own shape and a word, and colour adds to the shape. It appears in rows, the status bar, tabs and cards, fed by the shared status store.

Inventory IDs: X-AG-01 (Both). Android uses the same glyphs, with TalkBack names.

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Glyph | `agent.running`, `agent.waiting`, `agent.idle`, `agent.done`, `agent.error`, `agent.unreachable` | 12 px in rows and in the Status bar (`size.icon.xs`); 16 px in tabs and the Delegation tree (`size.icon.sm`); 20 px in cards (`size.icon.md`) |
| Row tint, waiting | `agent.waiting.bg` | Full row |
| Row tint, error | `agent.error.bg` | Full row |
| Dimmed row, unreachable | `text.tertiary` for text; no tint in Sidebar and list rows | Age in `caption` |
| Tint, unreachable (banner, notification card, Delegation tree row) | `agent.unreachable.bg` | Full banner, card or row |
| Child work count | `text.secondary`, `caption` | Beside the parent glyph |

## Variants and sizes

- Six states, one shape each: running (3/4 arc), waiting (filled circle with a question mark), idle (hollow ring), done (circle with a check mark, `circle-check`), error (triangle with an exclamation mark), unreachable (dashed ring with a slash).
- Three sizes: 12, 16 and 20 px. Stroke weights follow the 24 unit grid: 3 at 12 px, 2.25 at 16 px and 2.1 at 20 px.
- A question folds into waiting. Child work shows as a count on the parent row.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | per state | The glyph takes its state colour; the state word is the accessible name. Running turns; idle has no tint. |
| Waiting for you | `agent.waiting`, `agent.waiting.bg` | Filled circle with a question mark; the row is tinted. |
| Hover | none | Not interactive; the row owns hover. |
| Pressed | none | Not interactive. |
| Focus-visible | none | Decorative inside its row; the row shows the ring. |
| Selected or active | none | The row carries the selection. |
| Disabled | none | Not used; an unknown state is unreachable, not disabled. |
| Loading | none | No glyph until the first state arrives. Never show idle as a placeholder. |
| Empty | none | No session, no glyph. |
| Done | `agent.done` | Circle with a check mark; no tint. The colour aliases `status.success.fg`. |
| Error | `agent.error`, `agent.error.bg` | Triangle with an exclamation mark; the row is tinted. |
| Long content | none | The glyph never truncates; the row name truncates. |
| Disconnected host | `agent.unreachable`, `text.tertiary` (`agent.unreachable.bg` on banners, cards and Delegation tree rows) | Dashed ring with a slash; the row dims to text colour, with no opacity; the age reads "Last known: running, 5 min ago". |
| Writing | N/A | Not an input. |

## Behaviour

- Keyboard: none of its own. The containing row takes focus, and the state word is read with the row.
- Focus order: not a stop. The row or tab owns focus.
- Pointer: none of its own. A tooltip on a glyph in the status bar or a tab shows the state word after 400 ms.
- Responsive: fixed sizes; the glyph never scales below 12 px. Text around it scales.
- Motion: the running arc turns over `duration.spinner` (800 ms) with `ease.linear`. It keeps turning under reduced motion, as functional progress (design system 4.7). Other states do not animate, and tints do not fade.
- Accessibility: role `image`. The name is the state word: "Running", "Waiting for you", "Idle", "Done", "Error", or "Unreachable, last known state 5 min ago" (design system 8.4). Child work joins the row name, for example "Running, 2 child tasks".
- Extensions: extensions show agent states only through these glyphs. They cannot draw their own agent glyph (design system 7.1).

## GPUI notes

Draw the arc as a 3/4 path and rotate it each frame over 800 ms with linear easing. Draw the ring, the dot, the check circle and the triangle as paths in the state colour. If the GPUI stroke has no dash pattern, draw the dashed ring as short arcs. Row tints are solid fills (`agent.waiting.bg`, `agent.error.bg`). Dim the unreachable row by text colour, not by element opacity.

## React Native notes

Draw the same glyphs with `react-native-svg` at 12, 16 and 20 dp. The glyph has no tap target of its own; the row does, at 48 dp (`size.touch.large`). The glyph's label is the state word, with the role "image".

## Desktop and Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Names | Tooltip on hover and focus | TalkBack label |
| Sizes | 12, 16 and 20 px | 12, 16 and 20 dp |
| Motion | Spinner keeps turning under reduced motion | Same; the spinner is functional progress |

## Copy

- States: "Running", "Waiting for you", "Idle", "Done", "Error", "Unreachable".
- Dimmed row text: "Last known state, 5 min ago".
- Child work: "2 child tasks".

## Token gaps and open points

Token gaps: none. Every value uses a token from `tokens.json`; `agent.done` and `agent.unreachable.bg` were added for this spec and the Delegation tree.

Open points:
- Unreachable rows dim with `text.tertiary` instead of opacity, because the design system has no translucency for rows. Measured: `text.tertiary` on `surface.frame` is 6.4:1 in dark and 5.4:1 in light, and on `agent.unreachable.bg` 6.0:1 and 5.4:1 (design system 2.14). Decided: passes; the owner can confirm by eye on the dark Frame.
- Status bar items draw the glyph at 12 px (`size.icon.xs`, matching `size.statusbar`); tabs keep 16 px. Decided (pending-owner-review item 31).
- The child work count placement and wording are proposals. Confirm.
- A colour-blind-safe variant is open question 4 (design system 2.7).
