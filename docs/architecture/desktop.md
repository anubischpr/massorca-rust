# Desktop architecture

The desktop app is a GPUI client of the core service. This page shows how its
parts fit; each choice has a record in `docs/decisions/` (0020 to 0029).

## Shape

```
 docs/design/tokens.json ──(generator test, 0022)──> crates/ui/src/tokens/generated.rs
                                                           │
 crates/ui  (massorca-ui)                                  ▼
   theme      Theme { appearance, palette, accent }, cx.theme()
   assets     embedded fonts, Lucide icons, agent icons, logo (0024)
   components Button, IconButton, Kbd, Badge, AgentIcon, TextField, ListRow,
              SidebarRow, SegmentedControl, Select, Menu, Dialog, Toast, Card,
              EmptyState, ErrorState, ... (0021)
   bin        massorca-gallery: every component, dark and light side by side
                                                           │
 crates/desktop  (massorca-desktop)                        ▼
   shell      Frame and Canvas, Title bar, Activity rail with its usage popovers,
              Sidebar, Status bar with its visibility menu, command palette (Recent
              rows, Go to file), service notice, workspace and add-project dialogs
   model      view models (ShellModel, sidebar tree, status bar, palette, usage) and
              pure derivations: service_view, workspace_flow, project_add,
              status_items, palette_recent
   service    ServiceBridge (tokio thread), connection states, StoreUpdate
              reducer, typed Refusal, launch options (0028)
   store      ServiceStore entity, ClientDataSource
   data       DataSource trait: SampleDataSource, ClientDataSource (0023, 0028)
   bin        massorca-desktop
```

GPUI comes from one pinned Zed commit (0020). `crates/ui` never depends on
the protocol or the client; `crates/desktop` does, through `service` and
`store` only.

## Theme and tokens

- Names are the contract (handoff 2.1). `palette.surface.frame` in Rust is
  `surface.frame` in the design and in theme files.
- State fills are solid per surface (`fill.hover.canvas`, `fill.hover.frame`);
  a component asks for the fill of the surface it sits on. Nothing blends at
  run time.
- The theme switch on the rail swaps the palette and repaints; it does not
  animate (handoff 2.6).

## The frame

Agreements A1 and A14: the Activity rail, Title bar and Sidebar are one
L-shaped `surface.frame` with no seams; the canvas (`surface.canvas`) sits
inside with a rounded top-left corner (`radius.canvas`, 12 px). The shell
draws the frame as the window background and the canvas as one rounded
child, so no border can appear between frame parts.

## Data flow

```
 service <-> ServiceBridge (own tokio thread) --StoreUpdate--> ServiceStore --notify--> Shell
                  ^                                                 |                     |
                  +---- commands (create, remove, retry now) <------ +---- dialogs <---- input
```

Views read view models only, derived from the store by plain functions.
`--sample` fills the same store once from `SampleDataSource`; by default the
store follows a service through the bridge (0028).

- **Bridge.** One tokio runtime on the `massorca-service` thread owns the
  connection. States: `Connecting`, `Connected`, `Reconnecting {attempt,
  retry_at}` after a loss, and `Unreachable {reason}` after a failed first
  attempt. Backoff runs 250 ms doubling to 5 s; "Retry now" resets it.
- **Updates.** After each connect it subscribes to events, then lists
  projects and workspaces. Each event refetches its item (or drops it on
  `*.removed`); `stream.overflow` or a reconnect resyncs in full. Unknown
  topics are ignored. The UI applies updates in batches and notifies once.
- **Changes.** Create and remove stay busy until the call answers. The store
  changes only through the resulting event. A refusal picks the dialog:
  `workspace.dirty` asks to discard changes, `workspace.ignored_files` and
  `workspace.orphan` have their own waivers, each retrying with exactly one
  more flag; `workspace.locked` and `workspace.main_worktree` only inform.
  `project.add` works the same way: its refusals about the path
  (`path.not_found`, `path.not_directory`, `project.exists`,
  `project.not_git`) show under the path field of "Add a project".
