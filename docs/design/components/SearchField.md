# SearchField

## Purpose

A TextField for finding things in a list, tree or file set. It filters or searches as the user types, shows the result count, and can switch the scope (Names or Content).

Inventory IDs: D-CP-03 (search in files, Content mode), D-ED-11 (File explorer search with the Names or Content toggle). Also used by Sidebar search in Bot Teams mode (D-WS-29) and the Picker.

## Anatomy

- Field: as TextField, height `size.control.default` (28), fill `surface.sunken`, outline `border.strong`, radius `radius.md`.
- Leading icon: Lucide `search`, 16 px, `text.secondary`.
- Placeholder: "Search files", `text.tertiary`.
- Clear action: IconButton (compact), `x` glyph, shown when the field has a value. Named "Clear search".
- Scope control (optional): SegmentedControl, compact, two options (Names, Content), placed at the trailing edge inside the region's header.
- Count: `caption` style, `text.tertiary`, tabular figures, for example "12 matches".

## Variants and sizes

| Variant | Use |
|---|---|
| plain | filters a list or a tree in place (Sidebar, Picker) |
| with scope | searches file names or content (D-ED-11, D-CP-03) |

One size on desktop: 28 px. Android: 44 dp, with the scope as a Sheet option.

## States

| State | Rendering |
|---|---|
| idle | placeholder shown, no clear action |
| typing | filter applies after `SEARCH_DEBOUNCE_MS` (200 ms; behaviour constant, design system 4.8); clear action shown |
| searching | spinner in the trailing slot after 200 ms; the previous results stay visible, dimmed to `text.secondary` |
| results | matches highlighted with `match.bg`; the active match with `match.active.bg`; count shown |
| no results | EmptyState (section size) with the title "No matches for "{query}"" and one action, "Clear search" |
| filtered | list shows only matches; a caption says "Filtered" next to the scope |
| error | ErrorState (section size) with "Search failed" and Retry. The query stays in the field. |
| disabled | `fill.disabled`, `text.disabled`, `aria-disabled` |
| disconnected host | searching is disabled; the list shows the last known results with "Last known state, 5 min ago" |
| long content | the query truncates with an ellipsis in the field; a long query never widens the field |

## Behaviour (handoff)

- Keyboard: typing starts the search. Escape clears the query when the field has one, otherwise it leaves the field. Down arrow moves into the results; Enter opens the active result. The field itself has no global shortcut in this spec; focusing it is owned by the region or the command palette (Mod+K).
- Focus order: the field comes first in its region, then the scope control, then the results.
- Pointer: click focuses; the clear action is a pointer target of 24 px at least.
- Resizing: the field shrinks first; the scope control moves to an overflow Menu before the field goes below 160 px.
- Motion: result highlight changes colour in `duration.fast`. Reduced motion: instant.
- Accessibility: role `searchbox`, accessible name "Search files" (or the list it filters). The result count is announced once, politely, when a search finishes; not on every keystroke.
- Extensions: may use SearchField in sections and views; cannot change the highlight colours.

## GPUI notes

- Same field as TextField with a leading icon element.
- Highlights are text runs with the match background set; the active match uses its own fill.
- Debounce runs on the foreground executor; no timers inside the render path.

## React Native notes

- `TextInput` with `returnKeyType="search"` and `accessibilityRole="search"`.
- Clear action is 48 dp hit. Results announced through `AccessibilityInfo.announceForAccessibility` once per finished search.

## Desktop vs Android

- Desktop keeps the scope toggle inline; Android moves scope to a Sheet and keeps search at the top of the screen.
- Keyboard shortcut Escape on desktop; Android uses the back gesture to clear first.

## Copy

- Placeholder: "Search files", "Search teams", "Search branches".
- Scope labels: "Names", "Content".
- Count: "12 matches", "1 match", "No matches".
- No results title names the query: "No matches for "oppm"".

## Token gaps and open points

Token gaps: none. The 200 ms pause is the behaviour constant `SEARCH_DEBOUNCE_MS` (design system 4.8).

Open points:
- Whether Content search should show a second line per match with the file path in `mono-sm`. Spec says yes for D-ED-11; the result row is defined in ListRow.
