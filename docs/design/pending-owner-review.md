# Pending owner review

Decisions taken while the owner was away (night of 2026-11-12). Each follows the owner's earlier decisions as closely as possible. Read the first table; the rest are small and can be confirmed in one pass.

## Read first: decisions that change the most

| # | Decision taken | Alternative | What changes with the alternative |
|---|---|---|---|
| 1 | **Prototypes is core**, with its entry in the Workspaces Sidebar, because pv96 builds it as core and the owner's Sidebar screenshot shows it. This reverses the earlier default (extension). | Prototypes as an extension that contributes the Sidebar entry. | Prototype screens move to the extension catalogue; the core gets smaller. |
| 2 | **The Title bar stays as a band of the frame**, as the owner required. It carries what pv96 moved into its session bar: Sidebar toggle, back and forward, session title, region toggles, window controls. pv96 removed the title bar; that is not followed. | Follow pv96: no title bar, the canvas reaches the top edge. | The frame becomes rail plus Sidebar only. |
| 3 | **Extensions may show a fully custom UI as a sandboxed web view**, only as a Work panel tab or a full-canvas page, only on desktop. Everywhere else they use the shared components. This is not designed yet (see "Not finished" in the owner message). | Declarative components only in v1. | Smaller security surface and no dependency on the browser engine, but "build your own app" is limited to the shared components. |
| 4 | **Bot Teams on Android**: team list, Conference room, Board, Routines and a Calendar agenda are drawn for the phone. | Conference room only on Android. | Four Android screens go. |
| 5 | **Marketplace on Android** is a row inside Settings (as pv96 has it). | No Android Marketplace in v1. | Three phone screens and one Settings row go. |
| 6 | **Mode switch** (desktop) follows pv96: from any other page the first press returns to the mode's home; pressing it there switches between Workspaces and Bot Teams. On Android one press always switches. | One press always switches on desktop too. | Faster mode change, no one-press way home. |
| 7 | **Session summary is never hidden automatically** when a file or diff tab opens; the user hides it from the Title bar. The owner's diff screenshot showed it hidden. | Hide it whenever a file or diff tab is active. | Cards disappear without a user action. |
| 8 | **Six regions need a 1528 px window.** (Derived; the artboard `ws-narrow` measures the first step at 1408 px, see item 38.) Below that, regions give way in this order: Session summary, Project sidebar, Sidebar, Work panel, each reopening as an overlay. The message minimap shows only when the Conversation is at least 480 px wide. | Another collapse order, or a lower minimap threshold (360 px). | Collapse rules in design system section 5 and one spec. |
| 9 | **Typography**: Geist for the interface, JetBrains Mono for terminal, code and paths. Both open-licence and bundled. **Accent**: lavender (#7461d8 dark, #6447c9 light); users can choose another. | Inter (closer to the owner's reference screenshots); another accent. | Only `type` and accent tokens; no layout change. |
| 10 | **Light theme**: frame #eeeef2, canvas #fbfbfc (canvas is the lightest surface, as the owner asked). | White canvas #ffffff with frame #ebebed. | Light token values only. |
| 11 | **Security defaults**: approvals have no keyboard shortcut; terminal clipboard writes (OSC 52) are blocked until the user allows them; a high-risk extension install needs an extra acknowledgement; the composer chip shows a warning glyph in Full access mode. | Shortcut to approve; allow clipboard writes with a notice; no extra acknowledgement; no glyph. | Faster but riskier flows. |
| 12 | **Terminal keys on Windows and Linux**: while a terminal has focus, the shell keeps Ctrl+F, Ctrl+E, Ctrl+K, Ctrl+P, Ctrl+T and Ctrl+W; app shortcuts work again when focus leaves it. `Mod+W` elsewhere closes the active Work panel tab, never the window. | App shortcuts win inside the terminal. | Shell users lose those keys inside MassOrca. |
| 13 | **Keep running keeps the tab in the strip**: it hides the terminal's pane, and the process goes on in the service. The spec says the pane closes (D-TM-04, A9, app-flow J5 step 4). No screen lists terminals that run in no pane (app-flow G6), and this design does not invent one, so the tab is the only way back to a kept process. | The pane closes and the tab leaves the strip. | A kept process has no way back in the UI until the owner names a screen that lists kept terminals. |

## Smaller decisions (confirm in one pass)

Navigation and pages
- 13 Mode homes: Workspaces returns to the start page, Bot Teams to the team list.
- 14 "MassOrca Mobile" in the Workspaces Sidebar opens the phone pairing page on the desktop (alternative: the Mobile emulator tab).
- 15 Artifacts have no project-wide list; they live in the Results card and open in a Work panel tab.
- 16 Bot Teams pages keep the Sidebar (owner's Routines screenshot); other full pages hide it.
- 17 Extensions live in the Marketplace (owner, 2026-11-12): the Marketplace centralises discovering, installing, updating and managing them (Discover, Installed, Updates, Categories). Settings keeps only a link to Marketplace > Installed. The Vault opens from Settings.
- 18 No agents overview page: the Sessions tab and a Status bar popover cover it.
- 19 No notification centre or history in v1; the Sidebar bell expands the card stack.
- 20 The first-run wizard and session import are full canvas pages; the rail stays usable during the wizard.
- 21 On Android, rail entries move to the Home top bar (mode, theme, Settings), the overflow (Calendar) and the host card (usage).

Behaviour
- 22 New routine opens the simple routine form (pv96), not the full automation editor. Calendar shows routines and automations on one timeline, "Show done" off by default.
- 23 Team creation is a dialog on the teams page, "Workspaces in a team" off by default. Delegation depth limit: default 3, range 1 to 5.
- 24 Add host stays disabled until the connection test passes. A protocol mismatch blocks only that host. A failed local service shows an error in the canvas with the frame kept.
- 25 A message to an unreachable host stays in the composer; it is not queued.
- 26 Web addresses (webhooks) show a masked token; Copy gives the full URL. The transcription key is entered through the secret modal.
- 27 Choosing a model keeps the Model picker open so the effort can be set. Diff next and previous change: F7 and Shift+F7.
- 28 Android source control commits all changes (no staging); a quick command runs on tap.
- 29 Workspace label colours come from the usage colours, never from status colours.

Design system details
- 30 Agent states are six: running, waiting, idle, error, unreachable and done.
- 31 Sizes: 6 px dots, 16 px badges, 28 px Work panel tabs, 12 px Status bar icons.
- 32 Agent and provider marks are full colour inside their usage ring (alternative: single colour). Every place that shows an agent uses its own icon, never a letter (owner, 2026-11-12). Where space is tight (tree rows, session titles, tabs, chips, table cells) the icon replaces the agent name, which stays as tooltip and accessible label: "[Codex icon] Feed endpoints", not "Codex: feed endpoints". Mockups use drawn stand-ins until the official SVGs arrive.
- 33 Android ripple is the only translucent fill besides the dialog scrim and shadows.
- 34 Syntax colours stay off changed diff lines (code there is plain text colour).
- 35 v1 ships dark and light themes and a theme validator; no high-contrast or colour-blind theme yet. Android follows the system theme with a manual override, no dynamic colour.
- 36 Dialog widths 400, 440, 560; popover widths 280, 320, 360; behaviour constants (morph, debounce, scrollbar delay) are fixed values, not theme tokens.
- 37 UI copy uses the glossary names; pv96's code names ("side panel", "Chat summary", "right sidebar", "Jev") never appear in the UI.
- 38 Collapse breakpoints: the measured artboard ladder `ws-narrow` (below 1408, 1168, 928 and 728 px) replaces the values first derived from item 8 (1528, 1288, 1048, 848). Toasts and notification cards stay at least 8 seconds. The full list of resolved spec conflicts is in `handoff.md` section 6.5.

## Re-confirm: changes to the validated flow

The pv96 comparison changed some journeys the owner validated on 2026-11-12 (J3, J4, J5, J7, J8, J10, J12, J13). The list is at the end of `app-flow.md` under "Changes since owner validation".

## Marketplace item page order (2026-11-22)

Owner direction, binding. Where an artboard or section 7.2 differs from the owner's order, the owner's order wins. The item page has nine parts, top to bottom: logo, name with state chip and Copy link, host, invocation banner, Connection (agents), Information, Settings, Advanced Options (installed items), Danger Zone. The artboards show no banner, Connection or Settings part, and keep About and Permissions apart; section 7.2 now follows the owner's order.

| # | Question | Source | Artboard |
|---|---|---|---|
| M1 | The item page follows the owner's nine-part order, with Information holding About and Permissions. Confirm that the artboard layout is replaced by this order. | owner, 2026-11-22; decision 0059 | bt-permission-review (item page behind the dialog) |
| M2 | Settings lists installed agents only, each linking to its Marketplace page, and has no add entry. Confirm. | owner, 2026-11-22; decision 0059 | Settings > Agents and accounts (D-ST-02) |

## Consistency pass (2026-11-13)

Every screen was checked against [agreements.md](agreements.md) (93 agreements). Where it was unclear whether an agreement reaches a screen, the most consistent reading was applied and is listed here. Where two agreements conflicted, the later one won.

| # | Reading applied | Agreements | Where |
|---|---|---|---|
| C1 | Borderless buttons with hover fill cover toolbar, placement, composer, Title bar and chip buttons; secondary buttons in dialogs, sheets, forms and list rows keep their border. | A12 | Button, IconButton, every screen |
| C2b | Desktop Sidebars show a host row under each project even when the project lives on one host (A10, desktop); Android shows a host label only when the project lives on several hosts (A82). | A10, A82 | ws-*, bt-*, an-* |
| C2 | The repository icon is `book`; the later agreement wins over `folder`. | A21, A22 | design-system 4.6, Session summary cards |
| C3 | Access stays a separate chip on desktop and a lock on Android, as in the approved composers; the Model picker footer keeps only Standard \| Fast. | A27, A33 | Composer, ModelPicker, every composer |
| C4 | The Commit field and button use 12 px corners, like the Composer after it moved to 12 px. | A32, A49 | Commit box |
| C5 | Nested child sessions (agent icon, indent aligned to the parent's name, no lines) also apply to the desktop Sidebar, the Project sidebar Sessions list and the DelegationTree; the "Child" badge is removed. | A82 to A85 | SidebarTree, SidebarRow, DelegationTree, ws-* |
| C6 | Desktop workspace rows also carry the agent icon of their session; the agents shown were chosen for the mock-ups. | A84 | ws-* Sidebars |
| C7 | Where a title or label starts with an agent name, the icon replaces the name and the name stays as tooltip and accessible name; body prose and buttons such as "Ask Claude" keep the name. | A17 | every screen |
| C8 | Account rows and the default agent field keep icon plus name, because the name is the item itself. | A16, A17 | Accounts, wizard |
| C9 | Form field labels (Name, Scope, Address) get no icon; the icon goes in the value. | A21 | forms |
| C10 | The borderless sunken switch also covers the Tasks "Workspaces / Bot Teams" switch and Standard \| Fast. | A30 | SegmentedControl |
| C11 | Desktop team-level screens (Board, Routines, Conference room) use the same team pill as Android, with the team character with eyes; Board uses the Lucide kanban icon and All teams the grid. | A25, A57, A59 | bt-* |
| C12 | Each bot is drawn as a variant of its team character (A57 is general); bots of one team differ by tone or shape, keep their name and the Lead badge. | A57 | bt-*, an-botteams |
| C13 | The desktop Conference room keeps its toolbar; the Android "…" menu with Board, Routines and Calendar is not applied on desktop because the desktop bar has no Board icon. Confirm. | A60, A61 | bt-conference-room |
| C14 | The desktop Bot Teams home does not get the Android carousel. Confirm. | A58 | Bot Teams home |
| C15 | The Android Conference room composer stays one row with no lead-bot selector, following the owner's reference image; desktop shows the lead bot. | A34 | Composer |
| C16 | Android check rows are 28 dp as agreed, below the 48 dp touch target; the whole row is the target. Confirm. | A50 | an-source-control |
| C17 | The Android Summary keeps "Open chat" next to the Chat tab. Confirm whether to remove it. | A87 | an-summary-artifacts |
| C18 | The UI term stays "extension", not "plugin". | A63 | everywhere |
| C19 | The import table column is "Agent" and shows only the icon; the background banner is removed in favour of the quiet line. | A20, A42 | sh-import-* |
| C20 | Theme copy says "Match system" instead of "Match Windows", so it works on Windows, macOS and Linux. | Copy | sh-wizard-theme |
| C21 | The Android tablet adopts the floating bar, session pill, view group and floating composer inside the existing master and detail layout. | A76, A86 | an-tablet |
| C22 | The Marketplace catalogue (Discover) carries the same navigation as Installed: Discover, Installed, Updates, Categories, and the "Add extension" split button. | A63, A68 | bt-marketplace-catalogue, an-settings |
| C23 | File-type icons (A46) apply where a file is the item: file lists and trees, tabs, diff and preview headers, attachment chips, Results rows. They do not extend to grouped tool-step rows in the Conversation ("4 steps · Read child_runs.py, Edit export.ts"): such a row is a group of actions, may name several files, and stays in mono text; the file icon appears when the step is expanded and one file is its item. | A46, A21 | ws-* Conversations, MessageBlocks |
