# EmptyState

## Purpose

What a list, page, panel or Work panel shows when it has nothing yet: a title, one sentence of explanation, and one action, inside a bordered panel. It says what is true now and offers the next step. It is never an error and never a placeholder card.

Inventory IDs: D-SH-17 (Work panel empty state with four actions), D-WS-30 (team Board, "No tasks"), D-AG-25 (Routines page, "No routines yet"), D-AG-14 (automations empty), D-AG-23 (calendar empty), D-ED-11 (file explorer empty, "No files found"), D-WS-02 and D-AG-25 web addresses ("No web addresses yet").

## Anatomy

- Panel: `surface.raised` fill, `border.default` 1 px, radius `radius.lg` (8 px). Page size: padding `space.8` (32 px). Section size: padding `space.4` (16 px). No shadow.
- Icon (optional): Lucide, `size.icon.lg` (24 px) in `text.secondary`, above the title, `space.2` gap. Decorative: `aria-hidden`.
- Title: page size `display` (28 / 36 / 600, -0.01em); section size `title-sm` (14 / 20 / 600). Colour `text.primary`. Sentence case, a noun phrase or a plain fact: "No routines yet".
- Explanation: `body` (13 / 20 / 400), `text.secondary`, one sentence, at most `layout.empty-state.text.max-width` (60 characters) per line, max two lines. Says what the thing is for, or what to do.
- Action: exactly one Button below the explanation, `space.3` (12 px) gap. Primary when the empty state is the main path of the screen; secondary otherwise. The Work panel is the one exception: a centred column of four buttons, each as wide as its label with its icon on the left (New terminal, New browser tab, New Markdown, New mobile emulator); no panel, title or explanation (A43).
- Footer note (optional): `body-sm`, `text.tertiary`, for host or scope context: "Runs on VPS, the host Desarrollo uses."

## Variants and sizes

| Variant | Where | Title | Padding | Actions |
|---|---|---|---|---|
| page | a full page (Routines, Tasks, Calendar) | `display` | `space.8` | one primary |
| section | a sidebar section, a card or a panel | `title-sm` | `space.4` | one |
| compact | inside a list where the panel would be too heavy | `label` in `text.primary` with explanation on one line | `space.2` | optional text button |
| work panel | the Work panel empty state (D-SH-17) | none | `space.2` | four buttons in a centred column, no panel |

Android: page size title 28 sp; section 16 sp; one action at 48 dp.

## States

| State | Rendering |
|---|---|
| default | as above |
| loading | not an EmptyState. Show Skeleton in the same place. |
| filtered | title names the filter and the action clears it: "No matches for "{query}"" with "Clear search" |
| disconnected host | title says what is unknown, not that nothing exists: "Host unreachable" with the explanation "Last known state, 5 min ago" and no create action |
| error | ErrorState, never EmptyState |
| long content | explanation wraps to two lines and then truncates |
| hover / focus | the action owns hover and focus; the panel does not change |
| disabled action | the action is disabled and the explanation says why |

## Behaviour (handoff)

- Keyboard: the action is the only focus target in the panel; Tab reaches it, Enter activates.
- Focus order: the panel comes where its list would be; focus moves to the action only when the user has just cleared a filter.
- Pointer: the action is the only target.
- Resizing: the panel takes the width of its region; the explanation wraps; the title does not shrink below its size (it wraps before it scales down).
- Motion: none. The panel appears in place with no animation.
- Accessibility: the title is a heading of the region's level. The panel is a region only when it is the main content of a page. Announcements: none on appear.
- Extensions: may use EmptyState (allowed list) in extension views, with the same rules. The extension writes the title and explanation in its own words within the length limits.

## GPUI notes

- Panel `div` with a 1 px border in `border.default` and a `radius.lg` corner; text runs with the title and body metrics.
- Action is the normal Button element.
- No shadow; the panel sits on the canvas.

## React Native notes

- `View` with `borderWidth` and `borderRadius`; title as `Text` with `accessibilityRole="header"`; action as a 48 dp Button.
- Page size centres vertically in the screen; section size stays at the top of its list.

## Desktop vs Android

- Same copy, same action. Page-size empty states centre in the canvas on desktop; on Android they fill the screen with the action at the bottom.

## Copy

- Title: a plain fact. "No routines yet", "No tasks", "Nothing open", "No files found".
- Explanation: one sentence, what the thing is for. "Routines run a task on a schedule. Create one to start."
- Action: a verb phrase with the object: "New routine", "New terminal", "Add web address".
- Never "Oops", never "Nothing to see here", never an exclamation mark.
- Say what is true now: "Host unreachable", not "No hosts".

## Token gaps and open points

Token gaps: none. The explanation width is `layout.empty-state.text.max-width` (60ch).

Open points:
- Resolved: the Work panel's four actions are a centred column at every width (A43).
