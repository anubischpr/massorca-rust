# Composer

## Purpose

The box where the user writes to an agent or a team: text, attachments, mentions and commands, with the Model picker and the Send control. One component for sessions, the start page and Android chat.

Inventory IDs: D-AG-03 (Conversation composer), D-AG-01 (start page composer), M-CH-02 (Android composer and suggestions).


**One composer style (owner, 2026-11-12).** Every composer that needs placement (start page, empty app, Tasks) uses the Tasks layout: a placement bar attached to the top edge of the Composer (same `border.default`, top corners `radius.lg` plus 2, inset 12 px from the Composer sides, `surface.raised`) with borderless buttons for host, project and worktree; the Composer below with `radius.xl` corners; the "Chat | Terminal" switch sits in the Composer's bottom row next to the access chip, never in the placement bar. Session composers have no placement bar and keep the same Composer body.

## Anatomy

**Approved session body (owner, v47; agreements A29, A32, A33).** Every desktop composer uses the same body: placeholder "Ask anything, @ to mention files, / for commands"; bottom row `+`, access chip ▾ (for example "Workspace write"), then a flexible gap, the Model picker chip with the agent icon, microphone and a round Send; 12 px corners and a light shadow. Never a "Message Claude" field or a rectangular Send.

- Container: `surface.raised`, `border.default`, `radius.xl` (12 px), `shadow.e1`, padding `space.3`. Width follows the message column (maximum `layout.conversation.reading`), centred, with `space.4` gutters. It sits at the bottom of the Conversation, above the Status bar; QueuedMessage docks directly above it.
- Chip row (only when there is something): attachment and mention chips, `size.control.compact` high, pill shape (`radius.full`), `surface.sunken` with `border.subtle`, icon 16 px, name in `label`, a remove x on hover and focus. Images show a 20 px thumbnail instead of the icon. Overflow beyond one line wraps; past two lines it collapses to "+N".
- Text area: `chat` text, `text.primary`; placeholder "Ask anything, @ to mention files, / for commands" in `text.tertiary`. Minimum height 2 lines (`size.composer.min-height`, 44 px), grows to 8 lines (`size.composer.max-height`, 176 px), then scrolls. The caret is `text.primary`; selection `selection.bg`.
- Bottom row (height `size.control.default`, gap `space.2`):
  - Left: attach icon button (plus, opens the `+` menu: "Attach files", "Add from clipboard", extension composer actions). Then the access chip: shield icon 16 px, the access mode name in `label` (for example "Workspace write", "Ask for approval", "Full access") and a chevron-down 12 px; it opens a Menu of the agent's access modes, each with its icon. Access stays its own chip, not inside the Model picker (approved v47 desktop and v50 Android composers). In composers with placement (start page, empty start page, Tasks) the "Chat | Terminal" switch follows the access chip (SegmentedControl, sunken variant: no border, `surface.sunken` track, the active option in `fill.selected.raised`). Then up to 2 extension composer actions as icon buttons (contribution point 11).
  - Flexible gap.
  - Right: Model picker chip (agent icon, model, effort; see ModelPicker), a dictation icon button (mic, shown when voice is enabled), and the Send button. Tasks adds a "Start now" ghost chip (clock icon) before the Model picker chip.
  - All bottom-row chips and buttons (attach, access, Chat | Terminal, Start now, Model picker, mic) are borderless: no border and no fill at rest, `fill.hover.raised` and `text.primary` on hover, `fill.selected.raised` while their menu is open, the focus ring on keyboard focus. Chips use the pill shape (`radius.full`).
- Send button: `size.control.default` (28 px) circle (`radius.full`), arrow-up icon 16 px. Ready: `accent.default`, `text.on-accent`. Empty: `fill.disabled`, `text.disabled`. While the agent runs and the text area is empty it becomes Stop (square icon, `surface.raised` fill with `border.strong`, `text.primary`). With text while the agent runs it reads as Queue (arrow-up with a clock badge) and queues the message.

## Variants and sizes

