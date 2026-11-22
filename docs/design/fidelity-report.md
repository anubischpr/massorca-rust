# Desktop fidelity report

This report compares the desktop shell and the UI gallery with the design artboards and the screen and component documents, as of 2026-11-21. It lists what matches, what each fix PR changed, and what cannot match yet. Finding IDs (A*, B*, C*, D*, E*, F*, G-*, N*) come from the audit table.

**Read first:** the artboards settle two spec conflicts that earlier documents had as derived values: the sidebar breakpoint ladder (928 px, not 1048) and the palette width (600 px card with a 44 px input row, not 560). Both are resolved and measured. The code, tokens and component documents still carry the old values; one follow-up PR moves them. See [Spec conflicts](#spec-conflicts-resolved) and [Outstanding corrections](#outstanding-document-corrections).

## How it was compared

- **Shell:** 50 states in dark and light, shot headlessly on Linux at 1440×900 (#94, 100 PNGs). Reviewed as dark-over-light sheets, some at full size.
- **Gallery:** 51 stories in both themes (#94, 102 PNGs). Light renders were judged at sheet scale only.
- **Windows:** a real window captured off-screen with PrintWindow, never activated (#92, #93).
- **Linux:** invisible X11 probes under Xvfb with xfwm4 and openbox, and a Wayland probe on a headless sway (#106).
- **Source of truth:** the artboards in `docs/design/artboards/` (#100). An artboard settles a conflict first. If it cannot, the component document wins over a screen note. Between two screen documents, the later revision wins.
- **Re-run on Linux** (the `#[ignore]` screenshot tests; screenshots are not committed):

  ```
  cargo test --locked -p massorca-desktop --test screenshots -- --ignored --test-threads=1
  cargo test --locked -p massorca-ui --test screenshots -- --ignored --test-threads=1
  ```

### Fix PRs

| PR | Scope | State |
|---|---|---|
| #92 | Rose and agent marks | merged |
| #93 | Windows window icon | merged |
| #94 | Screenshot harness, gallery probe fix (H1) | merged |
| #98 | Shell chrome, notices, palette | merged |
| #100 | Artboards | merged |
| #101 | Sidebar, dialogs, start page | open |
| #102 | Settings | open |
| #104 | Bot Teams | open |
| #105 | Session and panel components | open |
| #106 | Linux window | open |
| #107 | Base components | open |

## Shell chrome and notices

**Matches**
- Palette copy and groups.
- Rail and status menu.
- Connecting and reconnecting copy.
- Empty start-page logo and heading.
- Narrow collapse at 1040 and 840 px. Re-check against the artboard ladder (1168, 928 and 728 px).
- Settings navigation and search field.

**Fixed**
- A6 usage popover: Plan row (#98). Reads "Plan not reported" until the service reports usage.
- A7, N1 status bar: Memory label and extra Service item removed. Memory, terminals and ports moved to the right group, before hosts (#98).
- A8 focus ring on title bar, status bar, Conversation, region columns and overlays (#98). The Sidebar ring is still open.
- D1, N11 service down: ErrorState page replaces the banner over the start page. Hosts item reads "Service not running" (#98).
- N10 reconnecting banner keeps only Retry (#98).
- N12 Bot Teams pages: labelled Back with the page title (#98).
- A2 start-workspace-demo: Chat/Terminal switch in the composer's bottom row, key-hint row, "Send" tooltip replaces the placeholder (#101).
- N8 composer, partial: switch and key-hint row (#101). The +, access chip, model picker and microphone are not listed as fixed.

**Superseded by owner ruling (branch `desktop/title-bars`)**
- N12 is superseded. Title bars read like the artboards, and back navigation stays in the Sidebar and the history controls. Measured per artboard at 1440×900:
  - `bt-board`, `bt-conference-room`, `bt-routines-empty`, `bt-routine-form`: Sidebar toggle, then "Desarrollo · {page}" (`{team} · {page}`), no Back control.
  - `bt-settings-appearance`, `bt-vault`, `bt-calendar-week`, `bt-marketplace-catalogue`, `sh-setup-checklist`: one icon Back, then the page title, no Sidebar toggle.
  - `ws-tasks`, `ws-automations`: one labelled Back, then the page title, no Sidebar toggle.
  - `ws-session`, `ws-start`, `sh-empty-app`: Sidebar toggle, Back and Forward icons, then the session title.
- No artboard exists for Pairing, Memory, Files or the team project page. Pairing draws the icon Back. Memory, Files and the project page take the team pattern, `{team} · {page}`.
- No keyboard shortcut for Back exists in `shell/actions.rs`, so none was added.

**Superseded by owner ruling (branch `desktop/status-bar-memory`)**
- A7 is superseded. The Memory label is restored: "Memory 1.22 GB" (`sh-empty-app`). The copy follows the artboards: "Agents: idle" with no agent running, "2 hosts", "1 of 2 hosts unreachable" (`sh-host-unreachable`), and in Bot Teams "Terminals 0", "Ports 0", "1 host unreachable" (`bt-board`).
- Not changed in this branch: the grouping. `sh-empty-app` puts Memory, terminals and ports at the left with hosts at the right. `ws-start` and `ws-session` keep agents at the left and the rest at the right, as the code does. The code keeps the Workspaces grouping; the owner decides the start page grouping.
- Artboard conflict for the owner: `ws-start`, `ws-session` and every `bt-*` status bar draw "1.22 GB" with no label. The label is applied in every mode.
- The live status view has no memory source and reads "Memory -" until the service reports one.

**Cannot match yet**
- A1 start frame: sample data and "not listed by the service" heading. Open (L4).
- A3 Conversation: page not built.
- A4 Tasks, Calendar, Marketplace, Prototypes, Automations: pages not built (D-WS-24, D-AG-23, D-EX-01, D-AG-14, D-AG-17).
- A5 "(host unreachable)" title suffix. Open (L4).
- N2 project owner line: needs protocol data.
- N3 Prototypes row opens nothing: no Prototypes page variant exists.
- N5 Work panel: no tab strip, no four empty-state buttons. `work_panel_tabs.rs` is built but unused by the shell. No PR claims it.
- N6 Project sidebar: icon tab strip missing. Built in `project_sidebar_tabs.rs`, gallery only. No PR claims it.
- N7 empty start page: placement and composer. Open (L4, `canvas.rs:145`).
- N9 start page facts table and two buttons: no spec line. Owner decision.

## Sidebar

**Matches**
- Bot Teams sidebar tree.

**Fixed**
- B2 row menu: D-WS-02 rows and divider (#101). Rows whose layer is missing are disabled with their reason. Copy path, copy branch name and Delete workspace work.
- B5, G-26 unreachable host name keeps 60% of the row; the state text truncates first (#107).
- B6 Prototypes row shows its chevron (#101).

**Resolved by correcting the document, not the code**
- B3 workspace row icon: the component document wins (git-branch or folder icon; the agent icon is for session rows). D-WS-01 corrected (#101).
- B4 empty tree: "No projects yet" with no button. D-WS-01 corrected (#101). The evidence is mobile only (see conflicts).

**Cannot match yet**
- B1, C1 New workspace: a 640 px page with Host, Type, Location, Base and Parent row and setup card. Not built (L4).
- N2 project owner line: needs protocol data.
- N3 Prototypes page: see Shell.
- Breakpoints: fixed in #114. The ladder is 1408 / 1168 / 928 / 728 px, as artboard `ws-narrow` measures.

## Dialogs

**Matches**
- None recorded.

**Fixed**
- C2 title and button: "Delete workspace <name>?" and "Delete workspace" (#101). Width still 440 px.
- C3, partial: dirty and ignored-files variant titles (#101).
- C4 start-page button reads "Delete workspace…" (#101).
- N13 keep-branch checkbox shown in every destructive variant, disabled with its helper (#101). Workspace removal has no keep flag yet.

**Cannot match yet**
- B1, C1 New workspace dialog: see Sidebar.
- C2 width 480 px: `DialogWidth` (`dialog.rs`) has no 480 option. Open (L4).
- C3 per-item counts, unpushed and nested variants: not in any PR.
- N13 real keep flag: needs protocol.
- N14 "Delete ignored files" variant: no spec line. Owner decision.

## Settings

**Matches**
- Navigation and search field.

**Fixed (#102 unless noted)**
- E1 rows use their spec control types. Unbacked settings are drawn disabled with "Not available yet", by design.
- N15 Updates row with "Check for updates".
- N16 "Conversations" without a project; editor defaults.
- N17 agent availability rows, computer use, browser access, session search.
- N18 delegation depth stepper.
- N19 segment labels no longer cut (#107, shared segmented control).
- N20 accent swatches, custom colour, contrast line.
- N21 text-size slider, 90 to 130 percent.
- N22 import theme, layout preset, interface and monospace fonts, status-bar items.
- N24 terminal palette, rendering, search engine, zoom, profiles, link routing. Defaults match the core: JetBrains Mono, 1000 rows of scrollback.
- N25 sources: one row per provider, default branch, API budget.
- N30 routine and extension notice sources, sound select, speech model, transcription key.
- N33 privacy rows: telemetry, cache, OS permissions, CLI registration, experimental flags, Advanced.
- N34 routing history.
- N23, partial: keyboard table header with filter and All/Conflicts/Extensions. Reset all and quick commands are not listed.
- N31, partial: the Extensions section says Marketplace > Installed is not built. It does not open it.

**Cannot match yet**
- N26 VPS host row: needs the remote hosts layer.
- N27 "This PC" pill reads "Connecting" while the sidebar and rail show Connected: not in #102. Open.
- N28 search field not focused on open: `shell/mod.rs`. Open (L4).
- N29 name-only Switch: `switch.rs`. Open (L4).
- N31 Marketplace > Installed: Marketplace not built.
- N32 vault: needs the vault layer.

## Bot Teams

**Matches**
- Sidebar tree.

**Fixed (#104)**
- F2 board: columns scroll to the window edge; header and note keep their 24 px inset.
- F3 new team: Lead bot Select (disabled, "No bots yet"); helper text wraps.
- F4 add bot: 560 px, with Agent and Host Selects. Partial: skills not listed.
- F5 team card More action (disabled until archive and delete exist).
- N35 host reads "VPS".
- N36 unreachable glyph and "Last known state, 5 min ago" on the team row; sample data matches the shell snapshot.
- N37 sidebar header shows Filters and New project only.
- N38 Filter ghost button (disabled until the filter layer exists); team pill chevron.
- N40 host is a Select that defaults to This PC.

**Cannot match yet**
- F1 Conference room: needs live agents.
- F4 skills picker: not listed as fixed.
- F6 team characters: accepted in decision 0027 (#104).
- N39 dashed "Nightly summary" border: `board_column.rs`. Open (L4).

## Palette and quick open

**Matches**
- Palette copy and groups.

**Fixed (#98)**
- N41 session rows: "project / workspace" beside the title, state word at the trailing edge, "Waiting for you" in the waiting colour.
- N42, partial: Esc key cap added.

**Cannot match yet**
- N42 input row: about 28 px in the shell; the artboard says 44 px. Follow-up PR.
- Width: fixed in #114. The palette is 600 px with a 44 px input row, as artboard `ws-palette` measures.

## Components (gallery, 51 stories, both themes)

**Matches**
- Button, IconButton, Kbd, Badge, Switch, Checkbox, Table, Text field, Skeleton, Usage ring, Tooltip, Dialog, Splitter, ScrollArea, Settings row, Queued message, Avatar (its states are not drawn in the gallery).
- G-22 sheet loading rows: did not reproduce (#107).
- G-03 and G-13: not defects (#107).

**Fixed**
- H1 gallery probe aborted at story 18; all 51 stories are shot in both themes (#94).
- #107, empty, error, search and picker: G-02, G-04, G-05, G-14, G-15, G-16, G-17, G-18, G-20.
- #107, indicators: G-06, G-07, G-08, G-09, G-31.
- #107, lists and rows: G-23, G-27, G-28.
- #107, segmented control: G-29, G-30.
- #107, menus and overlays: G-10 and G-12 (spacing, no overlap), G-11 (destructive label wraps).
- #105, notification, extension, secret and browser cards: G-35, G-36, G-37, G-38, G-44, G-45. G-40, partial: copy only.
- #105, diff, plan, delegation and summary: G-46, G-47, G-48, G-49, G-50, G-51, G-52, G-53, G-54, G-55, G-57, G-58.
- #105, board, model picker and composer: G-60, G-61, G-65, G-68, G-69, G-70. G-62, helper added (`BoardColumn::source_slot`).
- #105, G-41 dropped: the zoomed capture shows a neutral tab.

**Cannot match yet**
- G-01 empty-folder title token: FileTree and EmptyState disagree. Owner decision.
- G-19 Banner status word: Banner.md requires it, but its own examples omit it. Owner decision.
- G-21 Undo action accent: Button has no ghost label colour option.
- G-24 "Session with a trailing action" row has no agent icon. No PR claims it.
- G-25, G-56 typed file badges (TS, RS, PY): the helper is private to `file_tree.rs`.
- G-32 stacked notification cards show no body, actions or dismiss; G-33 folded "+2 more" row spans about 1170 px; G-34 Allow and Deny not in the copy list. No PR claims them.
- G-39 secret cards: stored, linked and declined rows show no host name. No PR claims it.
- G-42 work panel close marks shown at rest; the spec shows them on hover. No PR claims it.
- G-43, G-66 text field draws two borders (address bar) or a boxed search field (model picker); the fix is in `text_field.rs` (#105 leaves it open).
- G-59 stale delegation tree dimming: unverified.
- G-63 wide and emoji glyphs draw as empty boxes: the spec has no fallback rule. Not located.
- G-64 model picker trigger chip fill spans the full width: no PR claims it.
- G-67 model picker footer "Stand…": #105 leaves it open (segment width). #107 changes the segmented control, so recheck after merge.

## Spec conflicts resolved

Every resolution here is measured in the artboards, which settle conflicts first. The earlier values, 1048 px and 560 px, were derived before the artboards were available. They were not owner decisions and are superseded.

| Item | Measured in the artboards | Resolution as written | Document corrected | Status |
|---|---|---|---|---|
| Sidebar breakpoint | `ws-narrow` annotations: "Step 1: below 1408 px" (session summary), "Step 2: below 1168 px" (project sidebar), "Step 3: below 928 px" (Sidebar), "Step 4: the Work panel gives way below 728 px". `workspaces-sessions.md:563` agrees. | Ladder 1408, 1168, 928, 728 px | D-WS-01 back to 928 (#101 revert ae2d8ee). Design-system 5.1, decision 0027, FrameCanvas.md and code: follow-up PR. | Resolved, measured. |
| Palette width | `ws-palette` card: 600 px wide, 96 px from top, 44 px input row. D-CP-01 (`workspaces-sessions.md:536`) already says 600 and 44. | 600 px card, 44 px input row | CommandPalette.md, `tokens.json` and code: follow-up PR. D-CP-01 needs no change. | Resolved, measured. |
| Status glyph | `ws-session` status bar: 12 px icons (two of three SVGs in the bar; the third, 16 px, not identified). Tokens: status bar icons 12 px (`size.icon.xs`). | 12 px | D-SH-05 16 to 12 px (#98) | Resolved. |
| Project tab strip | 36 px in five artboards: `ws-session`, `ws-diff`, `ws-light`, `ws-narrow`, `ws-workpanel-empty`. `workspaces-sessions.md` says 36. | 36 px | ProjectSidebarTabs.md (#105, open). D-SH-04 outstanding. | Resolved; corrections pending. |
| Workspace row icon | Not re-measured in this pass. | Component document: git-branch or folder icon. | D-WS-01 (#101) | Resolved by the component document. |
| Empty tree without button | "No projects yet" appears only in mobile artboards (`an-home`, `an-workspaces`, `sh-empty-app`). No desktop artboard contains it. | No button. | D-WS-01 (#101) | Mobile evidence only. |

**Sidebar breakpoint: resolved from artboard `ws-narrow`**
- Measured ladder: session summary below 1408 px, project sidebar below 1168 px, Sidebar below 928 px, Work panel below 728 px. `workspaces-sessions.md:563` agrees.
- The 1048 px and 1528, 1288, 848 px figures in design-system 5.1 (`design-system.md:692`), decision 0027, `components/FrameCanvas.md:23`, `handoff.md:143` and `handoff.md:963` (X1) were derived before the artboards existed. They are superseded, not owner decisions.
- PR #101 first corrected D-WS-01 to 1048 px. Its revert commit ae2d8ee restores 928 px.
- The audit's "narrow collapse at 1040 and 840 px" match was checked against the old ladder. Re-check it against 1168, 928 and 728 px.

**Palette width: resolved from artboard `ws-palette`**
- Measured: a 600 px card, 96 px from the top, with a 44 px input row. D-CP-01 already matches.
- The 560 px values in `tokens.json` (`layout.palette.width`), CommandPalette.md and X56 were derived before the artboards existed. They are superseded, not owner decisions.
- `handoff.md:782` still flags 600 px as unmatched. Update it in the follow-up PR.

**Readings taken where specs are silent or conflict (#107)**
- Destructive menu labels wrap, so the loss clause stays visible (G-11).
- A name keeps 60% of a row; the state text truncates first (B5, G-26).
- Labelled segments grow to fit; the 64 px floor is dropped, because it cannot fit the spec's own "Chat / Terminal" example (G-29, G-30).
- The search clear action shows whenever the field holds text (G-04, SearchField.md).

## Still-open owner decisions

| Item | Question |
|---|---|
| G-01 | Which token the empty-folder title uses: the spec wants `text.secondary`, the code draws `text.primary`, and FileTree and EmptyState disagree. |
| G-19 | Does a Banner show its status word? Banner.md requires it; its examples omit it. |
| N9 | Keep the start-page facts table and the two buttons? No spec line exists. |
| N14 | Keep the "Delete ignored files" variant? No spec line exists. |
| Copy without a spec source | PR #102 says the list is in this report. It is not in the PR description and is not reproduced here. |
| Agent list and neutral swatches in Settings | Named in the brief. No PR description names them. Source needed. |
| G-71 | Keep running hides a terminal's pane, and its tab stays in the Work panel strip. The spec says the pane closes (D-TM-04, A9, app-flow J5 step 4). No screen lists kept terminals (app-flow G6), and this report does not invent one. Confirm that the tab stays, or name the screen that lists kept terminals. |

## Real-window and Linux results

**Windows**
- Rose edges (#92), 48 px start rose: mean absolute difference 0.0442 to 0.0159; blended edge pixels 7.2% to 86.5%. Gentle Shell row mark, 16 px: 0.1366 to 0.0092; blended 17.3% to 98.4%.
- Icon (#93): exe icon resource 1, none to the rose (256 px). `ExtractIconEx` count 0 to 1. Window class icons 0 to rose (large and 16 px). `WM_GETICON` stays empty by GPUI's design.
- Hit tests and close: listed for this pass, but no fix PR description records them. Source needed before this report is final.
- Unverified: Windows dead keys at runtime (#106). The probe never activates the window, so focus could not move into the text field.

**Linux (#106)**
- xfwm4 under Xvfb: one title bar. Before, a frame of 5, 5, 29, 5 with Motif "all"; after, no frame and Motif "none". Minimise, maximise and restore, drag, right-edge resize and close (exit 0) all work.
- openbox: one set of controls. The app hides its own minimise, maximise, close and resize edges when the window manager draws the frame; the title bar still drags.
- App id `massorca` on X11 (`WM_CLASS`) and Wayland (`app_id` under sway). Matches `massorca.desktop`, which passes `desktop-file-validate`.
- X11 window icon: `_NET_WM_ICON` 128×128. On Wayland the icon comes from the desktop entry.
- Dead keys on X11: Spanish ´ then a gives á, with no input method and with ibus (`xkb:es::spa`).

**Unverified**
- Wayland title-bar clicks and dead keys: the headless compositor's virtual input is per process.
- Windows dead keys at runtime (see above).
- 125%, 150% and 200% displays: covered by the selection rule and tests (1.6×, 1.33×, 1:1), not run on real displays (#92).
- IME on a real focused window: no PR records a result.

## Leftovers for the next pass

From L4:
- [ ] N28 focus the Settings search on open (`shell/mod.rs`)
- [ ] N29 name-only Switch (`switch.rs`)
- [ ] N39 dashed BoardCard (`board_column.rs`)
- [ ] A1 start frame on sample data (`canvas.rs`, `shell/mod.rs`)
- [ ] A5 "(host unreachable)" title suffix
- [ ] N2 project owner line (needs protocol data)
- [ ] N7 empty start page: placement and composer (`canvas.rs:145`)
- [ ] C2 480 px dialog width (`dialog.rs`, `DialogWidth`)
- [ ] N13 keep-branch flag (needs protocol)
- [ ] B1, C1 New workspace page
- [ ] Unused `_hosts` parameter in `team_dialog_view` (`shell/mod.rs:470`)
- [ ] Teams PROJECTS header duplicates `projects_header`

Found in the audit, not in L4, and not claimed by any PR:
- [ ] N5 Work panel tab strip and empty-state buttons
- [ ] N6 Project sidebar icon tab strip
- [ ] N26 VPS host row (needs the remote hosts layer)
- [ ] N27 "This PC" pill state
- [ ] N32 vault screen (needs the vault layer)
- [ ] G-24, G-32, G-33, G-34, G-39, G-42, G-63, G-64

The L4 note "no artboard source exists in the repo" is superseded. The artboards have been in `docs/design/artboards/` since #100.

## Outstanding document corrections

**One follow-up PR** moves the code and the documents to the measured values:
- Sidebar: 1048 px to 928 px in the shell (`regions.rs:32`), with design-system 5.1 (`design-system.md:692`), decision 0027, `components/FrameCanvas.md:23`, `handoff.md:143`, `handoff.md:963` (X1) and `pending-owner-review.md:54` aligned to the ladder 1408, 1168, 928, 728 px.
- Palette: 560 px to 600 px in `tokens.json` (`layout.palette.width`), CommandPalette.md (width, and the input row from 32 to 44 px), and the shell (`palette.rs`, width and input row).
- `handoff.md:782`: remove the note that 600 px is unmatched.
- D-WS-01 is already back at 928 px through #101's revert commit ae2d8ee. D-CP-01 (`screens/workspaces-sessions.md:536`) needs no change.

**Separate, still outstanding**
- **D-SH-04**, `screens/shell-setup-hosts.md:65`: 28 px tab strip to 36 px.
