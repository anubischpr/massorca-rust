# Delegation tree

## Purpose

Shows who is working on what when a lead bot or a coordinator agent hands parts of a task to child bots or agents: one tree under the parent, with each child's status. It lives inside the parent session or the Conference room, not in a floating panel.

Inventory IDs: X-DL-01 (delegation view and engine), X-CH-06 (delegation card and explanation). Where a child runs (owner, 2026-11-12): (1) by default each child gets its own worktree; (2) "Use the current worktree" is allowed only when the child only reads (research, review, explain), when the workspace is a plain folder without git, or when the user picks it by hand on the delegation card; (3) two sessions never write in the same worktree at once: if a child writes in the current worktree, the parent waits until it finishes (sequential, not parallel); (4) parallel launches always give each child its own worktree, with no option. Rows show where each child runs: a branch chip for its own worktree, a "Current worktree" chip otherwise; the parent shows "Waiting for <agent> to finish in this worktree" while such a child writes. Owner decision 2026-11-12: one shared engine for both modes. Bot Teams: the lead bot delegates by competence. Workspaces: a coordinator coding agent launches child agents, in sequence or in parallel, with worktree handoff.

## Anatomy

- Container: a card in the transcript (`surface.raised`, `border.default`, `radius.lg`, padding `space.3`) and the expanded body of the Child sessions summary card. It is not a region of its own.
- Header row: tree icon (users), title "Delegation" in `body-strong`, a summary in `caption` tabular `text.secondary` ("3 agents, 2 running, 1 done"), and a chevron to collapse the whole tree.
- Row (one per node), height at least `size.control.comfortable` (32 px), radius `radius.md`:
  - Expander chevron 12 px (only for nodes with children), `text.secondary`.
  - AgentStatus glyph 16 px with its word available to screen readers: running arc `agent.running`, waiting for you `agent.waiting`, idle ring `agent.idle` (used for "not started"), error triangle `agent.error`, unreachable dashed ring `agent.unreachable`. A finished child uses the check circle in `agent.done`.
  - Icon: the agent's own icon, 20 px (Workspaces), or the bot's team character, 28 px (Bot Teams). Characters move: a working bot leans and shows its effect (a plain aura below 24 px), an idle bot plays a move every 8 to 30 s, an unreachable bot is still. Motions use only transform and opacity, stop with reduced motion or when character animations are off in Settings. Character colours and motions are fixed art from pv96, not theme tokens.
  - Name in `body-strong`, truncated. In Workspaces the agent icon replaces the agent name (A17, A74): the row title is the child's task title ("[Codex icon] Extract the filter module"), and the agent name is the icon's tooltip and part of the accessible name. In Bot Teams the name is the bot's name. A role tag after it: "Lead bot" or "Coordinator" on the root, a competence chip ("Research", "Frontend") on Bot Teams children (Badge, `radius.sm`, `label`). Workspaces children show the model and effort in `caption` `text.tertiary`.
  - Status word in `label` (colour of its state) and duration in `caption` tabular `text.tertiary`, right-aligned.
  - Second line (`body-sm`, `text.secondary`, one line): the task given to the child, which is also the reason it was delegated.
  - Row action: "Open" (ghost, `size.control.compact`) opens the child's thread or session. Workspaces children with commits ready show a chip "2 commits ready" and a "Review" button that opens the bring-back card (X-CH-08).
- Group caption above a set of siblings: "Run in parallel" or "Run in order" in `caption` `text.tertiary`. Sequential children carry a step number in `mono-sm` `text.tertiary` before the glyph.
- No connector lines: children are shown by indentation only (A83, which supersedes the connectors of the DS v7 tree). The indent per level puts the child's agent icon or character in line with the first letter of the parent's name (A85); compute it from the parent's leading parts (expander, glyph, icon and their gaps), not a fixed step. The tree role carries the structure.
- Waiting row: fill `agent.waiting.bg`. Error row: fill `agent.error.bg`. Unreachable row: fill `agent.unreachable.bg`, text `text.tertiary`, glyph `agent.unreachable`. Done row: no fill.

## Variants and sizes

| Variant | Difference |
|---|---|
| Bot Teams | Team characters with their motions, competence chips, plain words ("Running", "Waiting for you", "Done", "Can't reach"). No model or worktree detail. |
| Workspaces | Agent icons, model and effort, worktree or branch name, commits-ready chip. |
| Card form (Child sessions summary) | One line per child: status glyph, agent icon (Workspaces) or still team character (Bot Teams), task title; no second line. |
| Android | Flat list in a sheet, indent capped at two levels, rows 56 dp. |

Depth limit is a setting (D-ST-02). Rows deeper than the depth limit (default 3, range 1 to 5, D-ST-02) show "+N deeper" which opens that subtree on its own.

## States

