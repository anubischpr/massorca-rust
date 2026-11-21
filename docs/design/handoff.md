# MassOrca UI handoff for GPUI (desktop) and React Native (Android)

Status: draft, prepared 2026-11-12 from `docs/design/` (design-system.md, glossary.md, app-flow.md, pending-owner-review.md and the 66 specs in `components/`). Scope: what a developer needs before building the desktop UI in GPUI (Windows, macOS, Linux) and the Android UI in React Native. Each spec stays the full reference. This file summarises it, gives the order to build in, and lists what must be settled first.

## 1. How to use this document

Read section 2 once. Then find each component in section 3 (same order and grouping as the index in design-system.md) and open its spec for anatomy, copy and full state tables. Section 4 covers the 14 extension contribution points, section 5 the build order for Phase 5 (desktop, GPUI) and Phase 6 (Android, React Native), and section 6 the spikes, owner decisions and spec conflicts to settle before the parts they touch are built. Every component row answers the same eight questions in the same order. "Not specified" means the source is silent, so ask rather than guess.

Source of truth, highest first: owner decisions, listed one per row in [agreements.md](agreements.md) (93 agreements from the screen reviews, 2026-11-13; the later agreement wins), with the readings of the consistency pass in pending-owner-review.md "Consistency pass" (C1 to C22) > the glossary (owner-approved on 2026-11-12) > glossary > design-system.md > component specs > this file. Items in pending-owner-review.md are not owner decisions yet. They follow the owner's earlier decisions, are cited here as [P#] using the current numbering (37 items), and must be confirmed before the parts they change are built. Where a spec and a pending item disagree, the conflict is listed in 6.3. Do not resolve it in code.

Mock-up caveat (consistency pass, 2026-11-13): the artboards are static HTML, and each family (`sh-`, `ws-`, `bt-`, `an-`) carries its own copy of the shared pieces: Sidebar, Status bar, the segmented control CSS (`.seg`), the Model picker and the Composer, often also as background behind a dialog or a panel. A copy can lag behind the component spec. Build from the component specs in section 3 and the design system artifact, never from one artboard's copy; when an artboard and a component spec disagree, the spec and [agreements.md](agreements.md) win.

Agent icons (A94, 2026-11-20): use the app's own assets, mapped by `src/renderer/src/lib/agent-catalog.tsx` in the pv96 reference. Claude and Codex are SVG paths in `src/renderer/src/components/status-bar/icons.tsx` (`ClaudeIcon`, fixed `#D97757`; `OpenAIIcon`, `currentColor`); OpenCode is `OpenCodeIcon` in `src/renderer/src/lib/agent-icon-glyphs.tsx`; Antigravity, Gentle Shell, Gemini and Cursor are PNGs in `src/shared/agent-icons/` (`antigravity.png`, `gentle-shell.png`, `gemini.png`, `cursor.png`), which Android bundles through `mobile/src/components/mobile-agent-icon-assets.ts`. Gentle Shell is the `pi` agent with the `gentle-shell` runtime and always shows the rose, never the Pi mark. See components/AgentIcon.md.

