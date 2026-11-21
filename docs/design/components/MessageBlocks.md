# Message and turn blocks

## Purpose

The building blocks inside a turn of the Conversation: prose, code, tool calls, diff snippets, approval and question cards, delegation cards, review notes, errors and reasoning. Each block has one job, one collapsed form and clear states, so a long agent turn stays readable and what needs the user stands out.

Inventory IDs: D-AG-03 (blocks in the transcript), X-CH-02 (approval and question cards), X-CH-06 (delegation card and explanation), X-RV-02 (review notes). Android counterparts: M-CH-03, M-CH-06.

## Anatomy

All blocks sit in the message list column (maximum `layout.conversation.reading`). Blocks that are cards use `surface.raised`, `border.default`, `radius.lg`, padding `space.3` and no shadow inside the transcript.

**Text** (Markdown)
- Style `chat`, `text.primary`. Headings step down: `title-sm` for the largest, then `body-strong`. Lists indent `space.4`. Links `accent.fg`, underlined. Block quote: `text.secondary` with a 2 px `border.strong` rule on the left, padding-left `space.3`. Tables use `border.subtle` rules, header in `label` `text.secondary`.
- Inline code: `mono-sm` on `surface.sunken`, `radius.sm`, padding `space.half` `space.1`.

**Code block**
- `surface.sunken`, 1 px `border.subtle`, `radius.md`. Header row 28 px: language in `caption` `text.tertiary`, copy and wrap icon buttons (`size.control.compact`). Body `mono`, syntax tokens from `syntax.*`. No wrap by default, horizontal scroll. Taller than 24 lines: clamped to 12 lines with "Show all 86 lines".

**Tool call**
- Collapsed: a 28 px row, no container: status glyph 16 px (running arc `agent.running`, done `status.success.fg` check circle, error `agent.error` triangle), tool name in `mono-sm` `text.primary`, target in `mono-sm` `text.secondary` truncated, duration in `caption` tabular `text.tertiary`, chevron-right 12 px. Hover `fill.hover.canvas`.
- Expanded: input and output each in a `surface.sunken` well (`mono`, `radius.md`, `space.3` padding, output clamped to 12 lines). Several consecutive tool calls group under one header "4 steps" that collapses them.

**Diff snippet**
- Card: header row with file path (`mono-sm`, directory `text.tertiary`, name `text.primary`), counts `+12 -3` (`diff.add.fg`, `diff.remove.fg`, tabular, each with its sign) and an "Open in Work panel" ghost button. Lines use the DiffView line styles (`diff.add.bg`, `diff.remove.bg`, gutters, `+` and `-` glyphs). Shows up to 12 lines and "Show 18 more lines".

**Approval card** (X-CH-02)
- `agent.waiting.bg` fill while pending, `border.default`. Header: `agent.waiting` glyph (circle with question mark) and the words "Waiting for you", title in `body-strong` ("Run command?"). Body: the command or action in a `surface.sunken` well (`mono`). Actions: "Deny" (secondary) and "Approve" (primary), right-aligned, `space.2` apart.
- Answered: collapses to one line on `surface.raised`: check or x glyph, "Approved" or "Denied", the title and the time. The record stays.

**Question card** (X-CH-02)
- Same frame as the approval card. Single choice uses radio rows; multiple choice uses checkboxes; free text uses a TextField (multi-line). Submit "Send answer" is disabled until there is an answer. Answered state is a one-line summary of what was sent.

**Delegation card** (X-CH-06)
- One row: bot or agent icon (`size.avatar.sm`), "Delegated to Research" in `body-strong`, the task in `body-sm` `text.secondary` (one line), the child's AgentStatus glyph and word, and an "Open" ghost button. A "Why?" disclosure shows the agent's reason in `body-sm`. The tree of several children is DelegationTree.

**Review notes block** (X-RV-02)
- Card "Review notes sent" with the comment count; expands to a list of `file:line` (`mono-sm`) and the comment text.

**Error block**
- `status.danger.bg`, `status.danger.border`, `octagon-alert` glyph in `status.danger.fg`, the word "Failed" in `body-strong`, the message in `body`, and Retry when the action can be repeated.

**Reasoning block**
- Collapsed single line "Thinking" (`label`, `text.tertiary`) with a spinner while running and a chevron; expanded text in `body-sm` `text.secondary`.

## Variants and sizes

| Block | Variants |
|---|---|
| Tool call | collapsed, expanded, grouped |
| Approval, question | pending, submitting, answered, error |
| Delegation card | running, done, error, waiting for you, unreachable |
| Code, diff | clamped, expanded |

Android: the same blocks with `a-chat` text and `a-mono` code; cards use full width; actions are 48 dp high.

## States

