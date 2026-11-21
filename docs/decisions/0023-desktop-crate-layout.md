# 0023: Desktop crates and the view-model layer

Status: accepted, 2026-11-20.

## Context

The desktop app is a client of the core service through the `client` crate,
which another session is building. The design system must be reviewable on
its own (a gallery), and screens must not change when sample data is replaced
by real data.

## Decision

Two crates:

```
crates/ui        massorca-ui       tokens, theme, assets, components, gallery binary
crates/desktop   massorca-desktop  the application: window, shell, view models, data sources
```

- `massorca-ui` knows nothing about the protocol or the service. It depends on
  GPUI and the generated tokens only. Its binary `massorca-gallery` shows every
  component in both themes.
- `massorca-desktop` builds the shell (title bar, Activity rail, Sidebar,
  canvas, Status bar, command palette) from `massorca-ui` components. Its
  binary is `massorca-desktop`.
- **View models.** Screens render from plain view-model structs
  (`ShellModel`, `SidebarModel`, `StatusBarModel`, `PaletteModel`) owned by
  GPUI entities. View models are filled by a `DataSource` trait:

  ```rust
  pub trait DataSource {
      fn snapshot(&self) -> ShellSnapshot;          // first paint
      fn subscribe(&self, sink: SnapshotSink);       // later changes
  }
  ```

  `SampleDataSource` returns fixed data today. A `ClientDataSource` built on
  the `client` crate will implement the same trait, so the views and their
  tests do not change. Views never call the client directly.
- View-model types are the desktop's own; they are mapped from protocol types
  in the data source, so a protocol change touches one mapping, not the views.
- Pure logic (region widths and collapse order, palette filtering, sidebar
  tree flattening) lives in modules without GPUI types and is unit-tested.

## Alternatives

- **One crate.** The gallery would drag in the shell and later the client.
- **A third `desktop-model` crate now.** Not needed until a second consumer
  (for example a test harness) exists; the module boundary is enough today.
- **Views calling the client directly.** Sample data and real data would take
  different paths.

## Consequences

- The gallery builds without the core crates.
- When the `client` crate lands, the work is one new `DataSource`
  implementation and its mapping tests.
