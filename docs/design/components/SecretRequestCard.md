# Secret request card

## Purpose

A card in the conversation that asks the user for a secret (an API key, token, username or password) when the user asked how to give one or when the agent finds it needs one. It shows the variable name and a state, never a value. Its Add button opens the Secret modal.

Inventory ID: X-CH-07 (Android: the same card in the session chat, M-CH-01). Owner requirement 2026-11-12; app-flow journey J14.

## Anatomy

- Container: `surface.raised` with `border.default`, `radius.lg`, padding `space.3`, no shadow. While pending it uses the same "needs you" treatment as an approval card: fill `agent.waiting.bg`.
- Header row: key icon 20 px in a 28 px plate (`surface.sunken`, `radius.md`, icon `text.secondary`), title in `body-strong`, and on the right the state: glyph plus word ("Waiting for you", "Stored", "Linked", "Declined", "Not saved").
- Title text: "Claude needs a secret" (agent name from the session) or, when the user asked, "Add a secret for oppm".
- Variable name chip: `mono-sm` on `surface.sunken`, `radius.sm`, `border.subtle`, padding `space.half` `space.2`, for example `OPENAI_API_KEY`. Long names break at any character; the maximum is 64 characters.
- Reason line (`body-sm`, `text.secondary`, up to 200 characters): why the agent needs it, as plain text. No Markdown, links or images in this line.
- Actions (pending): "Add secret" (primary) and "Decline" (secondary), right-aligned, `space.2` apart.
- Safety line: lock icon 12 px and "The value is stored on VPS and never goes into the chat." in `caption` `text.tertiary`, naming the host that executes the work.
- Resolved states keep the card as a compact row (height `size.control.comfortable` plus padding): state glyph, name chip, scope text ("for project oppm", "linked from project NexusCare") and, on Stored and Linked, an "Open vault" text link (`accent.fg`).

## Variants and sizes

| Variant | Difference |
|---|---|
| Agent needs one | Title names the agent; reason line shown. |
| User asked | Title "Add a secret for <project>"; no reason line; the name field in the modal starts empty. |
| Compact (resolved) | One-row summary as above. |
| Android | Full-width card, actions stack full width at 48 dp, Add first. |

## States

| State | Visual | Tokens |
|---|---|---|
| pending | Full card with Add and Decline, tinted. Glyph: `agent.waiting` circle with question mark and the words "Waiting for you" | `agent.waiting.bg`, `agent.waiting` |
| hover | Buttons use their own hover; the card does not change | `accent.hover`, `fill.hover.raised` |
| pressed | Buttons pressed fill | `accent.pressed`, `fill.pressed.raised` |
| focus-visible | Ring on the focused button | `border.focus`, 2 px, offset 2 px |
| entering | The modal is open: both buttons disabled, the state line reads "Entering secret" | `text.secondary` |
| stored | Compact row: check circle, "Stored", "for project oppm", host name | `status.success.fg`, `surface.raised` |
| linked | Compact row: link icon, "Linked", "from project NexusCare" | `text.secondary`, `accent.fg` for the link text |
| declined | Compact row: ban icon, "Declined"; the agent is told | `text.secondary` |
| disabled | "Add secret" disabled while the host is unreachable, with the visible line below | `fill.disabled`, `text.disabled` |
| error (failed) | octagon-alert, "Not saved: <reason>", buttons Retry and Decline | `status.danger.fg`, `status.danger.bg` row |
| loading | Not applicable on the card (the modal shows saving) | not applicable |
| empty | Not applicable | not applicable |
| long content | Name chip wraps at any character; the reason line wraps to 3 lines, then truncates with the full text in a tooltip | `text.secondary` |
| disconnected host | Pending card stays pending. Add is disabled and the line "Can't reach VPS. Nothing can be saved until it is back." shows with the unreachable glyph. Nothing is queued (A32). Decline still works | `agent.unreachable` |

## Behaviour (handoff)

- **Where it comes from.** The user asks ("How do I give you the OpenAI API key for the project?") or the agent proactively reports it needs a secret. In both cases the app shows this card; the message that triggered it never carries a value.
- **Add.** Opens the Secret modal with the variable name filled in. The card stays pending until the modal closes. If the user saves, the card becomes Stored (or Linked); if they cancel the modal, the card is unchanged.
- **Decline.** Marks the card Declined and tells the agent the owner declined; the agent carries on without the secret or stops (A33).
- **Delete at the owner.** If a linked or stored secret is later deleted, the next request for that name returns as a new pending card (A35); the old card stays as history.
- **Keyboard.** Tab order: Add secret, Decline. Enter or Space activates. There is no shortcut that adds or declines. The card never takes focus on its own; a polite announcement says "Secret requested: OPENAI_API_KEY". When the modal closes, focus returns to the control that opened it (Add secret), as in 8.2; if that control no longer exists, focus moves to the card container.
- **Pointer.** Buttons only. The name chip is selectable text; copying the name is allowed.
- **Resizing.** Follows the message column. Under 480 px actions stack, Add first.
- **Motion.** None beyond the transcript's own. State change swaps content instantly.
- **Accessibility.** `role="group"` named "Secret request: OPENAI_API_KEY". The state is a live status line ("Stored for project oppm"). The lock line is part of the group's description.
- **Security.** The agent-supplied title, name and reason are untrusted: render them as plain text, limit lengths as above, validate the name (letters, digits, underscores, not starting with a digit; A29) and never accept a value over this card's protocol. The card is core chrome: no extension can create it, restyle it, or read its state (contribution point 6 explicitly excludes receiving secret values).

## GPUI notes

- A normal transcript block. Keep the state in the session model (pending, stored, linked, declined, failed) so the card renders identically after a restart.
- Never route text from the modal through this card's view state; the card only holds name, scope label, host label and state enum.
- Disabled "Add secret" is a real disabled button with `aria-describedby` to the unreachable line.

## React Native notes (Android)

- Same blocks in the chat list. Buttons 48 dp, full width, Add first, 8 dp apart. Add opens the modal sheet. TalkBack reads the group name, the state and the lock line; the pending state announces on arrival.

## Desktop vs Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Buttons | Right-aligned, 28 px | Full width, 48 dp |
| Modal | Centred dialog | Full-height bottom sheet |
| Open vault | Link in the compact row | Link in the compact row (opens the list screen, M-VT-01) |

## Copy

- Titles: "Claude needs a secret", "Add a secret for oppm".
- States: "Waiting for you", "Stored", "Linked", "Declined", "Not saved: <reason>".
- Actions: "Add secret", "Decline", "Retry", "Open vault".
- Safety line: "The value is stored on VPS and never goes into the chat."
- Host: "Can't reach VPS. Nothing can be saved until it is back."
- Entering: "Entering secret".

## Token gaps and open points

- Token gap: none.
- Open: wording of the title when the agent names a service ("Claude needs a secret to call the OpenAI API") is agent-supplied; the card shows it as the reason line only.
- Open: team-scope secrets (G24): who besides the team's bots sees the name and usage is not decided; the card says "for team Desarrollo" only.
- Open: whether the card can offer "Link existing" directly; today it is in the modal only (A28).