- **Terminals.** `terminal.create`, `kill` and `close` run as calls on the
  bridge. `input` and `resize` share one queue per connection, so the service
  applies them in the order the UI sent them. An attach is a stream: the bridge
  forwards its items, and dropping the stream detaches without ending the
  terminal. The service sends no terminal events, so the store takes terminals
  from each resync and from the answers to create, kill and close. A lost
  connection ends each attach with `Lost`; the UI waits for the link to come
  back and attaches again without an offset, which gives a snapshot. The Work
  panel's strip lists the selected workspace's terminals from the store, and the
  window keeps only which one is selected. Each shown terminal is a
  `TerminalSurface` fed by `feed_events`. Its replies go back as input through
  one function (cursor reports, 0034); an attach that sends them declares
  `answers_queries` (0036). The pane and the integration test both declare it
  through `AttachRole::AnswersQueries`. Closing the tab of a running terminal
  asks in X-DLG-01: "Keep running" drops the view and the tab stays in the
  strip, and "Stop process" calls `terminal.close`. An unverifiable terminal
  shows a banner that says it may still run. The status bar counts the
  terminals that may still run.
- **Launch.** `massorca-desktop [--sample] [--profile <name>] [--home <dir>]
  [--no-autostart]`. Profile and `MASSORCA_*` variables follow the CLI. The
  service is started from the `massorca` executable next to the desktop.

Per-window UI state lives in `ShellModel`, not in the store: the open usage
popover, the Status bar items the window hides, and the palette's Recent rows
(the last five commands and workspaces run). None of it is saved to disk yet.

## Source control

The Project sidebar's Source control tab (D-DF-02) shows the selected workspace's branch, its changes
in four groups, the commit box and the first page of history. It runs on `git/1` through the bridge
(decisions 0010 to 0013):

- **Calls.** `service/git.rs` runs diff, stage, unstage, discard and commit on the bridge runtime. A
  mutation refetches the workspace's status (and history after a commit) before it replies, whatever
  the answer.
- **Watch.** While the tab is on screen, the bridge follows the selected workspace with `git.watch`.
  Each event shows at once and refetches the status beside the stream, so a slow git call never holds back
  the panel's readiness or a change. Answers are ordered: an older status never replaces a newer one.
  Watches survive a reconnect and end with their connection.
- **Store.** `StoreState.git` holds each workspace's status, first history page and degraded-watch
  state. A removed workspace takes them with it.
- **Models.** `model::source_control` derives the panel from the store; `model::source_control_diff`
  parses a patch into DiffView's data; `model::source_control_flow` holds the discard waivers and the
  commit form. A refusal that names no file is a failed dialog, never a waiver.
- **Shell.** `shell::source_control` holds the panel's own state and calls. `project_sidebar` draws the
  tabs and the panel. `diff_page` draws a file's diff in the Canvas, in place of the start page.
  `source_control_dialogs` draws the discard flow with the same `flow_dialog` as the remove dialog.
- **Not built.** Fetch, pull, push, branch operations and abort; the Work panel tab (the diff takes the
  Canvas until the Work panel is on main); a multi-line commit message, since the message field is one
  line; history beyond the first page.

## Bot Teams

Bot Teams is a mode of the same shell (D-SH-10), not a second app. Its data is
plain structs in `model::teams` with stable ids: teams, bots, Board tasks,
projects, routines, web addresses, memory and files. A `TeamsSource` fills it.
`SampleTeamsSource` holds team Desarrollo from the screen specs and is chosen
by `--sample`, the way `SampleDataSource` is. Without a service, `UnavailableTeams`
is used: the pages say that Bot Teams is not connected and nothing changes.
Every change goes through `TeamsState`, which refuses with a named reason. Until
the service has Bot Teams, changes land in the sample store only.

- **Navigation.** `Page::Team(TeamPage)` keeps the Sidebar open (screens 3,
  decision 1). In Bot Teams the Sidebar is the only region, and a Workspaces
  session is never shown there. The mode switch cancels a card pick.
- **Views.** `model::teams` builds the Sidebar, Board, card and address view
  models; `shell::teams` draws them. Forms and confirms are `TeamDialog` and
  `TeamConfirm`, held by the Shell, which reads a form's fields on submit.
