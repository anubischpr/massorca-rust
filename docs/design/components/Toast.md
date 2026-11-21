# Toast

## Purpose

A short, passing message with a status glyph, an optional single action and a close button: "Removed from queue, Undo", "Could not rename the session, Retry", "Setup script failed, Open log". It reports a result; it never asks for a decision and never replaces a state that lives in a region.

Inventory ID: X-NT-01 (one host for toasts; absorbs the setup script toast, the local base ref refresh, the auto-rename failure with retry and the runtime disconnect toast, which is now a Banner). Android uses the same component as a snackbar-style toast.

## Anatomy

- Toast: `surface.overlay`, `border.default`, `radius.lg`, `shadow.e2`, width 360 px (minimum 320, maximum 400; `layout.toast.width.*`), padding `space.3`, minimum height 44 px.
- Status glyph 16 px at the left, in the status colour, with its shape as the cue:

| Kind | Glyph | Colour | Auto-dismiss |
|---|---|---|---|
| info | `info` | `status.info.fg` | 8 seconds |
| success | `circle-check` | `status.success.fg` | 8 seconds |
| warning | `triangle-alert` | `status.warning.fg` | 8 seconds |
| error | `octagon-alert` | `status.danger.fg` | stays until dismissed |

- Message: `body`, `text.primary`, at most three lines, then an ellipsis with the full text in a tooltip. Sentence case, plain words, no trailing full stop on a short fragment. The kind is also in the accessible name ("Error: ...").
- Action (optional, one): a ghost button at `size.control.compact` in `accent.fg` text ("Retry", "Undo", "Open log"). A toast with an action stays until dismissed or acted on, except an Undo toast, which times out after 8 seconds (`UNDO_TIMEOUT_MS`, the minimum on every platform).
- Close: x icon button at `size.control.compact`, `text.secondary`, tooltip "Dismiss", always visible.
- Stack: bottom-right of the Canvas, `space.4` from the right edge and from the Status bar, `space.2` apart, newest at the bottom of the toast group. Notification cards occupy the lowest slots; toasts stack above them, so cards never move when a toast arrives or leaves. At most 3 toasts are visible; older ones wait their turn.

## Variants and sizes

One component, four kinds. Android: full width minus `space.4`, minimum 48 dp, action 48 dp, anchored above the Composer or bottom bar and the IME and system insets.

## States

| State | Visual | Tokens |
|---|---|---|
| default | As above | `surface.overlay`, `text.primary` |
| hover | Timer pauses; the toast keeps its fill | `fill.hover.overlay` |
| pressed | Action or close pressed | `fill.pressed.overlay` |
| focus-visible | Ring on the focused action or close button | `border.focus`, 2 px, offset 2 px |
| selected | Not applicable | not applicable |
| disabled | The action is disabled while its request runs, with a spinner in the button | `text.disabled`, `agent.running` |
| loading | A toast for a running task is not used (use a Banner or the task's own state) | not applicable |
| empty | Not applicable | not applicable |
| error | The error kind as above | `status.danger.fg` |
| entering | Slides in 8 px and fades in over `duration.moderate` with `ease.enter` | motion tokens |
| leaving | Fades out over `duration.base` with `ease.exit` (opacity only); the stack closes the gap over `duration.base` with `ease.standard` | motion tokens |
| long content | Three lines then an ellipsis; a message longer than 120 characters is a sign the message belongs in a card or a Banner | `text.primary` |
| duplicates | An identical message within 2 seconds increments a counter chip ("x2") instead of adding a toast | `text.secondary` |
| disconnected host | A toast about an action on an unreachable host says so: "Couldn't reach VPS. Try again later." (error). It never says the work stopped. The runtime disconnect itself is a Banner | `status.danger.fg` |

## Behaviour (handoff)

- **Timing.** Timers pause while the pointer is over the toast, while a control inside it has focus, and while the window is not focused. The remaining time resumes afterwards. Reduced motion does not shorten or lengthen timers.
- **Honest wording.** A toast says a thing happened only when it did: not "Saved" before the store confirms, not "Deleted" before the delete is done. For results that take time use a Banner, a card or the task's own state.
- **Keyboard.** Toasts never take focus. `Mod+Alt+N` ("Focus notifications", the same command as for Notification cards) moves focus to the toast group: Up and Down move between toasts, Tab moves to a toast's action and close, Enter activates, Escape dismisses and returns focus to where it was.
- **Focus order.** Not part of the F6 cycle.
- **Pointer.** Click the action or the close button. The toast body is not clickable. Swipe is not used on desktop.
- **Rate limits.** Core features share one queue. An extension may send at most 1 toast per 5 seconds, and at most 3 of its toasts are visible.
- **Resizing.** Fixed width; narrower than 400 px the stack uses the Canvas width minus `space.4`.
- **Motion.** As in States. Reduced motion: instant.
- **Accessibility.** The stack is a region named "Notifications" (shared with cards). Each toast is `role="status"` (polite); error toasts are `role="alert"` (assertive). A toast is announced once as "<Kind>: <message>". The close button is named "Dismiss". The action keeps its label; its name includes the message ("Undo, Removed from queue").
- **Extension limits** (contribution point 14). Kind info, success, warning or danger; a message up to 120 characters and at most one action; at most 1 toast per 5 seconds per extension and 3 visible. The extension's name prefixes the text ("Acme: Build finished"), so it cannot imitate a core message. It cannot take focus, play a sound, bypass notification settings or OS do-not-disturb, or send OS notifications without the user's per-extension permission (X-NT-02).

## GPUI notes

- One deferred layer anchored to the Canvas bottom-right, owned by the notification service (`z.toast`). A toast is a normal element with an opacity and offset animation; the stack positions come from the sum of heights below each toast, animated when a toast leaves.
- Timer state lives in the service so it can pause on window blur. Hover and focus pause it from the view.
- The shadow is the box shadow token; do not add blur behind.

## React Native notes (Android)

- A bottom-anchored overlay above the system insets and the IME; minimum height 48 dp, full width minus `space.4`; the action is a 48 dp target. Auto-dismiss times as on desktop (8 seconds, the minimum for every timed toast). Announce with `AccessibilityInfo.announceForAccessibility`; swipe to dismiss plus a visible close button. "Remove animations": no slide, fade only becomes instant.

## Desktop vs Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Placement | Bottom right of the Canvas | Bottom, full width, above the Composer and insets |
| Width | 360 px | Full width minus `space.4` |
| Dismiss | Close button or Escape when focused | Close button, swipe |
| Elevation | `shadow.e2` | Elevation 6 dp |

## Copy

- Actions: "Retry", "Undo", "Open log", "Dismiss".
- Examples: "Removed from queue", "Could not rename the session", "Setup script failed", "Couldn't reach VPS. Try again later.", "Copied".
- Duplicate chip: "x2".

## Token gaps and open points

- Token gap: none.
- Open: whether error toasts without an action should also time out after a long delay (for example 30 seconds); the proposal keeps them until dismissed so a failure is never missed.
- Open: the single stack for cards and toasts (cards lowest) is a decision shared with NotificationCard.
