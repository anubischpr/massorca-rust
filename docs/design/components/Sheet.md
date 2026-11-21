# Sheet

## Purpose

The Android bottom sheet. It replaces the desktop Popover and Menu on phones and tablets: action lists, option pickers, Select and Picker options, the model picker, and secondary content. It is not used on desktop.

Inventory IDs: X-DLG-02 (Android side). Used by M-WS-02 (workspace action sheet), M-SE-05 (background work), the Android model picker (X-AG-02), Select and Picker on Android, and the Menu on Android.

## Anatomy

- Scrim: full-screen layer in `scrim`, behind the sheet.
- Container: `surface.overlay`, top corners `radius.2xl` (16 px), no bottom radius. Max height `layout.sheet.max-height` (90 percent of the screen). `shadow.e3` (Android elevation 12 dp).
- Drag handle: `size.sheet.handle.width` by `size.sheet.handle.height` (32 by 4 dp), `border.strong`, radius `radius.full`, centred, `space.2` (8 dp) from the top edge.
- Header (optional): `title` (18 / 24 / 600 on Android), `text.primary`, with a close button of 48 dp at the trailing edge. Height 56 dp (`size.appbar.android`): the 24 dp title line plus `space.4` above and below.
- Body: list rows at 48 dp (ListRow), or a SearchField at the top for long lists, or a form.
- Footer (optional): full-width primary button at 48 dp, `space.4` (16 dp) from the edges.
- Safe area inset at the bottom.

## Variants and sizes

| Variant | Use |
|---|---|
| action | list of actions with icons; destructive last, in `status.danger.fg` |
| option | single choice with check marks (Select and Picker options) |
| content | a view with a header and body, for the model picker and background work |

Heights: content sizes to its rows, up to 90 percent of the screen. No fixed sizes.

## States

| State | Sheet |
|---|---|
| opening | slides from the bottom over `duration.slow` with `ease.enter`; scrim fades |
| dragging | follows the finger; releases to open or closed depending on the distance and speed |
| closing | slides down with `ease.exit`; swipe, scrim tap, Back and the close button all close |
| default | rows as listed |
| loading | Skeleton rows (`fill.skeleton`) in place of rows |
| empty | one line in `text.secondary` with the reason, and the sheet's one action if there is one |
| error | ErrorState (section size) with Retry, inside the sheet |
| selected | option rows show a check in `accent.fg` and `fill.selected.overlay` |
| disabled row | `text.disabled`, row not pressable, `accessibilityState.disabled` |
| long content | the body scrolls; the header and footer stay fixed |
| disconnected host | the body shows last known rows with "Last known state, 5 min ago" |

## Behaviour (handoff)

- Keyboard: a hardware keyboard closes it with Escape. Focus moves to the sheet's first row on open and returns to the opener on close.
- Focus: the sheet traps focus while open.
- Pointer and touch: tap a row to act; drag the handle or the header down to dismiss; tap the scrim to dismiss unless busy.
- Resizing: content sized; the screen rotation keeps the sheet and recomputes its height.
- Motion: slide in and out with `duration.slow` and `ease.enter` or `ease.exit`; drag follows the finger. "Remove animations" from the system makes it instant.
- Accessibility: role `dialog` with the header as its name; the title is announced when the sheet opens. Every row has `accessibilityRole` and `accessibilityLabel`. Swipe-only or long-press-only actions are also `accessibilityActions`.
- Extensions: an extension may open a Sheet only through the host API and only on Android where the extension's view is supported (v1: none). It may not change the drag handle, the corner radius or the scrim.

## GPUI notes

Not applicable on desktop. The desktop equivalent is Popover or Menu.

## React Native notes

- Built on a bottom-aligned Modal with a gesture handler for the drag; the sheet's height is measured from its content.
- Rows 48 dp; header 18 sp; the scrim colour is `scrim`.
- Reanimated or the platform's native sheet are both acceptable; the motion uses the shared easing curves, not the platform default.

## Desktop vs Android

- Desktop: no Sheet. Popover and Menu are anchored.
- Android: Sheet for every secondary surface. Dialog for confirms.

## Copy

- Header names the set: "Workspace actions", "Model", "Background work".
- Rows are verb phrases for actions and nouns for options, the same copy as the desktop Menu or Picker.
- Destructive rows name the loss, the same copy as the desktop Menu item.

## Token gaps and open points

Token gaps: none. The handle is `size.sheet.handle.width` by `size.sheet.handle.height`; the header height is `size.appbar.android` (title line plus `space.4` above and below).

Open points:
- Whether Android should show a Dialog instead of a Sheet for a destructive action with a long loss list. Spec says Dialog for confirms, always.
