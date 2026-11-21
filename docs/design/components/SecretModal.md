# Secret modal

## Purpose

The dialog where the user enters a secret's variable name and value, chooses where it applies (project, team or global) or links an existing secret, and saves it to the host that runs the work. The same dialog edits and rotates secrets from the Secret vault screen. The value never enters the chat, the model context or any log.

Inventory ID: X-VT-01 (Android: the same modal on the phone; the Vault list is D-VT-01 and M-VT-01). Owner requirement 2026-11-12; app-flow journeys J14 and J14b.

## Anatomy

- Dialog: `surface.overlay`, `border.default`, `radius.xl`, `shadow.e3`, `scrim` behind. Width `layout.dialog.width.md` (440 px), maximum height 90 percent of the window, padding `space.4`. Header, scrolling body, pinned footer. (On Android: a bottom sheet, `radius.2xl` top corners, full height when the keyboard is open.)
- Header: title in `title` ("Add secret", "Edit secret", "Rotate secret", "Link a secret") and a close icon button (x, `size.control.default`).
- Info line under the title in `body-sm` `text.secondary`: "Agents get it as an environment variable when they run. It is stored on VPS and never goes into the chat." The host is named.
- Fields (label above control, label `label`, gap `space.2`, fields `space.4` apart):
  1. Variable name: TextField, `mono`, placeholder "OPENAI_API_KEY". Help text "Letters, digits and underscores. Cannot start with a digit." Read-only in edit and rotate, and in link mode.
  2. Value: masked TextField (`mono`) with a Show button (eye icon, `aria-pressed`) at its right end. Hidden by default; shown only on request, and re-hidden after 10 seconds, on blur of the field, and on close. Accepts pasted multi-line values (a certificate or private key); a multi-line value shows "Multi-line value, 12 lines" instead of its text when hidden.
  3. Scope: SegmentedControl "Project | Team | Global" with the project name on the first segment ("Project oppm"). Project is preselected. Team appears only in Bot Teams mode or in a project that belongs to a team (name shown "Team Desarrollo"). A caption under it explains the choice: "Only agents working in oppm can use it." / "Every project and bot of the team can use it." / "Every project on VPS can use it."
- Link row: a text button "Link an existing secret instead" (`accent.fg`) below the Scope field. In link mode the Value field is replaced by a list of the owner's secrets held in other scopes: radio rows with the name (`mono`), owning scope chip ("Project NexusCare"), and last rotated date (`caption`). Empty list: "No secrets in other scopes. Enter a new value instead." with a "Back" text button. The name is fixed to the linked secret's name.
- Footer (top `border.subtle`): "Cancel" (secondary) and the primary action: "Save secret", "Rotate", or "Link secret". Error and host messages sit above the footer as a Banner.

## Variants and sizes

| Variant | Fields | Primary action |
|---|---|---|
| Add (from the card or the vault) | Name, value, scope, link option | Save secret |
| Edit | Name read-only, value (optional), scope read-only | Save secret |
| Rotate | Name read-only, new value only | Rotate |
| Link | Name read-only, list of secrets | Link secret |
| Android | Same fields, 48 dp targets, one column, sheet | Same |

Edit keeps the name fixed. Rotate changes only the value; running sessions keep the value they started with and the next run uses the new one (A30).

## States

| State | Visual | Tokens |
|---|---|---|
| entering | Fields empty or prefilled; Save disabled until the name is valid and the value is not empty | `surface.overlay`, `fill.disabled` for the disabled primary |
| hover | Fields: border steps up; buttons their own | `border.strong` |
| pressed | Buttons pressed fills | `accent.pressed`, `fill.pressed.overlay` |
| focus-visible | Field border and ring in `border.focus`; buttons ring 2 px, offset 2 px | `border.focus` |
| selected | Scope segment: `fill.selected.overlay` plus check or bold label; link row selected: radio filled | `accent.default` |
| invalid name | Field border `status.danger.border`, message under it with octagon-alert glyph: "Use letters, digits and underscores. It cannot start with a digit." | `status.danger.fg` |
| name in use | Message "OPENAI_API_KEY already exists in this scope." with two text buttons: "Pick another name" and "Link the existing secret" | `status.danger.fg`, `accent.fg` |
| saving | Spinner in the primary button after 200 ms, fields read-only, Cancel stays enabled (a save in flight is not cancelled; the dialog says so on Cancel) | `agent.running` |
| stored | The dialog closes at once; the card shows Stored. No success screen | not applicable |
| host unreachable | Banner above the footer: "Can't reach VPS. Nothing can be saved until it is back." with the unreachable glyph; primary disabled; fields stay editable; nothing is queued (A32) | `agent.unreachable`, `fill.disabled` |
| failed | Banner `status.danger.bg`: "Not saved: <reason>. Nothing was stored." Retry is the primary action again; the typed value stays in the field for the retry | `status.danger.fg`, `status.danger.border` |
| loading | Link list loading: 3 skeleton rows | `fill.skeleton` |
| empty | Link list empty, as above | `text.secondary` |
| long content | Name over 64 characters: "Use up to 64 characters." Value has no length limit up to 64 KB; larger values are refused with a message. A long name wraps in the link list | `status.danger.fg` |

