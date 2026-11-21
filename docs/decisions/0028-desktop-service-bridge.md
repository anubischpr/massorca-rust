# 0028: How the desktop UI talks to the service

Status: accepted, 2026-11-21.

## Context

The `client` crate is async on tokio: its connection, calls and
subscriptions use tokio sockets and tokio channels. GPUI runs its own
executors and has no tokio reactor, so a tokio socket polled from a GPUI task
panics. The UI thread must never wait on the service: a slow or missing
service has to show as a state in the shell, not as a frozen window.

Decision 0023 put a `DataSource` behind the view models so that sample data
and real data reach the views the same way. This record says how real data
gets there.

## Decision

- **One bridge, one runtime.** `ServiceBridge` owns a tokio runtime on its own
  thread (`massorca-service`, one worker). Every client call and every
  subscription runs there. GPUI code never touches a tokio socket. Tokio
  channels do not need a reactor to be awaited, so GPUI tasks await their
  receivers directly.
- **Requests.** A view model asks the bridge for a call and awaits the answer
  inside `cx.spawn`. The bridge runs the call on its runtime and returns the
  result through a oneshot channel. The UI thread is never blocked.
- **Connection states.** The bridge keeps the connection and publishes its
  state on a watch channel. The states are `Connecting`, `Connected`
  (with the service's welcome), `Reconnecting { attempt, retry_at }` and
  `Unreachable { reason }`. A lost connection goes to `Reconnecting` with
  backoff from 250 ms, doubling up to 5 s. After the first failed attempt the
  shell shows the service-unreachable state; "Retry now" resets the backoff.
  The desktop connects with the same autostart rule as the CLI.
- **One store, derived view models.** A `ServiceStore` entity holds the
  projects and workspaces the service reported. It is the only state that
  changes when the service changes. The Sidebar, status bar and dialogs derive
  their view models from it with plain functions that tests call without
  GPUI.
- **Subscriptions.** After each connect the bridge subscribes to events
  first, then lists projects and workspaces, so no change falls between the
  two. Events carry only a topic and an id. For each one the bridge fetches
  the item again, or drops it on a `*.removed` topic. It sends a
  `StoreUpdate` over an unbounded channel. A GPUI foreground task applies each
  update to the store and notifies once per batch. A reconnect, or
  `stream.overflow`, triggers a full resync.
- **Changes come back as events.** Create and remove show a busy state until
  the call answers. The store changes only through the resulting event or the
  refetch, never optimistically. A service refusal becomes a typed `Refusal`
  that picks its dialog:
  - `workspace.dirty` asks to discard changes (`force`);
  - `workspace.ignored_files` has its own waiver (`discard_ignored`);
  - `workspace.orphan` offers to forget the row (`forget_orphan`);
  - `workspace.locked` and `workspace.main_worktree` are plain refusals.

  Each waiver retries the same call with exactly one more flag.
- **Sources stay selectable.** `SampleDataSource` fills the same store once
  and never changes it. `ClientDataSource` drives the store through the
  bridge. The desktop uses the client by default. `--sample` selects sample
  data. The gallery and the view tests always use sample data.

## Consequences

- Views, view models and the store do not know which source is active.
  Tests cover the reducer, the view-model derivation and the refusal mapping
  without a service. One integration test starts a real service in a
  temporary profile and drives the bridge headlessly.
- The tokio runtime costs one thread and starts only with the client source.
- A newer service can send topics this build does not know. They are ignored
  and do not trigger a resync, so mixed versions keep working (0002).

## Alternatives considered

- **Running client futures on GPUI's background executor.** Tokio sockets
  need a tokio reactor and panic without one.
- **A runtime per call.** It throws away the connection and its
  subscriptions on every call.
- **Blocking calls on the UI thread.** A missing service would freeze the
  window instead of showing the unreachable state.
- **Optimistic updates.** They would need rollback on every refusal, and the
  service already reports each change as an event.
