# Extension card

## Purpose

One extension (or skill, tool, agent or service in the Marketplace) as a card: what it is, who made it, what it adds, what it asks for, and the one action that fits its state. The same card serves the browser (find and install) and the manager (enable, update, configure, remove).

Inventory IDs: D-EX-01 (extension browser, the Marketplace page), D-EX-03 (extension manager). Permission review before install is D-EX-02 (PermissionReview). Settings sections of an extension are D-EX-04.

## Anatomy

- Card: `surface.raised`, `border.default`, `radius.lg`, `shadow.e1`, padding `space.4`. Minimum width 280 px; the page lays cards out in a grid (1 column under 560 px, 2 from 560, 3 from 880 within `layout.page.default` 1120). Gap `space.4`.
- Icon plate: 40 px (`size.avatar.lg`), `surface.sunken`, `radius.md`. The extension's icon is single-ink, drawn at 24 px in `text.secondary` (its state colour; it cannot choose a colour). Agents and providers with a supplied brand mark show that mark.
- Title row: name in `title-sm` as a link (opens the item page, D-EX-08), one line. Under it: publisher and source in `body-sm` `text.secondary` ("Acme, Marketplace"), the item's category as a category-style Badge in the same row ("Agent", "Extension", "Skill", "Service", "Theme"), and the version in `mono-sm` tabular `text.tertiary` ("1.4.2").
- Description: `body`, `text.secondary`, two lines, ellipsis after.
- "Adds" line: chips (Badge, `radius.sm`, `size.control.compact`) for what it contributes, with an icon each: "Rail item", "3 commands", "Theme", "Status item", "Settings". At most two rows; the rest becomes "+3".
- Footer row: state on the left, action on the right.
  - Catalog: primary button "Review and install" (the permission review comes first, A12). A host chip "Installs on VPS" appears when more than one host exists.
  - Installed (Discover and Updates): Switch labelled "Enabled", a ghost "Settings" button (when the extension has a settings section), and a More icon button, which opens the item page. In Marketplace > Installed the extension is a row instead, with its own menu (ExtensionManagerRow.md).
  - Update available: Badge "Update 1.5.0" (`status.info.bg`, `status.info.fg`, info glyph) and a secondary button "Update".
- Mode chips: when an extension works in only one mode, "Workspaces" or "Bot Teams" appears as a chip; both modes show none.

## Variants and sizes

| Variant | Footer |
|---|---|
| Catalog | "Review and install" |
| Installed, enabled (Discover, Updates) | Switch on, Settings, More |
| Installed, disabled (Discover, Updates) | Switch off, name in `text.secondary`, chip "Disabled", More |
| Update available | Update badge and button |
| Installing | A 4 px Progress bar and "Installing, 40%", Cancel |
| Error | `status.danger.fg` octagon-alert, "Failed to start", "View log" |
| Incompatible | `status.warning.fg` triangle-alert, "Needs MassOrca 1.6 or later", no install action |

Density affects only the footer button height (24, 28, 32). Card padding does not change.

## States

| State | Visual | Tokens |
|---|---|---|
| default | As anatomy | `surface.raised`, `border.default` |
| hover | Card fill | `fill.hover.raised` |
| pressed | Title link or button pressed fill | `fill.pressed.raised` |
| focus-visible | Ring on the focused control (title link, button, switch, More), not on the card | `border.focus`, 2 px, offset 2 px |
| selected | Not applicable (cards are not selectable); the open item page is a separate view | not applicable |
| disabled | Disabled extension: name, icon and description in `text.secondary`, `text.tertiary` for the version; the Switch is off | `text.secondary` |
| loading | Skeleton card: plate, two text bars, footer bar | `fill.skeleton` |
| installing | Progress and Cancel; the card's other actions are disabled | `accent.default`, `fill.disabled` |
| empty | Not applicable to a card (the list has its own empty state) | not applicable |
| error | See Variants; the card keeps its place and shows a one-line reason | `status.danger.fg`, `status.danger.bg` for the reason row |
| long content | Name truncates with an ellipsis; description clamps to two lines; publisher and source truncate; chips wrap to two rows then "+N". Tooltips give the full text | `text.primary`, `text.secondary` |
| disconnected host | Cards of extensions installed on an unreachable host show their last known state with its age ("Last known: enabled, 5 min ago"), the Switch and Update disabled with the reason in visible text. Nothing says removed or stopped | `agent.unreachable`, `text.tertiary` |