- **Board.** A picked card is state in `TeamsState`; the Board changes only on
  the drop. Components `BoardColumn` and `Avatar` live in `massorca-ui`, with
  gallery stories.
- **Not built, and what each waits for.** The Conference room and agent-driven
  routines wait for live agents and sessions. The routine form and the Calendar
  wait for the scheduling editor and the calendar view. Team files wait for the
  file editor (D-ED-01). Drag on the Board, a multi-line brief, the Add bot
  advanced options and skills wait for their own rows.

## Testing

See 0025: state tests in plain Rust, element tests with GPUI's fake platform,
layout checks with real text shaping in `HeadlessAppContext`, screenshots on
Linux only. No automated run opens a visible window.

## Settings, start page and setup

- **Settings** is a full page (`Page::Settings`). Its twelve sections and their
  rows are data in `model/settings_sections.rs`, and each row names what backs
  it. Only the workspace folder (`workspaces.default_root`) is backed: the
  bridge's `config.list` and `config.set` read and write it, and a refusal keeps
  its service code. Every other row is drawn disabled with "Not available yet"
  until its layer exists. The folder is read only while the page is open. It
  applies on Enter or when the field loses focus; "Saved" appears only after the
  service answers, Escape puts the stored value back, and a refusal keeps the
  typed text with its reason (`model/settings.rs`).
- **The workspace start page** (`Canvas::Workspace`) opens from a click on a
  workspace in the Sidebar. It reads the workspace's record from the store:
  name, project, path, branch, state and whether its folder is still on disk.
  Actions that need sessions, terminals or git are drawn disabled with the
  reason. The project in the heading opens the create form's project menu;
  choosing a project opens its first workspace, and "No project" returns to the
  start page with no workspace. The page creates nothing.
- **Setup** (`Page::Setup`) is the checklist of D-OB-12, opened from Settings,
  General and account. A step the core cannot satisfy says so instead of
  claiming a result.
- Not built yet: search highlighting and the outline after a jump
  (SettingsRow.md), the vault page, the first-run wizard, and host rows beyond
  This PC. Sample data holds no workspace records, so the
  start page shows "not listed by the service" in `--sample` mode.

## Marketplace

- **The Marketplace** (`Page::Marketplace`) is a full page with a left
  navigation: Discover, Installed, Updates and the five categories. Its lists
  are in `model/marketplace/catalogue.rs`; an item page shows its nine parts in
  the owner's order (`model/marketplace/item_page.rs`, teams-extensions-settings.md 7.2). A part that does not
  apply to the item says why in its place. The Information part links the package's website, privacy policy and
  terms of service when it names them.
- The Installed list (`shell/marketplace/installed.rs`) shows one manager row per installed extension, the
  `ExtensionManagerRow` of the ui crate (teams-extensions-settings.md 7.4). Its status words come from
  `status_word` in `model/marketplace/catalogue.rs`. Discover, Updates and the categories keep the card grid.
- Its data comes from `MarketplaceSource`. The sample implements it in memory and
  changes only itself. Without a service, `UnavailableMarketplace` lists nothing
  and refuses every change. Actions the sample does not model are drawn disabled
  with "Not available yet": installing, the default agent and item settings.
- The host picker (`shell/marketplace/host_pick.rs`) aims the Marketplace at This computer or a connected paired host.
  Items carry a stable host key (`local`, `runtime:<name>`, `model/marketplace/host_target.rs`). A paired host's lists
  are read on its own connection, and each request carries its number: only the newest answer for the picked host is
  shown, so a late answer never replaces a newer pick. A pick whose host is not connected shows This computer and
  returns when the host connects. A pick is saved by its stable key in the UI state store (SET-02, `ui_state.rs`) and
  restored at start; a saved host that is gone shows This computer without being overwritten. Accounts and update
  checks stay on This computer.
- Settings lists the installed agents from the same snapshot, each linking to its
  item page. Settings never installs anything. The Extensions row opens
  Marketplace > Installed.
- A built-in agent is one item: its agent kind's item carries the agent's package
  (`massorca.<kind>`), so install, the Enabled switch and uninstall go to
  `extensions.*` like any package. An agent kind with no package stays read-only.