| State | Visual | Tokens |
|---|---|---|
| default | Rows as above | `text.primary`, `text.secondary` |
| hover | Row fill | `fill.hover.raised` |
| pressed | Row fill | `fill.pressed.raised` |
| focus-visible | Ring inset around the row | `border.focus`, 2 px |
| selected | The row whose thread is open: `fill.selected.raised` plus a 2 px `accent.fg` bar on the left edge | `fill.selected.raised`, `accent.fg` |
| disabled | "Open" disabled for a child whose host is unreachable and has no cached thread | `text.disabled` |
| loading | A child just created: row appears with the idle ring and "Starting"; skeleton only for the second line | `agent.idle`, `fill.skeleton` |
| empty | The tree is not rendered until the first child exists | not applicable |
| error | `agent.error` triangle, row tinted `agent.error.bg`, the word "Error" and the reason in the second line | `agent.error`, `agent.error.bg` |
| waiting for you | `agent.waiting` glyph, `agent.waiting.bg`, the word "Waiting for you" and an "Open to answer" action; the summary adds "1 needs you" | `agent.waiting`, `agent.waiting.bg` |
| done | Check circle, word "Done", duration | `agent.done` |
| long content | Names truncate with an ellipsis; second line truncates; more than 12 children under one parent show the first 8 and "Show 4 more". Full text in the tooltip | `text.secondary` |
| disconnected host | A child on an unreachable host shows the unreachable glyph, the words "Last known: running, 5 min ago" and dimmed text. The summary counts it apart: "2 running, 1 can't be reached". Nothing says stopped | `agent.unreachable`, `text.tertiary` |

## Behaviour (handoff)

- **Keyboard.** The tree follows the ARIA tree pattern with one tab stop: Up and Down move between visible rows; Right expands a collapsed node or moves to its first child; Left collapses or moves to the parent; Home and End go to the first and last row; Enter opens the child ("Open"); typing letters jumps to the next row starting with them. Tab leaves the tree. Row buttons (Open, Review) are reached with Right after the row is focused, then Left to return.
- **Focus order.** Header, tree, then whatever follows in the transcript.
- **Pointer.** Click the chevron to expand or collapse; click the row to select it and open the child's thread in place of the transcript or in a Work panel tab (Workspaces); double-click is not required.
- **Live updates.** Status, duration and counts update in place without animation. A new child inserts its row with a fade and 8 px move over `duration.base` with `ease.enter`; expanding and collapsing use height over `duration.base` with `ease.standard`. Reduced motion: instant.
- **Order.** Children appear in the order they were created. Finished children stay (no auto-collapse), so rows never move on their own. The user can collapse any node.
- **Accessibility.** `role="tree"` named "Delegation"; each row is a `treeitem` with `aria-level`, `aria-expanded` and a name that ends with the state: "Research, Bot, running, 2 min". Glyphs have words. A status change that needs the user is announced once, politely ("Writer is waiting for you"). There are no connector lines.
- **Extension limits.** None; the tree is core. Bots and agents contributed by extensions appear as ordinary rows with their own icon and name; an extension cannot add rows, actions or statuses.

## GPUI notes

- Flatten the visible tree into a vector of (node, depth) and render it as a list; expanding changes the vector, not the element tree. This also makes keyboard navigation and virtualisation trivial.
- Indent rows by depth from the flattened vector; draw no connector lines.
- The status glyph is the shared AgentStatus element, so the spinner stays in sync across the sidebar, status bar and tree.
- Keep the tree state (expanded set, selected id) in the session model so it survives re-rendering and window reloads.

## React Native notes (Android)

- Delegation shows as the card X-CH-06 in the chat (one child at a time) and as a sheet with a flat list for the whole tree (not designed in detail by the inventory; open). Rows 56 dp with two lines, 48 dp targets, indent at most two levels; deeper levels collapse into "+N deeper".
- TalkBack reads each row as "name, role, state, duration" and offers expand and open as accessibility actions.

## Desktop vs Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Structure | Full tree, indentation only | Flat list in a sheet, two levels |
| Navigation | Arrow keys | Touch, accessibility actions |
| Row actions | Open, Review in the row | Tap the row |
| Where | Transcript card and Child sessions card | Chat card (X-CH-06) and a sheet |

## Copy

- Title and summary: "Delegation", "3 agents, 2 running, 1 done".
- Roles: "Lead bot", "Coordinator".
- Group captions: "Run in parallel", "Run in order".
- Status words: "Running", "Waiting for you", "Done", "Error", "Not started", "Can't reach VPS, last known: running, 5 min ago". Both modes use the same words (pending-owner-review item 30).
- Actions: "Open", "Open to answer", "Review", "Show 4 more", "+2 deeper".

## Token gaps and open points

Token gaps: none. `agent.done` (the sixth AgentStatus state) and `agent.unreachable.bg` now exist; "not started" reuses the idle ring.

Open points:
- Which of Bring back (X-CH-08) actions can be taken from the row; the card is outside this spec.
- How a child that needs an approval is answered: here by opening the child's thread. Answering inline from the tree is not designed.
- Android tree (not designed in the inventory); this spec proposes the flat sheet.
