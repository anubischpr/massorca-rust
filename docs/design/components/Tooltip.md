# Tooltip

## Purpose

A short text label that explains an icon, a truncated value or a disabled control on hover or keyboard focus. It describes; it never holds an action, a link or a form.

Inventory IDs: none of its own. Used by every IconButton (its name), truncated rows and tabs, the Kbd shortcut hints, disabled controls that give a reason, and extension items (which name the extension).

## Anatomy

- Container: `surface.overlay`, `border.default` 1 px, radius `radius.lg` (8 px), `shadow.e2`.
- Padding: `space.1` (4 px) vertical, `space.2` (8 px) horizontal.
- Text: `body-sm` (12 / 18 / 400), `text.primary`. Long text wraps.
- Optional shortcut: Kbd (mono-sm) after the text, separated by `space.2`.
- Arrow or caret: none. Placement is above the trigger by default, below when there is no room.
- Max width: `layout.tooltip.max-width` (240 px). Wraps to three lines, then truncates with an ellipsis.

## Variants and sizes

| Variant | Use |
|---|---|
| label | one short phrase, for icon buttons and tabs |
| label and shortcut | a phrase plus its Kbd, for commands with a shortcut |
| reason | a phrase that gives why a control is disabled ("Host unreachable. Last known state, 5 min ago.") |
| full text | the full value of a truncated row, title or path |

One size. Desktop only; Android has no tooltips.

## States

| State | Tooltip |
|---|---|
| hidden | not rendered |
| pending | waiting for `duration.tooltip-delay` (400 ms) after the pointer rests; no delay for keyboard focus |
| shown | fades in with opacity only, `duration.fast` |
| moving between triggers | the next tooltip shows at once, with no delay |
| hidden by press | hides when the trigger is pressed, and stays hidden until the pointer leaves and returns |
| disabled trigger | still shows, because the reason is the content |
| long content | wraps to three lines, then truncates with an ellipsis |
| error | not applicable. A tooltip never reports an error; errors go to the region. |
| empty | never shown without text |

## Behaviour (handoff)

- Keyboard: a tooltip shows at once on keyboard focus of its trigger and hides on blur or Escape. Escape closes the tooltip only; it does not reach the trigger's own action.
- Focus: a tooltip never takes focus and never holds a control.
- Pointer: shows after the rest delay; hides on pointer leave or press.
- Resizing: stays inside the window, flips side when needed.
- Motion: opacity only, `duration.fast`, `ease.enter` for show and `ease.exit` for hide. Reduced motion: instant.
- Accessibility: the trigger references the tooltip with `aria-describedby`. For an IconButton the tooltip text is also the accessible name, so the tooltip is the description and the name at once.
- Extensions: an extension's tooltip starts with the extension's name (identity rule). It may not contain a link, a button or a form.

## GPUI notes

- Overlay element on the overlay layer, positioned against the trigger bounds; not a child of the trigger.
- Show and hide timing lives on the window's timer, not in the trigger's render path.
- Shadow `shadow.e2`; confirm the blur convention in the pinned GPUI version.

## React Native notes

- Not used on Android. Accessible name and hint come from `accessibilityLabel` and `accessibilityHint` on the control. Disabled reasons go in visible text beside the control.

## Desktop vs Android

- Hover and keyboard focus tooltips on desktop. None on Android.

## Copy

- A verb phrase for an action ("New terminal"), a noun phrase for a value ("~/projects/NexusCare/oppm-api").
- Reasons state the fact and the age: "Host unreachable. Last known state, 5 min ago."
- Sentence case, no full stop needed for one phrase; use full stops for two sentences.

## Token gaps and open points

Token gaps: none. The maximum width is `layout.tooltip.max-width`.

Open points:
- Whether touch users on desktop-tablet should see tooltips. Spec says no; Android uses labels.
