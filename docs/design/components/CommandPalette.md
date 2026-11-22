# CommandPalette

## Purpose

The one palette for jumping to workspaces, tabs, project actions and commands, opened with Mod+K. It has three modes: quick open (go to file, D-CP-02), search in files (D-CP-03) and settings search (D-CP-04).

Inventory IDs: D-CP-01 (Desktop), with D-CP-02, D-CP-03 and D-CP-04 as its modes. Android: none in v1.

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Scrim | `scrim` | Whole window, behind the panel |
| Panel | `surface.overlay`, `border.default`, `shadow.e3`, `radius.xl` | Width `layout.palette.width` (600, as drawn on artboard `ws-palette`); top offset `layout.palette.top` (96) |
| Mode label and input | `text.primary` input text; `text.tertiary` placeholder; bottom rule `border.subtle` | Input row height `layout.palette.input-height` (44, as drawn on `ws-palette`); the field inside is `size.control.default` (28), centred |
| Group header | `overline` type style, `text.tertiary` | 16 px line |
| Result row | `fill.hover.overlay` on hover; `fill.selected.overlay` and a 2 px `accent.fg` bar when active | Height `size.control.default` (28) |
| Icon | `text.secondary`; a file result shows its file-type icon (A46 to A48); a session result shows its agent icon in full colour (A13) | `size.icon.sm` (16) |
| Shortcut | Kbd: `mono-sm` on `surface.sunken`, `radius.xs` | Trailing edge |
| Results list | ScrollArea | Maximum height `layout.palette.max-height` (480) |

## Variants and sizes

- One panel with four content types: recent items, results, no match and loading.
- Result groups run Workspaces, Sessions, Tabs, Commands (A13). A Sessions row starts with the agent's icon, then the session title, "project / workspace" in `caption` `text.tertiary`, and the state or last activity at the trailing edge.
- Modes are shown as a label at the input's left: "Go to file", "Search in files", "Settings".
- Below the palette width, the panel fills the window minus `space.4` on each side.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | `surface.overlay` | Opens with the input focused; an empty query shows recent items. |
| Hover | `fill.hover.overlay` | Shows the row fill and does not move the active row. |
| Pressed | `fill.pressed.overlay` | While the pointer is down. |
| Focus-visible | `border.focus` | The input keeps focus; the active row shows its fill and bar. |
| Selected or active | `fill.selected.overlay`, `accent.fg` bar | The row that Enter runs. |
| Disabled | `text.disabled` | A command that cannot run now stays listed, disabled, with its reason in caption text. |
| Loading | `fill.skeleton` | Skeleton rows while search in files runs. |
| Empty | `text.secondary` | Recent items; with none, the line "Type to search". |
| No match | `text.secondary` | "No matches for" followed by the query. |
| Error | `text.secondary` | Missing ripgrep shows an inline message row, with the install command in `mono-sm`. |
| Long content | `text.primary` | Names truncate at the end; file paths truncate at the start so the file name stays visible. |
| Writing | `border.focus` | Typing filters the results; matching characters take `match.bg`. |

## Behaviour

- Keyboard: Mod+K opens the palette. Up and Down move the active row; Enter runs it; Escape closes it and returns focus to the control that opened it. Mod+P opens quick open, and Mod+Shift+F opens search in files. Tab does not leave the input. Settings search is reached from the palette list.
- Focus order and ring: the palette traps focus while open, as a dialog does (design system 8.3). The input keeps focus, and the active row is announced through the active descendant.
- Pointer: click runs a row. A click on the scrim or outside the panel closes it.
- Responsive: the panel keeps its width, centred near the top. Narrow windows follow the margin rule above.
- Motion: opens like a dialog, with scale 0.98 to 1 and opacity over `duration.moderate` and `ease.enter`; the scrim fades with it. Closes with `ease.exit`. Reduced motion: instant.
- Accessibility: role `dialog`, named "Command palette". The input is a combobox that controls a listbox; rows are options with `aria-selected`. Result counts are announced once per change, politely.
- Extensions: extensions register commands (title 60 characters, category 24). They cannot hide a command from the palette, cannot override a core shortcut without consent, and may only suggest shortcuts.

## GPUI notes

Draw the scrim as a full-window quad in `scrim`, the one translucent token besides shadows. Draw the panel in the overlay layer at `z.dialog` (50). Use a text input with IME support. Virtualise long result lists. Draw each shortcut as a small rounded rectangle with mono text. No blur behind the scrim.

## React Native notes

Not used on Android in v1. If a palette-like search is needed, use a Sheet with a search field and ListRow results.

## Desktop and Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Entry | Mod+K and the mode shortcuts | Per-screen search |
| Container | Centred overlay | Sheet, if designed |
| Keyboard | Full set | Hardware keyboard basics |

## Copy

- Placeholder: "Search workspaces, tabs and commands".
- Group headers: "Recent", "Workspaces", "Sessions", "Tabs", "Commands", "Files", "Settings".
- Modes: "Go to file", "Search in files", "Settings".
- No match: "No matches for" plus the query.
- Missing search tool: "Search in files needs ripgrep installed."

## Token gaps and open points

Token gaps: none. Width, input row height, maximum height and top offset are `layout.palette.*` tokens.

Open points:
- How a user enters each mode is not defined. This spec uses Mod+P, Mod+Shift+F and a palette entry for settings search. Confirm.
- Recent items show five entries (proposed). Confirm.
