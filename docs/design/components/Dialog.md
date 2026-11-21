# Dialog

## Purpose

A centred modal for a decision: a confirm (default), a destructive confirm that names what is lost and counts the affected items, an error that stops an action, or a short one-field form (rename, naming an untitled file). It is the one confirm pattern (X-DLG-01) that replaces about twenty separate confirms.

Inventory IDs: X-DLG-01 (both platforms). Absorbs D-OB-07, D-AC-03, D-HO-03, D-HO-04, D-TM-04, D-TM-05, D-AG-16, D-ED-05, D-OT-14, D-VT-02, M-WS-07 and the confirms listed in X-DLG-01.

## Anatomy

- Scrim: full-window layer in `scrim` (dark `#000000a6`, light `#14141e52`), behind the dialog.
- Container: `surface.overlay`, `border.default` 1 px, radius `radius.xl` (12 px), `shadow.e3`. Width `layout.dialog.width.md` (440 px) on desktop; `layout.dialog.width.sm` (400 px) for single-field and confirm dialogs that need less room; Permission review uses `layout.dialog.width.lg` (560 px). Android: `layout.dialog.android.min` to `layout.dialog.android.max` (280 to 560 dp). Padding `space.4` (16 px).
- Title: `title` (16 / 24 / 600), `text.primary`. A question or a noun phrase: "Delete secret DB_PASSWORD?".
- Body: `body` (13 / 20 / 400), `text.secondary`. Max three lines of plain text, then a list.
- Loss list (destructive only): up to five rows, `body` with the counts in tabular figures ("3 projects", "2 routines stop"). Beyond five: "and 4 more" as a last row.
- Footer: right-aligned Buttons, `space.2` (8 px) apart. Order left to right: Cancel (secondary), then the action (primary or danger), so the action is always last.
- Close: no X in destructive dialogs; Escape and Cancel close. A non-destructive dialog may have an IconButton close in the title row.

## Variants and sizes

| Variant | Action button | Body |
|---|---|---|
| default confirm | primary | one sentence on what happens |
| destructive | danger, named for the loss: "Delete secret (3 projects use it)" | what is lost, a loss list with counts, and "This cannot be undone" when true |
| error | primary "Retry" or "Close" | what failed and what is kept |
| one-field | primary | one TextField with its label |

Android: a dialog of 280 to 560 dp width, buttons at the bottom right, 48 dp.

## States

| State | Dialog |
|---|---|
| opening | opacity and scale 0.98 to 1 over `duration.moderate` with `ease.enter`; scrim fades with it |
| default | as above; focus starts on Cancel for destructive, on the primary for default confirm |
| destructive | focus starts on Cancel; the danger button is never the default focus or the Enter target |
| busy | action button shows a spinner (`size.icon.sm`, `duration.spinner`) and the label stays ("Deleting"); both buttons disabled; Escape is ignored until the action ends |
| error | a status line in `status.danger.fg` with `octagon-alert` and the words sits above the footer; the dialog stays open; the action button becomes "Retry" |
| empty | not applicable |
| long content | body scrolls after 40 percent of the window height; title and footer stay fixed |
| disconnected host | the action is disabled, with "Host unreachable. Last known state, 5 min ago" as body text |

## Behaviour (handoff)

- Keyboard: Escape closes unless busy. Tab and Shift+Tab cycle inside the dialog (focus trap). Enter activates the focused button; on destructive dialogs Enter on the default focus (Cancel) closes. Mod+Enter is not used.
- Focus: moves into the dialog on open; on close, focus returns to the control that opened it. Removing the opener moves focus to its neighbour.
- Pointer: click on the scrim closes a default confirm and a one-field form only when nothing is typed. Destructive dialogs close only by their buttons or Escape.
- Resizing: fixed width; height grows with content up to 80 percent of the window.
- Motion: opacity and scale 0.98 to 1 over `duration.moderate`, `ease.enter`; exit with `ease.exit`. Scrim fades. Reduced motion: instant.
- Accessibility: role `alertdialog` for destructive and error variants, `dialog` otherwise. `aria-labelledby` the title, `aria-describedby` the body. The loss list is a list.
- Extensions: an extension may open a Dialog only through the host API. It may not change the copy of a destructive confirm's loss list, the button order, or the danger style. Security chrome (Permission review, Secret modal, destructive confirm) cannot receive extension content.

## GPUI notes

- Overlay layer element with the scrim as a full-window `div` at `scrim` colour (alpha is part of the token).
- Focus trap is a key handler on the dialog element; focus returns to the stored handle on close.
- Shadow `shadow.e3`; confirm the blur convention in the pinned GPUI version.

## React Native notes

- A Dialog component built on Modal with `accessibilityViewIsModal`; focus is trapped by the platform. Android back closes unless busy.
- Buttons 48 dp; body 14 sp / 20 / 400; the loss list scrolls inside the dialog.

## Desktop vs Android

- Centred dialog on desktop; on Android the same dialog for confirms, sheets for everything else (see Sheet).
- Enter and Escape on desktop; back gesture on Android.

## Copy

- Title is a question or a short noun phrase: "Delete secret DB_PASSWORD?", "Remove project NexusCare?".
- Destructive body names the loss: "This removes the secret from 3 projects and 1 team. Agents that use it lose access. This cannot be undone."
- Action names the loss: "Delete secret (3 projects use it)". Cancel is "Cancel".
- Error body: what failed and what is kept: "Could not delete. The secret is still there."
- Never write a result verb before the result: not "Deleted".

## Token gaps and open points

Token gaps: none. Widths are `layout.dialog.*` tokens; the danger button uses `status.danger.solid`, `status.danger.hover` and `status.danger.pressed` (see Button).

Open points:
- Whether the scrim click should close one-field forms with typed text. Spec says no.