| State | Visual | Tokens |
|---|---|---|
| default | As anatomy | `text.primary`, `surface.raised`, `surface.sunken` |
| hover | Header rows and tool rows fill; card actions appear | `fill.hover.canvas` (rows), `fill.hover.raised` (card headers) |
| pressed | Pressed fill | `fill.pressed.canvas`, `fill.pressed.raised` |
| focus-visible | Ring around the focused header or button | `border.focus`, 2 px, offset 2 px |
| selected / expanded | Chevron points down; body visible | `text.primary` |
| disabled | Approve, Deny, Send answer while submitting or when the host is unreachable | `fill.disabled`, `text.disabled` |
| loading | Tool call running: arc glyph, no duration yet. Approval submitting: spinner in the clicked button, both disabled | `agent.running` |
| empty | A tool call with no output shows "No output" in `text.tertiary` | `text.tertiary` |
| error | Error block, or an `agent.error` glyph on a failed tool call. A failed submit on a card shows an inline `status.danger.fg` message and re-enables the buttons | `status.danger.fg`, `agent.error` |
| long content | Code, output and diffs clamp as above. A tool target truncates with an ellipsis and its full text is in the tooltip and the expanded body. Free text in a question card grows to 8 lines then scrolls | `text.secondary` |
| disconnected host | A running tool call shows the unreachable glyph and "Last known: running, 5 min ago". A pending approval stays visible; its buttons are disabled with the visible line "Can't reach VPS. Your answer would not reach the agent." Nothing is shown as failed or stopped | `agent.unreachable`, `text.tertiary` |

## Behaviour (handoff)

- **Keyboard.** Non-interactive blocks are not tab stops. Headers that expand (tool call, code, diff, delegation "Why?", reasoning) are buttons: Enter or Space toggles. In an approval card Tab goes Deny, then Approve; Enter activates the focused button. There is deliberately no shortcut that approves: an approval needs a deliberate press on the button. Focus is never moved into a new card by itself; the list announces it and the "Waiting for you" jump button offers it.
- **Focus order.** Message by message (see ConversationMessageList). Within a card: header, body controls, Deny, Approve.
- **Pointer.** Click anywhere on a collapsed header to toggle. Code and output text is selectable. Copy icon buttons copy the block's raw text.
- **Resizing.** Cards follow the column width. In a column under 480 px approval buttons stack to full width, Approve first.
- **Motion.** Expand and collapse change height over `duration.base` with `ease.standard`; inner content keeps its final width. A new approval card appears without animation (list rule). Reduced motion: instant.
- **Accessibility.** Tool call header: button with `aria-expanded`, name "Read file, src/main.rs, done, 1.2 s". Approval card: `role="group"` named by its title; the pending state is announced once, politely: "Waiting for you: Run command". Diff lines carry "Added line" and "Removed line" for screen readers. Expanded output regions are `role="region"` named by the tool.
- **Extension limits.** An extension may contribute a conversation card (contribution point 6): header (icon and title), a view description, up to 3 actions, a 320 px height cap with "Show more". It may not render as an approval, question or secret request card, may not run an action without a click, and its name is always shown in its header. Approval and secret chrome is core only.

## GPUI notes

- Markdown becomes a tree of block elements with `StyledText` runs. Cache shaped text per block and width; stream by appending runs to the last block.
- Code and diff bodies are virtualised rows of fixed 20 px height when expanded beyond 100 lines.
- Expanding uses a clipped container whose height animates to the measured content height; measure once, off-screen.
- Selection across blocks needs the shared selection layer (see ConversationMessageList).
- Approval buttons live in a fixed footer row so a growing command body never moves them under the pointer. The buttons ignore clicks for 300 ms after the card appears under the pointer.

## React Native notes (Android)

- Each block is a list item subcomponent with a stable height cache. Code blocks scroll horizontally inside their own ScrollView; nested scroll must not steal the list's vertical gesture.
- Approval and question buttons: 48 dp targets, 8 dp apart, full width in a column; Deny and Approve never sit closer than 8 dp. Long-press on text offers copy.
- TalkBack: a card is one group; the pending approval announces on arrival; answered cards read their summary.

## Desktop vs Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Code line height | 20 px (`mono`) | 18 sp (`a-mono`) |
| Diff snippet action | "Open in Work panel" | "Open diff" (full screen, M-DF-01) |
| Card actions | Right-aligned buttons | Full-width stacked buttons |
| Hover actions | Copy icons on hover | Long-press menu |

## Copy

- Approval: "Waiting for you", "Run command?", "Approve", "Deny". Answered: "Approved", "Denied".
- Question: "Send answer".
- Delegation: "Delegated to Research", "Why?", "Open".
- Errors: "Failed" and the message; "Retry".
- Disconnected: "Can't reach VPS. Your answer would not reach the agent."
- Reasoning: "Thinking".

## Token gaps and open points

Token gaps: none. Syntax colours are validated on `surface.sunken` and `surface.canvas`; snippet lines on `diff.*` backgrounds use `text.primary` (decided with DiffView, pending-owner-review item 34).

Open points:
- Approval options beyond Approve and Deny (for example "Approve for this session") are not in the inventory; the card reserves a menu button next to Approve but ships without it.
- Whether answered approval cards stay in the transcript forever (assumed yes).
