# Menu

## Purpose

A list of actions or options that opens from a button, from a row's overflow, or from a right click. Desktop menus and context menus are anchored overlays; on Android the same content is a Sheet.

Inventory IDs: none of its own. Used by D-WS-02 (workspace row menu), D-SH-03 (title bar overflow), the context menus of workspace, session, file, tab, message, selection and diff line, the Select options and the Picker's overflow.

## Anatomy

- Container: `surface.overlay`, `border.default` 1 px, radius `radius.lg` (8 px), `shadow.e2`, padding `space.1` (4 px). Width by content, from `layout.menu.min-width` (180 px) to `layout.menu.max-width` (280 px).
- Item: height `size.control.default` (28 px), padding `space.2` (8 px) horizontal, radius `radius.md`. Gap between icon and label `space.2`. Android: 48 dp.
- Icon: 16 px, `text.secondary` at rest, `text.primary` on hover or focus. Optional.
- Label: `body` (13 / 20 / 400), `text.primary`.
- Shortcut hint: right-aligned Kbd (mono-sm, `text.tertiary`). Shown only when the command has a shortcut.
- Submenu indicator: `chevron-right`, 12 px, at the trailing edge.
- Separator: 1 px, `border.subtle`, with `space.1` above and below.
- Group label: `overline` style, `text.tertiary`, optional.

## Variants and sizes

| Variant | Content |
|---|---|
| action | verb phrases with optional icon and shortcut |
| checkable | a leading check (`accent.fg`) on selected options; group radio when only one may be on |
| destructive | last item, `status.danger.fg` label and icon, after a separator; names what is lost |
| submenu | one submenu level; the parent item shows the chevron |

## States

| State | Item |
|---|---|
| rest | transparent on the overlay |
| hover | `fill.hover.overlay`; text stays `text.primary` |
| focus (keyboard) | `fill.selected.overlay` plus focus ring `border.focus` 2 px inset by 2 px, so it stays inside the menu |
| pressed | `fill.pressed.overlay` |
| selected (checkable) | check mark `accent.fg` and `fill.selected.overlay` |
| disabled | label and icon `text.disabled`; stays in the list with `aria-disabled`; its reason shows as visible caption text under the label, on desktop and Android |
| loading | one Skeleton row (`fill.skeleton`) while the list loads |
| empty | one line in `text.secondary` that says why ("Nothing to do here yet") with no item |
| error | one item in `status.danger.fg` that retries, plus the reason as a line above it |
| long content | labels truncate with an ellipsis; the menu width grows to `layout.menu.max-width` (280 px), then truncates |
| disconnected host | items that need the host are disabled with "Host unreachable" as their tooltip |

## Behaviour (handoff)

- Keyboard: opens with Enter or Space on the trigger, or Shift+F10 and the Menu key for context menus. Up and Down move the focus; Home and End go to the ends; a letter moves to the next item starting with it. Right opens a submenu; Left closes it. Enter activates; Escape closes and returns focus to the trigger. Tab closes the menu.
- Focus order: the menu takes focus on open, at the first enabled item.
- Pointer: hover moves focus. Click activates. Click outside closes without activating.
- Resizing: the menu repositions to stay inside the window; it flips above the trigger when there is no room below.
- Motion: opacity 0 to 1 and a 4 px move, `duration.base`, `ease.enter`; exit with `ease.exit`. Reduced motion: instant.
- Accessibility: role `menu`, items `menuitem`, `menuitemcheckbox` or `menuitemradio`. Name from the trigger. Destructive items are announced as such in the label ("Delete project, cannot be undone").
- Extensions: may add items to the named context menus in an "Extensions" group after a divider; five items per extension per menu; one submenu level. Cannot remove, reorder or precede core items; cannot run a destructive action without the core confirm (X-DLG-01).

## GPUI notes

- Menu is an overlay element on the overlay layer, anchored with the window-edge flip logic.
- Items are `div`s with the hover and selected fills precomputed for `overlay`.
- The keyboard model is a single handler on the menu element; items do not take focus individually.

## React Native notes

- On Android the menu is a Sheet: rows at 48 dp with icon, label and trailing shortcut omitted (no hardware shortcut hints).
- Each row is a `Pressable` with `accessibilityRole="menuitem"`; disabled rows set `accessibilityState.disabled`.
- Long-press actions also appear here (`accessibilityActions` mirror the list).

## Desktop vs Android

- Anchored overlay with keyboard model and shortcut hints on desktop; bottom Sheet with 48 dp rows on Android.
- Context menu on right click on desktop; long press on Android.

## Copy

- Action labels are verb phrases: "Rename", "Duplicate", "Copy path", "Open in editor".
- Destructive items name what is lost: "Delete workspace (uncommitted changes lost)".
- Separators group by intent, not by count.

## Token gaps and open points

Token gaps: none. Width limits are `layout.menu.min-width` and `layout.menu.max-width`.

Open points:
- Whether submenus open on hover on desktop or only on Right arrow. Spec says both, with a delay of `SUBMENU_HOVER_DELAY_MS` (150 ms; behaviour constant, design system 4.8).
