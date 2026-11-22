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
  connection ends each attach with `Lost`; the UI attaches again without an
  offset, which gives a snapshot. An attached client must answer the program's queries
  (cursor reports, 0034): the feed's replies go back as input, which the
  integration test does; the UI owner must do the same. The status
  bar counts the terminals that may still run. The work panel does not draw
  terminals yet.
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
  Each event refetches the status. Watches survive a reconnect and end with their connection.
- **Store.** `StoreState.git` holds each workspace's status, first history page and degraded-watch
  state. A removed workspace takes them with it.
- **Models.** `model::source_control` derives the panel from the store; `model::source_control_diff`
  parses a patch into DiffView's data; `model::source_control_flow` holds the discard waivers and the
  commit form.
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
  (SettingsRow.md), the Marketplace and vault pages, the first-run wizard, and
  host rows beyond This PC. Sample data holds no workspace records, so the
  start page shows "not listed by the service" in `--sample` mode.

## Browser pane

Not built yet. Direction in 0026: a `wry` child web view first, with the rule
that no GPUI overlay covers the page; CEF off-screen later if needed.
