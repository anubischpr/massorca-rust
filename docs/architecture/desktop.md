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
   shell      Frame and Canvas, Title bar, Activity rail, Sidebar,
              Status bar, command palette, service notice, workspace dialogs
   model      view models (ShellModel, sidebar tree, status bar, palette) and
              pure derivations: service_view, workspace_flow
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
- **Launch.** `massorca-desktop [--sample] [--profile <name>] [--home <dir>]
  [--no-autostart]`. Profile and `MASSORCA_*` variables follow the CLI. The
  service is started from the `massorca` executable next to the desktop.

## Testing

See 0025: state tests in plain Rust, element tests with GPUI's fake platform,
layout checks with real text shaping in `HeadlessAppContext`, screenshots on
Linux only. No automated run opens a visible window.

## Browser pane

Not built yet. Direction in 0026: a `wry` child web view first, with the rule
that no GPUI overlay covers the page; CEF off-screen later if needed.
