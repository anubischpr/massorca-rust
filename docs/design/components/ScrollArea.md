# ScrollArea

## Purpose

The scroll container for lists, trees, transcripts and wide tables. It draws a thin overlay scrollbar over the content, with no track, and it handles keyboard scrolling.

Inventory IDs: none; this is a design-system component. Android: system scroll indicators.

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Viewport | none | Fills its parent |
| Thumb | `border.strong`, `radius.xs` | `size.scrollbar.thickness` (6 px); inset `space.half` (2) from the edge |
| Track | none | No track fill |
| Focus ring on the viewport | `border.focus`, inset | `border.focus.width` (2), offset -2 |

## Variants and sizes

- Vertical and horizontal; each appears only when its content overflows.
- One size at every density. The thumb does not change with density.
- Used by the Sidebar, FileTree, CommandPalette results, wide Tables, the Work panel tab overflow and the Calendar week view on narrow windows.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | none | Thumb hidden until the user scrolls or the pointer is over the area. |
| Hover | `border.strong` | Thumb visible while the pointer is over the area. |
| Pressed | `border.strong` | Dragging the thumb keeps its colour. |
| Focus-visible | `border.focus` | Inset ring on the viewport. The viewport takes Tab only when nothing inside is focusable. |
| Selected or active | none | Not applicable. |
| Disabled | none | Not used. |
| Loading | none | The content shows its own Skeleton. |
| Empty | none | The content shows its own EmptyState, filling the viewport. |
| Error | none | The content shows its own ErrorState. |
| Long content | `border.strong` | The thumb length shows the content length; the minimum thumb is `size.scrollbar.min-thumb` (24 px). |
| Writing | N/A | Not an input. |

## Behaviour

- Keyboard: when focused, Up and Down scroll one row, PageUp and PageDown scroll one viewport, and Home and End go to the ends.
- Focus order and ring: one stop at the viewport when nothing inside is focusable; otherwise the children take focus. The ring is inset so the window edge does not clip it.
- Pointer: wheel and trackpad scroll; dragging the thumb scrolls; a click on the track pages once toward the click.
- Responsive: the thumb length follows the content and never falls below the minimum thumb.
- Motion: the thumb fades in and out over `duration.fast` with `ease.standard`; it hides `SCROLLBAR_HIDE_DELAY_MS` (1000 ms; behaviour constant, design system 4.8) after scrolling stops. Scrolling itself is not animated. Reduced motion: instant.
- Accessibility: the viewport has no role of its own; the content's role applies. The thumb is not focusable and is hidden from assistive technology, so keyboard scrolling is the accessible path. Screen readers announce the content, not the scrollbar.
- Extensions: extensions cannot place a ScrollArea. Their lists and tables scroll inside host containers that use the same thumb style.

## GPUI notes

Use the framework's scrollable container with a custom scrollbar element: a 6 px rounded rectangle in `border.strong` over the content, with no track. Fade the element by animating its opacity over `duration.fast`; the colour stays a solid token. Do not add a scrollbar background or a gradient.

## React Native notes

Use the platform scroll view with its system indicators, and add no custom scrollbar on Android. Keep any tappable row inside the scroll area at 48 dp.

## Desktop and Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Indicator | Thin overlay thumb | System indicator |
| Keyboard | Arrows, Page keys, Home and End | Hardware keyboard basics |

## Copy

- No visible text. The viewport has no label; a named list or table inside it carries its own name.

## Token gaps and open points

Token gaps: none. The thumb is `size.scrollbar.thickness` by `size.scrollbar.min-thumb`; the hide delay is the behaviour constant `SCROLLBAR_HIDE_DELAY_MS` (design system 4.8).

Open points:
- Overlay thumbs show only on scroll or hover, which can be hard to find with a trackpad. Confirm.
- Android uses system indicators. Confirm there is no custom Android scrollbar.
