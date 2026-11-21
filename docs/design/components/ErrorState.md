# ErrorState

## Purpose

What a page, panel, section or Work panel tab shows when its content failed to load or a surface crashed: a plain statement of what failed, what is kept, and a Retry. One error pattern for every page, modal and sidebar, so one crash does not take the shell down.

Inventory IDs: D-SH-06 (recoverable error screen). Used by D-SH-17 (Work panel tab errors), the Picker and Select error rows, SearchField errors, Dialog error bodies, and every list whose load fails.

## Anatomy

- Panel: `status.danger.bg` fill with a `status.danger.border` 1 px outline and radius `radius.lg` (8 px). Page size pads `space.8` (32 px); section and inline sizes pad `space.4` (16 px). Section size keeps the same tint; the inline variant has no panel at all.
- Glyph: `octagon-alert`, Lucide, `size.icon.lg` (24 px) in `status.danger.fg`. Always with the title; the glyph is not the only cue.
- Title: `title-sm` (14 / 20 / 600) at section size, `title` (16 / 24 / 600) at page size. `text.primary`. Says what failed: "Could not load routines".
- Explanation: `body` (13 / 20 / 400), `text.secondary`. One or two sentences: what is kept, and what can be done.
- Actions: Retry (secondary Button) first; "Show details" (ghost Button) second. Details open a mono block (`mono-sm`) with the error code and time, copyable, hidden by default.

## Variants and sizes

| Variant | Use | Title | Actions |
|---|---|---|---|
| page | a whole page or the Work panel tab body | `title` | Retry, Show details |
| section | a list, card, sidebar section, picker | `title-sm` | Retry |
| inline | a line inside a Dialog, Popover or Sheet, no panel | `body-strong` | Retry as text button |

## States

| State | Rendering |
|---|---|
| default | glyph, title, explanation, Retry |
| retrying | Retry shows a spinner (`size.icon.sm`, `duration.spinner`) and the label "Retrying"; disabled while running |
| retried, still failing | the same state, with the explanation updated to the new reason; no new panel |
| details open | mono block below the explanation, `surface.sunken` fill, `border.subtle` outline, `radius.md` |
| recovered | replaced by the content, with no flash |
| empty | not applicable; a successful empty result is EmptyState |
| loading | not applicable; while loading, Skeleton is shown |
| disabled | not applicable |
| long content | the explanation wraps; details scroll inside their block |
| disconnected host | when the cause is the connection, use Banner (X-CN-01) at the top of the region and show ErrorState only for the content that failed |

## Behaviour (handoff)

- Keyboard: Retry is the first focus target; Tab reaches Show details; Enter activates.
- Focus: when an error replaces content the panel announces itself once; focus does not move unless the user pressed Retry and it failed again, then focus stays on Retry.
- Pointer: click Retry; click Show details to expand.
- Resizing: the panel takes the region's width; the title wraps before it shrinks.
- Motion: none on appear; spinner during retry; reduced motion: spinner still turns.
- Accessibility: the panel has role `alert` once when it appears (assertive for page size, polite for section). Title is a heading of the region's level. The glyph is `aria-hidden`; the title carries the meaning.
- Extensions: may use ErrorState in extension views with the same rules. The extension writes its own title and explanation in plain words; it may not add a stack trace or raw error text outside Show details.

## GPUI notes

- Panel `div` with a fill and a 1 px border from the danger tokens; glyph as an SVG path tinted with `status.danger.fg`.
- Details block is a `div` with the mono text run, selectable.
- Retry is the normal Button element.

## React Native notes

- `View` with the danger border and fill; title with `accessibilityRole="header"`, and `accessibilityLiveRegion="assertive"` on page size only.
- Retry is a 48 dp Button; Show details a 48 dp text button.

## Desktop vs Android

- Same copy and layout. On Android the page variant fills the screen with Retry at the bottom; the section variant sits in its list.

## Copy

- Title says what failed, plainly: "Could not load routines", "Search failed", "Could not delete".
- Explanation says what is kept: "Your changes are kept." Only when that is true.
- Retry is the verb "Retry". No "Try again later", no "Oops", no exclamation marks.
- Never blame the user; never say "Stopped" for an unreachable host; say "Last known state, 5 min ago".

## Token gaps and open points

- No gaps: `status.danger.bg`, `status.danger.border` and `mono-sm` cover the panel and details.
- Open: whether error details should include an error code by default. Spec says yes in the details block only, never in the title.
