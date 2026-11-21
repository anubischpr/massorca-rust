# TextField

## Purpose

A single-line or multi-line text input for names, paths, notes and short values in forms, dialogs, settings and inline editors.

Inventory IDs: none of its own. Used by D-WS-14 (new workspace composer), X-VT-01 (secret name and value), D-ST-* settings rows and every form.

## Anatomy

- Label: style `label` (12 / 16 / 500), colour `text.secondary`, above the field with `space.1` (4 px) gap. Required fields say "required" in the label text, not with an asterisk alone.
- Field: fill `surface.sunken`, outline `border.strong` 1 px, radius `radius.md`, height `size.control.default` (28) by default, `size.control.comfortable` (32) for forms. Android: `size.touch.default` (44 dp).
- Text: `body` (13 / 20 / 400), `text.primary`. Paths, IDs and values use `mono` (13 / 20 / 400, JetBrains Mono).
- Placeholder: `text.tertiary`. A placeholder is an example, never the label.
- Leading or trailing icon (optional): 16 px, `text.secondary`. Clear action is an IconButton (compact) that shows when the field has a value and is focused or hovered.
- Helper line below: `body-sm`, `text.secondary`. Error line: `body-sm`, `status.danger.fg`, with glyph `octagon-alert` and the words.
- Padding: `space.2` (8 px) horizontal at default; `space.3` (12 px) at comfortable.

## Variants and sizes

| Variant | Use |
|---|---|
| single-line (default) | names, short values, search-like inputs |
| multi-line | notes and descriptions; minimum three lines (`size.field.multiline.min`, 68 px), grows to eight, then scrolls |
| mono | paths, IDs, commands; uses `mono` type |
| masked | secret values only (see X-VT-01); a reveal IconButton is the only way to show the value |

Sizes: default 28 px, comfortable 32 px (forms). Android 44 dp.

## States

| State | Field | Outline | Text |
|---|---|---|---|
| rest | `surface.sunken` | `border.strong` 1 px | `text.primary` |
| hover | `surface.sunken` | `border.strong` | `text.primary` |
| focus-visible / focused | `surface.sunken` | `border.focus` at 2 px with a 2 px offset outside the field border (8.2), so layout does not shift | `text.primary`; selection `selection.bg` |
| read-only | `surface.sunken` | `border.subtle` | `text.primary`; `aria-readonly` |
| disabled | `fill.disabled` | `border.disabled` | `text.disabled` |
| loading (validation or lookup) | unchanged | unchanged | spinner in the trailing slot after 200 ms |
| empty | placeholder in `text.tertiary` | | |
| error | `surface.sunken` | `status.danger.fg` 1 px | `text.primary`; error line below |
| long content | single-line scrolls horizontally with the caret; multi-line wraps; the value is never truncated in the field | | |
| disconnected host | read-only with the reason as helper text | | |

## Behaviour (handoff)

- Keyboard: Tab and Shift+Tab move between fields. Enter submits a single-line form when the form has a default action. In multi-line, Enter inserts a line and Mod+Enter submits. Escape in an inline editor restores the saved value; elsewhere Escape is not consumed, so an overlay can close.
- Focus order: label order in the form. Focus ring as the focus ring rule (border switches to `border.focus`).
- Pointer: click places the caret; the label also focuses the field.
- Resizing: fields fill their container; in a Dialog they keep the Dialog width.
- Motion: outline colour step in `duration.fast` with `ease.standard`. Reduced motion: instant.
- Accessibility: role `textbox` (`searchbox` for SearchField). The label is associated. Helper and error text are linked with `aria-describedby`. Error sets `aria-invalid="true"`.
- Extensions: may use TextField in extension views (allowed list). Cannot choose a font, a colour or a size; may set placeholder, helper and label text.

## GPUI notes

- Use the platform text input element with the field drawn as a rounded `div` (fill, 1 px border); focus replaces the border colour and width in place.
- Mono variant uses the mono family from the font set; no font loading at run time.
- Selection colour `selection.bg` precomputed per theme.

## React Native notes

- `TextInput` with `accessibilityLabel` and `accessibilityHint` for helper text; `accessibilityState={{ disabled }}`.
- `minHeight` 44 dp; label 14 sp / 20 / 500 above the field; text 16 sp so Android does not zoom on focus.
- Multi-line uses `multiline` with `minHeight` for three lines (`size.field.multiline.min` on desktop); text scales to 200 percent.

## Desktop vs Android

- 28 px field with pointer hover against 44 dp field with a 48 dp label row.
- Keyboard submit is Enter on desktop; Android uses the keyboard's action key.

## Copy

- Label names the thing: "Name", "Base branch", "Host address".
- Placeholder is an example: "e.g. oppm".
- Error names the fix: "Name is already in use. Choose another."
- Helper states the rule: "Letters, numbers and dashes only."

## Token gaps and open points

Token gaps: none. The multi-line minimum is `size.field.multiline.min` and the disabled outline is `border.disabled`.

Open points:
- The focus ring is drawn outside the field at 2 px with a 2 px offset (8.2, one ring style for all components); the GPUI version must confirm it does not change the field size. Assumed yes.
