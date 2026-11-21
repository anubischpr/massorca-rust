# Plan view

## Purpose

Shows the plan the agent reported for this session: the steps and their progress, and the plan document behind them. It is the detail of the Session summary "Plan" card and the "View plan" target from the conversation.

Inventory ID: X-CH-05 (Android: M-CH-05, a sheet). Entry points: the Plan card in the Session summary, an approval card that asks the user to review a plan.

## Anatomy

- Container: on desktop a Work panel tab (icon list-checks, title "Plan") so it sits beside files, terminals and diffs; on Android a bottom sheet. Body on `surface.canvas`, content column up to `layout.conversation.reading` wide, padding `space.4`.
- Header (sticky): title "Plan" in `title-sm`, progress "3 of 7 steps" in `caption` tabular `text.secondary`, "Updated 2 min ago" in `caption` `text.tertiary`, and a Progress bar 4 px high (`accent.default` fill on `progress.track`, `radius.full`). A SegmentedControl "Steps | Document" appears only when both exist.
- Step list: one row per step, `body`, `text.primary`. Left: status glyph 16 px. Sub-steps indent `space.4` (one level). Gap between rows `space.2`; row padding `space.1` `space.2`.
  - Pending: hollow ring (`agent.idle`), word in the accessible name only.
  - In progress: arc spinner (`agent.running`), row filled `accent.subtle.canvas` with a 2 px `accent.fg` bar on the left edge.
  - Done: check circle (`status.success.fg`), text `text.secondary`.
  - Skipped: dash in a ring (`text.tertiary`), text `text.tertiary`.
  - Blocked: triangle with exclamation (`agent.error`); the reason sits under the step in `body-sm` `text.secondary`.
- Document: the plan Markdown rendered with the same rules as message text, at `body` size (not `chat`), on the canvas. Checklist items in the document mirror the step glyphs.
- "Jump to current step" ghost button (pinned bottom-right of the body) when the in-progress step is out of view.

## Variants and sizes

| Variant | Difference |
|---|---|
| Tab (desktop) | Full body, sticky header |
| Card body (Session summary) | Up to 6 rows, one line each, no document; "Open plan" link opens the tab |
| Sheet (Android) | Same content, step rows 44 dp, sticky title bar with a close button |

## States

| State | Visual | Tokens |
|---|---|---|
| default | Steps and header | `text.primary`, `surface.canvas` |
| hover (row) | Row fill | `fill.hover.canvas` |
| pressed | Row fill | `fill.pressed.canvas` |
| focus-visible | Ring around the focused row, inset | `border.focus`, 2 px |
| selected | The in-progress row is marked; no user selection (steps are read-only) | `accent.subtle.canvas`, `accent.fg` bar |
| disabled | Not applicable (read-only) | not applicable |
| loading | Skeleton: a header and six rows of two lengths | `fill.skeleton` |
| empty | "The agent has not shared a plan yet." | `text.secondary` |
| error | ErrorState "Could not load the plan" with Retry | `status.danger.fg` |
| scrolled | Header keeps a `border.subtle` rule under it | `border.subtle` |
| long content | Step text wraps to the full text; the card form truncates to one line. The document scrolls; very long plans are virtualised | `text.primary` |
| disconnected host | The last received plan stays. Header says "Last update 5 min ago. VPS can't be reached." with the unreachable glyph; the in-progress step shows the unreachable glyph instead of the spinner; nothing is marked failed | `agent.unreachable`, `text.tertiary` |

## Behaviour (handoff)

- **Updates.** The agent updates the plan as it works. Steps change glyph in place with no animation (status colour changes never animate). A new step inserts at its position; a removed step disappears; a polite announcement says "Plan updated: step 4 done".
- **Keyboard.** The step list is one tab stop with roving focus: Up and Down move between steps, Right expands sub-steps, Left collapses. Home and End jump to the first and last step. `Mod+C` copies the focused step's text. The segmented control and Jump button are normal tab stops.
- **Focus order.** Header controls, step list, document, Jump button.
- **Pointer.** Click a parent step to expand or collapse its sub-steps. Text is selectable.
- **Resizing.** The column fills the tab up to `layout.conversation.reading`. In the card form the width is the Session summary width (288 default).
- **Motion.** Expanding sub-steps changes height over `duration.base`, `ease.standard`. The Jump button scrolls over `duration.moderate`. Reduced motion: instant.
- **Accessibility.** The step list is `role="list"`; each row is a list item whose name ends with its state ("Run the migration, in progress"). The progress bar is `role="progressbar"` with "3 of 7". The sheet announces its title on open.
- **Extension limits.** None; the core renders the plan. Extensions can add a summary card (contribution point 5) with their own plan-like data, which does not replace this view.

## GPUI notes

- Steps are a virtualised list only above about 200 rows; otherwise a plain column. The document uses the shared Markdown renderer.
- The in-progress bar is a quad on the row's left edge, not a border, so the row keeps its radius.
- The Progress bar is one rounded quad over a track quad.
- Open as a Work panel tab type owned by the core; the tab restores its scroll position.

## React Native notes (Android)

- Bottom sheet with a FlatList of steps; 44 dp rows with a 48 dp hit target for expandable parents. The sheet's close button is 48 dp. TalkBack reads each step with its state and progress, and announces plan updates politely.

## Desktop vs Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Container | Work panel tab | Bottom sheet |
| Segmented control | Steps and Document | Steps and Document as tabs in the sheet |
| Jump to current | Ghost button | Button above the safe area |
| Copy step | `Mod+C` | Long-press |

## Copy

- Title: "Plan". Progress: "3 of 7 steps". Age: "Updated 2 min ago".
- Empty: "The agent has not shared a plan yet."
- Disconnected: "Last update 5 min ago. VPS can't be reached."
- Buttons: "Jump to current step", "Open plan".
- Error: "Could not load the plan." "Retry".

## Token gaps and open points

- Token gap: none. Skipped and blocked step glyphs reuse existing colours; there is no dedicated "skipped" token.
- Open: whether users can edit or reorder the plan. The inventory says the view only shows the plan, so it is read-only; approving a plan is an approval card (MessageBlocks).
- Open: where plans come from when an agent writes only a Markdown file with no structured steps; the "Document" mode covers it, "Steps" is hidden.