- An uninstall confirm for a package that installed its CLI offers a tick to remove that CLI too. The tick is the
  person's acceptance: `service/extensions.rs` sends the consent fingerprint the service names with the removal.
- A card's logo, an item page's header logo and its banner image come from `extensions.asset`. The first draw
  that asks for one starts the call; the image is decoded off the UI thread and kept per package, version and path
  (`model/marketplace/item_images.rs`, `shell/marketplace/item_images.rs`). A new version fetches again. A failed
  call or an image that does not decode leaves the icon, and it is not asked for again.
- A card for an agent draws the agent's own mark, the one the sidebar and model picker draw for its kind
  (`rail::agent_kind`, reused in `shell/marketplace/catalogue.rs`). A kind the desktop has no mark for keeps its logo or
  the Bot icon.
- Add extension (`shell/marketplace/add_folder.rs`) sits in the catalogue header and takes a full folder path on This
  computer. Its review is the catalogue install's review (`InstallReview`, with no catalogue item): `extensions.inspect`
  answers for `dir:<path>`, and nothing installs until its button is pressed. The CLI reaches the same review with
  `massorca extension install dir:<path>`; it prints the review and needs `--consent` or `--yes`.
- The install review dialog (`model/marketplace/install_review.rs`) says only what its button does, in one of two
  cases: "Add <name> to MassOrca" when nothing is installed (an agent whose CLI is already here, or a package with no
  installer), or "Install <name>" with the one vendor command on a code line. The sentence uses the body type, one
  step above the rest of the dialog. Show details, collapsed by default, holds every technical line
  (`model/marketplace/install_details.rs`): the package, source, risk and its reasons, permissions, what each agent
  starts and where it keeps its sign-in, the installer line when it runs, the verify and uninstall commands, and the
  labelled SHA-256 and consent fingerprint. The consent fingerprint, sha256 and request sent are unchanged.
- The remove confirm (`model/marketplace/remove_review.rs`) reads "Remove <name> from MassOrca?" with one sentence: an
  agent stops being offered and its accounts added here are forgotten, and a CLI MassOrca did not install stays
  installed. A CLI MassOrca installed keeps the opt-in to uninstall it, and the button becomes "Remove and uninstall"
  when it is ticked. Show details lists the exact effects. A package whose vendor program stays is never said to be
  removed from this computer. A git URL is listed as
  not available yet, because the service's sources are `catalogue:`, `dir:` and `file:` only.
- Update checks (`shell/marketplace/extension_updates.rs`): each installed folder or archive package is inspected once
  per read of the lists, never per draw. A review whose sha256 differs from the installed one lists the package on
  Updates with that review's version. Its Update button sends `extensions.update` with the review's consent, and the
  sha256 for an archive. A refused inspect leaves the package off Updates until the next read. Built-in packages are
  not inspected: the service refreshes them at start. A refused update opens the item page, which shows the refusal.
- Rollback and the run log (`service/extensions.rs`, `model/marketplace/run_log.rs`): each list read also reads every
  installed package's run log when the service offers `extensions-log/1`, and records which of `extensions-rollback/1`
  and `extensions-log/1` it offers. A package's rollback target is the version its newest run that worked replaced, when
  that run was an update or a rollback; an install or a removal since leaves nothing kept. `extensions.rollback` without
  consent answers with the restored version and its consent (`RollbackAnswer`), and the call is sent again with that
  consent. Paired hosts' packages get neither yet.
- The Installed row's More menu lists "Roll back to <version>" first, above Remove, when the service offers
  `extensions-rollback/1` and a version is kept (`shell/marketplace/installed.rs`). Its confirm
  (`model/marketplace/rollback_review.rs`) reads "Roll back <name>?" with one sentence that names the version it puts
  back; Show details holds the versions and the consent. The first call names no consent, so the confirm shows the
  consent the service gives (`shell/marketplace/rollback.rs`). Nothing kept opens the item page with its sentence.
- The element tests (`shell/marketplace/tests.rs`) find a button as
  `button:<id>`, which every `Button` publishes under test.

## Browser pane

Not built yet. Direction in 0026: a `wry` child web view first, with the rule
that no GPUI overlay covers the page; CEF off-screen later if needed.