## Behaviour (handoff)

- **Security rules.** The value is held only in the field's buffer while the dialog is open. It is never written to view state that can be serialised, to logs, to the accessibility tree (the field exposes "Secret value, hidden"), to the clipboard by the masked field (copy is blocked while hidden), to screenshots of the debug tools, or to the transcript. Closing the dialog, by any means, zeroes the buffer. Autofill and autocomplete are off. Paste is allowed.
- **Opening.** From the card (Add), from the vault (Add, Edit, Rotate), or from an extension's settings secret reference through the host API (name only). Initial focus: the Value field when the name is prefilled, otherwise the Name field.
- **Keyboard.** Tab order follows the visual order: Name, Value, Show, Scope, link button, Cancel, primary. Enter submits when the form is valid and the focus is not in a multi-line value. Escape closes without confirmation and discards what was typed (the secret can be typed again; a confirm would add friction and risk leaving a value behind). Left and Right change the Scope segment. The dialog traps focus; on close focus returns to the card's Add button or the vault row that opened it.
- **Validation.** The name rules are A29: letters, digits and underscores, not starting with a digit; unique within the scope. A name already in use offers another name or linking the existing secret.
- **Scope and links.** A link creates a reference, not a copy: one value, many references. A linked secret is read-only where linked; updating the value at the owner updates every link; deleting at the owner deletes it everywhere (the vault warns with the count). The variable name stays the same in every linked scope.
- **Saving.** Stores on the executing host in its OS secret store, or an encrypted store on a headless server (G22). Refused while the host is unreachable.
- **Resizing.** Fixed width `layout.dialog.width.md` (440 px); in windows narrower than 480 px it uses the window width minus `space.4`. The body scrolls; header and footer stay.
- **Motion.** The dialog fades in with a 0.98 to 1 scale over `duration.moderate`, `ease.enter`; the scrim fades with it. Show and hide of the value are instant. Reduced motion: instant.
- **Accessibility.** `role="dialog"` with `aria-modal="true"`, named by the title. Fields have labels; errors are tied with `aria-describedby` and announced politely when they appear. The Show button has `aria-pressed` and the name "Show value". Success is announced by the card after the dialog closes: "OPENAI_API_KEY stored for project oppm".
- **Extension limits.** This is security chrome: it always uses core components and validated tokens. No extension can place content in it, restyle it or read the value. An extension may only ask the host to open it for a given name.

## GPUI notes

- Use the shared Dialog layer (deferred, `z.dialog`, scrim quad). The value field is a secure text element: it paints bullet glyphs per character, disables copy and cut while hidden, excludes itself from accessibility text, and clears its buffer in `Drop`.
- Do not store the value in the dialog's model; keep it in the field entity only, and pass it to the core with a single `save` call that takes ownership and zeroises after.
- Focus trap: a focus scope on the dialog root; restore the previous focus handle on close.

## React Native notes (Android)

- A bottom sheet (`radius.2xl`) with a 90 percent maximum height measured above the keyboard (IME inset). It does not expand to full height when the keyboard opens; its content scrolls. Fields 48 dp; the Show button is a 48 dp target; `secureTextEntry` toggles; `importantForAutofill="no"`; set `FLAG_SECURE` on the window while the sheet is open so screenshots and recents do not capture it.
- Scope is a segmented control with 48 dp height. TalkBack announces errors as they appear and the field as "Secret value, hidden".

## Desktop vs Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Container | Centred dialog, `layout.dialog.width.md` (440 px) | Bottom sheet, full height with keyboard |
| Screenshots | Not controllable | Blocked with `FLAG_SECURE` while open |
| Vault actions | Add, edit, rotate, delete, unlink | Vault list and add only; edit, rotate, delete, unlink on desktop (A36) |

## Copy

- Titles: "Add secret", "Edit secret", "Rotate secret", "Link a secret".
- Labels: "Variable name", "Value", "Scope", "Project oppm", "Team Desarrollo", "Global".
- Help: "Letters, digits and underscores. Cannot start with a digit."
- Link: "Link an existing secret instead", "No secrets in other scopes. Enter a new value instead."
- Buttons: "Cancel", "Save secret", "Rotate", "Link secret", "Show", "Hide".
- Messages: "OPENAI_API_KEY already exists in this scope.", "Can't reach VPS. Nothing can be saved until it is back.", "Not saved: <reason>. Nothing was stored."

## Token gaps and open points

Token gaps: none. The width is `layout.dialog.width.md`.

Open points:
- Team scope visibility (G24) and several hosts (G25): a secret lives on one host and cannot be linked across hosts; the link list shows only secrets on the same host.
- The headless-server store (G22): how it is unlocked is not designed; the info line says "encrypted store" when that is the case.
- Auto-hide after 10 seconds is a proposal.