## Behaviour (handoff)

- **Keyboard.** The card is a list item, not a tab stop. Tab visits, in order: the name link, the primary control (Install, Switch or Update), Settings, More. The card grid is roving: arrow keys move between cards, and Tab moves between the controls inside a card and out of the grid (8.3). Enter or Space on the card, or on its name link, opens the item page; Space toggles a focused Switch; Enter on More opens the item page.
- **Focus order.** Row by row, left to right, within the page.
- **Pointer.** Click anywhere on the card's body, its name or its icon to open the item page. The controls keep their own press: the Switch, Install, Update, Settings, Cancel, View log and More do not also open the page. Hover shows the whole-card fill. Switch toggles apply on release and confirm from the host before the Switch settles.
- **Enable and disable.** Disable keeps the files. Remove and Roll back open the confirm dialog (X-DLG-01), the destructive variant for Remove ("Remove Acme Metrics? Its settings are deleted."). New extensions are enabled at once after install (A13).
- **Resizing.** Grid columns follow the page width. A card is never narrower than 280 px; when the page cannot fit two columns the grid falls back to one, and the page never scrolls horizontally.
- **Motion.** Fill steps over `duration.fast`. The Switch uses its own motion. The progress bar fills without easing. Reduced motion: instant.
- **Accessibility.** Cards are `role="listitem"` in a list named by the page ("Extensions"). Each card is labelled by its name. The Switch is named "Enable Acme Metrics". The status line (Update available, Failed to start) is plain text, read in order. The install host chip is part of the primary button's description.
- **Extension limits.** The card is core chrome rendered from manifest data. Name, publisher, description and the contribution summary come from the manifest as plain text (no Markdown, no links, no colour). The icon is a Lucide name or a single-ink SVG (8 KB maximum, no gradients, no raster). An extension cannot restyle the card, add buttons, mark itself verified, or reuse core icons or agent status glyphs as its own icon. The tooltip and accessible description always show the extension's name.

## GPUI notes

- Grid of fixed-height cards (name, description clamp, two chip rows reserve space) so the list can be virtualised by rows. Compute columns from the container width and render rows with `uniform_list`.
- The title link is a text element with a click handler, underlined on hover; the Switch is the shared Switch element.
- Skeleton cards use `fill.skeleton` quads; no shimmer animation when reduced motion is on.

## React Native notes (Android)

Not designed for Android: extensions are desktop only in v1 (index platform D). On Android, Marketplace is a row in Settings (pending-owner-review item 5) and lists no extension cards.

## Desktop vs Android differences

Desktop only.

## Copy

- Actions: "Review and install", "Update", "Settings", "Cancel", "View log", "Roll back", "Remove".
- Status: "Enabled", "Disabled", "Installing, 40%", "Update 1.5.0", "Failed to start", "Needs MassOrca 1.6 or later".
- Host: "Installs on VPS".
- Disconnected: "Last known: enabled, 5 min ago".
- Adds chips: "Rail item", "3 commands", "Theme", "Status item", "Settings".

## Token gaps and open points

- Token gap: none.
- Open: whether the Marketplace marks publishers as verified. No such concept exists in the inventory, so the card has no verified mark; adding one needs a source of trust (signing) first.
- Open: the Marketplace also lists tools, agents, skills and services (pv96); this card covers them with the same layout, with the item's kind as the category Badge in the byline row and the "Adds" line kept for what the item contributes.
- Open: length limits for name, description and chip count come from the manifest limits (D-EX-02); the manifest limits for name and description are not set yet. Proposed: name 40, description 160.
