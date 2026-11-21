# Picker

## Purpose

One searchable picker for choosing a branch, base ref, folder, parent workspace, repo source or account, from the composer, PR base, remote folder browser and Android sub-pickers. It replaces the separate base-ref, parent and set-location pickers.

Inventory IDs: X-PK-01 (both platforms). Absorbs D-WS-14 base ref and parent pickers, D-HO-09 (remote folder picker), D-DF-06 (base branch for a new PR).

## Anatomy

- Trigger: a field-shaped button, as Select (height `size.control.default`, `surface.sunken`, `border.strong`, radius `radius.md`). Shows the current value; placeholder names the choice ("Choose a branch").
- Panel: on desktop an anchored Popover (`surface.overlay`, `border.default`, `radius.lg`, `shadow.e2`, width `layout.picker.width`, 320 px); on Android a Sheet.
- Search: SearchField at the top of the panel, focused on open.
- Body: a list of rows (ListRow, compact). Groups have an overline header (for example RECENT, ALL BRANCHES). Folder mode shows breadcrumbs in `mono-sm` and a parent row ".." at the top.
- Footer (optional): one ghost button for an action outside the list, for example "Browse folders" or "Create branch" with the typed name.

## Variants and sizes

| Variant | Use |
|---|---|
| list | branches, base refs, accounts, parent workspaces |
| folder | remote or local folder browser (D-HO-09, folder selection) |
| create-capable | list plus "Create branch "name"" as the first row when no match exists |

Panel width `layout.picker.width` (320 px) on desktop, the trigger's width if larger. Android: full-width Sheet, 90 percent max height (`layout.sheet.max-height`).

## States

| State | Panel |
|---|---|
| loading | Skeleton rows (`fill.skeleton`, three rows) under the search field. No spinner in the list. |
| empty | EmptyState (section size) with a title such as "No branches yet" and a one-sentence reason. Folder mode: "This folder is empty". |
| error | ErrorState (section size) with Retry; the last good list stays visible above it when there is one. |
| filtered | matches highlighted with `match.bg`; the active row uses `match.active.bg` at the top match |
| selected | the current value is marked with a check (`accent.fg`) and `fill.selected.overlay` |
| hover / focus | row fills from `fill.hover.overlay` / focus ring `border.focus` on the row |
| disabled | trigger disabled as Select |
| long content | row titles truncate with an ellipsis; the full path or name is in the row tooltip; folder breadcrumbs collapse the middle into "..." |
| disconnected host | list shows last known rows with "Last known state, 5 min ago" in the panel header; selection is still allowed for a path that already exists locally |

## Behaviour (handoff)

- Keyboard: Enter, Space or Down opens. Typing filters. Up and Down move the active row; Enter chooses it and closes the panel; Escape closes and returns focus to the trigger. In folder mode, Right enters a folder and Left or Backspace-on-empty-search goes to the parent.
- Focus order: trigger, then the search field, then the list, then the footer action.
- Pointer: click a row chooses it; click outside closes without a change.
- Resizing: the panel keeps 320 px desktop; on narrow windows it is anchored or moves to a Sheet-like overlay (the region rule).
- Motion: panel opacity and 4 px move in `duration.base` with `ease.enter`; rows appear without animation. Reduced motion: instant.
- Accessibility: trigger role `combobox`; the list is a `listbox` with `option` rows and `aria-selected`. Name from the field label. Loading announced once ("Loading branches").
- Extensions: may supply rows through the host API for a Picker a core command opens; cannot add their own Picker or change its layout.

## GPUI notes

- Panel is an overlay element anchored to the trigger; the list is a virtualised list (rows only are laid out).
- Match highlight is a run-level background.
- Keyboard state lives on the panel element; the search field keeps focus while the list is driven by arrow keys.

## React Native notes

- Trigger is a `Pressable` with `accessibilityRole="combobox"`. Panel is a Sheet with a search field at the top, rows at 48 dp, group headers in `overline` capitals.
- Folder mode uses a back row at the top, not a breadcrumb bar, to keep the 48 dp touch target.

## Desktop vs Android

- Anchored Popover on desktop with keyboard; full-width Sheet with a 48 dp row on Android.
- Desktop keeps breadcrumbs; Android uses a back row.

## Copy

- Trigger placeholder names the choice: "Choose a base branch", "Choose a parent workspace".
- Create row: "Create branch "{name}"".
- Empty folder: "This folder is empty". Empty list: "No branches match "{query}"".
- Error: "Could not load branches. Retry" with Retry as the action.

## Token gaps and open points

Token gaps: none. The panel width is `layout.picker.width`.

Open points:
- Whether the create row should always appear first or only when the query matches nothing. Spec says only when nothing matches.
