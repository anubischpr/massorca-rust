# Session summary card

## Purpose

One card in the Session summary column: a titled, collapsible block that shows one kind of information about the session (where it runs, the plan, context use, results, helpers). A card exists only when it has information and is inserted with a short animation; the column never shows empty placeholders.

Inventory ID: D-AG-27. Owner requirements and the results-card reference are from 2026-11-12. Extension cards: contribution point 5.

## Anatomy

- Column: the Session summary region (default `layout.summary.default` 288 px, minimum `layout.summary.min` 240, maximum `layout.summary.max` 400) on `surface.canvas`, padding `space.3`, cards stacked with `space.3` between them, scrolling as a whole. The column title "Session summary" (`label`, `text.secondary`) is optional chrome in the region header, not a card.
- Card: `surface.raised`, `border.default`, `radius.lg`, `shadow.e1`, padding `space.3`.
- Header (a button, height `size.control.default`): the card's icon 16 px `text.secondary` on the left of the title (A23: Environment `server`, Plan `list`, Checklist `list-checks`, Context `gauge`, Results `wand`, Subagents `users`, Connectors `plug`, Background tasks `loader`, QA servers `globe`), title in `body-strong`, value in `caption` tabular `text.secondary` (right, truncates last), chevron 16 px `text.secondary` (right of the value). Collapsed shows chevron-right, expanded chevron-down.
- Collapsed body: at most 6 rows, each one line, `body-sm` `text.secondary`, truncated with an ellipsis; identifiers (repo, branch, path, URL) in `mono-sm`. Every data row carries its icon on the left, 12 px `text.tertiary`, with the field name as its tooltip (A22, A23): account `person`, repository `book`, branch `git-branch`, pipeline `workflow`, host `server` beside its status dot, URL `globe`, file the file-type icon (generic files a grey document). Subagent rows show the status glyph then the agent icon (or the still team character in Bot Teams) before the task title.
- Expanded body: the card's full content (below) with its actions. The chevron expands the card in place; there is no separate detail screen in v1 (the owner's Results reference is this expanded form).

The nine core cards, in this fixed order (extension cards follow):

| Card | Appears when | Header value | Collapsed rows | Expanded content |
|---|---|---|---|---|
| Environment | The session has a host | Host name, with the host status glyph | Account (`person`), repository (`book`), branch (`git-branch`), pipeline (`workflow`) | All fields with their icons; host (`server`) with state and age |
| Plan | The agent reports a plan | "3 of 7" | Current step, next step | The step list (PlanView card form) and an "Open plan" link |
| Checklist | The session has checklist items | "2 of 5" | First 3 items with glyphs | All items |
| Context | Context use is known | "35%" | A 4 px Progress bar (`accent.default` on `progress.track`) | Used and total tokens; compaction note |
| Results | Something was reported: URLs, file changes, artifacts | "2 reported" | The first item | See below |
| Child sessions | At least one child session exists | "3 · 1 working" | One line per child session: status glyph, the agent icon, task title; a child session is a separate session with its own row in the Sidebar and its own worktree by default (owner 2026-11-21: not a subagent) | DelegationTree |
| Subagents | The agent ran at least one of its own subagents inside this session | "2 running" | One line per subagent: status glyph, the subagent's name as the agent reports it (for example "Explore"), its task; no agent icon change, no worktree, no Sidebar row. Every session has its own Subagents card, child sessions included: open a child session to see its subagents (owner, 2026-11-21) | — |
| Connectors | The provider reports connectors | "3 connected" | Name and state per connector | All connectors with state and reason |
| Background tasks | A background task exists | "1 running" | Name, status | Name, status, duration, output link |
| QA servers | A dev or QA server was detected | "2 detected" | Name, port, state | Name, URL, port, state |