| Variant | Difference |
|---|---|
| Session | As above. |
| Start page and empty start page (D-AG-01) | Text area minimum 3 lines (`size.composer.start-min-height`, 66 px). A placement bar (host `monitor`, project with the project icon, worktree `git-branch`, each a borderless button with its icon on the left) is attached to the Composer's top edge: same `border.default`, no bottom border, top corners `radius.lg` plus 2 (10 px), inset `space.3` from the Composer sides, `surface.raised`. The "Chat \| Terminal" switch sits in the bottom row, never in the bar. |
| Tasks | Centred at 760 px under a centred "Workspaces / Bot Teams" switch (sunken SegmentedControl); the same placement bar (in Bot Teams: team and project); bottom row access chip, Chat \| Terminal, Start now, Model picker, Send. |
| Conference room | Same body. Placeholder "Tell the team what you need, @ to mention a bot". The selector in the Model picker's place shows the lead bot (team character and name, for example "Mara") instead of a model. A bot mention chip carries the bot's character at `size.avatar.sm`. |
| Android | A floating card pinned above the keyboard, 1 to 6 lines: +, Model picker chip with the agent icon, reasoning bars, access lock, microphone and Send, in the desktop order. Send 44 dp visual with 48 dp hit target. A one-line pill carousel of options or suggestions sits above it and scrolls sideways; it never repeats agent, model and effort. |

## States

| State | Visual | Tokens |
|---|---|---|
| default | Placeholder, Send disabled | `surface.raised`, `border.default`, `text.tertiary` |
| hover | Border steps up | `border.strong` |
| pressed | Not applicable to the container; buttons use their own pressed fills | `fill.pressed.raised` |
| focus-visible | Container border becomes the focus ring (always shown when the text area has focus) | `border.focus`, 2 px |
| typing | Send ready; draft saved per session after 300 ms | `accent.default` |
| selected | Chips: selected chip (focused) shows the ring; text selection uses `selection.bg` | `border.focus`, `selection.bg` |
| disabled | The session is archived or the agent account needs sign-in (an unreachable host is the "disconnected host" state below): area fill and text, with the reason line "Sign in to Claude to continue" and its action | `fill.disabled`, `text.disabled` |
| loading | Starting a session (start page): Send shows a spinner and the area is read-only for the moment | `agent.running` |
| empty | As default | `text.tertiary` |
| error | A send failed: a line above the text "Message not sent: <reason>" with Retry, the draft stays | `status.danger.fg`, octagon-alert glyph |
| drop target | A file is dragged over: 1 px dashed `accent.fg` border, fill `accent.subtle.raised`, text "Drop to attach" | `accent.fg`, `accent.subtle.raised` |
| long content | Text area scrolls after 8 lines. A chip name truncates at 24 characters with an ellipsis, full name in the tooltip | `text.primary` |
| disconnected host | The Composer stays enabled, and the message stays in it: it is not queued (pending-owner-review item 25). A caption under the text area reads "VPS can't be reached. Your message stays in the composer." Send is disabled until the host is reachable | `agent.unreachable`, `text.tertiary` |

## Behaviour (handoff)

