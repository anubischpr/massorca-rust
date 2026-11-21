# Conversation message list

## Purpose

The scrolling transcript of one session or one Conference room: user messages, agent turns, cards and system rows, in order, with a stable scroll position while text streams in. It hosts the blocks defined in MessageBlocks and sits above the Composer.

Inventory IDs: D-AG-03 (Conversation), D-AG-24 (Conference room), M-CH-01 (Android chat view). Its right-edge minimap is MessageMinimap (D-AG-28).

## Anatomy

- Scroll area: fills the Conversation on `surface.canvas`. Content column is centred with a maximum width of `layout.conversation.reading` (720 px) and side gutters of `space.4`. Never narrower than `layout.conversation.min` (360).
- Turn: one user message followed by the agent's reply blocks. Gap between turns `space.6`; gap between blocks in one turn `space.2` (related) to `space.3`.
- User message: right-aligned bubble, `surface.raised` with `border.default`, `radius.lg`, padding `space.3`, maximum width `layout.message.bubble.max-width` (85 percent of the column), text `chat`. Attachments and mention chips sit above the text.
- Agent content: no container. Blocks sit directly on the canvas (text uses `chat`, `text.primary`). Card-type blocks bring their own surface (see MessageBlocks).
- Conference room only: an author line above each run of messages from one bot: avatar `size.avatar.sm` (20), name in `body-strong`, a "Lead" badge for the lead bot, time in `caption` `text.tertiary`. In a single-agent session no author line is drawn.
- Day separator: centred `caption` `text.tertiary` between two `border.subtle` hairlines, shown when the date changes.
- New messages divider: a 1 px `accent.fg` line with the label "New" in `label` `accent.fg`, inserted once where the user last left off.
- Turn footer (last block of an agent turn): status glyph (AgentStatus), duration in `caption` tabular, and a copy icon button on hover or focus.
- Jump button: a pill at the bottom centre of the scroll area, `surface.overlay`, `border.default`, `shadow.e2`, `radius.full`, height `size.control.default`, an arrow-down icon and a label. Variants: "Latest" with an unseen count badge (`accent.default` fill, `text.on-accent`), and "Waiting for you" (glyph `agent.waiting`, tint `agent.waiting.bg`) when an approval or question is pending out of view.
- Message actions (hover or focus, top-right of the message): copy, fork from here, retry. Icon buttons at `size.control.compact`, `text.secondary`.

## Variants and sizes

| Variant | Difference |
|---|---|
| Session | Single agent. No author line. |
| Conference room | Author lines, lead badge, several agents in one list. A task chip appears under a message that produced a Board task. |
| Android | Full width, gutter `space.4`, text `a-chat` (16 / 24 sp), no minimap, long-press menus. |

Density does not change the list; type sizes are fixed. Only the gap tokens above apply.

## States

| State | Visual | Tokens |
|---|---|---|
| default | Messages on the canvas | `surface.canvas`, `text.primary` |
| hover (message) | Message actions appear; no fill | `text.secondary` icons, `fill.hover.canvas` on the icon button |
| pressed | Icon button pressed fill | `fill.pressed.canvas` |
| focus-visible | The focused message gets the ring, inset: 2 px `border.focus` around its box | `border.focus`, `border.focus.width` |
| selected | A message selected for a bulk action is not part of v1. Text selection uses `selection.bg` | `selection.bg` |
| disabled | Not applicable to the list. Message actions that need an unreachable host are disabled with a reason in the tooltip | `text.disabled` |
| loading | First load: three skeleton turns (a short right bubble, two text blocks of 3 and 5 lines) after 100 ms. Loading older history: a 16 px spinner row at the top after 200 ms | `fill.skeleton`, `agent.running` |
| empty | The Conversation's empty state (EmptyState): "Describe what you want done" with the Composer focused | `text.primary`, `text.tertiary` |
| error | ErrorState in place of the list: "Could not load this conversation", Retry. A failed single turn shows its own error block (MessageBlocks) | `status.danger.fg` |
| streaming | The running turn ends with `agent.running` arc and the word "Working"; text grows in place | `agent.running` |
| long content | Wraps. URLs, paths and identifiers break at any character (`overflow-wrap: anywhere`). A user message taller than `MESSAGE_CLAMP_LINES` (12 lines, design system 4.8) is clamped with "Show more". Code and tool blocks collapse per MessageBlocks | `text.secondary` for "Show more" |
| disconnected host | A Banner (X-CN-01) at the top of the list. Everything already loaded stays, unchanged. The running turn's glyph becomes the unreachable glyph with the words "Last known: running, 5 min ago". Nothing is marked stopped or failed. New messages typed now stay in the composer and are not queued (pending-owner-review item 25) | `agent.unreachable`, `text.tertiary` |

## Behaviour (handoff)

