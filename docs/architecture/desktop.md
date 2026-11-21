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
              Status bar, command palette
   model      view models (ShellModel, SidebarModel, StatusBarModel, PaletteModel)
   data       DataSource trait: SampleDataSource now, ClientDataSource later (0023)
   bin        massorca-desktop
```

GPUI comes from one pinned Zed commit (0020). `crates/ui` never depends on
the protocol or the client; `crates/desktop` will, through its data source
only.

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
 DataSource ──snapshot / changes──> view model entity ──notify──> views
     ▲                                     │
     └──────────── actions (later: client requests) ◄──── user input
```

Views read view models only. With sample data the actions update the view
model directly; with the client they become requests and the view model
changes when the service reports the new state.

## Testing

See 0025: state tests in plain Rust, element tests with GPUI's fake platform,
layout checks with real text shaping in `HeadlessAppContext`, screenshots on
Linux only. No automated run opens a visible window.

## Browser pane

Not built yet. Direction in 0026: a `wry` child web view first, with the rule
that no GPUI overlay covers the page; CEF off-screen later if needed.
