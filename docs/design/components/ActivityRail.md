# ActivityRail

## Purpose

The narrow icon rail at the far left of the desktop window. It is part of the Frame, always visible, and holds the mode switch, the top-level entries, the theme toggle, the usage rings and Settings.

Inventory IDs: D-SH-09 (Desktop). Entries: D-SH-10 to D-SH-16. Extension icons: D-SL-09.

## Anatomy

| Part | Tokens | Size |
|---|---|---|
| Rail container, full window height, one fill with the Frame | `surface.frame`, no border, no shadow | width `size.rail.width` (48) |
| Top inset (macOS: traffic-light zone; Windows and Linux: none) | `size.titlebar.macos` on macOS | 40 px zone, then `space.2` (8 px) |
| Top group: ModeSwitch (D-SH-10), Tasks `list-checks` (D-SH-11), Calendar `calendar` (D-SH-12), Marketplace `store` (D-SH-13) | RailItem | items `size.rail.button` (36), gap `space.1` |
| Extension group: divider, extension icons (D-SL-09), overflow button `ellipsis` | divider `border.subtle`, `border.width.default` | same item size |
| Flexible gap | none | none |
| Bottom group: theme toggle `sun` or `moon` (D-SH-14), UsageRing per provider (D-SH-15), Settings `settings` (D-SH-16) | RailItem, UsageRing | items 36; ring `size.rail.ring` (32) |

Items are centred in the 48 px width, so no literal padding is needed.

## Variants and sizes

- One desktop variant. Rail density does not change: density sets control height, and the rail is not a control row.
- Rail width is fixed at 48 at every window width (window minimum 800 by 520).
- Height decides how many extension icons show. At the minimum height, three extension icons show. The rest move under the overflow button (`ellipsis`).

## States

| State | Token | Rule |
|---|---|---|
| Default | `surface.frame` | Rail fill only; no border or shadow to the Sidebar or Title bar. |
| Hover, pressed, focus-visible, selected, disabled | per RailItem | The rail itself has no hover or selected state. |
| Loading | none | Extension icons appear when their extension loads, without animation. |
| Empty | none | No extension group and no divider when no extension icon exists. |
| Error | none on the rail | A failed extension icon is hidden and reported through D-NT-01, not on the rail. |
| Long content | none | Labels never show on the rail. Names show in tooltips; extension labels are cut at 32 characters with an ellipsis. |
| Overflow open | `surface.overlay`, `border.default`, `shadow.e2`, `radius.lg` | The overflow menu lists the hidden extension icons. |
| Disconnected host | N/A | The rail shows no host state; UsageRing shows its own unavailable state. |

## Behaviour

- Keyboard: the rail is a navigation region named "Activity rail" with one tab stop. Up and Down move between items, Home and End go to the first and last item, Enter or Space activates. Tab leaves the rail. F6 and Shift+F6 cycle the regions. Settings is `Mod+,` (app-flow keyboard table). `Mod` is Ctrl on Windows and Linux and Cmd on macOS.
- Focus order: Title bar, Activity rail, Sidebar, Conversation, Session summary, Work panel, Project sidebar, Status bar.
- Focus ring: `border.focus`, `border.focus.width` (2 px), `border.focus.offset` (2 px). On a 36 px item the ring radius is `radius.lg` (item radius 6 plus 2 px offset).
- Pointer: click activates. Tooltips appear to the right of the rail after `duration.tooltip-delay` (400 ms) and show the name and the shortcut.
- Responsive: fixed width. Only height changes, through the overflow button.
- Motion: none on the rail. Item fills follow RailItem (`duration.fast`, `ease.standard`). Reduced motion: instant.
- Accessibility: landmark role `navigation`, name "Activity rail". Items expose name, state and tooltip as description.
- Extensions: may add icons only through D-SL-09 (one per extension, three with user consent). They may not restyle, resize, reorder above core items, join a core group, animate or open themselves.

## GPUI notes

Draw the rail as a column, 48 px wide, full height, filled with `surface.frame`, with no border. Items are 36 px rounded rectangles with precomputed solid fills. Tooltips and the overflow menu render in the overlay layer (`z.popover`, 40), so they are not clipped by the rail. Do not use backdrop blur, gradients or the black background in the reference screenshot. Icons are SVG paths at 20 px with an absolute stroke of 1.75 px.

## React Native notes

No rail on Android. Entries move as set in pending-owner-review item 21 and app-flow A24: mode switch, theme toggle and Settings to the Home top bar; Calendar to the overflow menu; usage on host cards. Touch targets are 48 dp.

## Desktop and Android differences

| Aspect | Desktop | Android |
|---|---|---|
| Presence | Always-visible vertical rail | None; top bar and overflow menu |
| Names | Tooltips on hover and focus | Labels through TalkBack; no tooltips |
| Keyboard | Roving focus, F6 region cycling | Hardware keyboard basics only |

## Copy

- Region name: "Activity rail".
- Tooltips: "Tasks", "Calendar", "Marketplace", "Settings", "Switch to light theme", "Switch to dark theme", "More". Mode tooltips are in ModeSwitch.

## Token gaps and open points

Token gaps: none. Every value uses an existing token.

Open points:
- The macOS top zone assumes native traffic lights sit in the 40 px zone. Confirm on a Mac.
- Extension limit of one icon per extension, three with consent, is the default from D-SL-09. Owner to confirm.
- The rail has no Automations entry. Automations is reached from the Workspaces Sidebar (owner, 2026-11-12); app-flow G26 is open for where Automations opens.