**Results card, expanded** (from the owner's reference): a subtitle in `body-sm` `text.secondary` ("URLs of the environment, file changes and visuals recorded in this chat."), then items:
- URL item: globe icon, the full URL in `mono-sm` wrapping at any character, then two buttons at `size.control.compact`: "Open" (secondary, external-link icon) and "Copy URL" (secondary, copy icon). When the URL is on a remote host, "Open" is disabled and a note in `body-sm` `text.secondary` says "This URL is on the remote host. Forward its port to open it on this PC."
- File item: the file-type icon (A46, A48), then the path as a link (`mono-sm`, `accent.fg`, underlined) that opens the file in a Work panel tab.
- Artifact item: icon, name, kind in `caption`; opens in a Work panel tab.

## Variants and sizes

| Variant | Difference |
|---|---|
| Core card | As above, nine kinds. |
| Extension card | Title up to 24 characters, optional value up to 12 characters, collapsed body up to 6 rows, optional detail view, a "has information" signal. Up to 3 per extension. The extension's name is in its tooltip and accessible description. |

On Android the same cards make up the session Summary screen (A87).

## States

| State | Visual | Tokens |
|---|---|---|
| default (collapsed) | Header and up to 6 rows | `surface.raised`, `border.default`, `text.primary`, `text.secondary` |
| hover (header) | Header fill | `fill.hover.raised` |
| pressed | Header fill | `fill.pressed.raised` |
| focus-visible | Ring around the header, inset by 2 px so the card edge does not clip it | `border.focus`, 2 px |
| selected / expanded | Chevron down; body shown; the expanded state is remembered per card and session | `text.primary` |
| disabled | Not applicable. Actions inside (Open) can be disabled: `fill.disabled`, `text.disabled` | `fill.disabled` |
| loading | The card exists only after its first record, so there is no placeholder. Refreshing data keeps the old values; a 12 px arc spinner can show next to the value after 200 ms | `agent.running` |
| empty | The card is not rendered. When its last record goes away (for example a checklist is cleared) it collapses out over `duration.base` with `ease.exit` | not applicable |
| error | A row with octagon-alert and the word "Error" and the reason (for example a connector that failed) | `status.danger.fg` |
| long content | Rows truncate with an ellipsis and show the full text in a tooltip; URLs in the expanded Results wrap at any character; the value truncates before the title | `text.secondary` |
| disconnected host | Cards fed by the host (Environment, Child sessions, Subagents, Background tasks, QA servers, Connectors) show the last known values dimmed with their age. Environment's value shows the unreachable glyph and "Last known state, 5 min ago". Nothing says stopped, failed or finished | `agent.unreachable`, `text.tertiary` |

## Behaviour (handoff)

- **Insert animation.** A card appears when its first record arrives: height grows from 0 to its measured content height, opacity 0 to 1 and an 8 px move up, over `duration.card-insert` (250 ms) with `ease.enter`; cards below slide down over the same time with `ease.standard`. The fixed order never changes. If the new card lands above the visible part of a scrolled column, it is inserted without animation and the scroll offset is anchored so nothing jumps. A polite announcement says "Plan card added". Reduced motion: the card appears instantly.
- **Hiding.** The whole Session summary can be hidden from the Title bar. A file or diff open in the Work panel does not hide it by rule (the owner's reference shows it hidden; that is a user choice).
- **Keyboard.** The column is a region (F6). Card headers form an accordion: Tab reaches the first header; Up and Down move between headers; Enter or Space toggles; Home and End go to the first and last. Inside an expanded card, Tab moves through its links and buttons, then on to the next header.
- **Focus order.** Header, then the card's own controls when expanded, then the next card.
- **Pointer.** Click anywhere on the header to toggle. Links open Work panel tabs; the URL "Open" button opens the system browser through the link-routing preference (X-DLG-01 confirm for external links).
- **Resizing.** Cards fill the column width. Under 240 px the column collapses (region rules).
- **Motion.** Expand and collapse over `duration.base` with `ease.standard`, content keeping its final width and clipped. Chevron rotates over `duration.fast`. Reduced motion: instant.
- **Accessibility.** Each header is a button with `aria-expanded`, named "Plan, 3 of 7". The column is `complementary` named "Session summary". A new card announces "<Title> card added" once, politely. Context percentage is also exposed as a meter in the card. Glyphs have words in names.
- **Extension limits** (contribution point 5). An extension provides a title, an optional value, a collapsed body of up to 6 rows and an optional detail view. It appears only when it has information (with the standard insert), after the core cards. It cannot show an empty placeholder, reorder or hide core cards, force itself open or animate beyond the standard insert.

## GPUI notes

- Column: a vertical flex container with `overflow_y_scroll`. Cards are plain elements, not separate layers.
- Insert and collapse need the measured content height: lay the card out once with unconstrained height off-screen, cache the height, then animate the clip height from 0 to it. The inner content is laid out at final width and clipped, so text never reflows during the move.
- Use one animation clock per insert and shift following cards by the same eased value (offset transform, not re-layout).
- Disabled "Open" for remote URLs is a normal disabled button with a visible note, so it is reachable by keyboard and explains itself.

## React Native notes (Android)

The Android session Summary (the clipboard icon in the session view group, A87) lists the same cards in the same order with the same title and row icons (A23), as a scrolling column of cards with 48 dp headers under the view group and the session pill.

## Desktop vs Android differences

Desktop: a column beside the Conversation. Android: the Summary screen of the session view group (A87), same cards and icons, 48 dp headers.

## Copy

- Titles: "Environment", "Plan", "Checklist", "Context", "Results", "Child sessions", "Subagents", "Connectors", "Background tasks", "QA servers".
- Values: "VPS", "3 of 7", "35%", "2 reported", "2 running", "3 connected", "2 detected".
- Results: "Open", "Copy URL", "This URL is on the remote host. Forward its port to open it on this PC."
- Host unreachable: "Last known state, 5 min ago".
- Announcement: "Plan card added".

## Token gaps and open points

- Token gap: none. Card spacing uses `space.3` (4.1 lists `space.3` for gaps between stacked cards). Confirm.
- Open: the owner's reference shows the chevron as a detail link; this spec expands in place (the Results reference is already the expanded form). If a separate detail screen is wanted, it opens as a Work panel tab.
- Open: "Forward port" as an action next to the remote URL note is not in the reference; the status bar has a ports item. Not added.
- Open: the Connectors card in the old app said "not available" when the provider reports nothing; by the owner's rule such a card is not shown.