Legend: D desktop, A Android, B both (platform column of the index). [P#] pending owner decision. [S#] spike in 6.1. K# keyboard collision in 2.3. X# spec conflict in 6.3. Token names are backticked and exact. A name tagged (absent) is used by specs but not defined in design-system.md (6.4). Sizes are px on desktop and dp on Android unless written otherwise.

## 2. Global behaviour

### 2.1 Tokens and theming

- **Names are the contract.** Token names never change across GPUI, React Native, theme files, extension view schemas and specs (design-system.md, header). In GPUI use the nested theme struct (`theme.surface.frame`). In React Native use the nested theme object. Any case change must come from a generator, never by hand.
- **Source file.** The specs and design-system.md refer to `docs/design/tokens.json`, which is now the repository copy of the token contract (2026-11-12). The design-system.md tables and the specs must match it. Recommendation: generate the GPUI struct and the React Native object from one `tokens.json`, so no value is copied by hand.
- **Aliases and derived values.** `agent.done` = `status.success.fg`; `agent.unreachable.border` = `border.default`; `badge.neutral.fg` = `text.primary`; `accent.on` = `text.on-accent`. Values written as `{name}` in the tables are aliases. A theme may omit the derived fills (`fill.*`, `accent.subtle.*`); the app derives them with the blend in 2.6 and validates them.
- **Solid state fills.** Hover, selected and pressed fills are precomputed solid colours per surface (for example `fill.hover.canvas`). Nothing blends at run time. The only translucent state fill is `fill.ripple` (Android press ripple, 12 percent). Translucent colour is limited to `scrim`, the shadow colours and `fill.ripple`.
- **Themes at runtime.** A theme is a JSON file (sketch in design-system 6.3) with `appearance` set to dark or light. The built-ins are `massorca.dark` and `massorca.light`. Desktop has the theme toggle on the rail. Android follows the system setting with a manual override, and has no dynamic colour [P35]. A theme switch is instant. The validator (6.2, rules V1 to V12) runs at theme load, when an extension contributes a theme, and on every colour change in the theme editor. A theme under a floor is rejected with the failing pair named. It is never adjusted silently.
- **Accent.** The user picks a seed (six presets or a custom colour). The app derives `accent.default`, `accent.hover`, `accent.pressed`, `accent.fg` and `border.focus`, then re-validates (design-system 6.4). The accent never changes status, agent, diff or terminal colours. Default is calm blue, `#7461d8` dark and `#6447c9` light [P9].
- **What a theme may not change** (design-system 6.1): the hue ranges of status colours (success 90 to 180, warning 25 to 65, danger 340 to 20, info 180 to 260 degrees), the focus ring, contrast floors, motion, region structure, the Frame and Canvas, security chrome (Permission review, Secret modal, destructive confirms), images, gradients, blur, translucency (scrim alpha 0.30 to 0.80 only) and icon sets.
- **Fonts.** Geist for the interface; JetBrains Mono for terminal, code, paths, IDs and kbd [P9]. Both are bundled as static files (Geist 400, 500, 600; JetBrains Mono regular, italic, bold, bold italic). The app never fetches a font. Missing glyphs fall back to the OS UI font.
- **Type and figures.** The type scale is in design-system 3.2 (desktop px, Android sp). Minimum text is 11 px and 12 sp. Android text scales to 200 percent; rail, badges and chips cap scaling at 130 percent. Numbers that change in place or line up use tabular figures (3.3; confirm in S2).
- **Behaviour constants** (design-system 4.8) are not tokens and cannot be themed [P36]. See 2.7.

### 2.2 Focus model

| Rule | Behaviour | Source |
|---|---|---|
| Ring | 2 px `border.focus`, offset 2 px, radius = element radius + 2. Drawn inset (offset -2) where it would be clipped (scroll area, Frame edge, inside a menu) and on accent fills, where the inset ring uses `text.on-accent`. | design-system 8.2 |
| Visibility | Shown on keyboard focus only (`focus-visible`). Text fields, search fields and the composer also switch their border to `border.focus` when focused. | 8.2 |
| Strength | A theme or extension can never remove or recolour the ring below 3:1. One ring style for all components. | 8.2, 6.1 |
| Region cycle | F6 moves forward, Shift+F6 back. Order: Title bar, Activity rail, Sidebar, Conversation, Session summary, Work panel, Project sidebar, Status bar, then back to the Title bar. Hidden or collapsed regions are skipped; overlay regions join while open. Laptops may need Fn on macOS. Also in the command palette, and rebindable. | 8.3 |
| Inside a region | Tab and Shift+Tab move between controls. Composite controls (rail, tree, tabs, list, menu, segmented control, toolbar) are one tab stop with roving arrow keys. | 8.3 |
| Overlays | Dialogs and sheets trap focus. Escape closes the topmost overlay, one at a time. PermissionReview states an exception (no Escape while checking; X37). | 8.3 |
| Restore | Closing a popover, menu, dialog or overlay returns focus to the control that opened it. Removing the focused item moves focus to its neighbour. | 8.2 |
| Initial focus | Destructive dialogs start on Cancel; other dialogs on the primary action. Popovers and pickers start in their first control or search field. Info popovers never take focus. | Button, Dialog, Popover, Picker |
| Passive content | Toasts, notification cards, banners and tooltips never take focus on their own. | Toast, NotificationCard, Banner, Tooltip |
| Extensions | An extension never takes focus on its own. Focus enters extension content only after a user action or F6. | design-system 7.1 |

### 2.3 Keyboard map

Shortcuts are stored as `Mod+K` and shown per OS by the Kbd badge: `Ctrl+K` on Windows and Linux, `⌘K` on macOS (design-system 8.1). Mod is Ctrl on Windows and Linux and Cmd on macOS. Alt is Alt, or Option on macOS. Ctrl is never Mod on macOS. Terminal copy and paste are the exception (Ctrl+Shift+C/V on Windows and Linux, Cmd+C/V on macOS). Every shortcut is rebindable in Settings, Keybindings (D-ST-04). The table lists every shortcut found in design-system.md, app-flow.md and the specs. "Owner" is the component or page that handles the key.

| Action | Windows / Linux | macOS | Owner | Notes |
|---|---|---|---|---|
| Command palette | Ctrl+K | Cmd+K | Command palette (D-CP-01) | Shell keeps it in a terminal [P12]; K1. |
| Quick open | Ctrl+P | Cmd+P | Command palette (D-CP-02) | Shell keeps it in a terminal [P12]; K1. |
| Search in files | Ctrl+Shift+F | Cmd+Shift+F | Command palette (D-CP-03); file tree | Terminal find is not in v1; K3. |
| Find in pane | Ctrl+F | Cmd+F | Find bar (D-FD-01) | Shell keeps it in a terminal [P12]; K1. Settings search uses Mod+F too; K4. |
| Dictation overlay | Ctrl+E | Cmd+E | D-OT-05 | Shell keeps it in a terminal (end of line) [P12]; K1. |
| Settings | Ctrl+, | Cmd+, | Settings (D-SH-02) | None. |
| Next region | F6 | F6 | Frame and all regions (design-system 8.3) | Reserved in a terminal, so the terminal never takes it. |
| Previous region | Shift+F6 | Shift+F6 | Frame (8.3) | As above. |
| Recent tab switcher | Ctrl+Tab | Ctrl+Tab | D-TM-11 (app-flow) | Reserved in a terminal. Written with literal Ctrl on macOS in app-flow. |
| Zoom in | Ctrl+= (Ctrl++ on layouts that need Shift) | Cmd+= | D-SH-08 (app-flow) | Browser page zoom uses Mod+= too; K5. |
| Zoom out | Ctrl+- | Cmd+- | D-SH-08 | As above; K5. |
| Zoom reset | Ctrl+0 | Cmd+0 | D-SH-08 | As above; K5. |
| Escape | Esc | Esc | Topmost overlay (8.3) | PermissionReview refuses it while checking; X37. |
| Close focused Work panel tab | Ctrl+W | Cmd+W | Work panel tabs (D-SH-17) | Never closes the window or the session [P12]; on macOS Cmd+W is the window-close key; K2. |
| Add terminal tab | Ctrl+T | Cmd+T | Work panel tabs (proposed) | Shell keeps Ctrl+T in a terminal [P12]; a browser pane may use it for a new tab (app-flow); K6. |
| Add browser tab | Ctrl+Shift+B | Cmd+Shift+B | Work panel tabs (proposed) | Reserved in a terminal. Not checked against browser shortcuts (app-flow). |
| Open recorded file in its own tab | Ctrl+Alt+click | Cmd+Option+click | Work panel (D-ED-01) | Some layouts use Ctrl+Alt as AltGr (app-flow). Plain Mod+click keeps its dialog. |
| Copy (terminal) | Ctrl+Shift+C | Cmd+C | Terminal surface (D-TM-02) | Ctrl+C stays the shell interrupt. |
| Paste (terminal) | Ctrl+Shift+V | Cmd+V | Terminal surface | None. |
| Interrupt (terminal) | Ctrl+C | Ctrl+C | Terminal surface | Kept for the shell. |
| Scroll scrollback | Shift+Page Up / Shift+Page Down; Shift+Home / Shift+End | same | Terminal surface | None. |
| Block selection | Alt+drag | Option+drag | Terminal surface | None. |
| Open link | Mod+click | Cmd+click | Terminal surface | None. |
| Previous / next user message | Alt+Up / Alt+Down | Option+Up / Option+Down | Conversation list; message minimap | None. |
| Copy message or plan step | Mod+C | Cmd+C | Conversation list; plan view | None. |
| Send | Enter (Mod+Enter also sends) | Enter (Cmd+Enter also sends) | Composer | Shift+Enter inserts a line. Enter does not send during IME composition. |
| Restore last sent message | Up (empty composer) | Up | Composer | Down moves forward. |
| Mention menu | @ | @ | Composer | A `/` at line start opens the command menu. |
| Open Model picker | Mod+Shift+M | Cmd+Shift+M | Composer; model picker | Rebindable. Not in app-flow's keyboard table. |
| Choose model 1 to 4 | Ctrl+1 to Ctrl+4 | Cmd+1 to Cmd+4 | Model picker (X-AG-02) | Only while the picker is open. Row position: the first four rows of the view shown; K7. |
| Toggle favourite | Alt+Enter | Option+Enter | Model picker | None. |
| Stop run | Escape twice (empty composer) | same | Composer | Proposal. No interrupt key is set. |
| Focus address bar | Ctrl+L | Cmd+L | Browser chrome | None. |
| Reload / reload without cache | Ctrl+R / Ctrl+Shift+R | Cmd+R / Cmd+Shift+R | Browser chrome | None. |
| Back / forward | Alt+Left / Alt+Right | Cmd+[ / Cmd+] | Browser chrome | None. |
| Next change (diff) | F7 | F7 | Diff view | Decided [P27]. |
| Previous change (diff) | Shift+F7 | Shift+F7 | Diff view | Decided [P27]. |
| Note on changed line | Ctrl+Alt+M | Cmd+Option+M | Diff view | None. |
| Focus notifications | Ctrl+Alt+N | Cmd+Option+N | Toast; notification cards; palette | In the specs only, not in app-flow's table. |
| Dismiss toast | Escape | same | Toast | Escape only, toasts and cards alike; K8. |
| Dismiss notification card | Escape | same | Notification card | Escape dismisses and returns focus; K8. |
| Context menu | Shift+F10 or the Menu key | Shift+F10 | Menu; rows; file tree | Hover-only actions must also be reachable here. |
| Expand / collapse tree row | Right / Left | same | Sidebar tree; file tree; delegation tree; list rows | Left goes to the parent when a row is collapsed. |
| Type-ahead | Letters | Letters | Sidebar tree; file tree; Select; Menu | Moves to the next match. |
| Resize splitter | Arrow keys (8 px); Shift+Arrow (24 px) | same | Splitter | Home and End are proposed. |
| Scroll | Up / Down (one row); Page Up / Page Down (one viewport); Home / End | same | Scroll area | None. |
| Roving move | Arrow keys | Arrow keys | Toolbars, tabs, lists, segmented control, menus | One tab stop per composite control. |
| Settings search | Mod+F | Cmd+F | Settings row | Same key as Find in pane; K4. |
| Board card pick up and drop | Space; Left / Right change column; Up / Down within the column; Escape cancels | same | Board column | Moves are announced. |
| Calendar move and open | Arrow keys; Enter | same | Calendar views | None. |
| Picker open and folder move | Enter, Space or Down open; Right enters a folder; Left or Backspace (empty search) goes up | same | Picker | None. |
| Select letter jump | Letter | Letter | Select | None. |
| Remove queued message | Delete | Delete | Queued message | Removes at once; X16. |
| Secret modal submit or cancel | Enter (when valid) / Escape | same | Secret modal | Escape discards the typed value. |
| Mode switch | None | None | Mode switch | Mod+K must not be used there. |

Collisions and conflicts to settle before build:

- **K1.** Resolved (X29): while a terminal has focus on Windows and Linux, the shell keeps Ctrl+F, Ctrl+E, Ctrl+K, Ctrl+P, Ctrl+T and Ctrl+W; app shortcuts work again when focus leaves it [P12]. The palette opens in a terminal with Ctrl+Shift+P (design-system 8.3).
- **K2.** Resolved (X26): Mod+W closes the active Work panel tab and never the window [P12]. Window close uses the OS key: Alt+F4 on Windows and Linux, Cmd+Q on macOS.
- **K3.** Resolved (X28): Ctrl+Shift+F is search in files, app-level. The terminal has no find in v1.
- **K4.** Mod+F opens the find bar in every pane and also focuses the search field on the Settings page (SettingsRow).
- **K5.** Mod+=, Mod+- and Mod+0 change app zoom (app-flow, D-SH-08) and browser page zoom (BrowserChrome). App-flow says app keys reach the app first from a browser pane, which does not settle page zoom.
- **K6.** Ctrl+T: the Work panel adds a terminal tab, the browser may use it for a new tab (app-flow, not checked), and a terminal keeps it for the shell [P12].
- **K7.** Resolved (X9): Ctrl+1 to Ctrl+4 select the first four rows of the view shown; the favourites reading is dropped.
- **K8.** Resolved (X18): Escape dismisses a focused toast or notification card; Delete and Backspace do not, on the one shared stack.

### 2.4 Pointer and touch

- **Desktop.** Hover fills step in `duration.fast`. Tooltips appear after `duration.tooltip-delay` (400 ms) at rest, at once on keyboard focus, and not between tooltips. Right-click opens the named context menus: workspace row, session row, file row, tab, message, selection and diff line. Double-click resets a splitter and renames list rows. Drag moves Work panel tabs, Board cards and splitters; drag follows the pointer without easing (design-system 4.7).
- **Desktop targets.** At least 24 by 24 px (`size.hit.pointer`). Icon buttons are 28 by 28 by default. Splitters have a 6 px hit area (design-system 8.5). Hover-only actions (row actions, favourite star) must also be reachable by keyboard or through the context menu.
- **Android.** No hover and no tooltips. A long press opens the row or message sheet. Every long-press or swipe action also exists as an `accessibilityActions` entry (8.6). Touch targets are 44 dp visual and 48 dp hit, with at least 8 dp between targets (5.4). Bottom sheets replace popovers and menus: 90 percent maximum height, a 32 by 4 dp drag handle, dismissed by swiping down, tapping outside or the system back gesture. Toasts and notification cards can be swiped away, and each has a close button.

### 2.5 Window, regions and collapse order

- **Window minimum** 800 by 520 px (desktop). Android window classes: compact under 600 dp (one pane at a time), medium 600 to 839 dp (master 360 dp plus detail), expanded 840 dp and up (as medium; the detail content column is capped at 720 dp) (5.4).
- **Region widths** (design-system 5.1; logical px):

| Region | Surface | Default | Min | Max | Collapsible |
|---|---|---|---|---|---|
| Activity rail | Frame | 48 | 48 | 48 | Never |
| Sidebar | Frame | 256 | 200 | 400 | Title bar toggle; drag below 200 collapses |
| Conversation | Canvas | Rest of the width | 360 | None | Last to give way; content column max 720 |
| Session summary | Canvas | 288 | 240 | 400 | Title bar toggle |
| Work panel | Canvas | 480 | 320 | Rest of the width | Title bar toggle |
| Project sidebar | Canvas | 288 | 240 | 480 | Title bar toggle |
| Status bar | Canvas, bottom | Height 24 | n/a | n/a | Visibility menu |
| Title bar | Frame | Height 36 (Windows, Linux) or 40 (macOS) | n/a | n/a | No |

- **Splitters** are 1 px lines with a 6 px hit area. On hover the line becomes `border.strong`; while dragging it becomes `accent.default` at 2 px. Double-click resets to the default width. Dragging below a minimum collapses the region at the minimum minus `SPLITTER_SNAP_PX` (40 px). Dragging back out reopens it at the minimum. A region never makes the Conversation narrower than 360 px. Widths are remembered per region, per window.
- **Collapse order** when the window narrows: 1 Session summary, 2 Project sidebar, 3 Sidebar, 4 Work panel. Thresholds are 1528, 1288, 1048 and 848 px (design-system 5.1; X1): the sums of the minimums of the regions still open, with the Conversation at 480 px. Each collapsed region opens as an overlay (`z.region-overlay`, `shadow.e2`); only one overlay is open at a time; it closes on Escape, on a click outside, or when focus leaves. Pending decision [P8] says six regions need 1528 px. This conflict is X1.
- **Full pages** (Tasks, Calendar, Marketplace, Settings and the Bot Teams pages) cover the whole Canvas. The Title bar shows Back and the page title. Maximum content widths: Settings 880, Tasks and Marketplace 1120 (`layout.page.*`, absent as tokens). Calendar and Board are fluid. The Sidebar is hidden on full pages in design-system 5.1; P16 keeps it on Bot Teams pages (X2).
- **Window chrome by OS** (5.2): macOS has traffic lights inside the Frame at the left, a 40 px title bar, and an 80 px left gutter when the Sidebar is hidden. The rail reserves a 40 px zone at its top, and the mode switch sits at y 48. Windows and Linux have a 36 px title bar with window controls at the right (46 by 36 px each), drawn by the app; the mode switch starts at y 8.
- **Activity rail** (5.3): fixed 48 px. Top group: mode switch, Tasks, Calendar, Marketplace, then extension icons (three visible at minimum height). Bottom group: theme toggle, usage rings (one per provider), Settings. Buttons 36 px, icons 20 px.

### 2.6 Motion and reduced motion

Durations and curves (design-system 4.7):

| Token | Value | Used for |
|---|---|---|
| `duration.instant` | 0 ms | Reduced-motion replacement for everything below |
| `duration.fast` | 100 ms | Hover and pressed fills, tooltip fade, checkbox tick |
| `duration.base` | 150 ms | Popover, menu and toast fades; focus ring appearing |
| `duration.moderate` | 200 ms | Region collapse and expand, dialog enter, mode-switch morph |
| `duration.slow` | 300 ms | Bottom sheet, page-level transitions |
| `duration.card-insert` | 250 ms | Session summary card insert |
| `duration.tooltip-delay` | 400 ms | Pointer rest before a tooltip shows |
| `duration.spinner` | 800 ms | One spinner revolution |
| `ease.standard` | cubic-bezier(0.2, 0, 0, 1) | Movement between two on-screen states |
| `ease.enter` | cubic-bezier(0.16, 1, 0.3, 1) | Things appearing (popovers, dialogs, toasts, cards) |
| `ease.exit` | cubic-bezier(0.3, 0, 0.8, 0.15) | Things leaving; exits last 70 percent of the enter time |
| `ease.linear` | linear | Spinners and indeterminate progress |

Both toolkits take an easing function, not a CSS string. The shared motion module implements the solver once and exposes these four names.

- **Animates:** hover, pressed and selected fills (colour step, `duration.fast`, no change on keyboard focus moves); popover, menu and model picker (opacity 0 to 1 and a 4 px move, `duration.base`, `ease.enter`); tooltip (opacity only); dialog (opacity and scale 0.98 to 1, `duration.moderate`, scrim fades); Android sheet (slides from the bottom, `duration.slow`); toast (slides in 8 px and fades); Sidebar, Session summary, Work panel and Project sidebar (width, `duration.moderate`, inner content keeps its final width and is clipped so text does not reflow); spinners and indeterminate bars.
- **Does not animate:** theme switch, status colour changes, text, list virtualisation, drag and resize (they follow the pointer), page changes between rail destinations.
- **Mode-switch morph** (ModeSwitch): two icons stacked in a 20 px box; over `duration.moderate`, the outgoing icon scales 1 to `MORPH_SCALE`, rotates to -`MORPH_ANGLE_DEG` and fades across the first 60 percent; the incoming icon mirrors it over the last 60 percent. The Sidebar content cross-fades in 100 ms. This is a cross-fade with scale and rotation; a true path morph is not built into GPUI.
- **Session summary insert:** the card grows from height 0 to its measured height, fades in and rises 8 px over `duration.card-insert` with `ease.enter`; cards below slide with `ease.standard`. If the new card lands above the visible area, it is inserted without animation and the scroll position is anchored.
- **Reduced motion.** Read the OS setting (Windows "Animation effects", macOS "Reduce motion", GNOME `enable-animations`, Android "Remove animations") and offer an override in Appearance. When on, every animation above uses `duration.instant`, except functional progress: a spinner keeps turning. Cursor blink follows the OS setting. Skeleton shimmer, where used, becomes a static `fill.skeleton`; whether shimmer exists in v1 is open (X49).

### 2.7 Behaviour constants

Constants tune behaviour and are not themeable. They are defined once in the shared motion and input modules, and specs refer to them by name. Changing one is a behaviour change that needs the owner's decision [P36].

| Constant | Value | Used by | Meaning |
|---|---|---|---|
| `MORPH_SCALE` | 0.6 | Mode switch | Scale of the outgoing and incoming icon at the ends of the morph |
| `MORPH_ANGLE_DEG` | 30 | Mode switch | Rotation of the outgoing (negative) and incoming (positive) icon |
| `MORPH_OVERLAP_PERCENT` | 20 | Mode switch | Share of `duration.moderate` in which both icons are visible |
| `SCROLLBAR_HIDE_DELAY_MS` | 1000 | Scroll area | Time after scrolling stops before an overlay thumb hides |
| `SEARCH_DEBOUNCE_MS` | 200 | Search field | Pause in typing before a filter or search applies |
| `SPLITTER_SNAP_PX` | 40 | Splitter | Distance below a region's minimum at which dragging collapses it |
| `SUBMENU_HOVER_DELAY_MS` | 150 | Menu | Pointer rest time before a submenu opens or closes on hover |
| `MESSAGE_CLAMP_LINES` | 12 | Conversation list | Lines of a long user message shown before "Show more" |

The skeleton delay (100 ms), the spinner delay (200 ms) and the Undo timeout (8 s) are now constants in design-system 4.8 (X17, X21, X22). Other literal timings in specs still need an owner decision before code relies on them: a 300 ms click guard on approval buttons (see 6.4).

### 2.8 Loading, empty, error and unreachable patterns

| State | Pattern | Source |
|---|---|---|
| Loading, skeleton | Blocks in `fill.skeleton`, static. Line height `size.skeleton.line` (12 px); line widths 100, 80 and 60 percent. Shown after a wait starts; the start delay differs between specs (X22). | Skeleton, DiffView, ConversationMessageList; design-system 4.7 |
| Loading, spinner | `duration.spinner`, `ease.linear`. Design-system 4.7 says it appears only after 200 ms on remote hosts, and nothing shows during the first 100 ms of any action. Several specs apply the 200 ms rule to every wait (X21). | design-system 4.7; Button, Progress |
| Loading, progress | Determinate bar width changes in `duration.base`. Indeterminate bar travels a 30 percent segment in `duration.spinner`. | Progress |
| Empty | EmptyState panel: one title, one explanation (`layout.empty-state.text.max-width`, 60ch) and one action. A filtered empty state names the filter in its title and offers "Clear search". The Work panel offers one action per tab type. Session summary cards render only when they have information; an empty card is never drawn. | EmptyState; glossary; design-system 2.5 |
| Error | ErrorState: danger tint and border, `octagon-alert` glyph in `status.danger.fg`, a title and a reason. Retry comes first (secondary Button); "Show details" expands a selectable mono block. Retrying shows a spinner, "Retrying", and disables Retry. Still failing keeps the same panel with a new reason. Recovered content replaces the panel. | ErrorState |
| Error on a control | Never on the button. The button returns to rest and the failure goes to the region (ErrorState, Banner or Toast). | Button |
| Crashed surface | A recoverable error screen with retry replaces the crashed surface; the rest of the shell keeps running. | app-flow, cross-cutting states |
| Unreachable host | Show the last known state with its age, dimmed. Silence is never "stopped" (principle 6). The glyph becomes the dashed ring with a slash in `agent.unreachable`; banners, notification cards and Delegation tree rows take `agent.unreachable.bg`. A Banner (X-CN-01) sits at the top of each affected region. Disabled actions show their reason as visible text, linked with `aria-describedby`. | design-system 2.8; Banner; app-flow |
| Queued messages | Queued messages send only after the host confirms the session (app-flow A10). A message to an unreachable host stays in the composer, not queued [P25]; X15. | app-flow; pending decision 25 |
| Auth failure | Ask once, in place, with one action. Secrets are never shown back or logged. | app-flow |
| Protocol mismatch | Block only the use that needs the newer side. Say which side must update. | app-flow |
| Long-running work | Work runs in the core, so closing the window does not stop it. Progress shows where the job started and in the agents overview. | app-flow |
| Notifications | The OS notification only announces attention. Acting happens on the card. Toasts carry short results only. | app-flow |
| Secret request | The value never enters the transcript, model context or logs. The chat shows only the name and a state. With the host unreachable, nothing is saved. | app-flow |

### 2.9 Accessibility

- **Roles and names** (design-system 8.4). Each row is the target; desktop support depends on S1.

| Region or control | Role | Accessible name |
|---|---|---|
| Activity rail | navigation | "Activity rail" |
| Sidebar | navigation | "Workspaces" or "Teams", following the mode (FrameCanvas.md uses "Sidebar"; X52) |
| Title bar | banner or toolbar | "Title bar" |
| Conversation | main | "Conversation" plus the session title |
| Session summary | complementary | "Session summary" |
| Work panel | region | "Work panel" |
| Project sidebar | complementary | "Project sidebar" |
| Status bar | status | "Status bar" |
| Icon button | button | Its tooltip text, required (a build error without a label) |
| Agent status glyph | image | The state word: "Running", "Waiting for you", "Idle", "Error", "Unreachable, last known 5 min ago" (Done is missing; X4) |
| Usage ring | meter or progressbar | For example "Claude usage 62 percent, resets in 2 days" |
| Diff line | text | "Added line" or "Removed line" before the content |
| Summary card inserted | live region, polite | "Plan card added" |
| Toast | status (polite); errors alert (assertive) | The message |

- Every image and icon has a name or is marked decorative. Tooltips double as accessible descriptions. Live regions announce a status change once, not repeatedly.
- **Status is never colour alone** (principle 5). Each status pairs a glyph with a word (design-system 2.7): success `circle-check` with "Done", "Saved" or "Connected"; warning `triangle-alert` with "Check", "Behind" or "Expiring"; danger `octagon-alert` with "Failed" or "Error"; info `info` with "Note".
- **Contrast floors**: 4.5:1 for text, 3:1 for icons, control borders, focus rings and graphical marks (design-system 2.14). Exempt by design: disabled text, decorative borders, and the two ANSI background colours.
- **Desktop screen readers**: GPUI's accessibility support is unverified (S1). Without it, v1 is keyboard-complete and high-contrast but not screen-reader-complete on desktop (design-system 10, question 1).
- **Android TalkBack** (8.6): every control has a label and a role; state goes in the label or `accessibilityState`; bottom sheets announce their title and trap focus; text scales to 200 percent; respect "Remove animations", "Bold text" and high-contrast text settings.
- **High contrast**: no high-contrast or colour-blind theme in v1 [P35]. Design-system 10, question 4 asks the same question and still reads as open (X31).

### 2.10 Desktop and Android differences

Condensed from design-system section 9.

| Aspect | Desktop (GPUI) | Android (React Native) |
|---|---|---|
| Toolkit | GPUI, GPU drawing, no browser | React Native with native components and `react-native-svg` |
| Navigation | Regions, Activity rail, full-canvas pages | Stack of screens; master-detail from 600 dp; no rail |
| Frame and Canvas | L-shaped Frame; inset Canvas with rounded top-left corner | Top app bar on `surface.frame`; content on `surface.canvas`; the rounded corner returns in master-detail |
| Theme source | Light and dark, toggle on the rail | System by default, with a manual choice; no dynamic colour [P35] |
| Control height | 24, 28 or 32 px by density | 44 dp visual, 48 dp hit target |
| Type | px scale; body 13 | sp scale; body 14; scales to 200 percent |
| Pointer | Hover states, tooltips, right-click menus, resizable splitters | No hover; long press and sheets; no tooltips |
| Popover and menu | Anchored popover or menu | Bottom sheet |
| Dialogs | Centred dialog, `radius.xl` | Dialog for confirms; sheets for everything else |
| Elevation | Borders, solid surfaces, box shadows | Same surfaces and borders; elevation 1, 6 and 12 dp |
| Keyboard | Full shortcut set; F6 region cycling; Mod mapping | Hardware keyboard, basic navigation; no custom shortcuts in v1 |
| Status | Status bar; usage rings on the rail | Host cards with usage rings; system status bar untouched |
| Density | Compact, default, comfortable | One density (touch) |
| Extensions | 14 contribution points | None rendered in v1 |
| Window | Minimum 800 by 520; per-OS title bar | Edge to edge with safe-area insets |
| Icons | Lucide at 12, 16, 20 and 24 | Lucide at 20 and 24 (16 in dense chips); X14 |
| Scrollbars | Thin overlay thumb in `border.strong`, `radius.xs` | System indicators |

Android in v1 has no Work panel, no rail, no status bar and no extension contributions (5.4, 7.1, app-flow navigation map).


## 3. Components, in index order

Note (2026-11-12): where an "Open:" note below cites an X# from 6.3, the decision in 6.5 governs and the spec file is the current text. The note itself was not rewritten.

Each component has one row with the eight answers, in the order of the index in design-system.md. The link opens the full spec. "Open" lists what the spec leaves open or what conflicts, with the number in 6.3 where one exists.

### 3.1 Shell and navigation

#### 3.1.1 Activity rail: [ActivityRail.md](components/ActivityRail.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; overflow open (`surface.overlay`, `shadow.e2`); loading and empty (no divider); error reported through card D-NT-01 | One tab stop; Up and Down move; Home and End jump; Enter or Space activate; Tab leaves; F6 cycles; Settings `Mod+,` | Order per 8.3; ring 2 px, offset 2 px, `radius.lg` | Fixed 48 px at every width; height changes only how many extension icons show (three at minimum height) | None; item fills in `duration.fast` | 48 px column on `surface.frame`; 36 px solid-fill items; tooltips and overflow in overlay layer (`z.popover`, absent); 20 px icons, 1.75 px stroke | No rail. Mode switch, theme and Settings move to the Home top bar; Calendar to overflow; usage on host cards [P21] | Desktop only. Desktop names are tooltips; Android uses TalkBack labels |

Open: one icon per extension, three with user consent (owner to confirm); no Automations entry (app-flow G26); the macOS 40 px zone needs a check on a Mac; the Android placement of Tasks is not stated.

#### 3.1.2 Rail item: [RailItem.md](components/RailItem.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover (`fill.hover.frame`); pressed; focus-visible; selected or active (`fill.selected.frame`, 2 px `accent.fg` bar, icon `text.primary`) | Enter or Space activate; roving Up and Down; `Mod+,` for Settings; keyboard focus shows the tooltip at once | As Activity rail; ring 2 px, offset 2 px, `radius.lg` | Fixed 36 px; density does not change rail items; badge 16 px high, dot 6 px [P31] | Fills in `duration.fast`; active bar appears without animation; reduced motion instant | 36 px rounded rectangle, solid fill per state; badge in overlay layer; 2 px active bar on the rail edge; 20 px icons | Not on Android. Moved entries become 48 dp targets (`size.touch.large`) with role button and the label as TalkBack name | Desktop: tooltip, active bar, roving focus. Android: top bar and overflow actions instead |

Open: active icon colour is `text.primary` here but `accent.fg` for active in design-system 4.6 (X6); the ripple rule (`fill.pressed.frame` or `fill.pressed.overlay`) conflicts with [P33] and design-system 2.6 (X7); badges for Tasks and Calendar are undefined; `border.width.thick` (absent).

#### 3.1.3 Usage ring: [UsageRing.md](components/UsageRing.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default (arc equals usage); hover opens popover; pressed; focus-visible; selected while open; unavailable; loading; empty; error keeps the last value; disconnected, dimmed | Focusable like a rail item; Enter or Space open the popover; Esc closes it and returns focus to the ring | Ring 2 px, offset 2 px, around the 36 px rail button | Fixed 32 px on both platforms (`size.usage.ring`); popover 280 px (`layout.popover.width.sm`) | Value sweep `duration.base`, `ease.standard`; loading turns at `duration.spinner`; the spec says instant under reduced motion, which conflicts with 4.7's functional-progress rule (X58) | Stroked track and arc from 12 o'clock, precomputed per percent; no conic gradient or blur; 16 px mark; 6 px warning or limit dots in overlay layer | Host card meter: 32 dp ring, 2 dp stroke, `react-native-svg` circle with dash array; read-only, inside the 48 dp card | Desktop: interactive rail item with popover and usage view (D-AC-06). Android: read-only meter on the host card [P21] |

Open: arc colour at 80 and 95 percent (`usage.ring.warn`, `usage.ring.limit`) to confirm against D-AC-06; one or two arcs per ring (pv96 only) for the owner; full-colour marks [P32].

#### 3.1.4 Mode switch: [ModeSwitch.md](components/ModeSwitch.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default shows the current mode, always selected; hover keeps the selected fill; pressed; focus-visible; loading (Sidebar skeleton); error (no morph); never disabled | Enter or Space switch; no shortcut; `Mod+K` must not be used | First item of the Activity rail; ring 2 px, offset 2 px | Fixed 36 px button; no responsive change | 200 ms morph (`duration.moderate`, `ease.standard`); scale to `MORPH_SCALE` 0.6, rotate `MORPH_ANGLE_DEG` 30; Sidebar cross-fades in 100 ms; reduced motion instant swap | Two stacked SVG icons (`layout-grid`, `bot`) in a 20 px box; opacity and scale per frame; drop rotation if the pinned GPUI lacks SVG transforms; no path morph | No rail. 48 dp icon button in the Home top bar; cross-fade only; role button with the current mode in the label [P21] | Desktop: rail icon, morph, tooltip. Android: top-bar icon, cross-fade, no tooltip |

Open: a single click switches the mode in this spec (line 44, "owner, 2026-11-12"), but [P6] says the first press returns home from other pages (X5); the accent bar as a second cue needs confirmation.

#### 3.1.5 Sidebar tree: [SidebarTree.md](components/SidebarTree.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; focus within; loading (3 to 5 `fill.skeleton` bars, no spinner); empty ("No projects yet", "No teams yet"); no match; error (retry, other rows keep working); disconnected host dimmed with age | One tab stop; Up and Down move; Right and Left expand or collapse; Enter opens; Home and End; type-ahead; Shift+F10 opens the row menu; F6 cycles | After the Activity rail, before Conversation; row ring per SidebarRow | Width 256 default, 200 min, 400 max, remembered per window; 6 px splitter; drag below 200 collapses at 160 (`SPLITTER_SNAP_PX`); double-click resets | Width `duration.moderate`, `ease.standard`; content keeps its final width and is clipped; reduced motion instant | Virtualised list inside a scroll area; precomputed row fills; row menus in overlay layer; "+ Create" flat accent button, no shadow | Not on Android. Workspaces are list M-WS-01 and teams have their own screens; do not port the tree | Desktop only. Android uses list screens, with no tree or splitter |

Open: "+ Create" uses `accent.default` (the reference is neutral; confirm); the open point says Prototypes is an extension, but [P1] makes it core (X50); footer icons have no action; the bell action follows [P19]; `layout.sidebar.*` tokens are absent (5.1 gives values only).

#### 3.1.6 Sidebar row: [SidebarRow.md](components/SidebarRow.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover (`fill.hover.frame`, actions shown); pressed; focus-visible; selected (fill, 2 px bar, bold label); disabled (unused); loading (skeleton bar); empty hints; error ("Check", "Failed"); disconnected, dimmed | Roving item in the tree; Enter opens; Right and Left expand or collapse; Shift+F10 or the Menu key opens the row menu | Ring 2 px, inset 2 px, because the Sidebar edge would clip an outer ring | Height 28 default, 24 compact, 32 comfortable; labels truncate, never wrap | Fills `duration.fast`; expand and collapse `duration.moderate`; reduced motion instant | Flex row in a fixed-height box; ellipsis; AgentStatus glyphs; 2 px selected bar on the inner edge; precomputed fills | Not on Android. ListRow (M-WS-01) with a 48 dp target; state in the TalkBack label; long-press actions as accessibilityActions | Desktop: 28 px rows, hover actions, tree keys. Android: list rows with long-press sheets (M-WS-02) |

Open: the selected bar colour is not named in the foundations (see X43 for the ListRow and Table difference); a stacked owner line would make rows two-line (44 px on desktop); `border.width.thick` (absent).

#### 3.1.7 Title bar: [TitleBar.md](components/TitleBar.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover; pressed; focus-visible; back and forward disabled without history; loading (title skeleton); empty ("No session open"); host unreachable suffix; full-page variant with Back | Toolbar with roving Left and Right; Enter or Space activate; F6 and Shift+F6 cycle regions; close and minimise keep their OS keys (K2) | First region after the Title bar in F6 order; ring 2 px, offset 2 px | 36 px on Windows and Linux, 40 px on macOS; the title truncates first; region toggles visible to 800 px; macOS 80 px left gutter when the Sidebar is hidden | None; reduced motion has no effect | One `surface.frame` rectangle, no border or shadow; drag region on the empty part; Windows and Linux controls drawn by the app (46 by 36 px) | Not on Android. The Home top bar (`surface.frame`, 56 dp) takes title, Back and overflow [P21] | Desktop: drag region, window controls, region toggles. Android: 56 dp app bar, 48 dp actions, no window controls |

Open: [P2] settles the band (the spec's open point is stale); the macOS 80 px gutter needs a check on a Mac; the split target is assumed to be the Work panel; Bot Teams pages keep the Sidebar [P16] (X2), so the title bar layout must allow it.

#### 3.1.8 Region toggle: [RegionToggle.md](components/RegionToggle.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Off; on (`fill.selected.frame`, `accent.fg` icon, `aria-pressed`); hover; pressed; focus-visible; disabled (for example Project sidebar on full pages, where the region is hidden instead) | Enter or Space activate; no default shortcut; the palette (`Mod+K`) reaches every toggle | After the session title, left to right; ring 2 px, offset 2 px | Fixed 28 px; visible down to 800 px; a region hidden for width opens as an overlay | Region animates `duration.moderate`, `ease.standard`; button fill `duration.fast`; reduced motion instant | 28 px rounded rectangles, solid fills; 16 px icons with an "absolute stroke of 2.25 px", which conflicts with 4.6 (X8); tooltip in overlay layer | Not on Android. Overflow menu and top bar take the actions; 48 dp targets | Desktop: title-bar toggles with tooltip and pressed state. Android: overflow menu |

Open: the split target is assumed to be the Work panel; Checklist uses `list-checks`, which is the Tasks rail icon in 4.6 (an icon clash); "Open in Work panel" is disabled with no file (confirm); the Canvas panel button is unnamed.

#### 3.1.9 Frame and Canvas: [FrameCanvas.md](components/FrameCanvas.md) · D (A: master-detail)

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default (no seam); loading (Frame paints, regions show skeletons); empty (start page); error per region; overlay (`surface.overlay`, `shadow.e2`); dialog (`scrim`, `shadow.e3`) | F6 and Shift+F6 cycle regions, skipping hidden ones; Escape closes the topmost overlay; Tab and Shift+Tab move inside a region | The Frame takes no ring; closing an overlay returns focus to its opener; open overlays join the F6 order | Collapse order: Session summary below 1408, Project sidebar below 1168, Sidebar below 928, Work panel below 728 (the sums of minimums); [P8] says 1528 for six regions (X1); Conversation minimum 360 | Region width `duration.moderate`, `ease.standard`; inner content keeps its width; reduced motion `duration.instant` | Frame on `surface.frame`; Canvas is one rectangle with a top-left `radius.canvas`; regions are flex columns; overlays in overlay layer; no border, shadow, blur or gradient | Compact: one pane under a 56 dp app bar. Medium and expanded: 360 dp master on `surface.frame`, detail on `surface.canvas` with `radius.canvas`, `space.4` padding | Desktop: Frame of rail, Sidebar and Title bar, plus the Canvas. Android: stack, then master-detail from 600 dp. Extensions are desktop only |

Open: overlays are not modal and have no scrim (confirm); the Sidebar footer runs to the window bottom and belongs to the Sidebar tree (confirm); the list at line 53 omits the rail item (X57); `z.region-overlay` is the only z token defined.

#### 3.1.10 Status bar: [StatusBar.md](components/StatusBar.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; loading (`text.tertiary`; spinner only after 200 ms on a remote host); empty ("No ports"); error per item; disconnected hosts dim with their age | F6 reaches the bar last; Left and Right move between items; Enter or Space run the command; Escape leaves the bar | After the Project sidebar, before the Title bar (the cycle wraps); each item shows an inset ring | Height fixed at 24 px; items drop by priority, lowest first; hidden items appear only in the visibility menu | None on the bar; item hover per StatusItem; the agent spinner keeps turning under reduced motion | 24 px row on `surface.canvas` under the Canvas column; flex items; 12 px glyphs; tabular figures; menus and tooltips in overlay layer | Not shown on Android. Host connection and usage sit on host cards [P21]; host list rows 48 dp | Desktop only. Android has no status bar; the system bar stays |

Open: the agent glyph replaces the owner's dot (confirm); splitting items across two groups is a proposal; the agent click target is not stated ([P18] says a Status bar popover covers agents).

#### 3.1.11 Status item: [StatusItem.md](components/StatusItem.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default (`text.secondary`); hover (`fill.hover.canvas`, `text.primary`); pressed (`fill.pressed.canvas`); focus-visible (inset ring); loading (last value or a dash); empty ("No ports"); error (`octagon-alert`, "Failed"); disconnected (text dims, hollow ring) | With the bar focused, Left and Right move between items; Enter or Space run the command; keyboard focus shows the tooltip at once | The bar's place in the F6 cycle; 2 px ring inside the item (offset -2) | Height never changes (24 px); text truncates before the item hides; extension text capped at 24 characters | Hover fill `duration.fast`, `ease.standard`; nothing blinks or pulses; reduced motion instant | Flex child: caption plus 12 px glyph [P31]; hover is a solid `fill.hover.canvas` with `radius.sm`, no alpha; tooltips in overlay layer | Not on Android. The same values appear on host cards and session rows, with the state word in TalkBack labels | Desktop: hover fill and tooltip; the state word is in the tooltip and name. Android: long press shows the label |

Open: success and warning words appear only in the tooltip and the name (confirm); per-item click targets are proposals.

### 3.2 Agent, work panel and conversation

#### 3.2.1 Agent status: [AgentStatus.md](components/AgentStatus.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Six states [P30]: running (3/4 arc), waiting (filled circle with question mark, row tint), idle (hollow ring), done (check circle), error (triangle with exclamation, row tint), unreachable (dashed ring with slash, dimmed row). Loading and empty: no glyph | No stop of its own; the containing row takes focus and the state word is read with it | Not a stop; the row or tab owns focus | Fixed 12, 16 and 20 px (`size.icon.xs` to `size.icon.md`); never below 12 px; surrounding text scales | Running arc turns over `duration.spinner` with `ease.linear`, also under reduced motion; other states and tints do not animate | Arc path rotated per frame; dashed ring as short arcs when there is no dash style; unreachable rows dimmed by text colour, not opacity | Glyphs through `react-native-svg` at 12, 16 and 20 dp (conflicts with the 20 and 24 dp Android icon sizes, X14); the row is the 48 dp target; the label is the state word | Desktop: tooltip with the state word after 400 ms. Android: TalkBack label, dp sizes |

Open: dimmed unreachable rows use `text.tertiary` (owner to confirm by eye); Android 12 and 16 dp sizes (X14); the status bar glyph is 12 px and tabs 16 px, per [P31]; a colour-blind-safe variant is open (design-system 10, question 4; [P35]).

#### 3.2.2 Work panel tabs: [WorkPanelTabs.md](components/WorkPanelTabs.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default (`text.secondary`); hover (`fill.hover.canvas`, close mark shows); pressed; active (`fill.selected.canvas`, underline); focus-visible (inset ring); loading (label shows, body skeleton); empty (four actions); error (tab colour unchanged) | One tab stop; Left and Right move; Enter or Space activate; proposed `Mod+T`, `Mod+Shift+B`, `Mod+W` (K2, K6) | After the Session summary in the F6 cycle; ring inset (offset -2) | Height 28 px [P31]; strip scrolls sideways on overflow; labels never wrap; maximum width 200 px (`layout.tab.max-width`) | Hover fills `duration.fast`; underline has no animation; reorder follows the pointer; reduced motion instant | Flex tabs with a solid hover fill; 2 px underline and drop indicator rectangles; horizontal scroll, no edge gradient | Not on Android in v1. If tabs become screens, a 48 dp target with role tab | Desktop only. Android has no Work panel in v1 |

Open: overflow scrolls sideways with no tab-list menu (confirm); the Lucide names square-terminal, globe, file-text and smartphone are proposals; `border.width.thick` (absent); `size.control.comfortable` and `size.touch.large` are shorthand only; the proposed `Mod+W` closes the focused tab while [P12] says the active tab (X27).

#### 3.2.3 Project sidebar tabs: [ProjectSidebarTabs.md](components/ProjectSidebarTabs.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default (`text.secondary`); hover (`fill.hover.canvas`, `text.primary`); pressed; selected (`fill.selected.canvas`, underline); focus-visible (inset ring); disabled (unused in v1) | One tab stop; Left and Right move; Enter or Space select; F6 reaches the Project sidebar | After the Work panel in the F6 cycle; the strip precedes the panel content | Icon-only, 28 by 28 px at every width from 240 to 480 px; six tabs (168 px) fit the 240 px minimum | Fill and underline `duration.fast`, `ease.standard`; tooltip after `duration.tooltip-delay`, at once on keyboard focus | 28 px square tabs, solid hover fill, 2 px underline; 16 px SVG icons with 1.5 px stroke | Not on Android in v1. Source control and checks are reached from item detail screens, which are not designed here | Desktop: icon strip; the label is the tooltip and accessible name. No Android presence in v1 |

Open: four core tabs (Files, Sessions, Source control, Checks); the Lucide names folder-tree, history, git-branch and badge-check are proposals; two tabs per extension (confirm the total cap); count badges are not in the owner notes.

#### 3.2.4 Conversation message list: [ConversationMessageList.md](components/ConversationMessageList.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Hover shows message actions; focus-visible (inset ring); loading (skeleton after 100 ms; 16 px spinner for older history); empty; error with Retry; disconnected (Banner at top, loaded content stays, running glyph becomes unreachable with "Last known: running, 5 min ago"); long user message clamps at `MESSAGE_CLAMP_LINES` 12 | One roving stop; Up and Down move; Home and End; Page Up and Page Down scroll; Alt+Up and Alt+Down go to the previous or next user message; Enter enters a message; Escape returns focus; Mod+F Find; Mod+C copies | The list, then the Composer; F6 enters and leaves; interactive blocks are reached with Tab | Column up to 720 px, never under 360 px; under 480 px the user bubble takes full width and its actions move inside it | New rows never animate (virtualised); the jump button fades in over `duration.base`, `ease.enter`; click scroll `duration.moderate`; reduced motion has no fade or smooth scroll | Variable-height list (ListState), bottom-aligned; heights cached by message id and width; cross-block selection needs S4 | FlashList with `estimatedItemSize` and `maintainVisibleContentPosition`; 16 dp gutter; long-press sheet; 44 dp jump button above the Composer | Android: full width, no minimap or Find, long-press menus. Desktop: minimap, Find, hover actions |

Open: right-aligned user bubbles by decision (confirm); per-block selection until GPUI supports cross-block selection (S4); fork-from-here is undecided (D-AG-05 is a dialog today); the disconnected copy differs from AgentStatus (X41).

#### 3.2.5 Message minimap: [MessageMinimap.md](components/MessageMinimap.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Ticks and viewport window (`fill.selected.canvas`, 2 px `accent.fg` edge); hover tick (`text.primary`, with a card); loading, empty and error hidden; disconnected: the last running tick becomes the unreachable mark | Not a tab stop; actions come from the list (Alt+Up and Alt+Down, Home, End, Page Up and Page Down) | Not focusable; `aria-hidden` with role presentation | Track 24 px, full scroll-area height, in the right gutter; shown only when content exceeds 1.5 viewports and the Conversation is at least 480 px [P8] | Scroll jump only (`duration.moderate`, `ease.standard`; instant under reduced motion); hover card after `duration.tooltip-delay` | One custom element painting quads; flat tick array; binary-search hit test; hover card deferred and non-capturing | Not designed; Android has no minimap | Desktop only |

Open: mark user messages only, or all (the spec marks all, with user ticks wider; the owner has not said); search matches shown with `match.bg` while the find bar is open; the tick sizes `size.minimap.tick.*` are shorthand.

#### 3.2.6 Message and turn blocks: [MessageBlocks.md](components/MessageBlocks.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Hover (`fill.hover.canvas` on rows, `fill.hover.raised` on headers); pressed; focus ring 2 px, offset 2 px; disabled (Approve, Deny, Send answer while submitting or host unreachable); loading; error; delegation card: running, done, error, waiting, unreachable [P30] | Non-interactive blocks are not stops; headers toggle with Enter or Space; approval Tab order is Deny then Approve; no shortcut approves [P11]; focus never moves into a new card | Message by message; inside a card: header, body controls, Deny, Approve | Cards follow the column width; under 480 px approval buttons stack full width, Approve first | Expand and collapse `duration.base`, `ease.standard`; a new approval card does not animate; reduced motion instant | Block tree of styled text; code and diff virtualised at 20 px rows beyond 100 lines; approval buttons in a fixed footer; clicks ignored for 300 ms after the buttons appear (not in 4.8) | Stable height cache for list items; code scrolls in its own ScrollView; approval buttons 48 dp, 8 dp apart | Android: "Open diff" opens full screen, buttons stacked. Desktop: "Open in Work panel" |

Open: approval options beyond Approve and Deny are not in the inventory (a menu slot is reserved, unshipped); answered approvals stay (assumed); snippet lines use `text.primary` on diff backgrounds, which matches [P34].

#### 3.2.7 Queued message: [QueuedMessage.md](components/QueuedMessage.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Queued (clock, `text.tertiary`); host not confirming (unreachable glyph, Send now disabled); sending (arc); confirm (Cancel focused); editing (`surface.sunken`, `border.strong`); disabled (archived or unreachable); error (Retry and Remove; stays queued) | Dock reached by Tab after the Composer controls; each row is one tab stop; Left and Right move between its three actions; Delete removes the row; Enter saves an edit; Shift+Enter inserts a line; Escape cancels an edit or confirm | After Remove, the next row; then the previous row; then the Composer. After Send now, the Composer | Follows the Composer width; under 480 px the reason caption wraps under the text | Rows enter and leave by height and opacity, `duration.base`, `ease.enter` and `ease.exit`; reduced motion instant | Plain column above the Composer, pushing the message list; queue held in core session state | FlatList, at most 3 rows visible, anchored to the IME inset; rows 56 dp; Send now 48 dp; Edit and Remove in the row sheet and accessibilityActions | Desktop: three icon buttons per row. Android: Send now plus overflow. Undo toast 6 s on both |

Open: whether Send now interrupts a running tool call or waits for the step to end (the spec says interrupt; confirm with the adapters); Remove has no confirmation, which conflicts with the Android rule for destructive actions (X16); no reorder control, by decision.

#### 3.2.8 Plan view: [PlanView.md](components/PlanView.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Hover (`fill.hover.canvas`); pressed; focus-visible (inset ring); in-progress row (`accent.subtle.canvas`, 2 px `accent.fg` bar); loading (`fill.skeleton`); empty ("The agent has not shared a plan yet."); error (Retry); disconnected (unreachable glyph replaces the spinner) | Step list is one roving stop; Up and Down move; Right expands and Left collapses; Home and End jump; Mod+C copies the step; the segmented control and Jump are stops | Header controls, step list, document, Jump button | Fills the tab up to `layout.conversation.reading` (prose only, 720 px); the card form uses the Session summary width (288 default) | Sub-step expand `duration.base`, `ease.standard`; Jump scroll `duration.moderate`; glyph and colour changes never animate | Plain column; virtualised only above about 200 rows; the in-progress bar is a quad on the row edge | Bottom sheet with FlatList; 44 dp rows; 48 dp hit for expandable parents; TalkBack announces updates | Desktop: Work panel tab (Mod+C). Android: bottom sheet, long-press copy |

Open: read-only, so there is no edit or reorder (approval is a card); Markdown-only plans show Document mode with Steps hidden; the header progress bar uses `usage.ring.track`, which is the wrong token for a progress bar (X42).

#### 3.2.9 Composer: [Composer.md](components/Composer.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Hover (`border.strong`); focus (2 px `border.focus` on the container); drop target (dashed `accent.fg`, `accent.subtle.raised`); error line (`status.danger.fg`); disabled (archived, sign-in needed, or host unreachable and cannot queue: `fill.disabled` and reason line); disconnected host (enabled; Send becomes Queue, with a caption "Messages wait in the queue.") | Enter sends or queues (not during IME composition); Shift+Enter inserts a line; Mod+Enter sends; Escape twice stops when empty (proposal); `@` and `/` open menus; Up restores the last message; Mod+Shift+M opens the Model picker | Opening a session focuses the text area; sending keeps focus; a closed popover returns focus to its trigger | Follows the column up to 720 px; text area 2 to 8 lines (`size.composer.min-height` 44 to `size.composer.max-height` 176); under 480 px extension actions move into the `+` menu; the start page uses a 3-line minimum of 66 px (X51) | Height changes instantly; drop border `duration.fast`; reduced motion has no effect | Needs a shared IME-capable text input (S7: Windows TSF, macOS NSTextInputClient, Linux IBus and Fcitx); mention chips are atomic inline runs | Multiline TextInput, at most 6 lines, IME-aware; Send 44 dp (48 hit); attach and model chips 44 dp, 8 dp apart; suggestion chips above the field when empty | Android: Send is a button, and the keyboard action key inserts a newline; attach opens a sheet |

Open: Escape-twice stop is a proposal; the access-mode chip warning glyph in Full access is decided [P11], but the spec still lists it as open; pastes over 2,000 characters as chips are not designed; the "Terminal / Chat" switch is unconfirmed; draft save (300 ms) and hint (3 s) are literals.

#### 3.2.10 Model picker: [ModelPicker.md](components/ModelPicker.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Trigger: hover, pressed, focus, open, disabled (reason tooltip), loading (12 px spinner after 200 ms), error (warning glyph). Popover: searching, no match, signed out, error with Retry, disconnected (last known list) | `Mod+Shift+M` opens (rebindable); Enter, Space, Down or click open; focus lands in search; Tab cycles columns; arrows rove; Alt+Enter toggles a favourite; printable keys go to search; Escape closes to the chip; `Mod+1` to `Mod+4` select while open | Non-modal popover; closes when focus leaves; Escape returns focus to the chip | 560 px wide; under 592 px available it shrinks by `space.4`; under 480 px the reasoning column moves below the models; the list scrolls within 420 px | Popover fades in with a 4 px move, `duration.base`, `ease.enter`; exit `ease.exit`; fills and star `duration.fast`; reduced motion instant | Deferred anchored popover on `z.popover` (absent) that flips when short of room; fixed 32 px rows for `uniform_list`; key context binds `Mod+1` to `Mod+4` only | Bottom sheet, 90 percent maximum height; one column with 48 dp agent chips, 56 dp model rows, star always visible | Desktop: popover, three columns, hover star, 24 px chip. Android: sheet, one column, 44 dp chip |

Open: `Mod+1` to `Mod+4` by row position (spec) or favourites (design-system 7.2) (K7; X9); the chip omits the access mode, although [P11] decided a warning glyph in Full access; the trigger chip density (X10); access-mode names are not listed; `layout.model-picker.*` values are in 5.5.

#### 3.2.11 Session summary card: [SessionSummaryCard.md](components/SessionSummaryCard.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Collapsed (up to 6 rows); hover; focus-visible (inset ring); expanded (remembered per card); disabled remote Open; loading (12 px spinner after 200 ms); empty (never rendered); error row; disconnected (dimmed, with age) | The column is an F6 region; headers form an accordion: Tab reaches the first header, Up and Down move, Enter or Space toggle, Home and End jump; Tab moves inside an expanded card | Header, then the card's controls when expanded, then the next card | Default 288 px (`layout.summary.default`), minimum 240, maximum 400 (values only in 5.1); cards fill the width; the column collapses below 240 px [P8] | Insert over `duration.card-insert` (250 ms), `ease.enter`: height from 0, fade in, 8 px rise; cards below slide with `ease.standard`; expand and collapse `duration.base`; reduced motion instant | Flex column with overflow scroll; measure content off-screen once, then animate the clip height; one animation clock per insert | Not on Android. Plan, background and status pill appear as sheets or a pill above the Composer | Desktop only. The Results card is the only artifact list [P15]; never hidden automatically [P7] |

Open: the card gap is `space.3` in the spec and `space.5` for card stacks in 4.1 (X13); the chevron expands in place; no "Forward port" action; the Connectors card is hidden when the provider reports nothing; the progress track uses `usage.ring.track` (X42).

#### 3.2.12 Delegation tree: [DelegationTree.md](components/DelegationTree.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover; pressed; focus-visible; selected; disabled; loading; empty; error; waiting for you; done; long content; disconnected host | One tab stop; Up and Down move rows; Right expands or enters a child; Left collapses or goes to the parent; Home and End; Enter opens; typing jumps; Tab leaves | Header, tree, then transcript; ring inset around the row | Not specified. Names truncate; over 12 children show the first 8 and "Show 4 more" | Row insert fades with an 8 px move (`duration.base`, `ease.enter`); expand and collapse change height (`duration.base`, `ease.standard`); reduced motion instant | Flatten the visible tree to (node, depth) pairs; connectors as quads; shared AgentStatus; state kept in the session model | Card (inventory X-CH-06) in chat plus a flat sheet; 56 dp rows; 48 dp targets; indent at most two levels; TalkBack reads name, role, state and duration | Desktop: full tree with connectors and arrow keys. Android: flat two-level sheet |

Open: Bring back (inventory X-CH-08) row actions are undecided; inline approval answers are not designed; the depth limit conflicts with [P23] (X25); the Bot Teams words "Working" and "Needs you" differ from the design-system state words (X40).

#### 3.2.13 Secret request card: [SecretRequestCard.md](components/SecretRequestCard.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Pending; hover; pressed; focus-visible; entering; stored; linked; declined; disabled; error (failed); long content; disconnected host. Loading and empty are not applicable | Tab order: Add secret, then Decline; Enter or Space activate; no shortcut adds or declines [P11] | Ring on the focused button; when the modal closes with a result, focus moves to the card container, which conflicts with the return-to-opener rule in 8.2 (X59) | Follows the message column; under 480 px the actions stack, Add first | None beyond the transcript; a state change swaps instantly | Transcript block; state enum kept in the session model; modal text never enters card state | Same blocks in chat; 48 dp full-width buttons, Add first, 8 dp apart; Add opens a sheet | Desktop: right-aligned 28 px buttons. Android: full width, 48 dp; "Open vault" opens the list screen |

Open: agent-supplied title wording; team-scope secrets (app-flow G24) are undecided; "Link existing" is not offered on the card (app-flow A28).

#### 3.2.14 Secret modal: [SecretModal.md](components/SecretModal.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Entering; hover; pressed; focus-visible; selected; invalid name; name in use; saving; stored; host unreachable (nothing queued [P25]); failed; loading; empty; long content | Tab order: Name, Value, Show, Scope, link button, Cancel, primary; Enter submits when valid; Escape closes and discards; Left and Right change Scope; focus is trapped | Initial focus on Value if the name is prefilled, else Name; on close, focus returns to the card's Add button or the opening vault row | Fixed width 440 (`layout.dialog.width.md`) [P36]; under 480 px the window width minus `space.4`; body scrolls | Fade and scale 0.98 to 1, `duration.moderate`, `ease.enter`; show and hide instant | Shared Dialog; secure text element clears its buffer on drop; the value never enters the dialog model | Bottom sheet, `radius.2xl`, expands to full height with the keyboard (X38); 48 dp fields; `secureTextEntry`; `FLAG_SECURE` on the window while open | Desktop: centred dialog with vault add, edit, rotate, delete and unlink. Android: sheet with add and list only |

Open: team scope (app-flow G24) and several hosts (G25) are undecided; headless-server store unlock (G22) is not designed.

### 3.3 Controls and inputs

#### 3.3.1 Button: [Button.md](components/Button.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover; pressed; focus-visible; disabled; loading (spinner in the icon slot after 200 ms, width fixed); long content (label ellipsis, full text in Tooltip); disconnected host (disabled, reason text linked by `aria-describedby`). Variants: primary, secondary, ghost, danger | Enter or Space activate; Tab and Shift+Tab move; a destructive Dialog starts focus on Cancel | 2 px `border.focus` ring, offset 2 px; on clipped accent fills the inset ring uses `text.on-accent` | Content-sized; the label truncates before the icon drops; minimum width not tokenised | Colour step `duration.fast`, `ease.standard`; the spinner keeps turning under reduced motion | Interactive div with focus handle; precomputed solid state colours; no box shadow, no hover opacity; spinner as an SVG arc | `Pressable`, role button, states disabled and busy; `minHeight` 44 dp with hitSlop to 48 dp; label 14/20/500; ripple: `fill.ripple` on secondary and ghost, the pressed token on primary and danger | Desktop 28 px (24 compact, 32 comfortable). Android 44 dp visual, 48 dp hit; the single bottom primary action is 48 dp |

Open: whether every Android primary action is 48 dp (the spec says only the single bottom action); the 200 ms spinner delay is a literal (X21).

#### 3.3.2 Icon button: [IconButton.md](components/IconButton.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover; pressed; focus-visible; selected (toggle, `aria-pressed`); disabled; loading (spinner after 200 ms); error (status icon only); disconnected host; optional status dot | Enter or Space activate; a toolbar or row group is one tab stop with roving focus; Left and Right (Up and Down if vertical); Home and End | 2 px ring, offset 2 px; inset 2 px where a scroll area or the Frame edge would clip it | Fixed square 28 px (`size.control.default`); in a narrow toolbar the lowest-priority buttons move into More | Fill step `duration.fast`; spinner `duration.spinner`; tooltip after `duration.tooltip-delay`, none between buttons | Square interactive div with focus handle; tooltip is an overlay element; the toolbar owns roving focus; no shadow | `Pressable`, role button; toggles use `checked`; a long press shows the name as a popup, which conflicts with the Button rule (X54) | Desktop: 28 px square with hover and tooltip. Android: 44 dp visual, 48 dp hit, 24 dp toolbar icon; overflow is a Menu (desktop) or a Sheet (Android) |

Open: the optional status dot is 8 px in the spec but 6 px in design-system 4.2 and [P31] (X12); whether title-bar region toggles need a 2 px accent bar is undecided.

#### 3.3.3 Segmented control: [SegmentedControl.md](components/SegmentedControl.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover; pressed; focus-visible; selected (quiet or strong); disabled; long content (64 px floor, then truncate); disconnected host (all segments disabled) | One tab stop; Left and Right move focus and select at once; Home and End go to the ends; Enter or Space select the focused segment | 2 px ring on the focused segment, inset 2 px so it stays inside the track | Equal widths when they fit, then content width, then truncate | Colour step `duration.fast`, `ease.standard`; no sliding indicator | Track div with a 1 px border and 2 px inner gap; the track handles keys; no animated indicator | Segments are `Pressable` with role radio, checked and disabled; visual 44 dp, hit 48 dp; label 14/20/500 | Desktop: 28 px track. Android: 44 dp track, equal widths; more than three segments move to a Sheet with radio rows |

Open: segments separate by fill only, with no dividers, so Android targets touch each other, against the 8 dp spacing rule in 5.4 (X34); the 64 px floor is a literal; the Android three-option inline rule needs confirmation.

#### 3.3.4 Switch: [Switch.md](components/Switch.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Off; on (rest and hover; pressed when on); focus-visible; disabled (`border.disabled` outline); loading (knob spinner while saving on a host); error (reverts; `octagon-alert` helper); disconnected host (disabled) | Space toggles; Tab and Shift+Tab move; Enter does nothing | 2 px ring, offset 2 px, around the track | Keeps its size; the label truncates first | Knob slides and track colour changes in `duration.fast`, `ease.standard`; reduced motion instant | Rounded track div; circular knob moved by an animated offset over `duration.fast` | `Pressable`, role switch, states checked, disabled and busy; `minHeight` 48 dp; track 36 by 20 dp; knob 14 dp | Desktop: 32 by 18 px track, 12 px knob, at the right edge of a Settings row. Android: 36 by 20 dp track, 14 dp knob |

Open: the Settings row height is 32 px in this spec, but design-system 4.2 gives the Settings row 40 px for a label alone (X11); the Android 14 dp knob is derived; `size.switch.touch.*` is a wildcard only; the state word on a single-switch page is decided as none.

#### 3.3.5 Checkbox: [Checkbox.md](components/Checkbox.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Unchecked; checked; indeterminate (hover outline `accent.fg`); focus-visible; disabled (outline `border.subtle`, see X32); error (danger outline, `octagon-alert` helper); disconnected host | Space toggles; Tab moves to the next checkbox; arrows unused; a click on an indeterminate box clears all if any child is checked, else selects all | 2 px ring, offset 2 px, around the box and label | The box keeps its size; the label wraps in forms and truncates in lists | Tick draws in `duration.fast`, `ease.standard`; reduced motion instant | 16 px div with a 1 px border; tick and dash are SVG paths; the focus ring is drawn around the row | `Pressable`, role checkbox, checked true, false or mixed; row 48 dp; box 20 dp; label 14/20/400 | Desktop: 16 px box with hover. Android: 20 dp box in a 48 dp row, with ripple |

Open: a hover outline in `accent.fg` may read as selected (accepted); the 20 dp Android box has no token.

#### 3.3.6 Radio: [Radio.md](components/Radio.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Unselected; selected (rest; hover outline `accent.fg`); focus-visible; disabled (outline `border.subtle`, see X32); error (danger outline, helper); disconnected host | One tab stop; arrow keys move focus and select the next or previous option; Tab leaves the group | 2 px ring, offset 2 px, around the row | The circle keeps its size; labels wrap in forms | The dot appears in `duration.fast`, `ease.standard`; reduced motion instant | Rounded circle div with a border; a child circle as the dot; the group element handles keys | `Pressable`, role radio inside a radiogroup, checked and disabled; row 48 dp; circle 20 dp; label 14/20/400 | Desktop: forms and settings. Android: full-width row list, or a Sheet with radio rows for more than three options |

Open: the same hover and disabled-outline questions as Checkbox; the 8 px dot and 20 dp circle are literals.

#### 3.3.7 Text field: [TextField.md](components/TextField.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Rest; hover; focused; read-only; disabled (outline `border.disabled`); loading (trailing spinner after 200 ms); empty; error (danger outline, error line); long content (single-line scrolls, multi-line wraps); disconnected host (read-only) | Tab and Shift+Tab move between fields; Enter submits a single-line form with its default action; Mod+Enter submits a multi-line form; Escape restores the saved value in inline editors | The border becomes `border.focus` at 2 px drawn inside the field, which differs from the 8.2 offset ring (X33) | Fills its container; in a Dialog it keeps the Dialog width | Outline colour `duration.fast`, `ease.standard`; reduced motion instant | Platform text input over a rounded div; focus replaces the border colour and width in place; selection in `selection.bg` | `TextInput` with `accessibilityLabel` and hint; `minHeight` 44 dp; label 14/20/500; input text 16 sp (3.2 has no 16 sp input style); scales to 200 percent | Desktop: 28 px (32 px in forms) with hover. Android: 44 dp field; the keyboard action key submits |

Open: the multi-line minimum is 68 px (`size.field.multiline.min`); the 16 sp input text needs a type style in 3.2; the 200 ms spinner delay is a literal (X21).

#### 3.3.8 Search field: [SearchField.md](components/SearchField.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Idle; typing; searching (spinner after 200 ms, old results dimmed); results (`match.bg`, `match.active.bg`); no results (EmptyState); filtered; error (ErrorState with Retry); disabled; disconnected (last known results) | Typing starts the search; Escape clears the query, else leaves the field; Down enters results; Enter opens the active result; `Mod+K` can focus the field (K1) | Order: field, scope control, results; no ring rule stated here, so the TextField rule applies | The field shrinks first; the scope control moves into an overflow Menu before the field drops below 160 px | Result highlight colour step `duration.fast`; reduced motion instant | TextField with a leading icon; highlights as text runs; debounce on the foreground executor (`SEARCH_DEBOUNCE_MS` 200) | `TextInput` with `returnKeyType` search and role search; results announced once per finished search | Desktop keeps the scope inline. Android moves the scope to a Sheet. Extensions may use it, though 7.1 names only Text field |

Open: `Mod+K` focuses the field in this spec but opens the palette elsewhere (K1); the file-path line per Content match needs confirmation; the 160 px threshold is not a token.

#### 3.3.9 Select and dropdown: [Select.md](components/Select.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Rest; hover; pressed; focus-visible; open (2 px `border.focus` drawn inside the trigger); selected; disabled; loading (static skeleton rows); empty options (one reason line); error; disconnected host (last known options) | Enter, Space or Down open; Up and Down move; Home and End jump; a letter jumps to the next match; Enter selects and closes; Escape closes to the trigger | Ring 2 px, offset 2 px; the open state uses an inside 2 px `border.focus` | The trigger fills its container; the Menu is as wide as the trigger or the longest option, up to the surface width, while the token caps Menus at 280 (X35) | Menu opacity and 4 px move in `duration.base`, `ease.enter`; exit `ease.exit`; reduced motion instant | Trigger div with focus handling; the Menu is an overlay anchored to the trigger | `Pressable`, role combobox; options in a Sheet with radio-style rows at 48 dp; value text 16 sp | Desktop: anchored Menu. Android: 44 dp trigger and bottom Sheet; search appears above 12 options |

Open: the 12-option search threshold is a proposed rule, not a token; the 12 and 40 option limits are literals; the `layout.menu.*` values (180 and 280) apply.

#### 3.3.10 Picker: [Picker.md](components/Picker.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Loading (three static skeleton rows); empty (EmptyState; "This folder is empty" in folder mode); error (ErrorState with Retry, last list kept); filtered (`match.bg`); selected (check); hover and focus; disabled; disconnected host (last known rows) | Enter, Space or Down open; typing filters; Up and Down move; Enter chooses and closes; Escape closes to the trigger; folder mode: Right enters a folder, Left or Backspace on an empty search goes up | Order: trigger, search field, list, footer action; the row ring's width and offset are not specified | Desktop panel fixed at 320 px (`layout.picker.width`), or the trigger width if larger; narrow windows anchor or become a Sheet-like overlay | Panel opacity and a 4 px move in `duration.base`, `ease.enter`; rows appear without animation | Overlay panel anchored to the trigger; virtualised list; keyboard state kept on the panel while the search keeps focus | `Pressable`, role combobox; Sheet with search at the top; rows 48 dp; back row in folder mode | Desktop: anchored popover with breadcrumbs. Android: full-width Sheet, 90 percent maximum height |

Open: the create row shows first, or only when nothing matches (the spec says the latter); `layout.sheet.max-height` is absent (6.4).

### 3.4 Overlays and feedback

#### 3.4.1 Menu and context menu: [Menu.md](components/Menu.md) · D (A: Sheet)

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Rest; hover (`fill.hover.overlay`); focus (`fill.selected.overlay` plus ring); pressed; selected (checkable); disabled (reason in a tooltip, X53); loading (one skeleton row); empty (one line); error (retry item); long content (truncate); disconnected (items disabled) | Enter or Space open from the trigger; Shift+F10 and the Menu key open context menus; Up, Down, Home, End and letters move; Right opens a submenu, Left closes it; Enter activates; Escape closes to the trigger; Tab closes | Takes focus on open at the first enabled item; items are not individually focusable in GPUI; ring 2 px, inset 2 px | Repositions inside the window; flips above the trigger without room below; 180 px (`layout.menu.min-width`) to 280 px (`layout.menu.max-width`), then labels truncate | Opacity 0 to 1 with a 4 px move, `duration.base`, `ease.enter`; exit `ease.exit`; reduced motion instant; submenu hover delay `SUBMENU_HOVER_DELAY_MS` 150 ms | Overlay element on the overlay layer that flips at the window edge; hover and selected fills precomputed for the overlay | Android is a Sheet with 48 dp rows and no shortcut hints; rows are `Pressable` with role menuitem; long-press actions mirrored in accessibilityActions | Desktop: anchored overlay with shortcut hints and right-click. Android: bottom Sheet with long press |

Open: submenus open on hover, or only on the Right arrow (the spec says both); the 48 dp Android row has no token (4.2 gives 44 dp as the default).

#### 3.4.2 Popover: [Popover.md](components/Popover.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Closed; open (`shadow.e2`); loading (skeleton blocks); empty (one `body-sm` line); error (ErrorState with Retry); focus (form and picker only); long content (body scrolls after 360 px); disconnected ("Last known state, 5 min ago") | Enter or Space on the trigger opens; Escape closes and returns focus to the trigger; Tab moves through, then out; a form closes on Tab-out only when nothing is unsaved | The trigger, then the first control (form and picker only); info popovers never take focus; close returns focus to the trigger | Stays inside the window; flips when needed; widths 280, 320 and 360 (`layout.popover.width.sm`, `md`, `lg`) [P36] | Opacity 0 to 1 with a 4 px move, `duration.base`, `ease.enter`; exit `ease.exit`; reduced motion instant | Overlay element anchored to the trigger on the overlay layer; confirm the shadow blur convention (S3); explicit focus handle on open | No Popover on Android; the same content goes in a Sheet; do not port anchored positioning | Desktop: anchored, with `shadow.e2`; hover-opened usage popovers are desktop only. Android: Sheet |

Open: the usage-ring popover stays open while the pointer is inside it (the spec says yes); the hover-open delay is "a short pause", with no named value.

#### 3.4.3 Tooltip: [Tooltip.md](components/Tooltip.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Hidden; pending (`duration.tooltip-delay`, 400 ms after rest); shown; no delay between tooltips; hidden by press; disabled trigger still shows its tooltip; long content (three lines, then ellipsis); empty (never shown) | Shows at once on keyboard focus; hides on blur or Escape; Escape closes the tooltip only, not the trigger's action | Never takes focus or holds a control; the trigger links to it with `aria-describedby` | Stays inside the window; flips side when needed; maximum width 240 px (`layout.tooltip.max-width`) | Opacity only, `duration.fast`; hide with `ease.exit`; reduced motion instant | Overlay element positioned against the trigger bounds, not a child; show and hide timers on the window timer; `shadow.e2` | Not used on Android; `accessibilityLabel` and `accessibilityHint` carry name and hint; disabled reasons go in visible text beside the control | Desktop: hover and keyboard focus only. Android: no tooltips |

Open: touch users on desktop-tablet get no tooltips (the spec says no); the 400 ms delay is a 4.7 token, not a constant.

#### 3.4.4 Dialog and confirm: [Dialog.md](components/Dialog.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Opening (scale 0.98 to 1); default; destructive (focus on Cancel); busy (spinner `size.icon.sm`; Escape ignored); error (status line; the action becomes "Retry"); long content (body scrolls after 40 percent of the window height); disconnected (action disabled) | Escape closes unless busy; Tab and Shift+Tab cycle inside (focus trap); Enter activates the focused button; Enter on Cancel closes a destructive dialog; `Mod+Enter` is not used | Moves in on open: Cancel for destructive dialogs, the primary action otherwise; returns to the opener on close; trapped while open | Fixed width 400, 440 or 560 (`layout.dialog.width.sm`, `md`, `lg`) [P36]; height grows to 80 percent of the window | Opacity and scale 0.98 to 1, `duration.moderate`, `ease.enter`; exit `ease.exit`; the scrim fades; reduced motion instant | Scrim is a full-window div in the `scrim` colour; focus trap as a key handler; focus returns to the stored handle; `shadow.e3` | `Modal` with `accessibilityViewIsModal`; Android back closes unless busy; buttons 48 dp; body 14 sp, 20, 400 | Desktop: centred dialog with `radius.xl`. Android: Dialog for confirms, Sheet for other secondary surfaces |

Open: a scrim click closes a one-field form with typed text (the spec says no); the Android dialog width range is `layout.dialog.android.min` 280 and `max` 560 dp (5.5).

#### 3.4.5 Sheet (Android bottom sheet): [Sheet.md](components/Sheet.md) · A

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Opening (`duration.slow`); dragging; closing; default; loading (skeleton rows); empty (one line plus its action); error (ErrorState with Retry); selected (check in `accent.fg`); disabled row (not pressable); long content (body scrolls); disconnected (last known rows) | Hardware Escape closes; focus moves to the first row on open and returns to the opener on close | Traps focus while open | Content-sized; rotation recomputes the height; maximum 90 percent of the screen (`layout.sheet.max-height`, absent) | Slides over `duration.slow` with `ease.enter` (exit `ease.exit`); the drag follows the finger; "Remove animations" makes it instant | Not applicable on desktop; Popover or Menu is used instead | Bottom-aligned `Modal` with a gesture handler; height measured from content; rows 48 dp; header title 18 sp; app bar height 56 dp (`size.appbar.android`) | Android only; desktop has no Sheet |

Open: confirms always use a Dialog, even with long loss lists; rows are 48 dp here, but two-line rows are 56 dp in 4.2 (X3).

#### 3.4.6 Toast: [Toast.md](components/Toast.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover (timers pause); pressed; focus-visible (2 px ring); disabled action (spinner, `agent.running`); error (`status.danger.fg`, `role="alert"`); entering; leaving; long content (three lines); duplicates ("x2" within 2 s); disconnected (error wording) | `Mod+Alt+N` focuses the toast group; Up and Down move between toasts; Tab reaches the action and close; Enter activates; Escape or Delete dismisses (K8, X18) | Never takes focus on its own; not in the F6 cycle | Fixed 360 px (min 280, max 400); narrower than 400 px, the stack uses the Canvas width minus `space.4` | Enters with an 8 px slide and fade, `duration.moderate`, `ease.enter`; leaves with opacity, `duration.base`, `ease.exit`; stack gap `ease.standard`; reduced motion instant; timers unchanged | One deferred layer at the Canvas bottom right, owned by the notification service (`z.toast`, absent); timers pause on window blur | Bottom overlay above the insets and IME; 48 dp minimum and action; `AccessibilityInfo.announceForAccessibility`; swipe plus a close button | Desktop: bottom right, 360 px, `shadow.e2`. Android: full width minus `space.4`, elevation 6 dp, above the Composer |

Open: Undo times out after 6 s on desktop, but on Android action toasts wait at least 8 s (X17); the minimum width is 280 here and 320 for cards (X19); error toasts without an action may time out after about 30 s (a proposal); full text goes in a tooltip, which Android does not have.

#### 3.4.7 Notification card: [NotificationCard.md](components/NotificationCard.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover (card fill); pressed; focus-visible; disabled (actions while the host is unreachable or running); loading (update states); empty (no stack drawn); error (update failed); entering; leaving; long content (title one line, body two); disconnected (`agent.unreachable.bg` plate, "Last known" line) | `Mod+Alt+N` focuses the first card; Up and Down move between cards; Tab moves within a card; Delete or Backspace dismisses; Escape returns focus only | Not in the F6 cycle; cards never take focus on their own | Fixed 360 px (min 320, max 400); in a Conversation narrower than 400 px the stack uses the Canvas width minus `space.4` | Entering slides 8 px and fades (`duration.moderate`, `ease.enter`); cards below move (`duration.base`, `ease.standard`); leaving `duration.base`, `ease.exit`; reduced motion instant | One deferred layer at the Canvas bottom right; per-card offset animates height changes; card list in the notification service | In-flow cards, not floating: inline at the top of Home and above the Composer; actions 48 dp with an 8 dp gap; swipe plus a dismiss button; system notifications for background | Desktop: floating stack with `shadow.e2`; OS notification for background. Android: inline with `surface.raised` and border, no shadow |

Open: cards and toasts share the bottom-right stack (decided here); attention cards after dismissal are not designed ([P19] has no notification centre); the card uses `surface.overlay` with agent tints that 2.8 resolves only on Canvas and Frame (X60); the kind priority is proposed.

#### 3.4.8 Banner: [Banner.md](components/Banner.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default per variant; hover and pressed (ghost actions); focus-visible; disabled action (while running); loading (spinner, also in Retry); empty (not rendered); error (danger); expanded ("Details"); long content (three lines, then ellipsis); disconnected (unreachable variant, age shown); several notices ("+1 more") | Banners are not tab stops; buttons are reachable in reading order at the top of the region; Escape dismisses a focused dismissible banner | Banner actions come before the content below in the region | Fills its container; text wraps to three lines; at narrow widths the actions wrap under the text, left aligned | Enter and leave over `duration.moderate`; the placement sentence says `ease.standard` but the motion sentence says `ease.enter` and `ease.exit` (X20); the spinner keeps turning under reduced motion | Flex row with a fixed minimum height; clipped height animation; the unreachable glyph is the shared AgentStatus element; the age is re-rendered once a minute by one timer | View under the top app bar, 56 dp minimum with actions; live region polite, assertive for danger and the first disconnect; no height animation under "Remove animations" | Desktop: `size.control.compact` actions, Details disclosure. Android: 48 dp actions, Details opens a sheet |

Open: a "Restored" auto-dismiss at 5 s is proposed; retry back-off wording comes from the transport layer; the role for "Auth failed" (warning) is unstated.

### 3.5 Indicators and states

#### 3.5.1 Badge and chip: [Badge.md](components/Badge.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover (not interactive); selected (not applicable); focus (not focusable unless wrapped in a button); disabled (`text.disabled`, no fill); loading (skeleton block for an unknown count); empty (removed at zero); error (danger, glyph and "Failed"); long content (truncates after 16 characters); disconnected (dashed ring, "Unreachable", neutral) | Not focusable unless wrapped in a button | Not specified beyond "not focusable" | Fixed height 16 px (`size.badge.height`); the text truncates before the badge wraps | None; count digits fade in `duration.fast`; reduced motion instant | Container div with a fill and a 1 px border; tabular figures through font features; the dot is a 6 px circle with no shadow | `View` with `accessibilityLabel` on the row; decorative dots `accessible={false}`; glyph through `react-native-svg` at 12 dp | Desktop: caption or label text. Android: 12 sp text, 20 dp height, scaling capped at 130 percent |

Open: the count pill is `radius.full` in the spec, while 4.3 gives chips `radius.sm`; extension badges support neutral, accent and danger kinds with dot or count only (7.2 point 1).

#### 3.5.2 Kbd shortcut badge: [Kbd.md](components/Kbd.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; selected (`text.primary`); disabled (`text.disabled`, no fill). Conflicts shown in D-ST-04 use `status.warning.fg`, not Kbd | None; Kbd shows a binding and takes no input | Not focusable | Fixed height 16 px (`size.kbd.height`); chords wrap only between rows; four keys at most | None | The platform check at start-up picks the mapping table, which the palette and editor share | Rendered only with a hardware keyboard; `Platform.OS` picks the mapping; Mod is Ctrl on Android | Desktop: bindings in tooltips, menus, palette and editor. Android: none unless a hardware keyboard is attached |

Open: the chord order in this spec (Ctrl or Cmd first, then Shift, Alt, key) conflicts with 8.1 (X36); JetBrains Mono glyph coverage for ⎋ and ↩ is unverified (the OS fallback is assumed); the 16 px minimum width and four-key limit are literals.

#### 3.5.3 Avatar and agent icon: [Avatar.md](components/Avatar.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; selected (2 px `accent.default` ring outside the circle); disabled (`text.disabled`, no glyph); loading (Skeleton circle); empty (initials); error (`agent.error` glyph, "Error" in the name); disconnected (dashed glyph, "Unreachable, last known 5 min ago") | Not focusable on its own | None of its own; the parent row or button owns hover and focus | Fixed circle; status glyph 12 px at md and lg, 10 px at sm (10 px is not a token size) | Running arc turns over `duration.spinner`; reduced motion keeps a static arc and the text | `rounded_full` circle; tinted SVG mark; glyph over the bottom-right edge with a frame or canvas ring; spinner is a rotating SVG | `View` with a half-size radius; `react-native-svg` or Image for full-colour marks; glyph 12 dp; `accessibilityLabel` | Desktop: rail rings and sidebar rows. Android: 40 dp in host cards and rows, 28 dp in chips |

Open: rail marks in full colour inside the usage ring follow [P32]; the spec cites "owner pending item 5", which is a stale number (X47); `size.avatar.*` is shorthand only; `border.width.thick` (absent); whether the Done glyph is in the set follows [P30].

#### 3.5.4 Progress (bar, ring, spinner): [Progress.md](components/Progress.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Determinate (width equals value, tabular %); indeterminate (30 percent segment travels); spinner (after 200 ms); complete ("Done" after 100 percent); error (`status.danger.solid`, "Failed at 62%"); paused; loading (nothing for the first 100 ms); disabled (`text.disabled`); disconnected ("Connection lost. Last known 62%, 5 min ago") | Not focusable; Cancel is a separate Button or IconButton with a tooltip | Not specified beyond "not focusable" | Fills its container; minimum width 80 px, below which the label goes first | Determinate width `duration.base`, `ease.standard`; indeterminate and spinner `duration.spinner`, `ease.linear`; reduced motion: the spinner keeps turning, and the indeterminate bar is static with "Working" | Two rounded divs for track and fill; fill width is a fraction of the track; spinner is an SVG arc rotated on the element | View-drawn bar with role progressbar; `ActivityIndicator` in `accent.default` after 200 ms | Desktop: 4 px bar, 16 px spinner. Android: 4 dp bar, 24 dp spinner; progress shown in a sheet or header |

Open: the track is `surface.sunken` with `border.subtle` in this spec, while PlanView uses `usage.ring.track` (X42); the indeterminate travel shows the step name after 30 s (not in 4.8); a static indeterminate bar under reduced motion conflicts with the 4.7 rule for functional progress.

#### 3.5.5 Skeleton: [Skeleton.md](components/Skeleton.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Loading (blocks shown after 200 ms in this spec; 100 ms elsewhere, X22); shimmer off in v1, static; resolved (replaced in place); empty (EmptyState); error (ErrorState); long content (not applicable; blocks match the expected length); disconnected (last known content, or ErrorState) | Not focusable; the region is the focus target | The region is the focus target | Blocks follow the container width; line widths 100, 80 and 60 percent; line height `size.skeleton.line` (12 px) | Static; no moving highlight; reduced motion is always static | Plain rounded divs in `fill.skeleton`; no gradient or opacity animation | Views with a `fill.skeleton` background and radius; `accessibilityElementsHidden` on the group; no native shimmer | Same shapes on both platforms: row 28 px on desktop, 48 dp on Android |

Open: the shimmer decision is open, because 4.7 describes it and no pending item decides it now (X49); the spec allows Skeleton in extension views, but the 7.1 allowed list omits it (X61); a later shimmer needs a highlight token per surface.

#### 3.5.6 Empty state: [EmptyState.md](components/EmptyState.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; loading (shows Skeleton instead); filtered (title names the filter, action "Clear search"); disconnected ("Host unreachable", no create action); error (ErrorState) | The action is the only focus target; Tab reaches it; Enter activates | The panel is not focusable; focus moves to the action only after a filter is cleared | The panel takes the region width; the explanation wraps to two lines (`layout.empty-state.text.max-width`, 60ch), then truncates; the title wraps before shrinking | None; the panel appears in place | `div` with a 1 px `border.default` and `radius.lg`; no shadow; the action is a normal Button | `View` with border and radius; title role header; 48 dp action; content centred vertically | Desktop: centred in the Canvas. Android: fills the screen, with the action at the bottom |

Open: the Work panel's 2 by 2 action grid becomes one column below 320 px (undecided); `size.icon.lg` is shorthand only.

#### 3.5.7 Error state: [ErrorState.md](components/ErrorState.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; retrying (spinner, "Retrying", Retry disabled); still failing (same panel, new reason); details open (mono block); recovered (content replaces the panel); disconnected (Banner; ErrorState only for failed content) | Retry is the first target; Tab reaches "Show details"; Enter activates | The panel announces once; focus stays on Retry after a failed retry | The panel takes the region width; the title wraps before shrinking; details scroll inside their block | None on appear; the retry spinner keeps turning under reduced motion | `div` with a danger tint and a 1 px danger border; glyph as SVG tinted `status.danger.fg`; selectable mono details; Retry is a normal Button | `View` with danger border and tint; title role header; assertive live region on page size only; 48 dp Retry and Show details | Desktop: section or tab body. Android: page fills the screen with Retry at the bottom; the section stays in its list |

Open: whether details show an error code by default (the spec says yes, in the details block only); "Retry" here and "Try again" in TerminalSurface (X44).

### 3.6 Lists, data and layout

#### 3.6.1 List row: [ListRow.md](components/ListRow.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover and pressed fills per surface; focus-visible 2 px inset ring; selected (2 px bar, `body-strong` title); disabled; loading (skeleton); error (`octagon-alert` glyph, never a red row); disconnected (dashed glyph, last-known line); status words (five listed, without Done, X4) | Up and Down move (one tab stop); Enter opens; Right and Left expand or collapse tree rows; Space selects in multi-select; Menu key or Shift+F10 opens the context menu | The list is one tab stop; top to bottom; the ring stays inside the row | The title truncates first, then the secondary line; the trailing slot stays longest; rows never wrap | Fills `duration.fast`, `ease.standard`; tree expand `duration.moderate`; reduced motion instant | Fixed-height virtualised rows per density; the selected bar is a 2 px child element, not a border; trailing actions drawn only on hover | `Pressable` in FlatList or FlashList with `getItemLayout`; two-line rows 56 dp; long press opens the M-WS-02 sheet with matching accessibilityActions | Desktop: hover actions, right-click menu, double-click rename, three densities. Android: no hover, long-press sheet, touch density only |

Open: the selected bar is `accent.default` here and `accent.fg` in Table (X43); the two-line Android row is 56 dp with a stale reference to pending item 23 (X48); `size.control.comfortable` and `size.touch.large` are shorthand; `fill.pressed.<surface>` and `fill.selected.<surface>` are templates; the ID D-SL-02 is the status bar item, not sidebar section rows (X45).

#### 3.6.2 Table: [Table.md](components/Table.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover and pressed canvas fills; focus-visible inset ring; selected (`fill.selected.canvas` plus an `accent.fg` bar); disabled actions shown disabled; loading (five static skeleton rows, proposed); empty (EmptyState); error (ErrorState replaces the body) | Tab enters; arrows move between cells; Home and End go to row ends; Enter opens a row; Space toggles a checkbox; Enter or Space sorts a sortable header | Sortable headers first, then rows; the ring is inset so the scroll area does not clip it | Columns keep a minimum width; wide tables scroll sideways in the scroll area; numeric columns right-aligned | None; sorting does not animate; the spec says no shimmer, which 4.7 does not settle (X49) | Fixed-height flex rows, virtualised when long; 1 px rules as border quads; ellipsis truncation; solid fills, no shadows | Below 600 dp it becomes ListRow lists with 56 dp two-line rows; sort in a Sheet (proposed) | Desktop: dense table, click-to-sort, horizontal scroll. Android: lists below 600 dp, vertical scroll |

Open: single-column sort and single-row selection only in v1; the D-WS-19 merge needs confirmation; the row IconButton uses a 24 px target, while 8.5 gives 28 px as the default.

#### 3.6.3 Card: [Card.md](components/Card.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; interactive hover and pressed fills; focus-visible ring, offset 2 px; selected (2 px `accent.default` leading edge, `fill.selected.raised`); disabled (`text.disabled`, `aria-disabled`); loading (skeleton card); empty (never rendered); error (ErrorState inside the card) | An interactive card is one target; Enter opens; Tab reaches inner actions after the card; arrows move between cards in a roving grid | Card order in the column; inside a card: header actions, then body controls, then footer actions | Fills its column; the body wraps; the title truncates; the body caps at 320 px | The summary insert uses `duration.card-insert` (250 ms) with `ease.enter`: grows, fades and rises 8 px; cards below slide with `ease.standard`; reduced motion instant | `div` with a raised fill and a 1 px border; `shadow.e1` as a box shadow (blur convention, S3); the insert animates height from 0 with scroll anchoring | `View` with border and radius; elevation 1 dp; interactive `Pressable` with role button; layout animation for the insert | Desktop: summary cards in a column beside the Conversation. Android: full-width cards in session detail, 16 dp gutters |

Open: whether settings groups carry `shadow.e1` (4.5 lists them at e1, and the spec matches); `layout.summary.default` is a value only (5.1).

#### 3.6.4 Divider and splitter: [Splitter.md](components/Splitter.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default (`border.subtle`, 1 px line); hover (`border.strong`, resize cursor); pressed (`accent.default`, 2 px while dragging); focus-visible (2 px `border.focus` line inside the hit area); disabled (not used); at limit (the line stays and drag stops) | Focusable separator; arrows move the edge 8 px (`space.2`); Shift with an arrow moves 24 px (`space.6`); Home and End to the limits (proposed) | Tab reaches it after the left region's controls; the focus line shows for keyboard focus only | Drag resizes; double-click resets; below the minimum it collapses at the minimum minus `SPLITTER_SNAP_PX` (40 px); the Conversation never goes below 360 px | No animation while dragging; collapse and reopen `duration.moderate`, `ease.standard`; reduced motion instant | 1 px line inside a 6 px hit element with a resize cursor; 2 px accent line drawn in the overlay; width applied once per frame | Not used. The master pane is fixed at 360 dp (`layout.android.master`); touch does not drag dividers in v1 | Desktop: drag, arrows, double-click reset. Android: no splitter; fixed master width |

Open: Home and End are proposed, and the spec cites a keyboard table that design-system section 8 does not contain (X55); the minimum and maximum region tokens exist only as values in 5.1.

#### 3.6.5 Scroll area: [ScrollArea.md](components/ScrollArea.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default (thumb hidden until scrolling or the pointer is over the area); hover and pressed thumb (`border.strong`); focus-visible inset ring when nothing inside is focusable; long content (thumb at least 24 px) | Up and Down scroll one row; Page Up and Page Down scroll one viewport; Home and End go to the ends | One stop on the viewport, only when nothing inside is focusable; the ring is inset | The thumb length follows the content; never below `size.scrollbar.min-thumb` (24 px) | The thumb fades in `duration.fast`, `ease.standard`; it hides `SCROLLBAR_HIDE_DELAY_MS` (1000 ms) after scrolling stops; scrolling itself is not animated | Framework scroll container with a custom 6 px rounded thumb in `border.strong`, no track; the fade is opacity | System scroll view and indicators; no custom Android scrollbar; tappable rows 48 dp inside | Desktop: thin overlay thumb with keyboard scrolling. Android: system indicators and basic hardware keys |

Open: overlay thumbs are hard to find on trackpads (the owner judges); web-view extension content ([P3]) would scroll outside host containers, which the spec does not cover.

#### 3.6.6 File tree: [FileTree.md](components/FileTree.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover and pressed canvas fills; focus-visible inset ring; selected (`fill.selected.canvas` plus a 2 px `accent.fg` bar); loading (tree-shaped skeleton); empty ("This folder is empty."); error (ErrorState; last list kept); filtered (`match.bg`); no match ("No files match your search.") | Up and Down move; Right expands or enters a child; Left collapses or goes to the parent; Home and End go to the first and last visible row; Enter opens a file or toggles a folder; letters jump; `Mod+Shift+F` searches files (K3); F6 reaches the tree | Search field, then the Names or Content toggle, then the tree as one stop | Fills the Project sidebar width; rows truncate, never wrap; deep trees scroll sideways in the scroll area | No expand or collapse animation in v1; hover and pressed fills `duration.fast`; reduced motion instant | Virtualised fixed-height rows; 12 px indent per level; git letter in a fixed right slot; `match.bg` behind text; no shadows | Not used on Android. The Android list is M-FI-01, built with ListRow; tablet reuse needs 48 dp rows and no hover | Desktop: keyboard tree with hover and git letters. Android: touch list (M-FI-01). Ignored files are dimmed on both |

Open: `R` (renamed) and `U` (conflict) need a letter and a token before Git status ships; single-click open is proposed; `size.touch.large` is shorthand; the "No match" wording differs from the EmptyState rule for filtered states (X62).

### 3.7 Work surfaces

#### 3.7.1 Diff view: [DiffView.md](components/DiffView.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; line and toolbar hover and pressed fills; focus-visible (2 px ring on the toolbar, inset ring on a line); selected (toggle fill and 2 px `accent.fg` underline); disabled at the first or last change, Edit when the host is unreachable, and narrow Split; loading (skeleton after 100 ms, spinner after 200 ms); empty, error, large, binary, conflict and disconnected banners | Tab enters the body and toolbar; Up and Down move the line cursor; Page Up and Page Down scroll; Home and End go to the file ends; Enter expands a block; Left and Right move toolbar buttons; F7 and Shift+F7 change (decided [P27]); Ctrl+Alt+M adds a note; Mod+F opens the find bar | Order: toolbar, review bar, body; the ruler is not focusable | Follows the Work panel width; below 640 px Split is unavailable and an open split falls back to inline; gutters stay sticky on horizontal scroll | Smooth scroll to a change (`duration.moderate`, `ease.standard`); reduced motion and block expansion instant | `uniform_list` with fixed 20 px rows; background and word quads per row; gutters painted apart from the code; diff computed off the render thread | FlatList of fixed-height rows; pinned gutter; tap a changed line for a comment sheet; 48 dp bottom bar for previous and next; sticky hunk headers | Desktop: Work panel tab with inline, split and rendered views, two gutters. Android: full screen, inline only, new-line gutter, tap to comment |

Open: syntax colours stay off changed lines [P34], but the spec cites a stale pending item (X48); the F7 keys are decided [P27], yet the spec still calls them proposed (X30); the file list and the More menu items are undesigned; the 640 px split threshold, the 2,000-character cut and the 20-line reveal are literals.

#### 3.7.2 Terminal surface: [TerminalSurface.md](components/TerminalSurface.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Starting (spinner and "Starting shell" after 200 ms); restoring; running (filled or hollow cursor); selected; exited (exit row; danger only on a non-zero code); error (ErrorState with "Try again"); reconnecting and restored (Banner; "restored" fades after 5 s); disconnected (last output with its age) | Every key goes to the process except the reserved F6, Shift+F6, Ctrl+Tab, copy and paste, and Mod+Shift+B; the shell keeps Ctrl+F, E, K, P, T and W while the terminal has focus [P12] (K1); Shift+Page keys scroll | Opening or clicking focuses it; F6 leaves; Work panel order is the tab strip, then the pane; the ring is inset 2 px at the pane edge | Rows and columns follow the pane after a 100 ms debounce; the core receives the new size; a size badge "120 x 32" shows for 800 ms | The spec says no motion, but the jump chip fades over `duration.base`; cursor blink follows the OS and stops under reduced motion | One custom element paints the grid, merging cells into runs; box-drawing drawn procedurally; IME cursor from the grid cursor; the emulator stays in the core | Renderer still open: WebView or native (S6). 12 sp mono; key row above the keyboard (44 dp keys, 48 dp hit); one-line input dock; "Read last output" reads the last 10 lines | Desktop: full keyboard, optional header, 13 px. Android: key row and input dock; the app bar carries the title |

Open: Shift variants Ctrl+Shift+F (find) and Ctrl+Shift+P (palette) are still proposed (K3); typed input while reconnecting is not covered; the Android renderer (S6); the screen-reader spike (S1); the OSC 52 clipboard rule [P11] is not in the spec; "Try again" differs from "Retry" (X44); the 100 ms debounce and 800 ms badge are literals.

#### 3.7.3 Browser chrome: [BrowserChrome.md](components/BrowserChrome.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover; pressed; focus-visible; selected; disabled; loading (2 px progress line); empty; error; client-hosted page; long content; disconnected host | `Mod+L` focuses the address bar; Enter navigates; Escape reverts the text or stops loading; `Mod+R`; `Mod+Shift+R`; `Alt+Left` and `Alt+Right` (`Cmd+[` and `Cmd+]` on macOS); `Mod+F` finds; `Mod+=`, `Mod+-`, `Mod+0` zoom the page (K5); F6 leaves the Work panel | Back, Forward, Reload, address, zoom chip, Find, Grab, agent chip, More, then the page; arrows rove over the first three buttons | The address bar flexes (minimum 160 px); buttons never shrink; below 400 px Find and Grab move into More | Progress line `ease.linear`; hover fills `duration.fast`; reduced motion shows a static partial bar | A native web view draws above GPUI, so overlays need a decision: a texture, a hidden view, or toolbar-only overlays (S5) | Not part of Android v1; links open in the system or an in-app browser (M-ST-04) | Desktop only |

Open: overlay layering depends on the browser-engine decision (S5, and [P3] for extension web views); the agent chip and "Take over" are proposed; history and suggestions are not designed; the Find bar placement conflicts with FindBar (X39); the spec cites contribution point 13 for page-link menus, which 7.2 does not name (X63); the 2 px progress line has no token (`size.progress.height` is 4).

#### 3.7.4 Calendar views: [CalendarViews.md](components/CalendarViews.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover; pressed; focus-visible; selected or active; disabled (not used); loading; empty; error; done; long content; disconnected host | Header controls, then filter chips, then the grid as one stop; arrows move between days and entries; Enter opens the active entry | Header, chips, grid; focus-visible ring on an entry or a day | Columns share the width; the week scrolls in a scroll area when seven columns do not fit; the day view is always one column; a day column's minimum is 96 (`layout.calendar.column.min`) | The view switch cross-fades over `duration.base`; reduced motion instant | Flex columns; a fixed time gutter; entries positioned absolutely on the hour scale; no shadow on entries | Full screen from the overflow menu; compact shows the Day view with a day pager; medium and expanded show Week (proposed); entries 48 dp | Desktop: Day, Week and Month, with hover. Android: Day and Week from medium width (proposed) |

Open: routine and automation icons are proposed; "Show done" is off by default [P22]; automation entries open D-AG-14; the Android agenda [P4] differs from the day pager; entries use `surface.raised` with `border.subtle`, where 2.2 and 4.5 call for `border.default` (X64).

#### 3.7.5 Board and kanban column: [BoardColumn.md](components/BoardColumn.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover; pressed; focus-visible; selected or active; disabled (not used); loading; empty; error; long content; disconnected host | Space picks up and drops the card; Left and Right move it to the next column; Up and Down move it within the column; Escape cancels to its original slot; moves are announced | Column by column, top to bottom; headers are not stops; the ring is inset at column edges | Columns fixed at `layout.board.column.width` (280); the Board scrolls sideways | Drop slides in `duration.fast`, `ease.standard`; the lifted card follows the pointer without easing; reduced motion instant | Fixed-width columns on `surface.sunken`; raised cards with `shadow.e1`; drag in the overlay layer; no blur | Not on Android in v1, although [P4] draws the Board for the phone (X23); if added: status sections and a Move action (proposed) | Desktop: side-by-side columns, with drag and keys. Android: none in v1 |

Open: four columns are proposed; the automatic move to Done is undefined; Enter on a card has no target.

### 3.8 Command, search and settings

#### 3.8.1 Command palette: [CommandPalette.md](components/CommandPalette.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover; pressed; focus-visible; selected or active; disabled; loading; empty; no match; error; long content | `Mod+K` opens; `Mod+P` opens quick open; `Mod+Shift+F` opens search in files; Up and Down move; Enter runs; Escape closes to the opener; Tab stays in the input; in a terminal, Ctrl+K and Ctrl+P go to the shell (K1) | Focus trapped as a dialog; the input keeps focus; the active row is shown through active descendant; Escape returns focus to the opener | Fixed 600 px width (`layout.palette.width`); top offset 96 px (`layout.palette.top`); maximum height 480 px; narrow windows use the window width minus `space.4` | Opens with scale 0.98 to 1 and an opacity change over `duration.moderate`, `ease.enter`; closes with `ease.exit`; reduced motion instant | Scrim quad; the panel sits at `z.dialog` (absent) in the overlay layer; IME-capable input; virtualised results | Not used on Android in v1. If needed, a sheet with search and ListRow results | Desktop: centred overlay with the full keyboard. Android: none in v1 |

Open: the mode entry is undefined (`Mod+P`, `Mod+Shift+F` and a palette entry are proposed); five recent items are proposed; the width of 600 does not match the dialog widths in [P36] (X56).

#### 3.8.2 Find bar: [FindBar.md](components/FindBar.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Closed; default; matches; no results; hover; pressed; focus-visible; disabled; long content | `Mod+F` opens with the text selected; Enter goes to the next match; Shift+Enter to the previous; Escape closes; Tab moves through the input and buttons; in a terminal Ctrl+F goes to the shell [P12] (K1) | The input is focused on open; order: input, previous, next, close; Escape returns focus to the pane's previous control | Width `layout.findbar.width` (360); a narrow pane shrinks the input first; the count never overlaps the buttons | Opacity 0 to 1 over `duration.base`, `ease.enter`; closes with `ease.exit`; reduced motion instant | Overlay element at the pane's top right, inset `space.2` (8 px); `match.bg` and `match.active.bg` runs; `shadow.e2`; no blur | Not on Android in v1; WebView uses the platform find, which is not designed here | Desktop: a strip in each searchable pane. Android: none in v1 |

Open: browser counts depend on the engine's find API; there is no cap for large terminal counts (for example "999+"); the placement conflicts with BrowserChrome (X39); `Mod+F` also focuses the Settings search (K4).

#### 3.8.3 Settings row and section: [SettingsRow.md](components/SettingsRow.md) · B

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover; pressed; focus-visible; selected; disabled; loading; saving; saved; error; modified; restart required; empty; long content (the label wraps to two lines, which 3.2 forbids in rows, X65); disconnected host | Tab moves in label order; Space toggles a Switch; arrows operate Segmented, Radio and Slider; Enter opens Link rows and applies text and number edits; Escape reverts; `Mod+,` opens Settings; `Mod+F` focuses the settings search (K4) | Settings navigation, then search, then rows top to bottom; the ring sits on the control, never on the row | Stacked variant below 560 px; no horizontal scroll; the control moves below the label when it does not fit at 200 percent font scale; control column `layout.settings.control.min` 160 and `max` 320 | Saved check fades over `duration.base`; search highlight fades after 2 s; reduced motion instant | Flex row with a label column and a control slot; the settings store computes "modified" and emits "saved"; search reuses the Find match run type | `SectionList`; rows 56 dp, and 72 dp with a helper line, which are not tokens (4.2 gives 40 and 48); inline Switch; Select and text open a sheet or screen | Desktop: label left, control right, grouped card, key recorder, extension sections. Android: full-width rows; Settings from the Home top bar [P21]; no extension sections |

Open: auto-apply for all rows is proposed (sign-in and host pairing stay explicit); the settings search is assumed (D-ST-01 is silent); "Managed" delivery is undefined; `layout.page.settings` is absent (5.1 prose gives 880).

#### 3.8.4 Extension card: [ExtensionCard.md](components/ExtensionCard.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Default; hover; pressed; focus-visible; disabled; loading; installing; error; long content; disconnected host. Variants: catalog, installed, update available, incompatible | The card is a list item, not a tab stop; Tab order: name link, primary, Settings, More; Space toggles the Switch; arrow keys do not move between cards, although 8.3 makes composite controls rove (X66) | Row by row, left to right; the ring sits on the control, not the card | Grid of 1 column under 560 px, 2 from 560 and 3 from 880, inside `layout.page.default` (1120); minimum 280 px; no horizontal scroll | Fills `duration.fast`; the Switch uses its own motion; the progress bar fills without easing; reduced motion instant | Fixed-height cards in `uniform_list` rows; shared Switch; skeleton quads, with no shimmer under reduced motion | Not designed; desktop only; the Marketplace is not on the phone, which conflicts with [P5] (X24) | Desktop only |

Open: no verified-publisher mark; the Marketplace also lists tools, agents, skills and services; name (40 characters) and description (160 characters) limits are proposed.

#### 3.8.5 Permission review: [PermissionReview.md](components/PermissionReview.md) · D

| States | Keyboard | Focus | Resizing/responsive | Motion | GPUI build notes | React Native build notes | Platform differences |
|---|---|---|---|---|---|---|---|
| Reviewing; hover; pressed; focus-visible; selected; disabled; installing; installed; failed; rollback; loading; empty; error; long content; disconnected host | Focus trapped; initial focus on the title; Tab order: expanders, Install target, acknowledgement, Cancel, primary; Escape cancels except while checking (X37); Enter installs when enabled | Initial focus on the title heading; close returns focus to the card that opened it; success moves focus to its Switch | Fixed width 560 (`layout.dialog.width.lg`) [P36], or the window width minus `space.4`; header and footer pinned | Opacity and scale 0.98 to 1 over `duration.moderate`, `ease.enter`; progress bars fill without easing | Shared Dialog; core computes the risk level from the manifest; the primary action stays disabled until acknowledgement and reachability | Not designed; extensions are not installed from the phone in v1 | Desktop only |

Open: the risk-level rules and the high-risk acknowledgement are proposals under [P11]; permission kinds beyond the four listed are not final; there is no verified-publisher state; the web view is not listed [P3]; `size.avatar.lg` is shorthand only.

## 4. Extension contribution points (14)

Source: design-system 7.2 (the table), 7.1 (rules) and the specs that render each point. Rules that apply to every point (7.1): an extension returns a declarative view description validated against a schema; it uses only semantic tokens and cannot define colours, fonts or raw pixels; it never takes focus on its own; contributed items show the extension's name in their tooltip and accessible description; a suggested shortcut that clashes with a core key loses; each contribution declares `modes` and `surfaces`; the host enforces limits (ellipsis, overflow, and over-limit manifests fail validation at install); permissions are shown in Permission review before install (D-EX-02).

The foundations do not define what happens when an extension crashes or is slow: there are no timeout values and no placeholder rules. The last column is a proposal and is marked as one. Settling it is part of the extension runtime decision (design-system 10, "Not covered"; app-flow surface-crash rule).

| Point (inventory ID) | Renders where | What the extension provides | Size and count limits | Can change | Cannot change | Degrades to (proposal) | Android |
|---|---|---|---|---|---|---|---|
| 1. Activity rail item (D-SL-09) | Rail, in an Extensions group under the top group, after a divider; excess goes to a More button | Icon (Lucide name or single-ink SVG); label; optional badge (dot, or count 1 to 99+) of kind neutral, accent or danger; a target: full-canvas page, Sidebar section or command | One per extension by default, three with user consent; label 32 characters | Its icon, label, badge count and badge kind | Restyle or resize the rail; sit above core items; animate; open itself; set badge colours | Keeps its slot with an error state and its label; the rail does not wait for it | None (no rail on Android) |
| 2. Sidebar section (D-SL-05) | Sidebar, below the workspaces or teams tree, as a collapsible section | Header (icon, label, up to two header icon buttons); body of Sidebar rows or a List | Header label 24 characters; row label 80 characters, then ellipsis; up to three trailing actions per row; body virtualised | Its rows, row icons, badges and actions; the default collapsed state | Restyle rows; add rows outside its section; expand itself on update; show in the other mode unless declared | Section body shows ErrorState; the tree and other sections keep working | None in v1 |
| 3. Work panel tab type (D-SL-08) | Work panel: the `+` menu and its own tabs (not among the four empty-state actions) | Tab type id, icon, label, view description, dirty marker, state to restore | Tab title 24 characters; the view fills the tab body | The tab's content and title | Replace or hide the four core tab types; take terminal focus; draw outside the tab body | Tab body shows ErrorState; other tabs and the terminal keep running | None (no Work panel on Android in v1) |
| 4. Project sidebar tab (D-SL-01) | Project sidebar tab strip, after the four core tabs (Files, Sessions, Source control, Checks) | Icon (the label is the tooltip and accessible name); optional count badge; view description | Two tabs per extension; label 24 characters | Its tab content and badge | Reorder before the core tabs; hide core tabs; change the region width | As point 3 | None in v1 |
| 5. Session summary card | Session summary column, after the core cards, in core card style | Card title; optional right-aligned value; collapsed body (up to six rows); optional detail view; a "has information" signal | Title 24 characters; value 12 characters; three cards per extension | Its card, which appears only when the extension has information (standard insert animation) | Show an empty placeholder; reorder or hide core cards; force itself open; animate beyond the standard insert | Card stays hidden until it has information; errors show inside the card | None in v1 |
| 6. Conversation card (D-SL-03) | Transcript, as a turn block | Header (icon, title); view description; up to three actions, sent as a message through the protocol | Up to 320 px tall, then "Show more"; three actions | Its card content | Edit or delete other messages; appear as the user or the agent; run an action without a user click; receive secret values | Card shows ErrorState in its own block; the transcript is unaffected | None in v1 |
| 7. Status bar item (D-SL-02) | Status bar, left or right group by priority | Icon (12 px); caption text (tabular); optional tooltip view; click command; state kind (info, success, warning, danger) | Two items per extension; text 24 characters; priority 1 to 100 (the lowest hides first when narrow) | Its text, icon and state kind | Change the bar's height or colours; render views in the bar (tooltip only); blink or animate | Item shows a danger state with its label; the bar is unaffected | None (no status bar on Android) |
| 8. Command (D-SL-07) | Command palette (D-CP-01) and keybinding editor (D-ST-04) | Id, title, category, optional icon, `when` conditions, suggested keybinding, argument form | Title 60 characters; category 24 characters | Its commands and suggested shortcuts | Run on its own; override a core keybinding without consent; hide itself from the palette; take focus beyond what its target declares | Unavailable commands show disabled; a failed run shows its error in the palette | None in v1 |
| 9. Settings section (D-SL-06, D-EX-04) | Settings page, as its own section under the Extensions group (D-ST-09) | Settings rows: toggle, text, number, select, keybinding, list; secret references (vault links, never values) | 40 rows per section; labels 60 characters | Its own settings and defaults | Edit other sections; restyle rows; show or store a secret value; open itself | Section shows ErrorState; the Settings page keeps working | None in v1 (no extension sections on Android) |
| 10. Full-canvas page (D-SL-04) | The whole Canvas, like Tasks and Calendar; the Title bar shows Back and the page title | Page id, title, icon, view description filling the content area | Title 40 characters; width fluid up to `layout.page.default` unless declared fluid | The page's content | Change the Frame, the Title bar controls or the Status bar; hijack Back; open without a user action | Page shows ErrorState; Back keeps working | None in v1 (desktop only, [P3]) |
| 11. Composer action | Composer bottom row, between attach and the Model picker (as an icon button), or the `+` menu | Icon; label (tooltip); optional popover view up to 320 px wide; an action on the draft or attachments | Two per extension; label 32 characters | What it does to attachments or draft text, through a visible step | Send a message; change the draft silently; move or hide Send or the Model picker | The action button shows ErrorState; the draft is untouched | None in v1 |
| 12. Model picker entries (D-SL-10) | Model picker (X-AG-02): an icon in the agent rail and rows in the list; reasoning levels in the right column | Agent or provider (icon 20 px, name); models (name, provider icon, reasoning level names, speed and access modes); a usage ring slot request | Model name 40 characters; level names 16 characters | Which agents and models it offers | Take Ctrl or Cmd 1 to 4 (7.2 says those follow favourites; K7); change the picker layout; become the default without consent | Entries are hidden with a notice; core models stay | None in v1 |
| 13. Context menu items | The named menus (workspace row, session row, file row, tab, message, selection, diff line), in a trailing Extensions group after a divider | Label; optional icon; `when` conditions; a command | Label 40 characters; five items per extension per menu; one submenu level | Its own items | Remove, reorder or precede core items; run a destructive action without the core confirm (X-DLG-01) | Items are hidden on error; the menu keeps its core items | None in v1 |
| 14. Notification and toast (X-NT-01, D-NT-01) | Toast stack at the bottom right of the Canvas, and the notification card stack | Kind (info, success, warning, danger); message; at most one action; for cards, a title, a body and up to two actions | Toast 120 characters; card title 60 and body 160; at most one toast per 5 s per extension and three visible | The message and its action | Take focus; play sound; bypass notification settings or do-not-disturb; imitate core messages (the extension name always prefixes the text); send OS notifications without per-extension permission (X-NT-02) | Failed extension messages are dropped and logged; the stack is unaffected | None in v1 (no extension contributions on Android) |

Notes:

- The "Degrades to" column is a proposal. The sources define no timeouts, so a timeout value is still to be set (6.4).
- Extension views may use only the components listed in 7.1 (stack, row and grid layout, Text, Icon, Button, Icon button, Segmented control, Toggle, Checkbox, Radio, Text field, Select, List and List row, Table, Card, Badge, Progress, Empty state, Error state, Banner, Tabs, Divider, Markdown block, Code block). Terminal surface, Browser chrome, Frame, Activity rail, Dialog and Sheet are not allowed; Dialog and Sheet open through the host API so the core controls them (X61 concerns Skeleton).
- Themes are a contribution too (7.3): they go through the same validator as user files and appear in Settings, Appearance.
- Extension web views are the open question in [P3]. Desktop only, as a Work panel tab or a full-canvas page, if the owner accepts them.

## 5. Build order

Phases are from docs/PLAN.md. Each wave depends on the waves above it. Items inside a wave are independent and can be built in parallel. Before a component is built, the owner decisions and spec conflicts listed against it in 6.2 and 6.3 must be settled, and the spikes in 6.1 must have answers for the parts that need them.

### 5.1 Phase 5: desktop in GPUI

PLAN.md: "Window, workspace sidebar, terminal panes, agent status. Then editor and diff views." Exit: "daily work is possible in the GPUI app on the first-version scope."

| Wave | Build | Depends on | Gate or decision first |
|---|---|---|---|
| 0. Gates | Spikes only; no UI | Nothing | S2 before any text-heavy primitive; S3 before Popover, Menu, Dialog, Toast and Card; S7 before Composer and rename fields; S4 before the conversation list; S5 before browser chrome and any overlay that can sit over a browser pane. S1 runs alongside and does not block layout work. |
| 1. Foundations | Token loader from one generated `tokens.json`; theme validator V1 to V12; motion module (four curves, durations, reduced motion); behaviour constants (4.8); focus manager (ring, F6 cycle, trap, restore); keyboard map with per-OS Mod mapping and the Kbd renderer; type scale with tabular figures (3.2, 3.3); Lucide SVG loader (4.6); region model (widths and collapse order, 5.1) | Wave 0 for S2 | Resolve X1 and [P8] before the region model is fixed; expand the shorthand tokens (6.4). |
| 2. Leaf primitives | Button; Icon button; Kbd; Badge; indicator dot (6 px); agent status glyph; Avatar; Switch; Checkbox; Radio; Segmented control; Text field; Progress; Skeleton (static); List row; Scroll area; Splitter; Empty state; Error state | Wave 1 | X11, X12, X3 (row heights); X21 and X22 (spinner and skeleton delays). |
| 3. Overlay layer and feedback | Popover; Menu; Tooltip; Dialog; Toast and notification-card stack (one service); Banner; Select (needs Menu); Picker (needs Popover and Empty state); Search field (needs Text field); Find bar | Waves 1 and 2; S3 | K8, X18, X19 (one dismiss rule and one minimum width for the shared stack); X35 (Menu width); X53 (disabled reasons); X36 (chord order). |
| 4. Shell | Frame and Canvas; Title bar (per OS); Region toggle; Activity rail with Rail item, Usage ring and Mode switch; Sidebar tree and row (needs Agent status); Project sidebar tabs; Work panel tabs; Status bar and Status item | Waves 1 to 3 | [P2], X2 (title bar and full pages); X5 and [P6] (mode switch); K2 and X26 (macOS Cmd+W); [P12] and X27 (tab close); X57 (rail item in the list). |
| 5. Session core | Conversation list with Message blocks and Minimap; Queued message; Composer (after S7); Model picker; Session summary card; Plan view; Delegation tree; Secret request card; Secret modal | Waves 1 to 4; S4, S7 | X15 and [P25] (composer queueing); X9 and K7 (keys 1 to 4); X25 and [P23] (depth); X59 (focus after secret modal); [P11] (approvals). |
| 6. Work surfaces and settings | Diff view; File tree; Terminal surface; Browser chrome (after S5 and [P3]); Calendar views; Board column; Command palette; Settings rows; Extension card; Permission review | Waves 1 to 5 | K1 and [P12] (terminal keys, the shell keeps Ctrl+K); [P27] and X30 (diff keys); K3 and X28 (Ctrl+Shift+F); X23 (Board); X24 (Marketplace); X37 (Escape in Permission review); X56 (palette width); X65 (settings label wrap). |
| 7. Extension hosts | The 14 contribution points (section 4), rendered by the shell and the primitives | Waves 1 to 6; [P3] answered | The extension runtime (design-system "Not covered"); the degradation rule (section 4 notes). |

Note: Wave 6 contains the largest risk. The terminal surface paints its own grid and depends on S2 and the text engine. Plan it as a separate piece of work with its own review.

### 5.2 Phase 6: Android in React Native

PLAN.md: "React Native UI on top of `client-ffi`. Pairing with a server, workspace list, terminal and agent status." Exit: "the Android app drives a server for the first-version scope."

| Wave | Build | Depends on | Gate or decision first |
|---|---|---|---|
| 0. Gates | Spikes and checks; no UI | Nothing | S6 (terminal renderer) before the terminal screen; keyboard and IME insets for the Composer; tabular figures through `fontVariant`; 200 percent text on fixed layouts; `react-native-svg` cost for agent glyphs and usage rings; `FLAG_SECURE` on secret entry. |
| 1. Foundations | The RN theme object from the same token source; sp type scale; edge-to-edge layout and safe areas; 56 dp app bar; motion module with "Remove animations"; Dialog and Sheet on `Modal` (sheet maximum 90 percent; back closes unless busy); toast overlay; theme follows the system with a manual override | Wave 0 | [P35]; [P9]; X38 (keyboard-open sheet height); X34 (touch spacing). |
| 2. Primitives | Button (48 dp hit); Icon button; Switch; Checkbox; Radio; Segmented control (a sheet above three options); Text field; List row (56 dp two-line); Badge (20 dp); Avatar (40, 28 and 20 dp); agent glyph; Progress; Skeleton; Empty state; Error state; Banner | Wave 1 | X14 (Android glyph sizes); X4 (state words). |
| 3. Phase 6 scope first | Pairing (add host with connection test, [P24]); workspace list (M-WS-01); host card with usage ring; agent status rows; Home top bar with the mode switch (cross-fade); Settings (SectionList); Marketplace row ([P5]) | Waves 1 and 2 | [P21], [P24], [P5]; X24. |
| 4. Session screens | Conversation list (FlashList with `estimatedItemSize`); message blocks; Composer (multiline, IME-aware); queued messages (sheet rows); plan (sheet); delegation tree (flat sheet); Model picker (sheet); session summary as sheets; Secret request card and secret modal (sheet, `FLAG_SECURE`) | Waves 1 to 3; S6 not needed | X15, [P25]; X16 (queue removal confirm); X9. |
| 5. Remaining Android screens | Terminal with key row and input dock (after S6); diff full screen (inline only); file list (List row); calendar (day pager and agenda, [P4]); Bot Teams screens and Board ([P4], X23) | Waves 1 to 4; S6 | [P4]; X23. |

Android v1 has no Work panel, no extensions, no status bar and no rail (5.4, 7.1, section 9). Do not build them for Android in this phase.

A wave is done when its components match the state tables in their specs and the reduced-motion, keyboard (desktop) and TalkBack (Android) rules in section 2 hold for them.

## 6. Open engineering questions, decisions and conflicts

### 6.1 Spikes

Each spike has one question, the parts it blocks, and what has to be established. Run them before the waves in section 5 that depend on them.

| ID | Question | Blocks | What to establish |
|---|---|---|---|
| S1 | Does GPUI expose an accessibility tree on Windows, macOS and Linux, so the roles and names in 2.9 can reach screen readers? (design-system 10, question 1) | Screen-reader names for every component on desktop | Whether the tree exists or can be added. If not, v1 is keyboard-complete and high-contrast, but not screen-reader-complete on desktop. |
| S2 | Font features: static Geist and JetBrains Mono weights (3.1); Geist `tnum` (3.3); GPUI font features; React Native `fontVariant` | Status bar, usage rings, diff counts, timestamps, tables | Tabular digits align at every size; the weights used match the bundled static files on each toolkit. |
| S3 | Shadow blur convention: design shadows (`offset-x offset-y blur spread colour`, design-system 4.5) mapped to GPUI box shadows | Popover, Menu, Tooltip, Dialog, Toast, Card (`e1`) | The blur radius maps one to one. If it does not, adjust the shadow token table once. |
| S4 | Text selection across blocks: conversation, Markdown, diff | Conversation list, message blocks, plan view | Cross-block selection in GPUI. Until it exists, selection stays per block (ConversationMessageList open point). |
| S5 | Native web view z-order over GPUI overlays | Browser chrome; any overlay that can sit over a browser pane; extension web views ([P3]) | Texture, hidden view, or toolbar-only overlays (BrowserChrome.md, GPUI notes). |
| S6 | Android terminal renderer: WebView with a web terminal, or a native text grid (app-flow G17) | Terminal surface on Android; key row and input dock | Choose one. The spec describes both. |
| S7 | Composer IME on desktop: Windows TSF, macOS NSTextInputClient, Linux IBus and Fcitx (Composer.md, GPUI notes) | Composer, search fields, rename fields | A shared IME-capable input works on all three systems. |

### 6.2 Owner decisions that gate build

These are the items in pending-owner-review.md (37, current numbering). Each is provisional until the owner confirms it. Confirm them in one pass, starting with the ones that gate the most parts.

| Decision | What it changes | Wait before building |
|---|---|---|
| [P1] Prototypes is core, with its entry in the Workspaces Sidebar | Sidebar tree; extension catalogue | Sidebar tree (X50) |
| [P2] The Title bar stays as a band of the frame | Region model; title bar layout | Title bar; Frame and Canvas |
| [P3] Extensions may show a sandboxed web view (desktop only, Work panel tab or full canvas) | Browser chrome; extension views; permission review; S5 | Browser chrome; extension hosts |
| [P4] Bot Teams on Android: team list, Conference room, Board, Routines and Calendar agenda | Android screens; Board column (X23) | Android wave 5; Board |
| [P5] Marketplace on Android is a row inside Settings | Extension card; Android Settings (X24) | Extension card; Settings on Android |
| [P6] Desktop mode switch: the first press returns home from other pages | Mode switch (X5) | Mode switch |
| [P7] Session summary is never hidden automatically | Session summary card; title bar toggle | Session summary card |
| [P8] Six regions need 1528 px; collapse order | Frame and Canvas; region model; minimap threshold (X1) | Region model |
| [P9] Geist and JetBrains Mono; accent `#7461d8` dark and `#6447c9` light | Type tokens; accent seed | Type and palette tokens |
| [P10] Light theme: frame `#eeeef2`, canvas `#fbfbfc` | Light token values | Light theme values |
| [P11] Security defaults: no keyboard shortcut for approvals; terminal clipboard writes (OSC 52) blocked until allowed; high-risk extension install needs an acknowledgement; warning glyph on the composer chip in Full access | Message blocks; terminal surface; permission review; composer chip | Permission review; terminal; composer |
| [P12] Terminal keys on Windows and Linux; Mod+W closes the active Work panel tab, never the window | Terminal surface; work panel tabs; title bar; palette (K1, K2) | Terminal surface; work panel tabs |
| [P13] Mode homes: Workspaces returns to the start page; Bot Teams to the team list | Mode switch behaviour | Mode switch |
| [P14] "MassOrca Mobile" in the Workspaces Sidebar opens the phone pairing page on desktop | Sidebar entry target | Sidebar tree |
| [P15] No project-wide Artifacts list; artifacts live in the Results card | Session summary card | Session summary card |
| [P16] Bot Teams pages keep the Sidebar; other full pages hide it | Full-page layout; title bar (X2) | Frame and Canvas; title bar |
| [P17] Extension manager in Marketplace > Installed (owner, A68), the Vault inside Settings | Settings rows; secret modal entry | Settings rows; secret modal |
| [P18] No agents overview page; the Sessions tab and a Status bar popover cover it | Status bar | Status bar |
| [P19] No notification centre or history; the Sidebar bell expands the card stack | Sidebar bell; notification cards | Notification card |
| [P20] First-run wizard and session import are full-canvas pages | Full-page layout | Frame and Canvas |
| [P21] Android rail entries move to the Home top bar, the overflow and the host card | Home top bar; host card; Android navigation | Android waves 3 and 4 |
| [P22] New routine opens the simple form; Calendar shows routines and automations; "Show done" off by default | Calendar views; routine form | Calendar views |
| [P23] Team creation is a dialog; delegation depth default 3, range 1 to 5 | Delegation tree (X25) | Delegation tree |
| [P24] Add host stays disabled until the connection test passes; a protocol mismatch blocks only that host | Pairing on Android | Pairing screen |
| [P25] A message to an unreachable host stays in the composer; it is not queued | Composer; queued message (X15) | Composer; queued message |
| [P26] Web addresses show a masked token; Copy gives the full URL | Web address rows in Settings and automations | Settings rows |
| [P27] Choosing a model keeps the Model picker open; diff next and previous change are F7 and Shift+F7 | Model picker; diff view (X30) | Model picker; diff view |
| [P28] Android source control commits all changes (no staging) | Android source control | Android source control (outside this handoff) |
| [P29] Workspace label colours come from the usage colours, never from status colours | Sidebar row label colour | Sidebar row |
| [P30] Six agent states: running, waiting, idle, error, unreachable and done | Agent status; avatar; list row; delegation tree; message blocks (X4) | AgentStatus and every state list |
| [P31] Sizes: 6 px dots, 16 px badges, 28 px Work panel tabs, 12 px Status bar icons | Icon button (X12); badge; Work panel tabs; Status bar | Those components |
| [P32] Agent and provider marks are full colour inside their usage ring | Activity rail; usage ring; avatar | Usage ring; avatar |
| [P33] Android ripple is the only translucent fill besides the dialog scrim and shadows | Buttons on Android; rail item ripple (X7) | Button; rail item |
| [P34] Syntax colours stay off changed diff lines | Diff view; message blocks | Diff view |
| [P35] v1 ships dark and light themes and a validator; no high-contrast or colour-blind theme; Android follows the system with a manual override, no dynamic colour | Theme set; accessibility claims (X31) | Theme validator scope |
| [P36] Dialog widths 400, 440 and 560; popover widths 280, 320 and 360; behaviour constants are fixed values | Dialog; secret modal; permission review; popover; picker (X56) | Dialog family |
| [P37] UI copy uses the glossary names; code names such as "side panel", "Chat summary", "right sidebar" and "Jev" never appear | Every visible string | Copy across all components |

### 6.3 Spec conflicts

Open conflicts: none. The 66 rows X1 to X66 were resolved on 2026-11-12 in the source order (pending-owner-review items 1 to 37, then the glossary, then design-system.md, then the component specs). Each decision is in 6.5; the spec files hold the resolved text. Keyboard collisions K1 to K8 are listed in 2.3, and each is marked resolved there.

### 6.4 Token gaps and literal values

- **tokens.json** exists: `docs/design/tokens.json` is the repository copy of the token contract, an exact copy of the token file after the fixes in this pass. design-system.md and this file say so.
- **Absent tokens, resolved.** Every token listed here before is defined: `border.width.thick`; `z.base`, `z.sticky`, `z.popover`, `z.dialog` and `z.toast`; `layout.sheet.max-height`; `layout.page.default` and `layout.page.settings`; the region tokens `layout.sidebar.*`, `layout.summary.*`, `layout.workpanel.*`, `layout.project.*` and `layout.conversation.min` and `.reading`; `layout.window.min-width` and `.min-height`; `layout.android.master`; and the Android type styles `a-body`, `a-chat` and `a-mono`. Added in this pass: `progress.track`, `agent.waiting.bg.overlay`, `agent.error.bg.overlay`, `agent.unreachable.bg.overlay`, `size.composer.start-min-height`, `layout.toast.width.min`, `layout.toast.width.default`, `layout.toast.width.max` and `terminal.min-contrast` (in a `contrast` group). `layout.palette.width` is now 560. There is no single menu token: Select uses `layout.menu.min-width` and `layout.menu.max-width`.
- **Shorthand tokens, expanded.** The slash forms in design-system 4.2 are one row per token. Every dotted name in the specs resolves in tokens.json (checked; see Checks run).
- **Literal values, resolved.** The 100 ms skeleton delay, the 200 ms spinner delay and the 8 s Undo timeout are in 4.8 as `SKELETON_DELAY_MS`, `SPINNER_DELAY_MS` and `UNDO_TIMEOUT_MS`. The 6 s Undo window and the 8 s action toast are the same rule now.
- **Literal values, still open.** A 300 ms approval click guard; a 5 s "Restored" banner; an 800 ms size badge and a 100 ms terminal resize debounce; the 640 px split threshold; the 2,000-character paste cut; the 20-line reveal; a 300 ms draft save; a 3 s composer hint; the 160 px search threshold; the 12 and 40 option limits in Select.
- **Android sizes in dp, still open.** Settings rows at 56 and 72 dp (4.2 gives 40 and 48); the 48 dp menu row (4.2 gives 44 dp as the default); the 20 dp checkbox and radio boxes.

### 6.5 Resolved conflicts

Source order applied: pending-owner-review items (1 to 37, current numbering, not edited), then glossary, then design-system.md, then component specs. Where no owner item decides a row, the choice follows design-system.md and the majority of specs; those rows say "derived" and should be confirmed in one pass.

| X# | Resolution |
|---|---|
| X1 | Breakpoints recomputed to 1528, 1288, 1048 and 848 px, with the Conversation at 480 px (the minimap width). Derived from the 1528 px owner figure, not stated by the owner: confirm. Updated design-system 5.1, FrameCanvas. [P8] |
| X2 | Bot Teams pages keep the Sidebar; other full pages hide it. design-system 5.1. [P16] |
| X3 | Two-line rows are 44 px on desktop and 56 dp on Android; 48 dp (`size.touch.large`) is for primary actions only. design-system 4.2. |
| X4 | Done is one of the six state words in 8.4; ListRow lists it. [P30] |
| X5 | Desktop: the first press from another page returns to the mode home; on Android one press switches. ModeSwitch. [P6] |
| X6 | Active rail icon is `accent.fg` (4.6 rule kept); RailItem follows. Derived from 4.6. |
| X7 | Rail ripple is `fill.ripple`, the only translucent state fill. RailItem. [P33] |
| X8 | Region toggle icon stroke is 1.5 px at 16 px (4.6). RegionToggle. |
| X9 | Mod+1 to Mod+4 select the first four rows of the view shown; the favourites reading is dropped. design-system 7.2 point 12 and ModelPicker agree. Set by the instruction for this pass. |
| X10 | Trigger chip uses `size.control.default` (28) in default density and `size.control.comfortable` (32) in comfortable density. ModelPicker. |
| X11 | Settings row height is 40 px (`size.row.settings`), not the 32 px control height; Switch updated. |
| X12 | Status dot is 6 px (`size.indicator.dot`). IconButton. [P31] |
| X13 | Gap between stacked cards is `space.3` (4.1 says so now; `space.5` is large control padding only). Derived from Card and SessionSummaryCard, the majority. |
| X14 | The Android icon rule adds 12 dp for status glyphs in rows; AgentStatus keeps 12, 16 and 20 dp. design-system 9. |
| X15 | A message to an unreachable host stays in the composer; it is not queued. Composer, ConversationMessageList. [P25] |
| X16 | On Android, Remove asks for confirmation first (5.4 rule); on desktop it removes at once with Undo. QueuedMessage. |
| X17 | Undo toasts time out after 8 s on every platform (`UNDO_TIMEOUT_MS`, 4.8). Toast and QueuedMessage updated; the 6 s value is gone. |
| X18 | Escape dismisses a focused toast or card; Delete and Backspace do not. One dismiss rule on the shared stack (K8). Toast, NotificationCard, 8.3. |
| X19 | Toast and Notification card share one width set: 320 minimum, 360 default, 400 maximum (`layout.toast.width.*`, 5.5). |
| X20 | Banner placement uses `ease.standard` (a move between two on-screen states); enter `ease.enter`; leave `ease.exit`. Banner. |
| X21 | Every spinner appears after 200 ms (`SPINNER_DELAY_MS`, 4.8), on buttons, rows, hosts and tabs; 4.7 updated, DiffView updated. |
| X22 | Skeleton appears after 100 ms (`SKELETON_DELAY_MS`, 4.8): the delay two specs and 4.7's first-100-ms rule already use. Skeleton updated. Derived. |
| X23 | Board is drawn for the phone. BoardColumn. [P4] |
| X24 | Marketplace on Android is a Settings row and lists no extension cards, since extensions are desktop only in v1. ExtensionCard. [P5] |
| X25 | Depth limit default 3, range 1 to 5; rows deeper than the limit show "+N deeper". DelegationTree. [P23] |
| X26 | Window close and minimise use the OS window keys; Mod+W never closes the window; macOS window close is Cmd+Q (K2). TitleBar, 8.1. [P12] |
| X27 | Mod+W closes the active Work panel tab. WorkPanelTabs. [P12] |
| X28 | Ctrl+Shift+F is search in files; the terminal has no find in v1 (K3). TerminalSurface. |
| X29 | Mod+K opens the palette except inside a terminal on Windows and Linux, where Ctrl+Shift+P opens it (8.3). TerminalSurface. [P12] (K1). |
| X30 | F7 and Shift+F7 are the decided next and previous change keys. DiffView. [P27] |
| X31 | Question 4 in design-system 10 is closed: no high-contrast theme in v1. [P35] |
| X32 | Disabled outlines use `border.disabled` (Checkbox, Radio). |
| X33 | One focus ring style everywhere: outside the border, 2 px with a 2 px offset (8.2). TextField updated; the field exception is dropped. |
| X34 | The segments of one segmented control form one Android target group (new 5.4 rule); the 8 dp rule applies between separate controls. SegmentedControl. |
| X35 | Select menu width runs from `layout.menu.min-width` (180) to `layout.menu.max-width` (280). Select. |
| X36 | Chord order: Ctrl, Alt, Shift, key on Windows and Linux; ⌃ ⌥ ⇧ ⌘ then the key on macOS (8.1). Kbd. |
| X37 | Escape does not close a permission review while its check runs; the exception is written in 8.3. PermissionReview. |
| X38 | A sheet keeps a 90 percent maximum of the height above the keyboard and never expands to full height (5.4). SecretModal. |
| X39 | The Find bar is an overlay at the pane's top right (FindBar); BrowserChrome follows. The engine technique is still the spike's decision (S5). |
| X40 | Bot Teams uses the same state words as Workspaces; "Working" and "Needs you" are removed. DelegationTree. [P30] |
| X41 | Visible copy is "Last known: running, 5 min ago" (AgentStatus, ConversationMessageList); 8.4 uses the same form in the name. |
| X42 | The progress track is the new token `progress.track` (the `surface.sunken` value). Progress and PlanView updated. |
| X43 | The selected-row bar is `accent.fg` (2.5 rule). ListRow follows Table and FileTree. |
| X44 | "Retry" everywhere; "Try again later" stays only inside one message sentence (Toast). TerminalSurface, BrowserChrome, SidebarTree, NotificationCard. |
| X45 | Sidebar section rows use D-SL-05; D-SL-02 is the status bar item. ListRow. |
| X46 | Index corrected to the inventory numbering: rail items D-SH-11 to D-SH-16; mode switch D-SH-10 only; D-WS-29 is the Sidebar tree's. screen-inventory.md was not edited (outside this pass). design-system index, RailItem, SidebarRow. |
| X47 | Avatar cites pending-owner-review item 32 (marks in full colour inside the ring). [P32] |
| X48 | Stale references fixed: the row height now sits in 4.2 (ListRow); the syntax decision cites item 34 (DiffView). [P34] |
| X49 | No shimmer in v1: skeletons are static `fill.skeleton` blocks (4.7). Not decided by the owner; chosen to match the Skeleton and Table specs. Derived. |
| X50 | Prototypes is core (item 1); the stale open point in SidebarTree is removed. [P1] |
| X51 | Start-page Composer minimum is 66 px through the new token `size.composer.start-min-height`. Composer, 4.2. |
| X52 | The Sidebar's accessible name is "Sidebar" in every mode (the glossary name). Mode-specific names dropped. Derived: confirm with the owner if screen-reader users need the mode name. |
| X53 | Disabled reasons are visible text on desktop and Android; no tooltip carries them. Menu, Tooltip. |
| X54 | Icon buttons have no long-press label; the Button rule applies (IconButton). Derived from the Button rule. |
| X55 | Splitter Home and End go to the limits, and 8.3 now lists the splitter keys (8 px, 24 px with Shift). Splitter, design-system 8.3. |
| X56 | The command palette is a dialog size: width 560 (`layout.dialog.width.lg`), not 600. CommandPalette. [P36] |
| X57 | Activity rail items are in the FrameCanvas extension list. FrameCanvas. |
| X58 | Reduced motion: the usage ring changes instantly; the loading spinner keeps turning (4.7). UsageRing. |
| X59 | After the modal closes, focus returns to the opener (Add secret), as 8.2 requires; the card container only if the opener is gone. SecretRequestCard. |
| X60 | Notification cards use overlay-resolved tints (`agent.*.bg.overlay`, new tokens in 2.8). Values = overlay plus the Canvas-to-tint offset. Derived: check by eye. |
| X61 | Skeleton is in the 7.1 allowed list for extension views. design-system 7.1. |
| X62 | A filtered empty file tree follows EmptyState: "No matches for {query}" with "Clear search". FileTree. |
| X63 | The browser page-link menu is named in contribution point 13. design-system 7.2, BrowserChrome. |
| X64 | Calendar entries use `border.default` on `surface.raised` (2.2). CalendarViews. |
| X65 | Settings row labels truncate with an ellipsis and never wrap (3.2). SettingsRow. |
| X66 | The extension card grid is roving: arrow keys move between cards (8.3). ExtensionCard. |

### 6.6 Not covered

From design-system section 10, unchanged: illustration, empty-state artwork, marketing and brand mark, app icon; right-to-left layout mirroring and per-language typography beyond the font fallback; sound, haptics and notification sounds; chart and data-visualisation palette beyond the usage rings (usage detail view, D-AC-06); mobile emulator device frames (D-SH-18) and Browser pane page content (the embedded engine draws it); Linux window decoration under different compositors and Windows snap layouts; the extension runtime, sandbox and distribution (decided in the extension-model research); iOS (out of the initial scope); print and export styles.

Found while digesting the specs: crash, timeout and placeholder rules for extensions (section 4); Android source control (P28) and pairing details (P24) beyond the names in the specs; the Android hardware-keyboard shortcut set (none in v1); the GPUI accessibility answer (S1); the owner decisions in 6.2.

## Checks run (2026-11-12)

- The 66 entries of the index in design-system.md match the 66 files in `components/` by name, and each file has a section in section 3 with a link to it.
- About 45 key patterns from 2.3 were searched in `components/`, `app-flow.md` and `design-system.md`; each one that the table relies on appears in at least one of them. app-flow writes the Ctrl and Cmd forms; the specs write Mod.
- Every table row has the column count of its header (checked by script). Line count at writing: about 1,030 lines.
- Most quotes behind the conflicts in 6.3 were checked against the source with grep. A few line references come from the spec summaries and were not re-checked: X43 (FileTree line), X49 (Skeleton and Table lines), X52, X58, X62 and X66.

## Checks run (conflict pass, 2026-11-12)

- Token references: every backticked dotted token name in design-system.md, handoff.md, glossary.md, pending-owner-review.md and components/*.md resolves in `docs/design/tokens.json` (320 names defined, 0 missing).
- Pending-owner-review citations: every "pending-owner-review item N" in design-system.md, handoff.md and components/*.md has N from 1 to 37. Each topic was read by hand against the item (items 1, 4, 5, 6, 8, 12, 16, 21, 25, 27, 30, 31, 32, 33, 34, 35). Historical quotes in section 3 "Open:" notes are labelled as such.
- `docs/design/tokens.json` is byte-identical to the token file after the fixes; it is valid JSON and keeps the file's CRLF format.
- Line endings are unchanged: design-system.md keeps CRLF, handoff.md keeps LF.
- Not run: visual check of the previews in a browser, and the GPUI and React Native generators (not in this repository). Literal values listed in 6.4 are still open.
