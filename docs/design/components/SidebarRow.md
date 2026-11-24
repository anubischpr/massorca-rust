# SidebarRow

## Purpose

One row in the Sidebar. It is a project, a workspace, a session or child session, a navigation entry, a team with its hint, a child of a team, a host row under a project, or a section header.

Inventory IDs: D-WS-02 (workspace row and menu), D-HO-01 (host rows under a project). Desktop only. The Bot Teams rows are part of D-WS-29, owned by SidebarTree.

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Row box, radius `radius.md`, horizontal padding `space.2` | fill from state | height `size.control.default` (28); compact 24; comfortable 32 |
| Leading: project icon, host `server` icon, workspace `git-branch` icon, nav icon; for sessions the AgentStatus glyph then the agent icon (full colour, never a letter or the agent name) | status token, or `text.secondary`; agent icons in their own colours | glyph `size.icon.xs` (12); icons `size.icon.sm` (16) |
| Label | `body` in `text.primary`; `body-strong` when selected | single line |
| Secondary text: owner or org, team hint | `body-sm` or `caption` in `text.secondary` or `text.tertiary` | single line |
| Trailing: count badge, chevron, row action (on hover or focus) | badge fill `badge.neutral.bg` with text `badge.neutral.fg`; chevron `size.icon.xs`; action `size.icon.sm` | badge height `size.badge.height` (16) |
| Selected bar | `accent.fg` | `border.width.thick` (2 px) |
| Section header | `overline` in `text.secondary` | height `size.control.compact` (24) |

## Variants and sizes

- Project (top level): the project icon 16 px (A41: folder glyph by default, or the colour and icon the user chose), name `body-strong`, owner or org `body-sm`.
- Workspace (under its host row): `git-branch` icon 16 px for a worktree or `folder` for a plain folder, name `body`.
- Session (under its workspace): AgentStatus glyph 12 px, gap `space.2`, agent icon 16 px (A84), gap `space.2`, title `body`. The agent name is not repeated in the title; it is the icon's tooltip and is in the accessible name (A17).
- Child session: same parts, nested under its parent by indentation only, with no connector line (A83). The indent is one step of `size.icon.sm` plus `space.2` (24 px) beyond the parent, so the child's agent icon lines up with the first letter of the parent's title (A85). Deeper children add the same step. Its secondary text names where it runs; a child in another worktree than its parent's has a 12 px `git-branch` icon before its branch (WS-35).
- Navigation entry: icon 16 px, label `body`, optional count badge, chevron when it has children.
- Team: team character 16 px (still at this size), name `body`, status hint at the trailing edge.
- Child (Conference room `messages-square`, Board kanban icon, Files `folder`, Memory `brain`, Routines `calendar-clock`): one indent step (`space.3`, 12 px), icon 16 px, hint trailing.
- Host row (under a project, indented one level, A10): `server` icon, host name, a 6 px connection dot, state word in `caption`. An unreachable host dims only its own row and its workspaces.
- Density changes the row height only.

## States

| State | Tokens | Rule |
|---|---|---|
| Default | label `text.primary`; secondary `text.secondary` | Sits on `surface.frame`. |
| Hover | fill `fill.hover.frame`; row actions shown | `duration.fast`. |
| Pressed | fill `fill.pressed.frame` | While the pointer is down. |
| Focus-visible | `border.focus`, 2 px ring, inset by 2 px | Inset, because the Sidebar edge clips an outer ring. |
| Selected | fill `fill.selected.frame`; bar `accent.fg`; label `body-strong` | Two cues: bar and bold text. |
| Disabled | label and icon `text.disabled`; `aria-disabled` | Not used for workspaces. |
| Loading | `fill.skeleton` bar where the label goes | No spinner in a row. |
| Empty | hint in `text.tertiary`: "Nothing open", "No tasks" | No placeholder card. |
| Error | `triangle-alert` in `status.warning.fg` with "Check"; `octagon-alert` in `status.danger.fg` with "Failed" | The row stays usable. |
| Long content | label truncated with an ellipsis; full name in the tooltip | Never wraps. |
| Disconnected host | row and children `text.tertiary`; glyph `agent.unreachable`; "Last known state, 5 min ago" | Dimmed, with the age. |

## Behaviour

- Keyboard: a row is an item of SidebarTree (roving focus). Enter opens. Right and Left expand and collapse groups. Shift+F10 or the menu key opens the row menu (D-WS-02).
- Focus ring: inset 2 px, `border.focus`.
- Pointer: click selects and opens. Right-click opens the row menu. Hover shows the row actions.
- Motion: fills change with `duration.fast`; expand and collapse use `duration.moderate`. Reduced motion: instant.
- Accessibility: role `treeitem` inside the tree, `button` for navigation entries. Name includes count and state: "Prototypes, 2, collapsed". Host rows expose "Connected" or "Last known state, 5 min ago". Session rows name the agent and state first: "Codex, Running, Feed endpoints"; child rows expose their level.
- Extensions: rows only inside an extension's own sidebar section (D-SL-05). Labels up to 80 characters with an ellipsis, up to 3 trailing actions. An extension cannot restyle core rows.

## GPUI notes

A row is a flex row in a fixed-height box. Truncate text with the layout's ellipsis. Use the AgentStatus component for glyphs. Draw the selected bar as a 2 px rectangle at the row's left inner edge. Precompute fills.

## React Native notes

Not on Android. Use the list row (ListRow, M-WS-01): 48 dp touch target, a TalkBack label with the state, and long-press actions as `accessibilityActions`.

## Desktop and Android differences

Desktop: 28 px rows, hover actions, keyboard tree navigation. Android: list rows with long-press action sheets (M-WS-02).

## Copy

- Entries: "Prototypes", "Tasks", "Automations", "MassOrca Mobile".
- Team children: "Conference room", "Board", "Files", "Memory", "Routines".
- Hints: "Nothing open", "No tasks". Host: "Last known state, 5 min ago".
- Error: "Check", "Failed".

## Token gaps and open points

Token gaps: none. The count badge is the neutral badge (`badge.neutral.bg`, `badge.neutral.fg`, `size.badge.height`).

Open points:
- Selected rows use the bar and bold text. Confirm both are wanted, or keep one.
- The bar colour is `accent.fg`, to match the rail. The foundations do not name a colour for selected rows. Confirm.