- **Scroll rules.** The list sticks to the bottom while the user is within `space.12` (48 px) of it: new content keeps the end in view. Scrolling up releases it; reaching the bottom again, or pressing End, re-arms it. When the user sends a message the list jumps to the bottom without animation. Prepending older history and streaming growth never move the content the user is reading (anchor to the first visible message).
- **Virtualisation.** Only rows near the viewport are built. Row heights are measured and cached; a measured height that changes (an image loads, a block expands) re-anchors the scroll offset so nothing jumps.
- **Keyboard.** The list is one tab stop with roving focus over messages. Up and Down move between messages; Home and End go to the first and last; Page Up and Page Down scroll a page; `Alt+Up` and `Alt+Down` jump to the previous and next user message (the same targets as the minimap ticks). Enter on a focused message moves focus to its first interactive element; Escape returns focus to the message. `Mod+F` opens the Find bar (FindBar). `Mod+C` copies the selection, or the focused message when nothing is selected.
- **Focus order.** Conversation region: list, then Composer. F6 moves in and out of the region. Interactive blocks (approval buttons, expanders) are reached with Tab from their message.
- **Pointer.** Text is selectable across blocks. Right-click opens the message menu (copy, copy as Markdown, fork from here, extension items in the trailing group). A click on the jump button scrolls to the target without animation under reduced motion, otherwise over `duration.moderate` with `ease.standard`.
- **Resizing.** The column follows the Conversation width up to 720 px. When the Conversation is narrower than 480 px the user bubble can take the full column and message actions move to the top-right inside the bubble.
- **Motion.** New rows appear without animation (virtualised lists never animate). The jump button fades in over `duration.base` with `ease.enter`. Reduced motion: no fade, no smooth scroll.
- **Accessibility.** The list is `role="log"` named "Conversation messages" with `aria-live="off"`; a separate polite live region announces one line when an agent turn finishes ("Claude finished") and when an approval needs the user ("Waiting for you: Run command"). Streaming text is never announced token by token. Each message is `role="article"` with a name made of author and time. The status glyph carries its word ("Running", "Unreachable, last known 5 min ago").
- **Extension limits.** A conversation card (contribution point 6) renders as a turn block between messages. It cannot appear as the user or the agent, edit or delete other messages, run an action without a click, or receive secret values. Its height is capped at 320 px with "Show more".

## GPUI notes

- Use the variable-height `list` element with a `ListState`, bottom-aligned, not `uniform_list`. Cache measured heights by message id and width; invalidate on width change.
- Streaming text should update one block's text runs, not rebuild the list. Throttle layout to one pass per frame.
- Cross-block text selection is not built into GPUI. It needs a selection layer that maps pointer positions to (block, offset) pairs. Treat it as a risk to spike early.
- Keep the jump button and the Banner as deferred layers over the scroll area so they do not take part in list layout.
- Never put per-message animation on the list. Cards animate their own height (SessionSummaryCard rules) only when they are in the Session summary.

## React Native notes (Android)

- Use FlashList (or an inverted FlatList) with `estimatedItemSize`; keep `maintainVisibleContentPosition` on so prepends and streaming growth do not jump. Content gutter 16 dp; text `a-chat`.
- Message actions: long-press opens a bottom sheet (copy, fork, retry). The same actions are exposed as `accessibilityActions`.
- The jump button is 44 dp with a 48 dp hit target and sits above the Composer, clear of the IME inset. When the keyboard opens, anchor to the last visible message.
- The Banner sits under the top app bar and pushes the list.
- TalkBack: the list is a list; each message is one focus stop with its blocks reachable inside it. Finished turns are announced with `AccessibilityInfo.announceForAccessibility`.

## Desktop vs Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Minimap | Yes (MessageMinimap) | No |
| Message actions | Hover or focus icon buttons, context menu | Long-press sheet |
| Find | FindBar, `Mod+F` | Not in v1 |
| Jump to message | `Alt+Up` / `Alt+Down`, minimap click | Scroll only |
| Conference room author line | Avatar 20 px | Avatar 28 px (`size.avatar.md`) |

## Copy

- Empty: "Describe what you want done." Hint under it: "Use @ to mention files and / for commands."
- Jump buttons: "Latest", "3 new", "Waiting for you".
- Divider label: "New".
- Disconnected line: "Last known: running, 5 min ago".
- Load error: "Could not load this conversation." Button: "Retry".
- Show more on a long user message: "Show more" / "Show less".

## Token gaps and open points

Token gaps: none. The bubble width is `layout.message.bubble.max-width`; the 12-line clamp is the behaviour constant `MESSAGE_CLAMP_LINES` (design system 4.8).

Open points:
- User messages are right-aligned bubbles by decision; the owner's references do not show them. Confirm.
- Cross-block text selection in GPUI (see notes); until it exists, selection is per block.
- Whether "fork from here" belongs in the v1 message actions (D-AG-05 is a dialog today).
