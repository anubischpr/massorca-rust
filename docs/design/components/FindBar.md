# FindBar

## Purpose

The strip at the top right of a pane that finds text in the terminal, chat, editor, browser or settings. It shows the match count, previous and next buttons, and a close button. It is one pattern; the scope changes with the pane.

Inventory IDs: D-FD-01 (Desktop). Android: none in v1.

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Strip | `surface.overlay`, `border.default`, `shadow.e2`, `radius.lg` | Width `layout.findbar.width` (360); height 36 (28 plus `space.1` padding) |
| Input | `surface.sunken`, `border.strong`; `border.focus` when focused | Height `size.control.default` (28) |
| Count | `caption` type style, `text.secondary`, tabular figures | Right of the input |
| Previous and next | IconButton; chevron-up and chevron-down (Lucide), `text.secondary` | Glyph `size.icon.sm` (16) in a 28 px target |
| Close | IconButton; x (Lucide), `text.secondary` | 28 px target |
| Match | `match.bg` | Behind each match |
| Active match | `match.active.bg` | Behind the current match |

## Variants and sizes

- One strip. The scope (terminal, chat, editor, browser, settings) changes what it searches, not how it looks.
- Mod+F opens it in the focused pane. A pane shows one strip at a time.
- Sits at `z.sticky` (10), above its pane's content.

## States

| State | Tokens | Rule |
|---|---|---|
| Closed | none | Not drawn; Mod+F opens it. |
| Default | `surface.overlay` | Strip open, input focused, with the last query selected. |
| Matches | `match.bg`, `match.active.bg` | The count reads "3 of 12"; the active match scrolls into view. |
| No results | `text.secondary` | The count reads "No results"; the input keeps its text. |
| Hover | `fill.hover.overlay` | On the icon buttons. |
| Pressed | `fill.pressed.overlay` | On an icon button while pressed. |
| Focus-visible | `border.focus`, inset | Input border when focused; inset ring on the buttons. |
| Selected or active | none | Not applicable. |
| Disabled | `text.disabled` | Previous and next are disabled when there are no matches. |
| Loading | none | Not applicable. |
| Empty | none | An empty query shows no count and no highlights. |
| Error | none | Not applicable. |
| Long content | `text.primary` | The input scrolls its text; the count never truncates. |
| Writing | `border.focus` | Matches update as you type, after a short debounce. |

## Behaviour

- Keyboard: Mod+F opens the strip and focuses the input with its text selected. Enter goes to the next match, Shift+Enter to the previous. Escape closes the strip and returns focus to the pane's previous control. Tab moves through the input and the buttons.
- Focus order: input, previous, next, close.
- Pointer: click the buttons; the close button closes the strip. Clicks in the pane do not close it.
- Responsive: the strip keeps its width until the pane is narrower than it; then the input shrinks first. The count never overlaps the buttons.
- Motion: opacity from 0 to 1 over `duration.base` with `ease.enter`; it closes with `ease.exit`. Reduced motion: instant.
- Accessibility: role `search`, named for its scope, for example "Find in terminal". The input is a `searchbox`. The count is announced once per change, politely. Buttons are named "Previous match", "Next match" and "Close find bar".
- Extensions: extensions cannot place a find bar. Their views use a Text field and their own search.

## GPUI notes

Draw the strip as an overlay element at the pane's top right, inset `space.2` (8 px). Paint matches as solid background runs in the pane's text layout: `match.bg` for all matches and `match.active.bg` for the current one. Apply the e2 shadow through the box shadow only. No blur.

## React Native notes

Not on Android in v1. Searching inside a WebView uses the platform find; this spec does not design it.

## Desktop and Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Presence | Strip in each searchable pane | None in v1 |
| Shortcuts | Mod+F, Enter, Shift+Enter, Escape | Not applicable |

## Copy

- Placeholder: "Find".
- Count: "3 of 12". Empty result: "No results".
- Scopes: "Find in terminal", "Find in chat", "Find in editor", "Find in browser", "Find in settings".
- Close tooltip: "Close find bar (Esc)".

## Token gaps and open points

Token gaps: none. The strip width is `layout.findbar.width`.

Open points:
- Browser counts depend on the browser engine's find API, so they may differ. Confirm once the engine is chosen (design system open question 2).
- No cap is defined for very large terminal scrollback counts, such as "999+". Confirm.
