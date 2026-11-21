# Kbd

## Purpose

A keyboard shortcut badge that shows the real binding for the current OS. On Windows and Linux a shortcut reads Ctrl+K; on macOS it reads ⌘K. Kbd is the only way a shortcut is rendered in MassOrca: a one-off `<kbd>` is not allowed.

Inventory IDs: D-ST-04 (keybinding editor and shortcut display, Input and shortcuts). Also used in Tooltip, Menu shortcut hints, the command palette (D-CP-01) and extension command shortcuts (D-SL-07).

## Anatomy

- Container: height `size.kbd.height` (16 px), min width 16 px, padding `space.1` (4 px) horizontal, radius `radius.xs` (2 px).
- Fill: `surface.sunken`. Border: `border.subtle` 1 px.
- Text: `mono-sm` (12 / 16 / 400, JetBrains Mono), `text.secondary`. Glyphs use the same family.
- Chord: one Kbd per key, with `space.half` (2 px) gap between keys on macOS and a "+" between keys on Windows and Linux.

## Variants and sizes

| Variant | Use |
|---|---|
| key | a single key ("F6", "Esc") |
| chord | modifiers and a key ("Ctrl+K" or "⌘K") |
| inline | in a Tooltip or Menu row, aligned to the text baseline |

One size on desktop. Not rendered on Android unless a hardware keyboard is attached.

## Platform mapping

| Role | Windows and Linux | macOS | Note |
|---|---|---|---|
| Mod | Ctrl | ⌘ | the primary modifier |
| Shift | Shift | ⇧ | |
| Alt | Alt | ⌥ | Option on macOS |
| Enter | Enter | ↩ | |
| Escape | Esc | ⎋ | |
| Backspace | Backspace | ⌫ | |
| Tab | Tab | ⇥ | |
| F-keys | F6 | F6 | same on every OS |

Chord format: Windows and Linux join with "+" ("Ctrl+K", "Ctrl+Shift+F"); macOS joins glyphs with no separator ("⌘K", "⇧⌘F"). Order on Windows and Linux is Ctrl, Alt, Shift, then the key (`Ctrl+Alt+Shift+K`); on macOS it is ⌃ ⌥ ⇧ ⌘ then the key (design-system 8.1).

## States

| State | Kbd |
|---|---|
| default | as above |
| hover | not interactive; the row or button owns hover |
| selected | inherits the row's text colour, `text.primary` |
| focus | not focusable |
| disabled | `text.disabled`, no fill |
| loading | not applicable |
| empty | not rendered without a binding |
| error | a conflict is shown in the keybinding editor with `status.warning.fg` and the glyph `triangle-alert` and the word "Conflicts with Open file" (in D-ST-04), not in the Kbd itself |
| long content | not applicable; a chord never exceeds four keys |
| disconnected host | not applicable |

## Behaviour (handoff)

- Keyboard: none. Kbd shows a binding; it does not take input.
- Pointer: none.
- Resizing: fixed height; a chord wraps only at the row level, never inside a key.
- Motion: none.
- Accessibility: read as the key names, in words ("Control plus K", "Command K"). Decorative glyphs are `aria-hidden` and the words are in `aria-label`.
- Suggested bindings: must not use OS-reserved combinations (Alt+F4, Ctrl+Alt+Del, Win+L, Cmd+Q, Cmd+H, Cmd+M, Cmd+Tab, Ctrl+Esc). Extensions may suggest shortcuts; a clash with a core key loses and the clash shows in D-ST-04.
- Extensions: may suggest a shortcut for a command. The Kbd is rendered by the host, so the extension cannot change the format.

## GPUI notes

- Platform check at start-up picks the mapping table; the same table feeds the palette and the editor.
- Glyphs (⌘ ⇧ ⌥ ↩ ⌫ ⇥) must be in the bundled mono font or fall back to the OS font for the glyph only. Check glyph coverage in the font spike.

## React Native notes

- Only shown when a hardware keyboard is attached. Uses `Platform.OS` to pick the mapping; Android maps Mod to Ctrl.

## Desktop vs Android

- Desktop: shows bindings in tooltips, menus, the palette and the editor. Android: no shortcut hints unless a hardware keyboard is attached.

## Copy

- Names are the key labels, not the action: "Ctrl+K" next to "Open command palette".
- Use "Ctrl" and "Shift" in text on Windows; use "⌘" and "⇧" on macOS. Never mix.

## Token gaps and open points

Token gaps: none. The key cap height is `size.kbd.height`; the Model picker shortcut keys use the same height.

Open points:
- Glyph coverage for ⎋ and ↩ in JetBrains Mono is not confirmed; fallback to the OS font is assumed.
- Whether a chord uses "+" on macOS too. Spec says no, to match the platform convention.
