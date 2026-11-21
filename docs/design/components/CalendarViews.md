# CalendarViews

## Purpose

The Calendar page (D-AG-23): a full page over the Canvas that shows routines and automations on one timeline, in Day, Week or Month view, with filter chips and a Show done switch. A routine opens its Routines entry (D-AG-25); an automation opens D-AG-14.

Inventory IDs: D-AG-23 (Desktop). Android: a full screen from the overflow menu (see ActivityRail, React Native notes).

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Page title | `heading` type style, `text.primary` | Page gutters `space.6` (24) |
| Previous and next | IconButton; chevron-left and chevron-right (Lucide), `text.secondary` | 28 px target |
| Today | Button, secondary | Height `size.control.default` (28) |
| Range title | `title` type style, `text.primary`, for example "9 to 15 nov 2026" | Beside the controls |
| Show done | Switch | Track as the Switch spec |
| Day, Week, Month | SegmentedControl | Height 28 |
| Filter chips | `radius.sm`, `label` type style, `border.default` | Height `size.control.compact` (24) |
| Day column | `surface.canvas`; hour rules `border.subtle`; today outlined 1 px `border.strong` | Minimum width `layout.calendar.column.min` (96) |
| Entry | `surface.raised`, `border.default`, `radius.md`; icon, word, time and title | Minimum height `size.control.default` (28); one hour `layout.calendar.hour` (48) |

## Variants and sizes

- Day: one column. Week: seven columns, with today outlined. Month: a grid of weeks.
- Entries: routine (repeat icon, word "Routine") and automation (workflow icon, word "Automation"). Both icons are Lucide proposals.
- A time gutter sits at the left, with caption text in `text.tertiary`.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | `surface.canvas` | Week view with its entries. |
| Hover | `fill.hover.raised` | On entries. |
| Pressed | `fill.pressed.raised` | While the pointer is down. |
| Focus-visible | `border.focus`, inset | On the focused entry or day. |
| Selected or active | `accent.subtle.raised`, `accent.fg` bar | The selected entry; today's date number uses `accent.fg`. |
| Disabled | `text.disabled` | Not used in v1. |
| Loading | `fill.skeleton` | Skeleton blocks in the columns while runs load. |
| Empty | `text.tertiary` | An empty day reads "Nothing scheduled" with no card; an empty week uses EmptyState. |
| Error | per ErrorState | The page shows ErrorState with Retry; a failed entry shows ErrorState inline. |
| Done | `status.success.fg` | Done entries show the circle-check glyph and "Done", with the title in `text.tertiary`. Shown only when Show done is on. |
| Long content | `text.primary` | Entry titles clamp to one line with an ellipsis; the time stays visible. |
| Disconnected host | `agent.unreachable` | An entry on an unreachable host shows the unreachable glyph and "Last known state, 5 min ago". |
| Writing | N/A | Not an input. |

## Behaviour

- Keyboard: header controls come first in Tab order, then the filter chips, then the grid as one stop. Arrow keys move between days and entries; Enter opens the active entry.
- Focus order: header, filter chips, grid.
- Pointer: click an entry to open it. A routine opens D-AG-25 and an automation opens D-AG-14. In Month view, a click on a day number switches to Day view for that date (proposed). Entries cannot be dragged in v1.
- Responsive: columns share the width. Where seven columns no longer fit, the week scrolls sideways in a ScrollArea. Day view always shows one column.
- Motion: switching views cross-fades the content over `duration.base`. Reduced motion: instant.
- Accessibility: the page is a heading named "Calendar". The grid has column headers that name the weekday and date, with "today" on the current day. An entry is named by its word, time and title, for example "Routine, 09:00, Weekly backup, Done".
- Extensions: the Calendar has no extension contribution point in v1.

## GPUI notes

Draw the grid as flex columns with a fixed time gutter. Place entries as absolutely positioned rounded rectangles on the hour scale. Hour rules are 1 px `border.subtle`. The today outline is a 1 px `border.strong` stroke. Clip entry text to its box. No shadow on entries.

## React Native notes

Opened from the overflow menu as a full screen. Compact width shows Day view with a horizontal day pager (proposed); medium and expanded widths can show Week (proposed). Entries are at least 48 dp tall.

## Desktop and Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Views | Day, Week, Month | Day; Week from medium width (proposed) |
| Entry | Hover, focus and pointer | Touch targets of 48 dp |

## Copy

- Title: "Calendar". Range: "9 to 15 nov 2026".
- Buttons: "Previous", "Today", "Next". Switch: "Show done". Segments: "Day", "Week", "Month".
- Chips: "Team" for the team filter, "Automations" for the automation filter.
- Words: "Routine", "Automation", "Done".
- Empty day: "Nothing scheduled". Empty week title: "Nothing scheduled this week". Explanation: "Routines and automations show here when they have a run time."

## Token gaps and open points

Token gaps: none. The day column minimum and the hour height are `layout.calendar.*` tokens.

Open points:
- The routine and automation icons (repeat, workflow) are Lucide proposals. Confirm.
- Show done starts off in this spec. Confirm.
- Opening an automation goes to D-AG-14, as the inventory assumes. Confirm.
