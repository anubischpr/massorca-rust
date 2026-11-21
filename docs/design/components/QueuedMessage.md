# Queued message

## Purpose

Shows follow-up messages that are waiting to be sent because the agent is busy or the host is not confirming, and lets the user send one now, edit it or remove it. Replaces the old send-now dialog with an inline confirm.

Inventory ID: X-CH-03 (Android: M-CH-04). Related: A10 (queued messages wait for the host to confirm the session state).

## Anatomy

- Dock: a block directly above the Composer, same width as the Composer (maximum `layout.conversation.reading`), `space.2` gap to it. It exists only while at least one message is queued.
- Header (shown from two messages): "Queued (2)" in `label` `text.secondary`, with a chevron to collapse the list.
- Row: height at least `size.control.comfortable` (32 px), padding `space.2` `space.3`. Left: clock icon 16 px (`text.tertiary`). Middle: the message text, one line, `body`, `text.primary`, truncated with an ellipsis; under it, in `caption` `text.tertiary`, the reason it waits. Right: three icon buttons at `size.control.compact`: send now (arrow-up), edit (pencil), remove (x), `text.secondary`.
- Container: `surface.raised`, `border.default`, `radius.lg`, `shadow.e1`. Rows inside are separated by `border.subtle`. A single queued message is one row in its own container.
- Inline confirm (replaces the row's content): text "Send now? The agent will stop what it is doing and read this." in `body-sm`, then two buttons: "Cancel" (secondary, focused) and "Send now" (primary).
- Inline editor (replaces the text): a multi-line TextField on `surface.sunken` with Save and Cancel.

## Variants and sizes

| Variant | Difference |
|---|---|
| One queued | Single row container, no header |
| Several | Header and a list; more than 3 rows shows the first 2 and a "+3 more" row that expands the list (the list then scrolls at 160 px) |
| Android | Rows 56 dp, actions in one overflow button (48 dp) plus Send now |

## States

| State | Visual | Tokens |
|---|---|---|
| queued, agent busy | Clock icon, caption "Waits for the agent" | `text.tertiary` |
| queued, host not confirming | Unreachable glyph, caption "Waits for VPS to confirm. Sends when it reconnects." Row stays active; Send now is disabled | `agent.unreachable`, `text.tertiary` |
| hover | Row fill; actions already visible | `fill.hover.raised` |
| pressed | Icon button pressed | `fill.pressed.raised` |
| focus-visible | Ring on the focused icon button or row | `border.focus`, 2 px, offset 2 px |
| sending | Arc spinner replaces the clock, caption "Sending", all actions disabled; the row leaves when the session confirms | `agent.running` |
| confirm | Inline confirm as above; Cancel focused | `surface.raised`, `text.secondary` icon, `text.primary` text |
| editing | Inline editor | `surface.sunken`, `border.strong`, ring `border.focus` |
| disabled | Send now disabled when the host is unreachable or the session is archived | `text.disabled` |
| error | Send failed: octagon-alert glyph, caption "Not sent: <reason>", actions Retry and Remove; the message stays queued | `status.danger.fg` |
| empty | The dock is not rendered | not applicable |
| loading | Not applicable (state is local) | not applicable |
| long content | One line with an ellipsis; the editor shows the full text; the tooltip shows the first 400 characters | `text.primary` |

## Behaviour (handoff)

- **Queueing.** Pressing Enter in the Composer while the agent is busy (or the host is not confirming) queues the message. Messages send in order, one at a time, as soon as the agent is ready and the host has confirmed the session state (A10). There is no reordering in v1.
- **Send now.** The arrow button on a row opens the inline confirm; confirming interrupts the current run and sends that message next, ahead of earlier queued ones (they stay queued behind it). It never sends on the first click.
- **Edit.** The pencil replaces the text with an editor in place. Enter saves, Shift+Enter inserts a line, Escape cancels. The message keeps its position.
- **Remove.** On desktop the x removes the row at once and shows a Toast "Removed from queue" with "Undo" for 8 seconds (`UNDO_TIMEOUT_MS`). On Android, Remove asks for confirmation first (`X-DLG-01`), as 5.4 requires for destructive actions. Undo restores the message at its old position.
- **Keyboard.** The dock is reachable by Tab after the Composer's controls. Rows are one tab stop each; inside a row, Left and Right move between the three actions (roving focus). Delete removes the focused row. Escape in the confirm or editor returns focus to the row.
- **Focus.** After Remove, focus moves to the next row, then the previous, then the Composer. After Send now (confirmed), focus returns to the Composer.
- **Pointer.** Click the text to start editing. Actions have tooltips with their shortcuts where one exists.
- **Resizing.** Follows the Composer width. Under 480 px the reason caption wraps under the text and actions keep their size.
- **Motion.** A row appears and leaves by height and opacity over `duration.base`, `ease.enter` and `ease.exit`. Reduced motion: instant.
- **Accessibility.** The dock is a list named "Queued messages". Each row is a list item whose name is the message text and the reason. Actions are named "Send now", "Edit", "Remove". State changes are announced politely: "Message queued", "Sending", "Message sent". The confirm is a `role="alertdialog"`-like inline group that does not trap focus but takes it on open.
- **Extension limits.** None. Extensions cannot add to or read the queue; a composer action cannot send a message (contribution point 11).

## GPUI notes

- A plain column of rows above the Composer, drawn in normal layout (not a floating layer), so it pushes the message list's visible height instead of covering it.
- Keep the queue in the core session state, not in the view, so it survives a window reload and is the same on desktop and phone.
- Inline confirm and editor swap the row's child tree; keep the row's focus handle stable so focus does not drop.

## React Native notes (Android)

- A FlatList (max 3 visible) above the Composer, anchored to the IME inset. Rows 56 dp. Primary tap on a row edits it. Actions: Send now as a visible 48 dp button; Edit and Remove inside a row menu (bottom sheet) and as `accessibilityActions` so swipe is never the only path.
- Swipe to remove is optional and always paired with the Undo snackbar (Toast).

## Desktop vs Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Actions | Three icon buttons per row | Send now plus an overflow menu |
| Confirm | Inline in the row | Inline in the row, buttons 48 dp |
| Reorder | None | None |
| Undo | Toast, 8 s | Snackbar-style Toast, 8 s |

## Copy

- Header: "Queued (2)". Reasons: "Waits for the agent", "Waits for VPS to confirm. Sends when it reconnects.", "Sending".
- Confirm: "Send now? The agent will stop what it is doing and read this." Buttons: "Cancel", "Send now".
- Remove toast: "Removed from queue". Action: "Undo".
- Failure: "Not sent: <reason>". Actions: "Retry", "Remove".

## Token gaps and open points

- Token gap: none.
- Open: whether "Send now" should interrupt a running tool call or wait for the current step to end. The design says interrupt, as the old dialog implied; confirm with the agent adapters.
- Open: no reorder control by decision; add drag handles later if users ask.