- **Send keys.** Enter sends (or queues while the agent is busy; while the host cannot be reached, Send is disabled and the message stays in the composer, never queued); Shift+Enter inserts a new line. `Mod+Enter` also sends. A setting "Enter inserts a new line" swaps them so that only `Mod+Enter` sends. While an IME composition is active, Enter never sends.
- **Stop.** The Stop button stops the run. With the text area empty and an agent running, Escape shows a hint "Press Escape again to stop" for 3 seconds; a second press stops. This avoids accidental interrupts.
- **Mentions and commands.** `@` opens the mention menu (X-CH-09): files, apps, teams, bots, projects; a pick becomes a chip. `/` at the start of a line opens the command menu. Menus are Popovers anchored to the caret; Up and Down choose, Enter accepts, Escape closes.
- **History.** Up in an empty Composer restores the last sent message; Down moves forward.
- **Attachments.** Paste (images and files), drag and drop, the `+` menu. Files over the size limit are refused with an inline line, not a toast. Dropped files from a remote host path are referenced, not uploaded, when the session runs on that host.
- **Model picker.** `Mod+Shift+M` opens it (rebindable). Tab order is: placement bar buttons (when present), text area, attach, access chip, Chat | Terminal (when present), extension actions, Start now (Tasks), Model picker chip, dictation, Send.
- **Focus.** Opening a session focuses the text area. Sending keeps focus there. A closed popover returns focus to its trigger.
- **Draft.** The draft is saved per session and restored on return, including chips. Switching sessions never loses it.
- **Resizing.** The Composer follows the column. Under 480 px extension action buttons collapse into the `+` menu.
- **Motion.** Height changes while typing are instant. The drop target border appears over `duration.fast`. Reduced motion: no change.
- **Accessibility.** The text area is a multi-line text box named "Message to Claude" (agent name) with the placeholder as description. Send is named "Send message", "Queue message" or "Stop", and states its disabled state. Chips are a list named "Attachments". Announcements: "Message queued", "Attached 2 files". Errors are polite status lines, tied to the area with `aria-describedby`.
- **Extension limits** (contribution point 11). An extension may add up to 2 composer actions (icon, tooltip label up to 32 characters, optional popover up to 320 px wide) that change attachments or draft text through a visible step. It cannot send a message, change the draft silently, or move or hide Send or the Model picker.

## GPUI notes

- GPUI has no ready multi-line text editor with IME, selection, undo and mentions. Plan a shared text input element used by the Composer, text fields and the terminal composer; spike IME on all three OSes first (Windows TSF, macOS NSTextInputClient, Linux IBus and Fcitx).
- Chips are inline elements inside the text flow when they come from `@`, and block elements in the chip row when they are attachments. Keep the mention chip as an atomic inline run (delete removes it as one).
- The container's focus ring is the border colour swap, not an extra outline, so the layout does not shift.
- Drop targets: handle file drop events on the container and draw the dashed border as a quad pattern, not a CSS style.

## React Native notes (Android)

- A TextInput with `multiline`, `maxHeight` 6 lines, inside a container that respects the IME inset (`KeyboardAvoidingView` or edge-to-edge insets). Send is 44 dp with a 48 dp hit target; the attach and model chips are 44 dp with 48 dp hit targets and 8 dp spacing.
- Attach opens a bottom sheet (files, camera, gallery). The Model picker is a sheet.
- Suggestions: a horizontal chip row above the field when the draft is empty; each chip is a 44 dp target.
- TalkBack: the field announces its hint; Send announces its disabled state; queued messages are announced.

## Desktop vs Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Send key | Enter, Shift+Enter, `Mod+Enter` | Send button; the keyboard's action key inserts a new line |
| Attach | Menu, paste, drag and drop | Sheet, paste |
| Mentions and commands | `@` and `/` menus (Popover) | `@` and `/` menus as an anchored list above the keyboard |
| Suggestions | None | Chip row above the field |
| Model picker | Popover chip | Sheet chip |
| Dictation | Overlay (`Mod+E`) | Keyboard dictation plus the mic button |

## Copy

- Placeholder: "Ask anything, @ to mention files, / for commands". Conference room: "Tell the team what you need, @ to mention a bot".
- Send labels: "Send message", "Queue message", "Stop".
- Hints and errors: "Press Escape again to stop", "Drop to attach", "Message not sent: <reason>", "VPS can't be reached. Your message stays in the composer.", "Sign in to Claude to continue".

## Token gaps and open points

Token gaps: none. The text area limits are `size.composer.min-height` and `size.composer.max-height`; the Send button diameter is `size.control.default`.

Open points:
- Escape twice to stop is a proposal; the owner has not set an interrupt key.
- Large pastes (over 2,000 characters) could become an attachment chip; not designed.
- Resolved (consistency pass, 2026-11-13): access is its own chip in the bottom row (A33), not only inside the Model picker; the "Chat | Terminal" switch survives in the bottom row of composers with placement (A30, A32).
