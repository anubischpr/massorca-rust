# Terminal surface

## Purpose

Draws one terminal (a shell or an agent's terminal UI) as a grid of text cells, with scrollback, selection, find and the states a long-lived process needs: starting, running, exited, reconnecting and unreachable. The process and the terminal emulation live in the core; this component only paints cells and sends keys.

Inventory IDs: D-TM-02 (desktop terminal pane), M-TM-01 (Android terminal panel). Terminal tabs and split panes are D-TM-01 (Work panel tabs). Android key row: M-TM-02.

## Anatomy

- Pane: a Work panel tab body filled with `terminal.bg`. Cell padding `space.2` on all sides. Text style `terminal` (13 / 16, `terminal.fg`, user line-height 1.0 to 1.5, default 1.2), JetBrains Mono at weights 400 and 700 (bold only). Ligatures off.
- Header (optional, height `size.control.default`): shell or agent icon 16 px, title in `label` (for example "zsh, oppm"), a host chip when the terminal runs on another host (Badge, "VPS"), the state glyph and word, and an icon button for the pane menu. Surface `surface.canvas` with a `border.subtle` rule under it. The header is hidden when the tab strip already shows the title and the setting "Terminal headers" is off.
- Cells: foreground and background from `terminal.ansi.*` (16 colours) and 256 and truecolor values. Default foreground `terminal.fg`. Bold uses weight 700; faint reduces to `text.tertiary`-like contrast by blending in the core; italic, underline, strikethrough and inverse are supported.
- Cursor: `terminal.cursor`. Focused: filled block (or bar or underline by setting), blink follows the OS setting. Unfocused: 1 px hollow block.
- Selection: `terminal.selection` behind the text; `terminal.fg` stays at least 4.5:1 on it.
- Minimum contrast: any foreground the program draws is adjusted to at least `terminal.min-contrast` (default 4.5) against its cell background. `terminal.ansi.black` (dark) and `terminal.ansi.white` (light) are exempt as backgrounds.
- Scrollbar: thin overlay, thumb `border.strong`, `radius.xs`, visible on hover and while scrolling.
- Links: detected URLs and file paths are underlined while the modifier key (`Mod`) is held and the pointer is over them; `Mod+click` opens them.
- Overlays on the pane (all drawn by the app, not the shell):
  - Exit row at the bottom: "Process exited with code 1" with a "Restart" button, on `surface.raised` with `border.default`, `radius.md`, margin `space.2`.
  - Banner row at the top for "Restored from earlier" (the pane shows history, not a live process), "Reconnecting to VPS" and "Can't reach VPS" (see Banner).
  - Jump chip: "Jump to bottom" pill at the bottom right while scrolled up and new output arrives; `surface.overlay`, `shadow.e2`.
  - Size badge: "120 x 32" in `caption` on `surface.overlay` for 800 ms after a resize.

## Variants and sizes

| Variant | Difference |
|---|---|
| Shell | Header title is the shell and folder. |
| Agent TUI | An agent's own terminal UI; header shows the agent icon; keys are passed through unchanged. |
| Floating terminal (D-TM-13) | Same surface in a floating frame; no header. |
| Android | Rendered in a WebView or a native view (renderer open); text 12 sp (`terminal` Android size), key row above the keyboard (M-TM-02), an input line dock (M-TM-03). |

Font size follows Settings (D-ST-05). Rows and columns are computed from the pane size and the cell metrics.

## States

| State | Visual | Tokens |
|---|---|---|
| starting | Empty surface; a spinner and "Starting shell" after 200 ms | `agent.running`, `terminal.fg` |
| restoring | "Restoring" while the core replays the screen | `terminal.fg` |
| running, focused | Filled cursor; focus is shown by an inset 2 px ring at the pane edge | `terminal.cursor`, `border.focus` |
| running, unfocused | Hollow cursor, no ring | `terminal.cursor` |
| hover | Pointer is an I-beam; links underline with `Mod` held | `terminal.fg` |
| selected | Selection fill | `terminal.selection` |
| disabled | Input ignored while exited, restoring or unreachable; the cursor is hidden | not applicable |
| loading | See starting and restoring | `agent.running` |
| empty | A fresh shell shows its prompt; there is no empty-state graphic | `terminal.fg` |
| exited | Output stays; the exit row appears with the code and Restart; the cursor is hidden. A clean exit (code 0) reads "Process finished" | `status.danger.fg` only on a non-zero code, with the word "exited" |
| error | The process could not start: ErrorState in the pane with the reason and "Retry" | `status.danger.fg` |
| reconnecting | SSH dropped: last output kept, dimmed by drawing at reduced contrast no lower than 4.5:1; Banner "Reconnecting to VPS" with a spinner; input is queued by the shell session layer only if the transport supports it, otherwise ignored with a visible note | `agent.running` spinner, `terminal.fg` |
| restored | The reconnect worked: Banner "Reconnected to VPS", then it fades after 5 seconds | `status.success.fg` |
| disconnected host | Last output kept with its age. Banner: "Can't reach VPS. Last output from 5 min ago. The process may still be running." The pane never says exited or stopped | `agent.unreachable`, `terminal.fg` |
| long content | Scrollback is limited by the setting (default 10,000 lines); a very long line wraps by the terminal's rules, not by the surface | `terminal.fg` |

## Behaviour (handoff)

- **Keyboard.** When focused, every key goes to the process, except the app's reserved keys. Reserved on all platforms: `F6` and `Shift+F6` (region focus), `Ctrl+Tab` (recent tab switcher), copy and paste (`Ctrl+Shift+C` and `Ctrl+Shift+V` on Windows and Linux, `Cmd+C` and `Cmd+V` on macOS), `Mod+Shift+B` and the other Work panel shortcuts that use `Shift`. On macOS `Cmd` shortcuts are app shortcuts and `Ctrl` goes to the shell. On Windows and Linux, shortcuts that shells use (`Ctrl+F`, `Ctrl+E`, `Ctrl+K`, `Ctrl+P`, `Ctrl+T`, `Ctrl+W`) go to the shell while a terminal has focus; the command palette opens with `Ctrl+Shift+P` (pending-owner-review item 12); `Ctrl+Shift+F` stays file search, and the terminal has no find in v1. `Shift+Page Up` and `Shift+Page Down` scroll the scrollback; `Shift+Home` and `Shift+End` go to the top and the bottom.
- **Copy.** Selecting does not copy (a setting offers copy on select). Copy needs the copy shortcut or the context menu, so the shell's `Ctrl+C` interrupt keeps its meaning.
- **Paste.** Pastes with bracketed paste when the program asks for it. Multi-line pastes into a shell that does not use bracketed paste show a confirm (X-DLG-01) listing the first lines. Dropped files paste their quoted path.
- **Focus.** Opening a terminal focuses it. `F6` leaves it. Clicking the pane focuses it. Focus order in the Work panel: tab strip, pane.
- **Pointer.** Click and drag selects; double-click selects a word; triple-click a line; `Alt+drag` (Option on macOS) selects a block. The wheel scrolls the scrollback, or sends arrow keys to a program that asked for mouse reporting. Right-click opens the menu: Copy, Paste, Select all, Find, Clear, Restart (when exited).
- **Resizing.** Columns and rows follow the pane after a 100 ms debounce, and the core sends the new size to the process. The size badge shows while dragging a splitter.
- **Motion.** None. Cursor blink follows the OS setting and stops when reduced motion is on. The jump chip fades over `duration.base`.
- **Accessibility.** A terminal is not readable by a screen reader through the grid. The pane exposes `role="application"` named "Terminal: zsh, oppm" and, when screen reader mode is on (setting), mirrors the visible lines to an off-screen text region updated in bursts, announcing new output politely after 500 ms of quiet. This depends on the desktop accessibility spike (design system question 1). State changes (exited, reconnecting, unreachable) are announced.
- **Extension limits.** The Terminal surface is not an allowed component in extension views. Extensions may add context menu items to the pane menu (contribution point 13, trailing group) and a status bar item for terminal count; they cannot draw into the pane or read its contents.

## GPUI notes

- One custom element paints the whole grid: merge adjacent cells with the same style into runs, draw background quads per run, then the shaped text per row, then the cursor and selection quads. Never create one element per cell.
- Cache shaped lines per row by content hash and style; repaint only dirty rows. Use a fixed-advance font so a column is `col * cell_width`.
- Draw box-drawing and block characters procedurally (quads) so they connect without seams at any scale.
- The emulator (alacritty_terminal or vte) runs in the core. The view receives damage events. Do not parse escape sequences in the render thread.
- IME composition and the cursor position for it must come from the grid cursor.
- Native webview terminal (Android) is not applicable on desktop.

## React Native notes (Android)

- The renderer choice is open (Phase 6): a WebView with a web terminal, or a native text-grid view. Either way: 12 sp mono, key row above the keyboard (Esc, Tab, arrows, Ctrl; 44 dp keys, 48 dp hit), a one-line input dock, pinch to change size. TalkBack: output is not announced automatically; a "Read last output" action reads the last 10 lines. Selection by long-press with handles.
- The reconnecting and unreachable banners are the same Banner component, at the top of the panel.

## Desktop vs Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Keys | Full keyboard, reserved set above | Key row and input dock |
| Copy and paste | Shortcuts and menu | Long-press handles and sheet |
| Header | Optional | App bar carries the title |
| Renderer | GPUI custom element | WebView or native view (open) |
| Text size | 13 px | 12 sp |

## Copy

- States: "Starting shell", "Restoring", "Process exited with code 1", "Process finished", "Restart", "Jump to bottom".
- Banners: "Restored from earlier. This is history, not a running process.", "Reconnecting to VPS", "Reconnected to VPS", "Can't reach VPS. Last output from 5 min ago. The process may still be running."
- Menu: "Copy", "Paste", "Select all", "Find", "Clear".
- Paste confirm: "Paste 12 lines?" with "Paste" and "Cancel".

## Token gaps and open points

Token gaps: none. The cell padding is `space.2`; the line-height setting (1.0 to 1.5, default 1.2) is a user setting with its range in this spec, not a token.

Open points:
- Which shortcut wins in a terminal on Windows and Linux for find, dictation and the command palette (G16 in app-flow). The proposal above lets the shell win and offers Shift variants; the owner decides.
- Input typed while reconnecting; whether the session layer buffers it is a transport question.
- Android renderer (reuse the WebView terminal or go native).
- Screen reader support (design system question 1).
