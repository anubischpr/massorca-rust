# MassOrca design system

Status: draft, 2026-11-12. Foundations only. Component specs follow in a second pass (see [Components](#components)).

This file is the contract between design and code for the GPUI desktop app (Windows, macOS, Linux) and the React Native Android app. **Token names are stable API**: a theme file, the GPUI theme struct, the React Native theme object, the extension view schema and every component spec use these exact names (`surface.frame`, `text.secondary`, `space.3`). In code, dots become nested keys or snake/camel case; the name never changes.

The repository copy of the token contract is `docs/design/tokens.json`. Every token name and value below must match it, and the GPUI and React Native code generate from that file.

Region and concept names (Activity rail, Sidebar, Title bar, Frame, Canvas, Conversation, Session summary, Work panel, Project sidebar, Status bar, Model picker, Secret vault) come from `docs/design/glossary.md` and are used as written. Screen IDs (`D-SH-09`, `X-AG-02`) come from `docs/design/screen-inventory.md`.

Reference visuals: the Design System artifact (cover, colour and type specimens, brand book) is generated from the same values. If the artifact and this file ever disagree, this file wins and the artifact is regenerated.

Contents: 1 Principles, 2 Colour, 3 Typography, 4 Spacing, sizing, radius, elevation, icons, motion, 5 Layout, 6 Theming and customisation, 7 Extension contribution points, 8 Keyboard and accessibility, 9 Desktop and Android differences, 10 Open questions and not covered, then the Components index.

---

## 1. Principles

1. **A dense, quiet tool.** MassOrca is where people spend hours. Chrome recedes, content leads. Colour is reserved for state and for one accent; everything else is neutral grey. Default control height is 28 px, default body text 13 px. Density is a setting, not a redesign (compact, default, comfortable).
2. **One structure, two audiences.** Workspaces mode (software engineers: worktrees, terminals, diffs, full options) and Bot Teams mode (people who are not necessarily technical: a Conference room, a Board, Routines) share the same regions, tokens and components. Bot Teams hides depth; it never swaps the toolkit. Labels are plain words in both modes; jargon lives in Workspaces.
3. **Customisation is a design principle.** Every visual value is a named token, every region has contribution points, and the core's own features use the same points an extension uses. If the core needs a private colour, size or drawing trick, the system is missing a token or a component; add it here instead.
4. **Native-buildable.** Everything is drawn by a GPU toolkit with no browser: solid fills, rounded rectangles, 1 px borders, vector icons, text, and box shadows with offset, blur, spread and colour. There is no backdrop blur, no CSS blend modes, no `color-mix`, no gradients in the core. Every hover, selected and pressed fill is a precomputed solid colour. The same token names exist on Android.
5. **Status is never colour alone.** Every state that matters (agent running, waiting for you, error, unreachable, diff added or removed, success, danger) carries a shape, a glyph or a word besides its colour. Contrast floors (4.5:1 text, 3:1 icons, borders and focus) apply to every theme, including themes written by users; the app rejects a theme that misses them.
6. **Honest state.** The host that executes work owns its status. A disconnected host shows its last known state with its age, dimmed, in the `agent.unreachable` style. Silence never looks like "stopped". Nothing says "saved" or "deleted" before it is true.
7. **Motion explains, then gets out of the way.** Animation shows where something came from or went (a card inserted, a region collapsed, a mode switched). It is short, never decorative, and every animation has an instant replacement when reduced motion is on.

---

## 2. Colour

### 2.1 How colour is organised

Four surface levels, three text levels, one accent, four statuses, six agent states. Every token has a dark and a light value, and the two themes ship together with the same structure.

Naming: `group.role[.variant][.surface]`, lower case, dot separated, hyphens inside a word. Surfaces that hold text have their own state fills (`fill.hover.canvas`), because a hover fill is a colour on top of a specific surface.

Total colour tokens: 129 (each with a dark and a light value, a few identical in both).

**The frame and the canvas** (owner requirement, 2026-11-12):

```
+----+---------------------------------------------------------------+
|    |  Title bar                                  region toggles    |   surface.frame
|Rail+----------+----------------------------------------------------+   (one L-shaped fill,
|    | Sidebar  | Canvas (surface.canvas), rounded top-left corner   |    no seam, no border)
|    |          | Conversation | Summary | Work panel | Project bar  |
|    |          |              |         |            |              |
|    |          +--------------+---------+------------+--------------+
|    |          | Status bar                                         |
+----+----------+----------------------------------------------------+
```

- The Frame is the Activity rail, the Sidebar and the Title bar, filled once with `surface.frame`. There is no divider, border or shadow between the rail, the Sidebar and the Title bar.
- The Canvas is a rounded rectangle on top of the Frame, filled with `surface.canvas`. Only its top-left corner is rounded (`radius.canvas`, 12 px); the other corners meet the window edge. With the Sidebar hidden, the rounded corner sits next to the rail.
- Dark: `surface.frame` is #1b1b1c and `surface.canvas` is #151517 (fixed by the owner). The Canvas is darker than the Frame, so it reads as carved into it.
- Light: the Frame is a slightly darker, cool-tinted neutral and the Canvas is the lightest surface (`#eeeef2` and `#fbfbfc`, the values proposed earlier). Decision: **keep them.** Measured step between Frame and Canvas is 1.12:1 in light against 1.06:1 in dark, so the corner reads at least as clearly in light as in dark with no border. A stronger step would make the Sidebar text (`text.secondary` on the Frame, 7.6:1) and the rail icons lose contrast for no gain. Both themes share a faint cool tint (blue channel 1 to 4 steps above red and green) so grey never looks warm next to the accent.

### 2.2 Surfaces

| Token | Dark | Light | Use |
|---|---|---|---|
| `surface.frame` | `#1b1b1c` | `#eeeef2` | The Frame: Activity rail + Sidebar + Title bar, one L-shaped fill with no seam. Dark value fixed by the owner. Text and icons on it use the text tokens and the frame fill states. |
| `surface.canvas` | `#151517` | `#fbfbfc` | The Canvas inset in the Frame (rounded top-left corner, radius.canvas): Conversation, Session summary, Work panel, Project sidebar, Status bar and full-canvas pages. Dark value fixed by the owner; the lightest surface in light. |
| `surface.sunken` | `#101012` | `#f4f4f7` | Inset wells on the Canvas: text fields, search fields, code blocks, terminal and editor backgrounds. Darker than the Canvas in both themes. |
| `surface.raised` | `#1e1e21` | `#ffffff` | Panels and cards that sit on the Canvas: composer, summary cards, list cards, settings groups. Always paired with border.default. |
| `surface.overlay` | `#252528` | `#ffffff` | Anything floating above the Canvas: menus, popovers, model picker, tooltips, dialogs, toasts. Always paired with border.default and shadow.e2 or shadow.e3. |

Depth is shown by surface level plus `border.default`: Canvas (0), raised (+1), overlay (+2). Sunken is the inverse (-1) and marks places where the user types or reads code. Never nest raised inside raised; use `border.subtle` dividers instead.

### 2.3 Text

| Token | Dark | Light | Use |
|---|---|---|---|
| `text.primary` | `#ececef` | `#17171c` | Default text and icons on every surface and every fill state. Headings, labels, values. |
| `text.secondary` | `#b4b4bd` | `#4a4a54` | Supporting text, inactive icons and rail icons, captions, table headers. On every surface and fill state. |
| `text.tertiary` | `#9d9da7` | `#5f5f6b` | Placeholders, timestamps, metadata, line numbers. On every surface and fill state at 4.5:1; never use it for anything the user must act on. |
| `text.disabled` | `#5f5f67` | `#a2a2ac` | Disabled labels and icons. Exempt from the WCAG contrast rule; always paired with a disabled cursor and aria-disabled. |
| `text.on-accent` | `#ffffff` | `#ffffff` | Text and icons on accent.default, accent.hover and accent.pressed fills (primary button label, selected checkbox tick). |

### 2.4 Borders and focus

| Token | Dark | Light | Use |
|---|---|---|---|
| `border.subtle` | `#2a2a2e` | `#e4e4e9` | Decorative hairlines: separators inside a panel, list dividers, table rules. Never the only cue that a control exists. |
| `border.default` | `#36363b` | `#d3d3db` | Outlines of raised surfaces: cards, composer, popovers, dialogs. Decorative. |
| `border.strong` | `#787880` | `#7c7c88` | Borders that identify a control: text field, checkbox, radio, switch off-track, segmented control, button outline. At least 3:1 on frame, canvas, raised and overlay. Also the splitter hover line. |
| `border.focus` | `#b8a8f8` | `#5b3fc4` | The focus ring (2 px, offset 2 px). At least 3:1 on frame, canvas, sunken, raised, overlay in its theme. Never hidden by a theme. |
| `border.disabled` | `#2f2f34` | `#dcdce3` | Outline of a disabled control (secondary button, text field, switch track), on `fill.disabled`. Decorative and exempt from contrast, like `text.disabled`; always paired with `aria-disabled`. |

### 2.5 Accent

The default accent is lavender (owner choice, 2026-11-12). The user can pick another accent in Settings (see section 6.4); the set below is regenerated from the chosen seed and re-validated.

| Token | Dark | Light | Use |
|---|---|---|---|
| `accent.default` | `#7461d8` | `#6447c9` | Accent fill: primary button, switch on, selected checkbox/radio, active tab underline, progress. |
| `accent.hover` | `#6c59d2` | `#5a3fbd` | Accent fill on hover. |
| `accent.pressed` | `#6552c4` | `#4f36a8` | Accent fill while pressed. |
| `accent.fg` | `#b8a8f8` | `#5b3fc4` | Accent as text, icon, link and active-indicator colour on frame, canvas, raised and overlay (4.5:1). |
| `accent.on` | `#ffffff` | `#ffffff` | Alias role of text.on-accent for accent surfaces in extension-built views; identical value. |

Rules: use `accent.default` for the one primary action in a view, selected switches, selected checkboxes and radios, progress, and the active tab underline. Use `accent.fg` for links, the active rail bar, the model picker's active bar, the selected-row bar in lists, tables and trees, and accent text. Never use accent for status; never put `accent.fg` text on `accent.default`.

### 2.6 State fills (solid, per surface)

Hover, selected and pressed are **solid colours resolved per surface**, because GPUI draws them directly and nothing blends at run time. They are the surface blended in sRGB with white (dark) or near-black `#14141a` (light): hover 4 percent (dark) or 5 percent (light); selected 7 percent or 9 percent; pressed 10 percent or 13 percent. Accent subtle is the surface blended with `accent.default`: 18 percent (dark), 11 percent (light). The percentages are for theme authors who want to regenerate the table; the app only ever draws the resolved solid values.

| Token | Dark | Light |
|---|---|---|
| `fill.hover.frame` | `#242425` | `#e3e3e7` |
| `fill.selected.frame` | `#2b2b2c` | `#dadadf` |
| `fill.pressed.frame` | `#323233` | `#d2d2d6` |
| `accent.subtle.frame` | `#26223a` | `#e2dcf5` |
| `fill.hover.canvas` | `#1e1e20` | `#efeff1` |
| `fill.selected.canvas` | `#252527` | `#e6e6e8` |
| `fill.pressed.canvas` | `#2c2c2e` | `#dddddf` |
| `accent.subtle.canvas` | `#211d33` | `#ece8fb` |
| `fill.hover.raised` | `#27272a` | `#f3f3f4` |
| `fill.selected.raised` | `#2e2e31` | `#eaeaea` |
| `fill.pressed.raised` | `#353537` | `#e0e0e1` |
| `accent.subtle.raised` | `#2b2642` | `#f0ecfd` |
| `fill.hover.overlay` | `#2e2e31` | `#f3f3f4` |
| `fill.selected.overlay` | `#343437` | `#eaeaea` |
| `fill.pressed.overlay` | `#3b3b3e` | `#e0e0e1` |
| `accent.subtle.overlay` | `#302a4a` | `#f0ecfd` |

Rules:
- A selected row or tab always carries a second cue besides the fill: a 2 px accent bar, a check, or bold text. Fill steps are about 1.1:1 apart on purpose, so they never carry meaning alone.
- `text.primary`, `text.secondary` and `text.tertiary` are all at least 4.5:1 on hover and selected fills; on pressed fills `text.tertiary` is at least 3:1 (pressed lasts only while the pointer is down).
- Borderless buttons (owner, A12, A30): placement buttons (host, project, worktree), toolbar buttons, composer buttons and chips, and the "Chat | Terminal" switch have no border and no fill at rest; on hover the fill lights up (`fill.hover.<surface>`) and the label turns `text.primary`; open shows `fill.selected.<surface>`; keyboard focus shows the focus ring. Title bar buttons have no fill of their own (A11). Secondary buttons in dialogs, sheets and forms keep their `border.strong` outline.
- Disabled controls use `fill.disabled` and `text.disabled` and `aria-disabled`; they are exempt from contrast rules and never the only carrier of information.
- The only translucent colour tokens are `scrim`, the shadow colours and `fill.ripple` (Android press ripple, 12 percent; the platform composites it, so it cannot be a resolved solid, and accent and danger fills ripple with their pressed token instead). If a component ever needs another translucent colour, add a solid token for each surface instead.

### 2.7 Status

Each status has `fg` (text and icon), `solid` (fill), `on` (text on the fill), `bg` (tinted background, solid, resolved on the Canvas) and `border`. Danger also has `hover` and `pressed` fills for the destructive button; the other statuses have no interactive fills.

| Token | Dark | Light | Use |
|---|---|---|---|
| `status.success.fg` | `#5fd18a` | `#146c37` | Completed, saved, connected, passed. |
| `status.success.solid` | `#1a8040` | `#15803d` | Solid success fill (badge, banner icon plate, destructive button for danger). |
| `status.success.on` | `#ffffff` | `#ffffff` | Text and icons on status.success.solid. |
| `status.success.bg` | `#11271a` | `#e6f5ea` | Tinted success background, resolved solid on the Canvas (also fine on raised). |
| `status.success.border` | `#1f5a36` | `#9fd5b3` | Outline of a success banner or chip. |
| `status.warning.fg` | `#f2bd4a` | `#8a5700` | Needs attention but not blocked: ahead/behind, slow, expiring. |
| `status.warning.solid` | `#e9a11b` | `#a65f00` | Solid warning fill (badge, banner icon plate, destructive button for danger). |
| `status.warning.on` | `#1a1100` | `#ffffff` | Text and icons on status.warning.solid. |
| `status.warning.bg` | `#2a2210` | `#fdf3d9` | Tinted warning background, resolved solid on the Canvas (also fine on raised). |
| `status.warning.border` | `#6b5316` | `#e5c366` | Outline of a warning banner or chip. |
| `status.danger.fg` | `#ff7b7b` | `#b42323` | Failed, destructive, irreversible. |
| `status.danger.solid` | `#c93a3a` | `#c62828` | Solid danger fill (badge, banner icon plate, destructive button for danger). |
| `status.danger.on` | `#ffffff` | `#ffffff` | Text and icons on status.danger.solid. |
| `status.danger.bg` | `#2d1517` | `#fdeaea` | Tinted danger background, resolved solid on the Canvas (also fine on raised). |
| `status.danger.border` | `#6d2a2e` | `#eba4a4` | Outline of a danger banner or chip. |
| `status.danger.hover` | `#cf4141` | `#b52222` | Danger button fill on hover (dark steps lighter, light steps darker, like `accent.hover`). `status.danger.on` is at least 4.5:1 on it. |
| `status.danger.pressed` | `#b43030` | `#a01c1c` | Danger button fill while pressed. `status.danger.on` is at least 4.5:1 on it. |
| `status.info.fg` | `#63b8f0` | `#0b6a9e` | Neutral information and tips. |
| `status.info.solid` | `#1c75b0` | `#0b6a9e` | Solid info fill (badge, banner icon plate, destructive button for danger). |
| `status.info.on` | `#ffffff` | `#ffffff` | Text and icons on status.info.solid. |
| `status.info.bg` | `#10222f` | `#e5f2fa` | Tinted info background, resolved solid on the Canvas (also fine on raised). |
| `status.info.border` | `#1f4d6b` | `#9ccbe6` | Outline of a info banner or chip. |

Shape and word pairing (mandatory):

| Status | Glyph (Lucide) | Word in banners and badges |
|---|---|---|
| success | `circle-check` | "Done", "Saved", "Connected" |
| warning | `triangle-alert` | "Check", "Behind", "Expiring" |
| danger | `octagon-alert` | "Failed", "Error" |
| info | `info` | "Note" |

Success and danger are never told apart by hue alone: they always differ in glyph and in word. A colour-blind-safe status variant is open question 4.

### 2.8 Agent status

Six states, each with a colour and a **shape**. The shape is the primary cue, the colour is secondary. Sidebar rows, the status bar, the Delegation tree, host cards and the Android session list all use `AgentStatus` with exactly these glyphs.

| State | Colour token | Shape | Animation | Meaning |
|---|---|---|---|---|
| Running | `agent.running` | arc spinner (3/4 circle) | rotates, `duration.spinner` | The agent is working. |
| Waiting for you | `agent.waiting` | filled circle with a question mark | none; row tinted with `agent.waiting.bg` | Approval or an answer is needed. Needs the user. |
| Idle | `agent.idle` | hollow ring | none | Nothing running. |
| Error | `agent.error` | triangle with an exclamation mark | none; row tinted with `agent.error.bg` | The run failed. |
| Done | `agent.done` | circle with a check mark (`circle-check`) | none | The agent finished its work. Shown for a child agent in the Delegation tree and for a finished session where a state is needed. |
| Unknown or unreachable | `agent.unreachable` | dashed ring with a slash | none; the whole row is dimmed and shows the age of the last known state; banners, notification cards and Delegation tree rows also take the `agent.unreachable.bg` tint | The host cannot be reached. This is not "stopped". |

Glyph sizes: `AgentStatus` draws at `size.icon.xs` (12 px, Status bar items and dense rows), `size.icon.sm` (16 px, tabs and the Delegation tree) and `size.icon.md` (20 px, cards). Stroke follows the iconography rule for that size.

| Token | Dark | Light | Use |
|---|---|---|---|
| `agent.running` | `#4fc3d6` | `#0a7285` | Agent status: running. |
| `agent.waiting` | `#ff9d4d` | `#a84f00` | Agent status: waiting for you (approval or question). |
| `agent.idle` | `#9d9da7` | `#5f5f6b` | Agent status: idle. |
| `agent.error` | `#ff7b7b` | `#b42323` | Agent status: error. |
| `agent.done` | `{status.success.fg}` | `{status.success.fg}` | Agent status: done. An alias of `status.success.fg` (dark `#5fd18a`, light `#146c37`), so the two always agree. |
| `agent.unreachable` | `#b3a8d6` | `#6a5d9a` | Agent or host unknown or unreachable: the last known state is shown dimmed with its age. |
| `agent.waiting.bg` | `#2e2113` | `#fdeedd` | Row or card tint for a session waiting on the user, resolved on the Canvas and Frame. |
| `agent.error.bg` | `#2d1517` | `#fdeaea` | Row or card tint for a session in error, resolved on the Canvas and Frame. |
| `agent.unreachable.bg` | `#221f30` | `#efecf8` | Tint of an unreachable banner, notification card or Delegation tree row, resolved on the Canvas and overlay. |
| `agent.unreachable.border` | `{border.default}` | `{border.default}` | Outline of an unreachable banner or card. An alias of `border.default`; a theme may give it its own hue. |
| `agent.waiting.bg.overlay` | `#3e3124` | `#fff2e0` | Tint of a Notification card for a session waiting on the user, resolved on `surface.overlay`. Its offset from the overlay equals the offset of `agent.waiting.bg` from the Canvas. |
| `agent.error.bg.overlay` | `#3d2528` | `#ffeeed` | Tint of a Notification card for a session in error, resolved on `surface.overlay` by the same rule. |
| `agent.unreachable.bg.overlay` | `#322f41` | `#f3f0fb` | Tint of a Notification card for an unreachable host, resolved on `surface.overlay` by the same rule. |

### 2.9 Diff

Added and removed lines are told apart by glyph (`+` and `-` in the gutter), by position (the gutter column), and by colour. Added leans teal-green and removed leans rose so they differ in more than red-green hue.

| Token | Dark | Light | Use |
|---|---|---|---|
| `diff.add.bg` | `#12301f` | `#e3f5e8` | Added line background in the diff view and in chat diff snippets. |
| `diff.add.gutter` | `#1a4a2d` | `#c5ebd0` | Gutter behind line numbers of an added line, with a + glyph. |
| `diff.add.word` | `#1f6039` | `#a9e0ba` | Intra-line added-word highlight over diff.add.bg. |
| `diff.add.fg` | `#8be0a4` | `#116b34` | Added counts (+12), + glyph and added file letter on the Canvas. |
| `diff.remove.bg` | `#3a1719` | `#fdeaea` | Removed line background. |
| `diff.remove.gutter` | `#5a2229` | `#f8cdd1` | Gutter behind line numbers of a removed line, with a minus glyph. |
| `diff.remove.word` | `#722b34` | `#f2adb4` | Intra-line removed-word highlight over diff.remove.bg. |
| `diff.remove.fg` | `#ff9ea4` | `#a4262c` | Removed counts (-3), minus glyph and deleted file letter on the Canvas. |

Code text on every diff background is `text.primary`; line numbers are `text.secondary`.

### 2.10 Terminal

The terminal has its own palette group. It follows the app theme by default and can be set separately (a dark terminal in a light app, imported Warp or Ghostty themes).

| Token | Dark | Light | Use |
|---|---|---|---|
| `terminal.bg` | `#101012` | `#f8f8fa` | Terminal surface. |
| `terminal.fg` | `#dcdce0` | `#1f1f26` | Default terminal text on terminal.bg. |
| `terminal.cursor` | `#e8e8ec` | `#1f1f26` | Terminal cursor block. |
| `terminal.selection` | `#2f4f85` | `#bcd3ff` | Terminal selection background; terminal.fg stays 4.5:1 on it. |

ANSI 16, with measured contrast against `terminal.bg` of the same theme:

| Token | Dark | Ratio on terminal.bg | Light | Ratio on terminal.bg |
|---|---|---|---|---|
| `terminal.ansi.black` | `#3d3d46` | 1.8 | `#1f1f26` | 15.4 |
| `terminal.ansi.red` | `#f0717a` | 6.7 | `#b3261e` | 6.2 |
| `terminal.ansi.green` | `#7fd491` | 10.6 | `#1b7a3e` | 5.1 |
| `terminal.ansi.yellow` | `#e6c36a` | 11.2 | `#8a6100` | 5.2 |
| `terminal.ansi.blue` | `#6ea6f5` | 7.6 | `#1f55c7` | 6.2 |
| `terminal.ansi.magenta` | `#c792e8` | 7.9 | `#8e3fb5` | 5.6 |
| `terminal.ansi.cyan` | `#5cc8d0` | 9.6 | `#0a7580` | 5.1 |
| `terminal.ansi.white` | `#c9c9d1` | 11.5 | `#767683` | 4.2 |
| `terminal.ansi.bright-black` | `#868692` | 5.3 | `#52525c` | 7.3 |
| `terminal.ansi.bright-red` | `#ff8f96` | 8.7 | `#c42b2b` | 5.3 |
| `terminal.ansi.bright-green` | `#9be8a8` | 13.1 | `#16753a` | 5.4 |
| `terminal.ansi.bright-yellow` | `#f5d98a` | 13.7 | `#7a5600` | 6.3 |
| `terminal.ansi.bright-blue` | `#8bbcff` | 9.7 | `#1a4bb8` | 7.2 |
| `terminal.ansi.bright-magenta` | `#dbaaf5` | 10.0 | `#7a2fa6` | 7.1 |
| `terminal.ansi.bright-cyan` | `#7fe0e6` | 12.4 | `#066872` | 6.1 |
| `terminal.ansi.bright-white` | `#f2f2f5` | 17.0 | `#4a4a54` | 8.3 |

`terminal.ansi.black` (dark) and `terminal.ansi.white` (light) are background colours that programs also print as text. They are exempt from 4.5:1; the terminal applies a minimum-contrast adjustment of 4.5:1 to any foreground it draws (setting `terminal.min-contrast`, default 4.5).

### 2.11 Syntax highlighting

Used by the editor, code blocks in chat, the diff view and Markdown code fences. Each token is at least 4.5:1 on both `surface.sunken` and `surface.canvas`.

| Token | Dark | Light | Use |
|---|---|---|---|
| `syntax.comment` | `#8b93a1` | `#667082` | Comments. |
| `syntax.keyword` | `#c792ea` | `#7a3fc4` | Keywords, control flow. |
| `syntax.string` | `#e3a56f` | `#a14a14` | Strings. |
| `syntax.number` | `#b8d59a` | `#2d7a2f` | Numbers, booleans. |
| `syntax.type` | `#56c9b6` | `#0a7a6b` | Types, classes, interfaces. |
| `syntax.function` | `#e3d58f` | `#7a5a00` | Function and method names. |
| `syntax.variable` | `#9fd8ff` | `#0b6aa5` | Variables and parameters. |
| `syntax.property` | `#8cc2ff` | `#1a54b5` | Properties and fields. |
| `syntax.constant` | `#7fb4ff` | `#1a4db0` | Constants and enum members. |
| `syntax.operator` | `#cfcfd8` | `#3c3c46` | Operators. |
| `syntax.punctuation` | `#9a9aa6` | `#5c5c66` | Brackets, commas, delimiters. |
| `syntax.tag` | `#f2777f` | `#b5243a` | Markup tags. |
| `syntax.attribute` | `#e8b96c` | `#8a5200` | Markup attributes, decorators. |
| `syntax.invalid` | `#ff6b6b` | `#b42323` | Syntax errors. |

Mapping: highlight capture names (for example tree-sitter `@comment`, `@keyword`) resolve to these 14 tokens; anything unmapped uses `text.primary`.

### 2.12 Usage rings (per provider slot)

The Activity rail shows one ring per agent provider. The ring colour belongs to a **provider slot** (1 to 6), assigned on first use and changeable by the user; the provider is identified by its icon inside the ring, never by the colour. The slots follow a colour-blind-safe set (blue, orange, bluish green, reddish purple, yellow, violet).

| Token | Dark | Light | Use |
|---|---|---|---|
| `usage.ring.1` | `#5aa2ff` | `#1f5fd0` | Usage ring, provider slot 1. |
| `usage.ring.2` | `#ff9f5a` | `#b85a00` | Usage ring, provider slot 2. |
| `usage.ring.3` | `#4fd1a5` | `#0b7d5c` | Usage ring, provider slot 3. |
| `usage.ring.4` | `#e58ac0` | `#b53d88` | Usage ring, provider slot 4. |
| `usage.ring.5` | `#e6d45a` | `#8a7400` | Usage ring, provider slot 5. |
| `usage.ring.6` | `#a98bff` | `#6a46d8` | Usage ring, provider slot 6. |
| `usage.ring.track` | `#3a3a40` | `#cfcfd8` | Unfilled part of a usage ring. |
| `usage.ring.warn` | `#f2bd4a` | `#8a5700` | Arc colour once usage is at or above 80 percent, with a warning dot on the icon. |
| `usage.ring.limit` | `#ff7b7b` | `#b42323` | Arc colour once usage is at or above 95 percent, with a limit mark on the icon. |

Ring anatomy: `size.usage.ring` (32 px), stroke 2 px, track `usage.ring.track`, arc from 12 o'clock clockwise, provider icon 16 px centred. At 80 percent usage the arc becomes `usage.ring.warn` and a warning dot (`size.indicator.dot`, 6 px) shows at the icon's top right; at 95 percent it becomes `usage.ring.limit` with a limit dot of the same size. The accessible name carries the numbers: "Claude usage 62 percent, resets in 2 days".

### 2.13 Other tokens

| Token | Dark | Light | Use |
|---|---|---|---|
| `selection.bg` | `#2c4a7a` | `#bfd4fb` | Text selection in chat, editor and fields. |
| `match.bg` | `#5a4a14` | `#fbe58a` | Find-bar match background. |
| `match.active.bg` | `#6e5412` | `#f6c443` | The active find match. |
| `fill.skeleton` | `#26262a` | `#e6e6eb` | Skeleton placeholder block on the Canvas. |
| `fill.disabled` | `#1a1a1d` | `#f0f0f4` | Disabled control fill on the Canvas and raised surfaces. |
| `fill.ripple` | `#ffffff1f` | `#14141a1f` | Android press ripple on neutral surfaces (12 percent). The only translucent state fill; see 2.6. |
| `badge.neutral.bg` | `#34343a` | `#d5d5dd` | Fill of a neutral count or category badge on the Frame, Canvas and raised surfaces (rail and Sidebar counts, tab counts). |
| `badge.neutral.fg` | `{text.primary}` | `{text.primary}` | Text of a neutral badge. An alias of `text.primary`. |
| `switch.knob.off` | `#a9a9b3` | `#5b5b67` | Knob of a switch in the off state, on a `surface.sunken` track. |
| `scrim` | `#000000a6` | `#14141e52` | Dimming layer behind a modal dialog or a sheet. |
| `progress.track` | `#101012` | `#f4f4f7` | Track of a Progress bar, with `border.subtle` 1 px. The `surface.sunken` value. |

### 2.14 Measured contrast

All ratios use WCAG 2 relative luminance, computed from the token values above. Required: 4.5:1 for text, 3:1 for icons, control borders, focus rings and graphical marks.

| Pair (foreground on background) | Dark | Light | Required | Result |
|---|---|---|---|---|
| `text.primary` on `surface.frame` | 14.6 | 15.4 | 4.5:1 | pass |
| `text.primary` on `surface.canvas` | 15.5 | 17.3 | 4.5:1 | pass |
| `text.primary` on `surface.raised` | 14.1 | 17.9 | 4.5:1 | pass |
| `text.primary` on `surface.overlay` | 13.0 | 17.9 | 4.5:1 | pass |
| `text.primary` on `surface.sunken` | 16.1 | 16.3 | 4.5:1 | pass |
| `text.secondary` on `surface.frame` | 8.4 | 7.6 | 4.5:1 | pass |
| `text.secondary` on `surface.canvas` | 8.9 | 8.5 | 4.5:1 | pass |
| `text.secondary` on `surface.overlay` | 7.4 | 8.8 | 4.5:1 | pass |
| `text.tertiary` on `surface.frame` | 6.4 | 5.4 | 4.5:1 | pass |
| `text.tertiary` on `surface.canvas` | 6.8 | 6.1 | 4.5:1 | pass |
| `text.tertiary` on `surface.overlay` | 5.7 | 6.3 | 4.5:1 | pass |
| `text.tertiary` on `fill.selected.overlay` | 4.6 | 5.2 | 4.5:1 | pass |
| `text.secondary` on `fill.pressed.overlay` | 5.4 | 6.6 | 4.5:1 | pass |
| `text.on-accent` on `accent.default` | 5.3 | 5.2 | 4.5:1 | pass |
| `text.on-accent` on `accent.hover` | 4.5 | 6.2 | 4.5:1 | pass |
| `text.on-accent` on `accent.pressed` | 6.8 | 7.5 | 4.5:1 | pass |
| `accent.fg` on `surface.frame` | 8.0 | 5.8 | 4.5:1 | pass |
| `accent.fg` on `surface.canvas` | 8.5 | 6.5 | 4.5:1 | pass |
| `accent.fg` on `surface.overlay` | 7.1 | 6.7 | 4.5:1 | pass |
| `accent.fg` on `accent.subtle.canvas` | 7.3 | 5.6 | 4.5:1 | pass |
| `border.strong` on `surface.frame` | 3.9 | 3.6 | 3:1 | pass |
| `border.strong` on `surface.canvas` | 4.2 | 4.0 | 3:1 | pass |
| `border.strong` on `surface.raised` | 3.8 | 4.1 | 3:1 | pass |
| `border.strong` on `surface.overlay` | 3.5 | 4.1 | 3:1 | pass |
| `border.strong` on `surface.sunken` | 4.3 | 3.8 | 3:1 | pass |
| `border.focus` on `surface.frame` | 7.9 | 5.8 | 3:1 | pass |
| `border.focus` on `surface.canvas` | 8.3 | 6.5 | 3:1 | pass |
| `border.focus` on `surface.sunken` | 8.7 | 6.1 | 3:1 | pass |
| `border.focus` on `surface.overlay` | 7.0 | 6.7 | 3:1 | pass |
| `status.success.fg` on `surface.canvas` | 9.5 | 6.3 | 4.5:1 | pass |
| `status.warning.fg` on `surface.canvas` | 10.6 | 5.9 | 4.5:1 | pass |
| `status.danger.fg` on `surface.canvas` | 7.3 | 6.3 | 4.5:1 | pass |
| `status.info.fg` on `surface.canvas` | 8.4 | 5.7 | 4.5:1 | pass |
| `status.success.fg` on `status.success.bg` | 8.3 | 5.8 | 4.5:1 | pass |
| `status.warning.fg` on `status.warning.bg` | 9.1 | 5.5 | 4.5:1 | pass |
| `status.danger.fg` on `status.danger.bg` | 6.8 | 5.7 | 4.5:1 | pass |
| `status.info.fg` on `status.info.bg` | 7.5 | 5.2 | 4.5:1 | pass |
| `status.success.on` on `status.success.solid` | 5.0 | 5.0 | 4.5:1 | pass |
| `status.warning.on` on `status.warning.solid` | 8.5 | 4.9 | 4.5:1 | pass |
| `status.danger.on` on `status.danger.solid` | 5.1 | 5.6 | 4.5:1 | pass |
| `status.info.on` on `status.info.solid` | 5.0 | 5.9 | 4.5:1 | pass |
| `agent.running` on `surface.frame` | 8.3 | 4.8 | 4.5:1 | pass |
| `agent.waiting` on `surface.frame` | 8.3 | 4.8 | 4.5:1 | pass |
| `agent.idle` on `surface.frame` | 6.4 | 5.4 | 4.5:1 | pass |
| `agent.error` on `surface.frame` | 6.9 | 5.7 | 4.5:1 | pass |
| `agent.unreachable` on `surface.frame` | 7.8 | 5.0 | 4.5:1 | pass |
| `agent.waiting` on `agent.waiting.bg` | 7.6 | 4.9 | 4.5:1 | pass |
| `text.primary` on `diff.add.bg` | 12.1 | 15.7 | 4.5:1 | pass |
| `text.primary` on `diff.remove.bg` | 13.5 | 15.4 | 4.5:1 | pass |
| `text.primary` on `diff.add.word` | 6.4 | 12.0 | 4.5:1 | pass |
| `text.primary` on `diff.remove.word` | 8.4 | 9.7 | 4.5:1 | pass |
| `text.secondary` on `diff.add.gutter` | 5.0 | 6.7 | 4.5:1 | pass |
| `text.secondary` on `diff.remove.gutter` | 6.0 | 6.1 | 4.5:1 | pass |
| `terminal.fg` on `terminal.bg` | 13.9 | 15.4 | 4.5:1 | pass |
| `terminal.cursor` on `terminal.bg` | 15.6 | 15.4 | 3:1 | pass |
| `syntax.comment` on `surface.sunken` | 6.1 | 4.5 | 4.5:1 | pass |
| `syntax.keyword` on `surface.sunken` | 7.9 | 5.7 | 4.5:1 | pass |
| `syntax.string` on `surface.sunken` | 8.9 | 5.5 | 4.5:1 | pass |
| `syntax.function` on `surface.sunken` | 12.8 | 5.8 | 4.5:1 | pass |
| `usage.ring.1` on `surface.frame` | 6.6 | 5.0 | 3:1 | pass |
| `usage.ring.5` on `surface.frame` | 11.4 | 4.0 | 3:1 | pass |
| `usage.ring.warn` on `surface.frame` | 10.0 | 5.3 | 3:1 | pass |
| `usage.ring.limit` on `surface.frame` | 6.9 | 5.7 | 3:1 | pass |
| `status.danger.on` on `status.danger.hover` | 4.7 | 6.5 | 4.5:1 | pass |
| `status.danger.on` on `status.danger.pressed` | 6.2 | 7.8 | 4.5:1 | pass |
| `agent.done` on `surface.frame` | 9.0 | 5.6 | 4.5:1 | pass |
| `agent.done` on `surface.canvas` | 9.5 | 6.3 | 4.5:1 | pass |
| `agent.unreachable` on `agent.unreachable.bg` | 7.3 | 4.9 | 4.5:1 | pass |
| `text.secondary` on `agent.unreachable.bg` | 7.8 | 7.5 | 4.5:1 | pass |
| `text.tertiary` on `agent.unreachable.bg` | 6.0 | 5.4 | 4.5:1 | pass |
| `badge.neutral.fg` on `badge.neutral.bg` | 10.5 | 12.2 | 4.5:1 | pass |
| `text.secondary` on `badge.neutral.bg` | 6.0 | 6.0 | 4.5:1 | pass |
| `switch.knob.off` on `surface.sunken` | 8.2 | 6.1 | 3:1 | pass |
| `switch.knob.off` on `fill.hover.raised` | 6.4 | 6.0 | 3:1 | pass |
| `text.primary` on `fill.ripple` over `surface.canvas` | 11.0 | 13.4 | 4.5:1 | pass |
| `text.primary` on `fill.ripple` over `surface.overlay` | 8.9 | 13.9 | 4.5:1 | pass |
| `text.tertiary` on `fill.ripple` over `surface.overlay` (transient) | 3.9 | 4.9 | 3:1 | pass |
| `border.disabled` on `fill.disabled` (decorative) | 1.3 | 1.2 | none | exempt |

Class checks:

- dark: lowest syntax token is syntax.comment at 5.89:1
- dark: lowest ANSI colour (excluding the exempt one) is terminal.ansi.bright-black at 5.28:1
- light: lowest syntax token is syntax.comment at 4.55:1
- light: lowest ANSI colour (excluding the exempt one) is terminal.ansi.green at 5.08:1

The ripple rows use the colour the user sees: the 12 percent ripple blended over the surface. `agent.done` and `badge.neutral.fg` are aliases, so their ratios equal those of `status.success.fg` and `text.primary` on the same backgrounds.

**Failures: none.** Items that are exempt by design: disabled text, `text.tertiary` on the transient pressed state (checked at 3:1), the two ANSI background colours noted above, and decorative borders (`border.subtle`, `border.default`, `border.disabled`, `agent.unreachable.border`, `usage.ring.track`).

---

## 3. Typography

### 3.1 Families

| Role | Family | Licence | Files bundled |
|---|---|---|---|
| UI sans | **Geist** | SIL Open Font License 1.1 | Regular 400, Medium 500, SemiBold 600 (static) |
| Monospace | **JetBrains Mono** | SIL Open Font License 1.1 | Regular, Italic, Bold, Bold Italic (static) |
| Terminal symbols (fallback only) | Symbols Nerd Font Mono | MIT and OFL components; confirm terms before shipping | one file, used only for private-use glyphs |

Why these two:
- **Geist** is designed for developer tools: a clear shape at 11 to 13 px, a generous x-height, an open licence we can embed, and (to be confirmed in the font spike) tabular figures for counters and timestamps. It also keeps the look of the current app, which the owner already knows. A UI sans that is not a system font gives all three desktop systems and Android the same metrics, so layouts do not shift between platforms.
- **JetBrains Mono** is built for code and terminals: stable cell metrics, box-drawing and block characters, and clear distinction of `0 O 1 l I`. One monospace serves the terminal, the editor, code blocks, paths, IDs and kbd badges. Ligatures are off by default in the terminal and available as an editor option.
- Fonts are **bundled**; there are no run-time web fonts. The app never fetches a font. The artifact preview loads the two families from Google Fonts for display only.
- Static instances, not variable fonts, are bundled so GPUI and React Native pick exact weights on every platform (variable-font weight selection differs between toolkits; to confirm in the GPUI version we pin).
- Fallback stack for glyphs the bundled files do not contain: the OS UI font (Segoe UI, system UI on macOS, Noto Sans on Linux and Android). Language coverage of the bundled files is to be checked with Spanish first, then the other target languages.
- Themes may choose another installed or bundled family within limits (section 6).

### 3.2 Type scale

Sizes are logical pixels on desktop and scale-independent pixels (sp) on Android. Line height is fixed per style. Weights used: 400, 500, 600 only (700 only for terminal bold).

| Style | Desktop size / line / weight | Android (sp) size / line / weight | Used for |
|---|---|---|---|
| `caption` | 11 / 16 / 400 | 12 / 16 / 400 | Status bar, timestamps, counts, metadata |
| `overline` | 11 / 16 / 600, +0.04em, capitals | 12 / 16 / 600 | Sidebar and settings section headers |
| `label` | 12 / 16 / 500 | 14 / 20 / 500 | Chips, tabs, small buttons, form labels |
| `body-sm` | 12 / 18 / 400 | 13 / 18 / 400 | Helper text, secondary row text, tooltips |
| `body` | 13 / 20 / 400 | 14 / 20 / 400 | Default UI text |
| `body-strong` | 13 / 20 / 600 | 14 / 20 / 600 | Row titles, inline emphasis |
| `title-sm` | 14 / 20 / 600 | 16 / 22 / 600 | Card and section titles |
| `title` | 16 / 24 / 600 | 18 / 24 / 600 | Dialog and sheet titles |
| `heading` | 20 / 28 / 600 | 22 / 28 / 600 | Page titles |
| `display` | 28 / 36 / 600, -0.01em | 28 / 36 / 600 | Empty states, onboarding |
| `chat` | 14 / 22 / 400 | 16 / 24 / 400 | Message text in the Conversation |
| `mono-sm` | 12 / 16 / 400 | 12 / 16 / 400 | Inline code, paths, IDs, kbd |
| `mono` | 13 / 20 / 400 | 13 / 18 / 400 | Code blocks, editor, diff, tool calls |
| `terminal` | 13 / 16 / 400 (1.2 times, user 1.0 to 1.5) | 12 / 16 / 400 (user 10 to 16) | Terminal cells |

Rules:
- Minimum text size is 11 px (desktop) and 12 sp (Android). A theme or the font-scale setting never goes below that.
- No negative letter spacing except `display`. Body text uses the font's own spacing.
- Android text must scale with the system font size up to 200 percent. Layouts use min-height, not fixed height. Rail, badges and chips cap scaling at 130 percent.
- Truncate with an ellipsis, never wrap, in rows, tabs and the status bar; give the full text in a tooltip (desktop) or the detail view (Android).
- Prose in the Conversation uses `chat`; Markdown headings inside a message step down from `title-sm` to `body-strong`, not up.

### 3.3 Tabular numbers

Turn on tabular figures (OpenType `tnum`) for every number that changes in place or lines up: usage percentages, counts in badges and the status bar, memory use, terminal count, ports, timestamps and durations, file sizes, diff counts (+12 -3), line numbers, calendar day numbers, table number columns (right-aligned). Do not use it in running prose. JetBrains Mono is tabular by design. Geist's `tnum` feature is to be confirmed in the font spike, and the feature enabled through GPUI font features and React Native `fontVariant: ['tabular-nums']`.

---

## 4. Spacing, sizing, radius, elevation, icons, motion

### 4.1 Spacing (4 px base)

| Token | Value | Use |
|---|---|---|
| `space.0` | 0 | none |
| `space.half` | 2 | hairline gaps (only step not a multiple of 4) |
| `space.1` | 4 | icon to label in chips; row padding in compact density |
| `space.2` | 8 | gap between related controls; row and menu padding |
| `space.3` | 12 | card and field padding; label to control; gap between stacked cards |
| `space.4` | 16 | panel and dialog padding; Android screen gutter |
| `space.5` | 20 | large control padding |
| `space.6` | 24 | page gutters; section gap |
| `space.8` | 32 | empty-state padding |
| `space.10` | 40 | page top padding |
| `space.12` | 48 | onboarding rhythm |
| `space.16` | 64 | maximum gap |

Rules: spacing inside a component is at most one step smaller than the gap around it; related items sit at `space.2` or less, groups at `space.4` or more. Align to the 4 px grid; do not use 6, 10, 14.

### 4.2 Sizing

| Token | Value | Use |
|---|---|---|
| `size.control.compact` | 24 | Control and row height, compact density. |
| `size.control.default` | 28 | Control and row height, default density (the default). |
| `size.control.comfortable` | 32 | Control and row height, comfortable density. |
| `size.touch.default` | 44 dp | Android control visual height. |
| `size.touch.large` | 48 dp | Android visual height for primary actions. Two-line rows use `size.row.two-line.touch` (56 dp). |
| `size.hit.pointer` | 24 | Smallest desktop pointer target (WCAG 2.2, 2.5.8) |
| `size.hit.touch` | 48 dp | Smallest Android target, both axes (hit slop around smaller visuals) |
| `size.icon.xs` | 12 | Icon size (4.6): glyphs inside badges, status glyphs in rows and the status bar. |
| `size.icon.sm` | 16 | Icon size (4.6): default in controls, rows and tabs. |
| `size.icon.md` | 20 | Icon size (4.6): rail, composer, Android rows. |
| `size.icon.lg` | 24 | Icon size (4.6): empty states, Android toolbar, page headers. |
| `size.rail.width` | 48 | Activity rail |
| `size.rail.button` | 36 | Rail button. |
| `size.rail.ring` | 32 | Usage ring in the rail. |
| `size.titlebar.windows` | 36 | Title bar height on Windows (also Linux). |
| `size.titlebar.macos` | 40 | Title bar height on macOS. |
| `size.statusbar` | 24 | Status bar |
| `size.avatar.sm` | 20 | Agent and team icon, small. |
| `size.avatar.md` | 28 | Agent and team icon, medium. |
| `size.avatar.lg` | 40 | Agent and team icon, large. |
| `size.indicator.dot` | 6 | Status, unread, connection and dirty dots; the Badge dot variant |
| `size.badge.height` | 16 | Badge height on desktop (Android: 20 dp) |
| `size.tag.height` | 20 | Badge tag variant height on desktop (Android: 24 dp); ruling B9, 2026-11-25 |
| `size.kbd.height` | 16 | Kbd key cap height |
| `size.toolbar` | 36 | Toolbar strip in a Work panel tab (Browser chrome, Diff view) |
| `size.progress.height` | 4 | Progress bar height. |
| `size.skeleton.line` | 12 | Skeleton text line height. |
| `size.switch.track.width` | 32 | Switch track width on desktop. |
| `size.switch.track.height` | 18 | Switch track height on desktop. |
| `size.switch.knob` | 12 | Switch knob on desktop. |
| `size.switch.touch.track.width` | 36 dp | Switch track width on Android. |
| `size.switch.touch.track.height` | 20 dp | Switch track height on Android. |
| `size.switch.touch.knob` | 14 dp | Switch knob on Android. |
| `size.scrollbar.thickness` | 6 | Overlay scrollbar thumb thickness. |
| `size.scrollbar.min-thumb` | 24 | Overlay scrollbar minimum thumb length. |
| `size.composer.min-height` | 44 | Composer text area minimum. |
| `size.composer.max-height` | 176 | Composer text area maximum. |
| `size.composer.start-min-height` | 66 | Start-page Composer text area minimum (three lines). |
| `size.field.multiline.min` | 68 | Multi-line text field minimum (three lines of `body` plus 4 px above and below) |
| `size.minimap.track` | 24 | Message minimap track. |
| `size.minimap.tick.sm` | 8 | Minimap tick, small. |
| `size.minimap.tick.md` | 12 | Minimap tick, medium. |
| `size.minimap.tick.lg` | 16 | Minimap tick, large. |
| `size.row.two-line` | 44 | Two-line list row, desktop. |
| `size.row.two-line.touch` | 56 dp | Two-line list row, Android (fits two lines at 200 percent font scale). |
| `size.row.settings` | 40 | Settings row minimum height, label alone. |
| `size.row.settings.helper` | 48 | Settings row minimum height, with a helper line. |
| `size.diff.gutter` | 40 | Diff view line number column. |
| `size.diff.marker` | 20 | Diff view marker column. |
| `size.usage.ring` | 32 | Usage ring on desktop and Android (same value as `size.rail.ring`) |
| `size.appbar.android` | 56 dp | Android top app bar, also the Sheet header |
| `size.sheet.handle.width` | 32 dp | Android bottom sheet drag handle width. |
| `size.sheet.handle.height` | 4 dp | Android bottom sheet drag handle height. |
| `size.titlebar.control.width` | 46 | One Windows or Linux window control; height `size.titlebar.windows` |

Density: compact (24), default (28), comfortable (32). Density changes control height, row height and row padding only; type sizes and the rail do not change. Work panel tabs are always 28 (`size.control.default`) and do not follow the comfortable density. Component widths and heights (dialog, popover, picker, menu, tooltip, palette and the rest) are `layout.*` tokens, listed in 5.5.

### 4.3 Radius

| Token | Value | Use |
|---|---|---|
| `radius.none` | 0 | Edges on the window edge |
| `radius.xs` | 2 | Scrollbar thumb, kbd corner |
| `radius.sm` | 4 | Badges, compact buttons, tags (composer chips are pills, see `radius.full`) |
| `radius.md` | 6 | Buttons, fields, rows, menu items, checkbox |
| `radius.lg` | 8 | Cards, popovers, menus, tooltips, toasts, the sunken segmented track |
| `radius.xl` | 12 | Dialogs, command palette, model picker, the Composer (A32, A33), the Commit message field and Commit button (A49, taking the Composer's soft corners) |
| `radius.2xl` | 16 | Android bottom sheet top corners |
| `radius.canvas` | 12 | **The Canvas top-left corner** where it meets the Frame |
| `radius.full` | pill | Pills, avatars, switches, rings, composer chips (attachment, mention, suggestion, access and Model picker chips, A29), the round Send (half the short side in native code) |

A nested shape's radius is the parent radius minus the padding between them, never smaller than `radius.xs`. A theme may scale the whole radius scale 0.5 to 1.5 and set `radius.canvas` 0 to 20.

### 4.4 Borders

One width: `border.width.default` 1 px (a logical pixel; two device pixels on a 2x display). Use `border.default` around raised and overlay surfaces, `border.subtle` for dividers inside a surface, `border.strong` for the outline of a control. No border between the rail, Sidebar and Title bar, and none around the Canvas. Thick (2 px) is for the active tab underline, the active rail bar, the selected-row accent bar and the splitter drag line.

### 4.5 Elevation (no blur, no translucency)

Depth is shown with surface level, a border and, for floating layers, a drop shadow. Nothing uses backdrop blur or translucency to separate layers.

| Level | Used for | Surface | Border | Shadow token | Dark value | Light value |
|---|---|---|---|---|---|---|
| e0 | Frame, Canvas, regions, rows | frame or canvas | none | `shadow.e0` | none | none |
| e1 | Composer, summary cards, settings groups | `surface.raised` | `border.default` | `shadow.e1` | `0 1px 2px 0 rgba(0,0,0,0.40)` | `0 1px 2px 0 rgba(20,20,40,0.08)` |
| e2 | Menus, popovers, tooltips, toasts, model picker, regions shown as overlays | `surface.overlay` | `border.default` | `shadow.e2` | `0 6px 16px -2px rgba(0,0,0,0.55)` | `0 6px 16px -2px rgba(20,20,40,0.16)` |
| e3 | Dialogs, command palette, Android sheets | `surface.overlay` | `border.default` | `shadow.e3` + `scrim` behind | `0 16px 40px -4px rgba(0,0,0,0.65)` | `0 16px 40px -4px rgba(20,20,40,0.22)` |

Shadow values are `offset-x offset-y blur spread colour`. They map one to one to a GPUI box shadow (offset, blur radius, spread radius, colour; confirm the blur convention in the pinned GPUI version). On Android, map e1 to elevation 1 dp, e2 to 6 dp, e3 to 12 dp and use the surface-level difference in dark; Android draws its own shadow colour. In dark, the lighter surface and the border do most of the work; shadows are a secondary cue.

### 4.6 Iconography

- **One set: Lucide** (ISC licence), 24 by 24 grid, outline only, round caps and joins. Icons are bundled as SVG; GPUI draws them as SVG paths, React Native through `react-native-svg`. No second icon set, no emoji as icons, no filled variants. Exceptions: agent and provider brand marks (supplied as assets, see below) and the status glyphs in 2.7 and 2.8 (drawn from Lucide shapes plus the spinner arc).
- **Sizes**: 12 (chevrons and glyphs inside badges), 16 (default in controls, rows, tabs), 20 (rail, composer, Android rows), 24 (empty states, Android toolbar, page headers).
- **Stroke**: absolute stroke in logical pixels, so lines stay crisp at every size: 1.5 px at 12 and 16, 1.75 px at 20, 2 px at 24 (scaled stroke in the 24 unit grid is 3, 2.25, 2.1 and 2). Never override per icon.
- **Colour**: icons use `text.secondary` at rest, `text.primary` on hover, `accent.fg` when selected or active, `text.disabled` when disabled. Status icons use their status token. The Activity rail's selected entry is its active entry, so its icon is `accent.fg`.
- **Mode switch icons**: Lucide `layout-grid` (Workspaces) and `bot` (Bot Teams). Rail: `list-checks` (Tasks), `calendar` (Calendar), `store` (Marketplace), `sun` and `moon` (theme), `settings` (Settings).
- **Brand marks** for agents and providers are supplied per provider as full-colour assets (owner, A16 and A18); an agent that registers none uses the generic agent mark. Provider names are trademarks of their owners.
- **Agent icon instead of the agent name (owner, A16, A17, A20).** Every place that shows an agent uses its own icon, never a letter (C, X, G, O, K). Where a label, row, title, tab, chip or cell starts with an agent name, the icon (14 px in text, 16 px in rows) replaces the name: "[Codex icon] Feed endpoints". The name stays as the tooltip and in the accessible label. A table's agent column is titled "Agent" and shows only the 16 px icon.
- **Project icon (owner, A41).** A project shows its own icon (the folder glyph by default, or the colour and icon the user chose) wherever it appears: Sidebar, tabs, pickers, project settings.
- **Icon before label (owner, 2026-11-12).** Every label that names a kind of thing carries an icon on its left that shows what the label holds: field labels in cards and detail panes (Host `server`, Branch `git-branch`, Repo `book` (owner, v30 and v36; it replaces the earlier `folder`), Account `person`, Pipeline `workflow`, URL `globe`, Last seen `clock`, Schedule `calendar`, Agent `bot`, Access `shield`, Memory `brain`, Board the kanban icon), buttons, menu items, chips, tabs and rows. The icon sits 4 to 6 px before the text, at the size of its context (12 beside `caption` and `label`, 16 in rows and buttons) and in the label colour. Exceptions: table column headers, plain prose, numbers and values (the value never repeats the label icon), and a label whose meaning no icon shows better than the word (then no icon, never a decorative one).
- **File icons (owner, 2026-11-12).** Every file name in trees, lists, tabs, diff headers and search results carries a file-type icon on its left, as in the owner's reference: each file type uses its representative colour (owner, 2026-11-12) as a small rounded badge with its short name: `TS` #3178c6 with white, `TSX` and `JSX` React cyan #61dafb with #0b2a33, `PY` Python blue #3776ab with Python yellow #ffd43b, `JS` #f7df1e with #1b1b1c, `RS` #dea584, `CSS` #663399, `{}` JSON #cbcb41; agent instruction files use the agent's icon (`CLAUDE.md` the Claude icon, `AGENTS.md` the Codex icon); any other file is generic and gets a filled document glyph in grey (`text.tertiary`). Folders keep the outline folder glyph. Badge colours are fixed file-type colours, not theme tokens; extensions can add a mapping for new file types.
- Extensions supply an icon as a Lucide name or a single-ink 24-unit SVG (at most 8 KB, no gradients, no embedded raster). It is drawn in the token colour for its state; it cannot choose its own colour.

### 4.7 Motion

Durations and curves (tokens `duration.*` and `ease.*`):

| Token | Value | Used for |
|---|---|---|
| `duration.instant` | 0 ms | Reduced-motion replacement for everything below |
| `duration.fast` | 100 ms | Hover and pressed fills, tooltip fade, checkbox tick |
| `duration.base` | 150 ms | Popover, menu, toast fade; focus ring appearing |
| `duration.moderate` | 200 ms | Region collapse and expand, dialog enter, mode switch morph |
| `duration.slow` | 300 ms | Bottom sheet, page-level transitions |
| `duration.card-insert` | 250 ms | Session summary card insert |
| `duration.tooltip-delay` | 400 ms | Pointer rest time before a tooltip shows |
| `duration.spinner` | 800 ms | One spinner revolution |

| Curve | Value | Used for |
|---|---|---|
| `ease.standard` | `cubic-bezier(0.2, 0, 0, 1)` | Move between two on-screen states: collapse, expand, reflow, morph |
| `ease.enter` | `cubic-bezier(0.16, 1, 0.3, 1)` | Things appearing: popovers, dialogs, toasts, summary cards |
| `ease.exit` | `cubic-bezier(0.3, 0, 0.8, 0.15)` | Things leaving; exits last 70 percent of the enter time |
| `ease.linear` | `linear` | Spinners and indeterminate progress |

GPUI and React Native both take an easing function, not a CSS string; the shared motion module implements the cubic-bezier solver once and exposes these four names.

What animates:

| Element | Animation |
|---|---|
| Hover, pressed, selected fills | colour step, `duration.fast`, `ease.standard`; none on keyboard focus moves |
| Popover, menu, model picker | opacity 0 to 1 and 4 px move toward its final position, `duration.base`, `ease.enter`; exit `ease.exit` |
| Tooltip | opacity only, `duration.fast`, after `duration.tooltip-delay`; no delay when moving between tooltips |
| Dialog | opacity and scale 0.98 to 1, `duration.moderate`, `ease.enter`; scrim fades with it |
| Android bottom sheet | slides from the bottom, `duration.slow`, `ease.enter`; drag follows the finger |
| Toast | slides in 8 px and fades, `duration.moderate`; leaves with opacity only |
| Sidebar, Session summary, Work panel, Project sidebar | width, `duration.moderate`, `ease.standard`; inner content keeps its final width and is clipped, so text never reflows during the move |
| Spinner, indeterminate bar | rotation or travel, `ease.linear`; every spinner appears only after `SPINNER_DELAY_MS` (200 ms) of loading, on buttons, rows, hosts and tabs alike. A skeleton appears after `SKELETON_DELAY_MS` (100 ms) |

What does **not** animate: theme switch (instant, so no surface flashes), status colour changes, text, list virtualisation, drag and resize (they follow the pointer), page changes between rail destinations.

**Mode switch icon morph** (`ModeSwitch`, D-SH-10): the two icons (`layout-grid`, `bot`) sit stacked in one 20 px box. On click, over `duration.moderate` with `ease.standard`: the outgoing icon scales 1 to `MORPH_SCALE`, rotates 0 to -`MORPH_ANGLE_DEG` degrees and fades 1 to 0 across the first 60 percent; the incoming icon scales `MORPH_SCALE` to 1, rotates +`MORPH_ANGLE_DEG` to 0 degrees and fades 0 to 1 across the last 60 percent. The two overlap in the middle `MORPH_OVERLAP_PERCENT` percent (constants in 4.8). The Sidebar content swaps at the same moment with a 100 ms cross-fade. Reduced motion: instant swap of both. This is a cross-fade with scale and rotate; a true path morph is not built into GPUI.

**Session summary card insert** (`SessionSummaryCard`, D-AG-27): a card appears when its first record arrives and never as an empty placeholder. Over `duration.card-insert` with `ease.enter`: the card's height grows from 0 to its measured content height, opacity goes 0 to 1 and it moves up 8 px into place; the cards below slide down with `ease.standard` over the same time. Cards keep the fixed order (Environment, Plan, Checklist, Context, Results, Subagents, Connectors, Background tasks, QA servers, then extension cards). If the column is scrolled and the new card lands above the visible area, it is inserted without animation and the scroll position is anchored so nothing jumps. A polite screen-reader announcement says "Plan card added". Reduced motion: the card appears instantly in place.

**Reduced motion**: read the OS setting (Windows "Animation effects", macOS "Reduce motion", GNOME `enable-animations`, Android "Remove animations") and offer an override in Appearance. When on: every animation above uses `duration.instant`, except functional progress (a spinner keeps turning). Skeletons are static `fill.skeleton` blocks in v1: there is no shimmer, so reduced motion changes nothing for them. Cursor blink follows the OS setting.

### 4.8 Behaviour constants (not tokens)

These values tune behaviour, not appearance. They are not theme-able and not in `tokens.json`; the shared motion and input modules define them once under these names, and specs refer to them by name. Names are upper case so they are never mistaken for tokens.

| Constant | Value | Used by | Meaning |
|---|---|---|---|
| `MORPH_SCALE` | 0.6 | ModeSwitch | Scale of the outgoing and incoming icon at the ends of the morph |
| `MORPH_ANGLE_DEG` | 30 | ModeSwitch | Rotation of the outgoing (negative) and incoming (positive) icon, in degrees |
| `MORPH_OVERLAP_PERCENT` | 20 | ModeSwitch | Share of `duration.moderate` in which both icons are visible; each icon fades across 60 percent |
| `SCROLLBAR_HIDE_DELAY_MS` | 1000 | ScrollArea | Time after scrolling stops before an overlay thumb hides |
| `SEARCH_DEBOUNCE_MS` | 200 | SearchField | Pause in typing before a filter or search applies (equals `duration.moderate` by value only) |
| `SPLITTER_SNAP_PX` | 40 | Splitter | Distance below a region's minimum at which dragging collapses it |
| `SUBMENU_HOVER_DELAY_MS` | 150 | Menu | Pointer rest time before a submenu opens or closes on hover |
| `MESSAGE_CLAMP_LINES` | 12 | ConversationMessageList | Lines of a long user message shown before "Show more" |
| `SPINNER_DELAY_MS` | 200 | Button, Progress, DiffView, TerminalSurface, host and row loaders | Time a loading action waits before its spinner shows |
| `SKELETON_DELAY_MS` | 100 | Skeleton, ConversationMessageList, DiffView | Time a loading view waits before its skeleton shows |
| `UNDO_TIMEOUT_MS` | 8000 | Toast, QueuedMessage | Auto-dismiss time of an Undo toast; 8 seconds is the minimum on every platform |

Changing a constant is a behaviour change: it needs the owner's decision, not a theme.

---

## 5. Layout

### 5.1 Desktop region model

Regions are named as in the glossary. Widths are logical pixels.

| Region | Surface | Default | Min | Max | Resizable | Collapsible |
|---|---|---|---|---|---|---|
| Activity rail | Frame | 48 | 48 | 48 | no | never (always visible) |
| Sidebar | Frame | 256 | 200 | 400 | yes, drag the right edge | yes (title bar toggle); hides only the Sidebar |
| Title bar | Frame | height 36 (Windows, Linux) or 40 (macOS) | - | - | no | no |
| Conversation | Canvas | fills the rest | 360 | none | no (flex) | no; it is the last region to give way |
| Session summary | Canvas | 288 | 240 | 400 | yes | yes (title bar toggle) |
| Work panel | Canvas | 480 | 320 | none (takes what is left) | yes | yes (title bar toggle) |
| Project sidebar | Canvas | 288 | 240 | 480 | yes | yes (title bar toggle) |
| Status bar | Canvas column, bottom | height 24 | - | - | no | yes (visibility menu) |

Window minimum: 800 by 520. Message column and composer have a maximum content width of 720 px, centred in the Conversation when it is wider; the minimap sits on the Conversation's right edge.

Order, left to right in a session: Activity rail, Sidebar, Conversation, Session summary, Work panel, Project sidebar. The Status bar runs under the Canvas column. Region toggles live in the Title bar (right side).

Resizing rules:
- Splitters are 1 px lines with a 6 px hit area (`border.splitter.hit`). On hover the line becomes `border.strong`; while dragging it becomes `accent.default` at 2 px. Double-click resets to the default width. The cursor is the horizontal resize cursor.
- Widths are remembered per region, per window.
- Dragging a region below its minimum collapses it (snap at the minimum minus `SPLITTER_SNAP_PX`, 40 px; see 4.8); dragging it back out reopens at the minimum.
- A region never makes the Conversation narrower than 360 px: the resize stops there.
- Opening a region that does not fit uses the collapse order below.

Collapse order when the window narrows (the Canvas width cannot satisfy the minimums). The first to give way is the least central:

| Step | Region that gives way | Window width at which it happens (all regions open) | Becomes |
|---|---|---|---|
| 1 | Session summary | below 1408 | Hidden; the title bar toggle opens it as an overlay (`z.region-overlay`, `shadow.e2`) |
| 2 | Project sidebar | below 1168 | Same: hidden, opens as an overlay |
| 3 | Sidebar | below 928 | Same: hidden, opens as an overlay over the Canvas |
| 4 | Work panel | below 728 | Hidden; the Conversation fills the Canvas; the Work panel opens as an overlay |

(The thresholds are the widths drawn on the owner's artboard `ws-narrow`, Steps 1 to 4. They replace the earlier 1528, 1288, 1048 and 848 px, which were sums of minimums derived before the artboards existed (decision 0027). They are logical widths after UI zoom, so a zoomed UI hits them in a wider window. At the 800 px window minimum with zoom 100 percent, steps 1 to 3 occur; step 4 needs UI zoom above about 110 percent.) Regions the user opened stay "open" in their state; the app restores them when the window grows again. An overlay region closes on Escape, on a click outside, and when focus leaves it. Only one overlay region is open at a time.

Full-canvas pages: Tasks, Calendar, Marketplace, Settings and the Bot Teams pages open as **full pages over the whole Canvas**. The Conversation, Session summary, Work panel and Project sidebar give way, the Sidebar is hidden except on Bot Teams pages, which keep it (pending-owner-review item 16), and the Title bar shows a Back button and the page title. Back returns to the session in the state it was left. Content maximum widths: Settings 880, Tasks and Marketplace 1120 (`layout.page.*`); Calendar and Board are fluid. Pages use `heading` for the title, `space.6` gutters, and section spacing of `space.8`.

### 5.2 Title bar and window chrome per OS

| | macOS | Windows | Linux |
|---|---|---|---|
| Title bar height | 40 | 36 | 36 |
| Window controls | Traffic lights at the **left, inside the Frame**, 16 px from the left edge, centred vertically; native | Minimise, maximise, close at the **right**, 46 by 36 px each (`size.titlebar.control.width` by `size.titlebar.windows`), drawn by the app | Same as Windows, drawn by the app (client-side decorations) |
| Rail top | Reserve a chrome zone of 40 px at the top of the rail for the traffic lights, which extend about 76 px to the right across the rail and the start of the Sidebar; the mode switch sits below it at y 48 | The mode switch starts at y 8; the rail has no reserved zone | Same as Windows |
| Title bar left inset | 80 px gutter when the Sidebar is hidden or narrower than the lights; Sidebar toggle starts after it | Rail width 48, then the Sidebar toggle | Same as Windows |
| Drag area | The empty part of the Title bar and the chrome zone | The empty part of the Title bar | Same |
| Right group | Region toggles | Region toggles, then window controls | Same as Windows |

The Title bar is part of the Frame: same fill, no border, no shadow. Its content order: Sidebar toggle, back and forward, session title (left aligned, `title-sm`, truncated), flexible gap, region toggles (new chat, terminal, split, checklist, open in Work panel, Project sidebar), window controls on Windows and Linux. Double-click on the empty part follows the OS (maximise or zoom).

### 5.3 Rail anatomy

Top group (top to bottom): mode switch, Tasks, Calendar, Marketplace, then extension icons (3 visible at the minimum window height, more as height allows, overflow in a "More" button). Flexible gap. Bottom group: theme toggle, usage rings (one per provider), Settings. Buttons are 36 px with `radius.md`; icons 20 px; gaps `space.1`. The active item has `fill.selected.frame` plus a 2 px `accent.fg` bar on the left edge of the rail. Rail icons rest at `text.secondary`. Tooltips show the name and the shortcut, on the right of the rail.

### 5.4 Android layout

| Window class (dp width) | Layout |
|---|---|
| Compact, under 600 (phones) | A stack of single panes, no bottom tabs (as `M-SH-01`). Top app bar 56 dp (`size.appbar.android`) on `surface.frame`; content on `surface.canvas`, square corners. One screen at a time; back follows the system back gesture. |
| Medium, 600 to 839 | Master and detail. The master pane (360 dp, `layout.android.master`) is on `surface.frame`; the detail pane is on `surface.canvas` with the rounded top-left corner `radius.canvas`, the same frame and canvas structure as desktop. |
| Expanded, 840 and up | Same as medium. Extra width goes to the detail pane, whose content column is capped at 720 dp. |

- **Bottom sheets** are the Android form of popovers and menus (`X-DLG-02`): modal, 90 percent maximum height (with the keyboard open, 90 percent of the height above it; a sheet never grows to full height), `radius.2xl` top corners, a 32 by 4 dp drag handle (`size.sheet.handle.width` by `size.sheet.handle.height`) in `border.strong`, `scrim` behind, `shadow.e3`, dismissed by swiping down, tapping outside or the system back gesture. Focus moves into the sheet and returns to the trigger.
- **Safe areas**: draw edge to edge and apply insets. The top app bar extends under the status bar (frame colour); the bottom inset is the gesture area (about 24 dp) or the three-button bar (48 dp); the display cutout and rounded corners inset the sides in landscape; the composer sits above the keyboard (IME inset). Minimum horizontal padding is `space.4` (16 dp).
- **Touch**: controls are 44 dp with a 48 dp hit target; spacing between targets at least 8 dp; destructive actions need a confirmation (`X-DLG-01`). Segments of one segmented control form one target group: they touch with no 8 dp gap, each keeping a 48 dp hit area; the 8 dp spacing applies between separate controls.
- No status bar on Android (the system bar stays). Usage rings sit on host cards (`M-HM-02`). Extensions are not designed on Android in v1.

### 5.5 Component widths and heights

Fixed widths and heights of overlays and panels. Values are logical pixels (dp on Android). A component narrower than its width token shrinks to fit the window with `space.4` margins.

| Token | Value | Used by |
|---|---|---|
| `layout.dialog.width.sm` / `md` / `lg` | 400 / 440 / 560 | Dialog (md by default), Secret modal (md), Permission review (lg); sm for single-field confirms |
| `layout.dialog.android.min` / `max` | 280 / 560 dp | Android dialog width range |
| `layout.popover.width.sm` / `md` / `lg` | 280 / 320 / 360 | Usage ring popover (sm), composer actions (md), other popovers (lg) |
| `layout.picker.width` | 320 | Picker panel |
| `layout.model-picker.width` | 560 | Model picker: `provider-column` 48, `model-column` 296, `reasoning-column` 216, `max-height` 420, chip `chip.max-width` 220 |
| `layout.menu.min-width` / `max-width` | 180 / 280 | Menu |
| `layout.tooltip.max-width` | 240 | Tooltip |
| `layout.toast.width.min` / `default` / `max` | 320 / 360 / 400 | Toast and Notification card, one set of widths |
| `layout.palette.width` / `input-height` / `max-height` / `top` | 600 / 44 / 480 / 96 | Command palette, as drawn on artboard `ws-palette`; not a dialog size |
| `layout.findbar.width` | 360 | Find bar |
| `layout.tab.max-width` | 200 | Work panel tab |
| `layout.calendar.column.min` / `layout.calendar.hour` | 96 / 48 | Calendar day column; one hour |
| `layout.board.column.width` | 280 | Board column |
| `layout.settings.control.min` / `max` | 160 / 320 | Settings row control column |
| `layout.message.bubble.max-width` | 85% | User message bubble in the message column |
| `layout.empty-state.text.max-width` | 60ch | Empty state explanation line |

---

## 6. Theming and customisation

### 6.1 What a theme may and may not change

| A theme may override | A theme may not |
|---|---|
| Every semantic colour token in section 2 (surfaces, text, borders, accent, fills, status, agent colours, diff, terminal, syntax, usage rings, scrim), in a dark or a light variant | The **meaning** of a status colour: success, warning, danger and info must stay in their hue ranges (success 90 to 180 degrees, warning 25 to 65, danger 340 to 20, info 180 to 260) and keep their glyphs and words |
| The accent (set from one seed colour, section 6.4) | **Focus ring visibility**: `border.focus` must always be present at 2 px and at least 3:1 on every surface |
| Fonts: UI family and mono family, from the bundled fonts or fonts installed on the machine, and a font scale 0.9 to 1.3 | **Minimum contrast**: the validation table below (a theme under the floors is rejected, not adjusted) |
| Density: compact, default, comfortable | Region structure: order of regions, the rail width, the existence of the Frame and Canvas, the 11 px minimum text size |
| Radius: scale 0.5 to 1.5 and `radius.canvas` 0 to 20 | Motion beyond the tokens (durations, curves) and reduced-motion behaviour |
| A separate terminal palette (imports Warp and Ghostty themes) | Images, gradients, blur, translucency (not drawable natively; `scrim` alpha only 0.30 to 0.80) |
| Layout defaults: region default widths within min and max | The look of **security chrome**: the Permission review, the Secret modal and destructive confirm dialogs always use core components with validated tokens; no extension can place content in them |
| | Icon sets (v1 has one icon set; icon themes are not supported) |

### 6.2 Validation (a theme below any minimum is rejected with the failing pair named)

| Rule | Check | Minimum |
|---|---|---|
| V1 | `text.primary`, `text.secondary`, `text.tertiary` on the five surfaces and on hover and selected fills | 4.5:1 |
| V2 | `text.on-accent` on `accent.default`, `accent.hover`, `accent.pressed` | 4.5:1 |
| V3 | `accent.fg` on frame, canvas, sunken, raised, overlay | 4.5:1 |
| V4 | `border.strong` and `border.focus` on the five surfaces | 3:1 |
| V5 | Status `fg` on the four surfaces and on its `bg`; `on` on `solid`, and `status.danger.on` on `status.danger.hover` and `pressed` | 4.5:1 |
| V6 | `agent.*` on frame, canvas, raised, overlay; `agent.unreachable` and `text.tertiary` on `agent.unreachable.bg` | 4.5:1 |
| V7 | `text.primary` on every `diff.*` background; `text.secondary` on diff gutters | 4.5:1 |
| V8 | `terminal.fg` and every ANSI colour on `terminal.bg` (black and white exempt) | 4.5:1; cursor 3:1 |
| V9 | Every `syntax.*` on `surface.sunken` and `surface.canvas` | 4.5:1 |
| V10 | `usage.ring.1` to `6`, `warn`, `limit` on `surface.frame` | 3:1 |
| V11 | Frame against Canvas; each hover, selected, pressed fill against its surface | 1.05:1; selected against hover 1.04:1 |
| V12 | Status hue ranges (6.1); font scale, radius scale and density inside their ranges; the mono family must be monospaced | pass or reject |

The validator runs when a theme is loaded, when an extension contributes one, and in the theme editor as the user changes a colour. Derived fills (`fill.*`, `accent.subtle.*`) may be omitted from a theme; the app derives them with the documented blend percentages and then validates them. If a theme gives them, they are validated as given.

### 6.3 Theme file format (sketch)

JSON. Token names are the keys. Colours are `#rrggbb` solid values (and `#rrggbbaa` for `scrim` only). Missing tokens inherit from the base theme named in `extends`.

```json
{
  "format": "massorca.theme/1",
  "id": "acme.midnight",
  "name": "Acme Midnight",
  "appearance": "dark",
  "extends": "massorca.dark",
  "colors": {
    "surface.frame": "#1a1b26",
    "surface.canvas": "#16161e",
    "surface.sunken": "#12121a",
    "surface.raised": "#1c1d29",
    "surface.overlay": "#23243a",
    "text.primary": "#e6e8f2",
    "border.strong": "#7a7f99",
    "border.focus": "#8ab4ff"
  },
  "accent": { "seed": "#3b82f6" },
  "terminal": { "extends": "massorca.dark", "colors": { "terminal.bg": "#12121a" } },
  "fonts": { "ui": "Geist", "mono": "JetBrains Mono", "scale": 1.0 },
  "density": "default",
  "radius": { "scale": 1.0, "canvas": 12 }
}
```

A theme is a pair or a single variant: `appearance` says which. A "theme pack" lists a dark and a light file so the theme toggle and the OS appearance both work. The built-in themes are `massorca.dark` and `massorca.light` (the values in this file). Layout presets (region default widths, which regions are open) are a separate file kind (`massorca.layout/1`) and are not a theme.

### 6.4 User-chosen accent

Settings, Appearance offers six presets (lavender, the default; blue, teal, rose, orange, graphite) and a custom colour. The user gives a **seed**; the app derives the accent set per theme and validates it:

1. Convert the seed to OKLCH and keep its hue and chroma (clamped for graphite).
2. `accent.default`: the lightness at which `text.on-accent` (white or near-black, whichever reads better) is at least 4.5:1.
3. `accent.hover` moves one lightness step from `accent.default` (lighter in dark, darker in light) and `accent.pressed` one step darker in both; each is re-checked so `text.on-accent` stays at 4.5:1.
4. `accent.fg`: the lightness at which the accent reads at least 4.5:1 on frame, canvas, raised and overlay of the active theme (lighter in dark, darker in light).
5. `border.focus`: the `accent.fg` value, adjusted until it is at least 3:1 on all five surfaces.
6. `accent.subtle.*`: the blend percentages in 2.6.
7. If no lightness satisfies a pair (for example a saturated yellow), the app shifts lightness and chroma until it does and shows the result; it never shows a seed it cannot make accessible.

The accent never changes status colours, agent colours, diff colours or terminal colours.

---

## 7. Extension contribution points

Extensions add to MassOrca at the defined points below and nowhere else. They render with the same components and tokens as the core; in v1 they have **no raw pixel drawing**, no custom colours and no custom fonts. (Whether a sandboxed web view tab may exist as an escape hatch depends on the extension model and the browser-engine choice: open question 2.)

### 7.1 General rules

- **Declarative views.** An extension returns a **view description**: a tree of allowed shared components with data and events, validated against a schema (option G1 in `docs/inventory/customisation.md`). The core renders it on desktop; the same description can render on Android later (option R1). Allowed components in an extension view: stack, row and grid layout, Text, Icon, Button, Icon button, Segmented control, Toggle, Checkbox, Radio, Text field, Select, List and List row, Table, Card, Badge, Progress, Skeleton, Empty state, Error state, Banner, Tabs, Divider, Markdown block, Code block. Not allowed: Terminal surface, Browser chrome, Frame, Activity rail, Dialog and Sheet (opened through the host API so the core controls them).
- **Same tokens.** Extension views use only semantic tokens (`text.secondary`, `status.warning.fg`). They cannot define colours. An extension that wants a theme contributes a theme file (section 6), which is validated like any other.
- **Focus.** An extension never takes focus on its own. Focus moves into extension content only after a user action (click, a command the user ran, F6). Contributed content gets the standard focus ring automatically.
- **Identity.** Every contributed item exposes the extension's name in its tooltip and accessible description. Core icons and agent status glyphs cannot be reused as an extension's own icon.
- **Keybindings.** An extension may suggest default shortcuts. A shortcut that clashes with a core key loses; the clash is shown in Settings, Input and shortcuts (D-ST-04), and the user decides. An extension never rebinds a core key without the user's consent.
- **Modes.** A contribution declares `modes`: Workspaces, Bot Teams or both. It is hidden in the other mode.
- **Surfaces.** A contribution declares `surfaces`: `desktop` (v1). Android renders no extension contributions in v1.
- **Limits** below are enforced by the host: text is truncated with an ellipsis, counts beyond the limit go to an overflow, and over-limit manifests fail validation at install (D-EX-02).
- **Permissions** that a contribution needs (network, files, commands, notifications) are shown in the permission review (D-EX-02) before install.

### 7.2 The contribution points

Fourteen points. IDs refer to the inventory slots (D-SL-nn) where they exist.

| # | Contribution point (inventory) | Renders where | What the extension provides | Size limits | Can change | Cannot |
|---|---|---|---|---|---|---|
| 1 | **Activity rail item** (D-SL-09) | Rail, in an "Extensions" group under the top group, after a divider; excess goes to a "More" button | Icon (Lucide name or single-ink SVG), label, optional badge (dot or count 1 to 99+) with kind neutral, accent or danger, and a target: a full-canvas page, a Sidebar section or a command | 1 item per extension by default (3 with user consent); label 32 characters | Its own icon, label, badge count and badge kind | Restyle or resize the rail; join the core groups; sit above core items; animate; open itself automatically; set badge colours |
| 2 | **Sidebar section** (D-SL-05) | Sidebar, below the workspaces or teams tree, as a collapsible section | Section header (icon, label, up to 2 header icon buttons) and a body view built from Sidebar rows or a List | Header label 24 characters; row label 80 characters with ellipsis; up to 3 trailing actions per row; body is virtualised | Its rows, row icons, badges and actions; default collapsed state | Restyle rows; add rows outside its section; expand itself on update; appear in the other mode unless declared |
| 3 | **Work panel tab type** (D-SL-08) | Work panel: appears in the `+` menu (not in the empty-state four) and in its own tabs | Tab type id, icon, label, a view description, a dirty marker, a state to restore | Tab title 24 characters; the view fills the tab body | The tab's content and title | Replace or hide the four core tab types; take the terminal's focus; draw outside the tab body |
| 4 | **Project sidebar tab** (D-SL-01) | Project sidebar tab strip, after the four core tabs (Files, Sessions, Source control, Checks) | Icon (tab shows icon only, label is the tooltip and accessible name), optional count badge, a view description | 2 tabs per extension; label 24 characters | Its tab's content and badge | Reorder before the core tabs; hide core tabs; change the region width |
| 5 | **Session summary card** | Session summary column, after the core cards, in the card style of the core | Card title, optional right-aligned value, a collapsed body (up to 6 rows) and an optional detail view; a "has information" signal | Title 24 characters; value 12 characters; 3 cards per extension | When its card exists (it appears only when the extension has information, with the standard insert animation) | Show an empty placeholder; reorder or hide core cards; force itself open; animate beyond the standard insert |
| 6 | **Conversation card** (D-SL-03) | Inside the transcript as a turn block | Header (icon, title), a view description, up to 3 actions, sent through the protocol as a message | Up to 320 px tall, then "Show more"; 3 actions | Its card content | Edit or delete other messages; appear as the user or the agent; run an action without a user click; receive secret values |
| 7 | **Status bar item** (D-SL-02) | Status bar, left or right group by priority | Icon (12 px), text (caption, tabular), optional tooltip view, a click command, a state kind (info, success, warning, danger) | 2 items per extension; text 24 characters; priority 1 to 100 (lowest hides first when narrow) | Its text, icon and state kind | Change the bar's height or colours; render views in the bar itself (tooltip only); blink or animate |
| 8 | **Command** (D-SL-07) | Command palette (D-CP-01) and the keybinding editor (D-ST-04) | Id, title, category, optional icon, `when` conditions, a suggested keybinding, an argument form | Title 60 characters; category 24 characters | Its commands and suggested shortcuts | Run on its own; override a core keybinding without consent; hide a command from the palette; take focus beyond what the command's target declares |
| 9 | **Settings section** (D-SL-06, D-EX-04) | Settings page: its own section under the Extensions group (D-ST-09) | A list of Settings rows: toggle, text, number, select, keybinding, list, and secret references (vault links, never values) | 40 rows per section; labels 60 characters | Its own settings and defaults | Edit other sections; restyle rows; display or store a secret value; open itself |
| 10 | **Full-canvas page** (D-SL-04) | The whole Canvas, like Tasks and Calendar; Title bar shows Back and the page title | Page id, title, icon, a view description that fills the content area | Title 40 characters; content width fluid up to `layout.page.default` unless it declares fluid | The page's content | Change the Frame, the Title bar controls or the Status bar; hijack Back; open without a user action |
| 11 | **Composer action** | Composer bottom row, between attach and the Model picker, as an Icon button, or as an entry in the `+` menu | Icon, label (tooltip), an optional popover view up to 320 px wide, an action on the draft or attachments | 2 per extension; label 32 characters | What it does to attachments or draft text, through a visible step | Send a message; change the draft silently; move or hide Send or the Model picker |
| 12 | **Model picker entries** (D-SL-10) | Model picker (X-AG-02): an icon in the agent rail, and rows in the list; reasoning levels in the right column | Agent or provider (icon 20 px, name), models (name, provider icon, reasoning level names, speed modes, access modes), a usage ring slot request | Model name 40 characters; level names 16 characters | Which agents and models it offers | Take Ctrl or Cmd 1 to 4 (the first four rows of the view shown); change the picker layout; become the default without consent |
| 13 | **Context menu items** | The named menus (workspace row, session row, file row, tab, message, selection, diff line, browser page link) in a trailing "Extensions" group after a divider | Label, optional icon, `when` conditions, a command | Label 40 characters; 5 items per extension per menu; one submenu level | Its own items | Remove, reorder or precede core items; run a destructive action without the core confirm (X-DLG-01) |
| 14 | **Notification and toast** (X-NT-01, D-NT-01) | Toast stack (bottom right of the Canvas) and the notification card stack | Kind (info, success, warning, danger), message, at most one action; for cards a title and body and up to 2 actions | Toast 120 characters; card title 60 and body 160; at most 1 toast per 5 s per extension and 3 visible | The message and action | Take focus; play a sound; bypass notification settings or OS do-not-disturb; imitate core messages (the extension name always prefixes the text); send OS notifications without the user's per-extension permission (X-NT-02) |

Contribution manifests are data; behaviour comes from the extension's runtime, which is decided separately (open decisions in `docs/inventory/customisation.md`, section C).

### 7.3 Themes as a contribution

An extension may also contribute **themes** (section 6) and **layout presets**. They go through the same validation as user files and appear in Settings, Appearance. They are data only.

---

## 8. Keyboard and accessibility foundations

### 8.1 Modifier mapping

| Role | Windows and Linux | macOS | Written in specs as |
|---|---|---|---|
| Primary modifier | `Ctrl` | `Cmd` (command key) | `Mod` |
| Secondary | `Alt` | `Option` | `Alt` |
| Shift | `Shift` | `Shift` | `Shift` |
| Control (macOS only) | - | `Control` | `Ctrl` is never used for `Mod` on macOS |

Shortcuts are stored as `Mod+K` and **displayed per OS** by the Kbd badge: `Ctrl+K` on Windows and Linux, `⌘K` on macOS (symbols `⌃ ⌥ ⇧ ⌘` in that order). Windows and Linux order is `Ctrl+Alt+Shift+Key`. Terminal copy and paste are the exception: `Ctrl+Shift+C/V` on Windows and Linux, `Cmd+C/V` on macOS, so the shell's interrupt key keeps its meaning (see `app-flow.md`, Keyboard). Every shortcut is rebindable in D-ST-04. `Mod+W` closes the active Work panel tab and never the window; the window closes with its OS key (`Alt+F4` on Windows and Linux, `Cmd+Q` on macOS).

### 8.2 Focus ring

- Colour `border.focus`; width 2 px (`border.focus.width`); offset 2 px (`border.focus.offset`); radius = the element's radius plus the offset.
- Shown on keyboard focus (`focus-visible`); always shown on text fields, search fields and the composer when focused (their border switches to `border.focus` as well).
- Where the ring would be clipped (a scroll area, the Frame edge, inside a menu): draw it inset (offset -2) in `border.focus`; on accent-filled controls draw the inset ring in `text.on-accent`.
- A theme or extension can never remove or recolour it below 3:1. One ring style for all components.
- Focus is never lost silently: closing a popover, menu, dialog or overlay region returns focus to the control that opened it; removing the focused item moves focus to its neighbour.

### 8.3 Focus order and regions

- `F6` moves focus to the next region and `Shift+F6` to the previous, on all three OSes (macOS may need `Fn` on laptop keyboards; the same action is available from the command palette and rebindable). Order: Title bar, Activity rail, Sidebar, Conversation, Session summary, Work panel, Project sidebar, Status bar, then back to the Title bar. Hidden or collapsed regions are skipped. Overlay regions (narrow windows) join the order while open.
- `Tab` and `Shift+Tab` move between controls inside a region; arrow keys move inside a composite control (rail, tree, tabs, list, menu, segmented control, toolbar): one tab stop for the whole control (roving focus).
- Dialogs and sheets trap focus; `Escape` closes the topmost overlay, one at a time. A permission review does not close on Escape while its check is running; Escape works again when the check finishes. Escape on a focused toast or Notification card dismisses it; Delete and Backspace do not.
- `Mod+K` opens the command palette, which can reach every region toggle and page by name. Inside a terminal on Windows and Linux the shell keeps `Ctrl+K`, so the palette opens there with `Ctrl+Shift+P` (pending-owner-review item 12).
- A focused splitter moves its edge with the arrow keys: 8 px per press, 24 px with Shift. Home and End go to the limits.

### 8.4 Screen reader names and roles

| Region or control | Role | Accessible name |
|---|---|---|
| Activity rail | navigation | "Activity rail" |
| Sidebar | navigation | "Sidebar" |
| Title bar | banner or toolbar | "Title bar" |
| Conversation | main | "Conversation" plus the session title |
| Session summary | complementary | "Session summary" |
| Work panel | region | "Work panel" |
| Project sidebar | complementary | "Project sidebar" |
| Status bar | status | "Status bar" |
| Icon button | button | Its tooltip text, required (a build error without a label) |
| Agent status glyph | image | The state word: "Running", "Waiting for you", "Idle", "Error", "Done", "Unreachable. Last known: running, 5 min ago" |
| Usage ring | meter or progressbar | "Claude usage 62 percent, resets in 2 days" |
| Diff line | text | "Added line" or "Removed line" before the content |
| Summary card inserted | live region (polite) | "Plan card added" |
| Toast | status (polite); errors alert (assertive) | The message |

Also: every image and icon either has a name or is marked decorative; tooltips are also the accessible description; live regions announce a status change once, not repeatedly. GPUI's accessibility support on desktop is to be verified (open question 1); the roles and names above are the target.

### 8.5 Hit targets

Desktop pointer targets are at least 24 by 24 px (`size.hit.pointer`), which is also the compact control height; icon buttons are 28 by 28 by default. Android targets are at least 48 by 48 dp in both axes, with hit slop around smaller visuals and at least 8 dp between targets. Resizable splitters have a 6 px hit area, plus keyboard resize with arrow keys when focused.

### 8.6 Android and TalkBack

- Every control has a label (`accessibilityLabel`) and a role; a state is in the label or `accessibilityState` ("selected", "expanded").
- Swipe-only or long-press-only actions (row menus, delete) also exist as `accessibilityActions`, so TalkBack users reach them.
- Reading order follows the visual order; bottom sheets announce their title when they open and trap focus; the composer's send button announces its disabled state.
- Text scales to 200 percent; no information depends on a fixed height; color is never the only cue (shape and word as in 2.7 and 2.8).
- Respect "Remove animations", "Bold text" and high-contrast text settings.

---

## 9. Desktop and Android differences

| Aspect | Desktop (GPUI) | Android (React Native) |
|---|---|---|
| Toolkit | GPUI, native GPU drawing, no browser | React Native with native components and `react-native-svg` |
| Navigation | Regions, Activity rail, full-canvas pages | Stack of screens; master-detail from 600 dp; no rail |
| Frame and Canvas | L-shaped Frame, inset Canvas with rounded top-left corner | Top app bar on `surface.frame`, content on `surface.canvas`; the rounded corner returns in master-detail layouts |
| Theme source | Light and dark themes plus the toggle in the rail; OS appearance optional | Follows the system by default, with a manual choice (open question 5); no Material You dynamic colour |
| Tokens | Same names and values | Same names and values |
| Control height | 24 / 28 / 32 px by density | 44 dp visual, 48 dp hit target |
| Type | px scale, body 13 | sp scale, body 14, scales to 200 percent |
| Pointer | Hover states, tooltips, right-click menus, resizable splitters | No hover; long-press and sheets instead; no tooltips |
| Popover and menu | Anchored popover or menu (e2) | Bottom sheet (e3) |
| Dialogs | Centred dialog, `radius.xl` | Dialog for confirms; sheets for everything else |
| Elevation | Borders, solid surfaces, box shadows | Same surfaces and borders; Android elevation 1, 6, 12 dp |
| Motion | Tokens as defined; reduced motion from the OS | Same tokens; "Remove animations" from the system; native feel for the sheet gesture |
| Keyboard | Full shortcut set, F6 region cycling, `Mod` mapping | Hardware keyboard: basic navigation; no custom shortcuts in v1 |
| Status | Status bar, rail usage rings | Host cards with usage rings; system status bar untouched |
| Density | Compact, default, comfortable | One density (touch) |
| Extensions | 14 contribution points | None rendered in v1 |
| Window | Min 800 by 520; per-OS title bar | Edge to edge with safe-area insets |
| Icons | Lucide at 12, 16, 20, 24 | Lucide at 20 and 24 (16 in dense chips, 12 for status glyphs in rows) |
| Scrollbars | Custom thin overlay scrollbars drawn in `border.strong`, `radius.xs` | System scroll indicators |

---

## 10. Open questions and not covered

### Open questions

1. **Screen reader support on desktop.** Does GPUI, in the version we pin, expose an accessibility tree (or can we add one) so screen readers on Windows, macOS and Linux read the roles and names in 8.4? If not, v1 would be keyboard-complete and high-contrast but not screen-reader-complete on desktop. The same spike should confirm static font weights, tabular figures and the box-shadow blur convention.
2. **Extension UI beyond declarative views.** May an extension show a sandboxed web view as a Work panel tab or page in v1? It is the only way to a fully custom UI, but it adds a second web surface, depends on the browser-engine decision, and cannot exist on Android.
3. **Provider marks in the rail.** Closed: agent icons are drawn in full colour everywhere, rail rings included (owner, A16 and A18).
4. **Built-in accessibility themes.** Closed: v1 ships dark and light themes and a theme validator, with no high-contrast or colour-blind theme (pending-owner-review item 35).
5. **Android appearance.** Follow the system light and dark setting only, or also offer a manual choice and Material You dynamic colour? This file assumes system plus manual, without dynamic colour, so the frame and canvas colours stay the product's.

### Not covered

- Component specs (anatomy, states, props, behaviour): the next pass, `docs/design/components/<Name>.md`.
- Illustration, empty-state artwork, marketing and brand mark; app icon.
- Right-to-left layout mirroring and per-language typography beyond the font fallback rule.
- Sound, haptics and notification sound design.
- Chart and data-visualisation palette beyond the usage rings (usage detail view, D-AC-06).
- Mobile emulator device frames (D-SH-18) and Browser pane page content (the embedded web engine draws it).
- Linux window decoration details under different compositors; Windows snap layouts.
- The extension runtime, sandbox and distribution (decided in the extension-model research).
- iOS (out of the initial scope).
- Print and export styles.

---

## Components

The component pass fills one file per row, written on top of these tokens. This section is an index only. IDs are from the screen inventory; `-` means a shared primitive with no screen of its own. Platforms: D desktop, A Android, B both.

| Component | ID(s) in inventory | Spec file | Platforms |
|---|---|---|---|
| Activity rail | D-SH-09 | `docs/design/components/ActivityRail.md` | D |
| Rail item | D-SH-11 to D-SH-16, D-SL-09 | `docs/design/components/RailItem.md` | D |
| Usage ring | D-SH-15, D-AC-06, M-HM-02 | `docs/design/components/UsageRing.md` | B |
| Mode switch | D-SH-10 | `docs/design/components/ModeSwitch.md` | D |
| Sidebar tree | D-WS-01, D-WS-29, D-HO-01 | `docs/design/components/SidebarTree.md` | D |
| Sidebar row | D-WS-02 | `docs/design/components/SidebarRow.md` | D |
| Title bar | D-SH-03 | `docs/design/components/TitleBar.md` | D |
| Region toggle | D-SH-03 | `docs/design/components/RegionToggle.md` | D |
| Frame and Canvas | D-SH-01 | `docs/design/components/FrameCanvas.md` | D (A: master-detail) |
| Status bar | D-SH-05 | `docs/design/components/StatusBar.md` | D |
| Status item | D-SH-05, D-SL-02 | `docs/design/components/StatusItem.md` | D |
| Agent status | X-AG-01 | `docs/design/components/AgentStatus.md` | B |
| Tabs (Work panel) | D-SH-17, D-TM-01 | `docs/design/components/WorkPanelTabs.md` | D |
| Project sidebar tabs | D-SH-04 | `docs/design/components/ProjectSidebarTabs.md` | D |
| Conversation message list | D-AG-03, D-AG-24, M-CH-01 | `docs/design/components/ConversationMessageList.md` | B |
| Message minimap | D-AG-28 | `docs/design/components/MessageMinimap.md` | D |
| Message and turn blocks (text, code, tool call, diff snippet, approval card) | D-AG-03, X-CH-02, X-CH-06, X-RV-02 | `docs/design/components/MessageBlocks.md` | B |
| Queued message | X-CH-03 | `docs/design/components/QueuedMessage.md` | B |
| Plan view | X-CH-05 | `docs/design/components/PlanView.md` | B |
| Composer | D-AG-03, D-AG-01, M-CH-02 | `docs/design/components/Composer.md` | B |
| Model picker | X-AG-02 | `docs/design/components/ModelPicker.md` | B (A: sheet) |
| Session summary card | D-AG-27 | `docs/design/components/SessionSummaryCard.md` | D |
| Delegation tree | X-DL-01, X-CH-06 | `docs/design/components/DelegationTree.md` | B |
| Secret request card | X-CH-07 | `docs/design/components/SecretRequestCard.md` | B |
| Secret modal | X-VT-01 | `docs/design/components/SecretModal.md` | B |
| Button | - | `docs/design/components/Button.md` | B |
| Icon button | - | `docs/design/components/IconButton.md` | B |
| Segmented control | - | `docs/design/components/SegmentedControl.md` | B |
| Toggle and switch | - | `docs/design/components/Switch.md` | B |
| Checkbox | - | `docs/design/components/Checkbox.md` | B |
| Radio | - | `docs/design/components/Radio.md` | B |
| Text field | - | `docs/design/components/TextField.md` | B |
| Search field | D-CP-03, D-ED-11 | `docs/design/components/SearchField.md` | B |
| Select and dropdown | - | `docs/design/components/Select.md` | B |
| Picker | X-PK-01 | `docs/design/components/Picker.md` | B |
| Menu and context menu | - | `docs/design/components/Menu.md` | D (A: Sheet) |
| Popover | X-DLG-02 | `docs/design/components/Popover.md` | D |
| Tooltip | - | `docs/design/components/Tooltip.md` | D |
| Dialog and confirm | X-DLG-01 | `docs/design/components/Dialog.md` | B |
| Sheet (Android bottom sheet) | X-DLG-02 | `docs/design/components/Sheet.md` | A |
| Toast | X-NT-01 | `docs/design/components/Toast.md` | B |
| Notification card | D-NT-01, X-UP-01, X-AC-01 | `docs/design/components/NotificationCard.md` | B |
| Banner (disconnected host) | X-CN-01 | `docs/design/components/Banner.md` | B |
| Badge and chip | - | `docs/design/components/Badge.md` | B |
| Kbd shortcut badge | D-ST-04 | `docs/design/components/Kbd.md` | D |
| AgentIcon | every agent mark | `docs/design/components/AgentIcon.md` | B |
| Avatar and agent icon | X-AG-01, D-AC-06 | `docs/design/components/Avatar.md` | B |
| Progress (bar, ring, spinner) | - | `docs/design/components/Progress.md` | B |
| Skeleton | - | `docs/design/components/Skeleton.md` | B |
| Empty state | D-SH-17, D-WS-30, D-AG-25 | `docs/design/components/EmptyState.md` | B |
| Error state | D-SH-06 | `docs/design/components/ErrorState.md` | B |
| List row | D-WS-02, M-WS-01 | `docs/design/components/ListRow.md` | B |
| Table | - | `docs/design/components/Table.md` | D |
| Card | - | `docs/design/components/Card.md` | B |
| Divider and splitter | - | `docs/design/components/Splitter.md` | D |
| Scroll area | - | `docs/design/components/ScrollArea.md` | D |
| File tree | D-ED-11 | `docs/design/components/FileTree.md` | D |
| Diff view | D-DF-01, M-DF-01 | `docs/design/components/DiffView.md` | B |
| Terminal surface | D-TM-02, M-TM-01 | `docs/design/components/TerminalSurface.md` | B |
| Browser chrome | D-BP-01 | `docs/design/components/BrowserChrome.md` | D |
| Calendar views (day, week, month) | D-AG-23 | `docs/design/components/CalendarViews.md` | D |
| Board and kanban column | D-WS-30 | `docs/design/components/BoardColumn.md` | D |
| Command palette | D-CP-01 | `docs/design/components/CommandPalette.md` | D |
| Find bar | D-FD-01 | `docs/design/components/FindBar.md` | D |
| Settings row and section | D-ST-01 to D-ST-11, M-ST-01 | `docs/design/components/SettingsRow.md` | B |
| Extension card | D-EX-01, D-EX-03 | `docs/design/components/ExtensionCard.md` | D |
| Permission review | D-EX-02 | `docs/design/components/PermissionReview.md` | D |
