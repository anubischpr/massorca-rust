# MassOrca in Rust: migration plan

Status: draft, 2026-11-12. Nothing is built yet.

## Goal

Rebuild MassOrca as a native application with a Rust core, replacing the
Electron app. One core serves every surface:

| Surface | UI | Core |
|---|---|---|
| Desktop (Windows, macOS, Linux) | GPUI | Rust |
| Android | React Native | Rust client core |
| Server (VPS, headless) | none | Rust |
| CLI | terminal | Rust |

## Decisions already made

1. **Separate project.** This repository starts empty. The current MassOrca
   repository is not modified by this work.
2. **Reimplement the functionality, not the code.** The current app is a
   behaviour reference: we look at what a feature does, then design and write
   our own version. No source is ported line by line.
3. **No compatibility with the current protocol.** The new core defines its
   own protocol. Current Electron and mobile clients will never connect to the
   new server, and no translation layer is built. This keeps us free to model
   things differently from Orca.

## What we are replacing

Measured in the current repository, excluding tests:

| Area | Lines |
|---|---|
| Main process (server and desktop logic) | ~929,000 |
| Renderer (UI) | ~932,000 |
| Shared | ~199,000 |
| Mobile (Expo 55, React Native 0.83) | ~220,000 |
| Relay, CLI, cloud | ~89,000 |

About 2.3 million lines and roughly 13,000 test files. Full parity is not a
realistic first target, so the first version is a deliberately small subset
(see Phase 0).

## Architecture

### One core, one protocol

The core always runs as a service and every surface is a client of the same
protocol:

- On a desktop, the core runs as a local background process and the GPUI app
  connects to it over local IPC. Terminals and agents survive a UI restart.
- On a server, the same binary listens on the network.
- The Android app and a remote desktop connect to a server over the network.
- The CLI is a client too.

There is no second, in-process API. A feature that works from the CLI works
from every UI, and the CLI doubles as the test harness for the core.

### Workspace layout (proposed)

```
crates/
  protocol/        message types, versioning, capability negotiation
  core-workspace/  workspaces: git worktrees and plain folders
  core-git/        git operations through the user's git binary
  core-terminal/   PTY sessions, scrollback, terminal state
  core-agents/     agent launch, status store, hooks
  core-remote/     SSH hosts and remote execution
  core-store/      persistence
  server/          the service binary (local daemon and headless server)
  client/          client library: connection, sync, local cache
  client-ffi/      UniFFI bindings of `client` for React Native
  cli/             command-line client
  desktop/         GPUI application
mobile/            React Native app (Android)
docs/
```

### Rules carried over from the current product

These are product behaviours worth keeping, restated so the new design honours
them from the start:

- A workspace is either a git worktree or a plain folder.
- Remote (SSH) execution is a first-class case, never an afterthought. The
  host that executes work owns its status; losing contact is not proof that a
  process died.
- Agent status lives in one store on the execution host; every UI reads it.
- Clients and servers update independently, so the protocol is versioned and
  new capabilities are negotiated from day one.
- Git runs through the user's own binary, with old versions supported.
- Windows, macOS and Linux are all targets from the first phase.

## How we test without the old clients

- The CLI is the first client. Every core feature gets a CLI entry point and
  is tested through it, end to end.
- Protocol contract tests run a real client against a real server in-process.
- The current app is used by hand as a behaviour reference only.

## Phases

Each phase ends with something usable. Order matters more than dates; no
estimates are given until Phase 0 fixes the scope.

### Phase 0: functional inventory and scope

- List the features of the current app by behaviour, grouped by domain.
- Mark each one: first version, later, or dropped.
- Note where we want to behave differently from Orca and why.
- Decide the open questions below.

Exit: a written scope for the first version, agreed.

### Phase 1: core foundations

Protocol, persistence, the service process, the client library and a skeleton
CLI. Workspaces (create, list, remove) for worktrees and folders.

Exit: `cli` can manage workspaces against a local service on all three
desktop systems.

### Phase 2: terminals and git

PTY sessions that outlive clients, attach and detach, scrollback. Git status,
diff, branch and worktree operations.

Exit: open a terminal from the CLI, disconnect, reconnect, and find it alive.

### Phase 3: headless server and remote

Network listener, pairing and authentication, SSH hosts.

Exit: the service runs on the VPS and the CLI on a desktop drives it.

### Phase 4: agents

Launching agent CLIs, the status store and hooks. Start with one or two
agents; the rest are added one by one later.

Exit: an agent runs in a workspace and its status is visible from the CLI.

### Phase 5: desktop in GPUI

Window, workspace sidebar, terminal panes, agent status. Then editor and diff
views.

Exit: daily work is possible in the GPUI app on the first-version scope.

### Phase 6: Android

React Native UI on top of `client-ffi`. Pairing with a server, workspace list,
terminal and agent status.

Exit: the Android app drives a server for the first-version scope.

### Phase 7: long tail and switch

Remaining agents, source-control providers (GitHub, GitLab and others),
integrations, and whatever Phase 0 marked as "later". Data migration from the
Electron app if wanted. Retire the Electron app.

## Risks

- **Scope.** The current product is very large. Without a strict first-version
  scope the project does not finish.
- **GPUI is pre-1.0.** Breaking changes between versions are expected. Zed 1.0
  ships on all three desktop systems, so the base is proven, but we must pin
  versions and budget for upgrades.
- **No webview in GPUI** (not confirmed in depth). The integrated browser pane
  has no direct equivalent.
- **Upstream is lost.** Improvements made to Orca no longer arrive for free;
  every new feature is ours to build.
- **No ready-made test bench.** Dropping protocol compatibility means our own
  tests carry all the verification from the first day.

## Product decisions (2026-11-12)

- **Browser pane: included.** GPUI has no webview, so a web engine is embedded
  for that pane only. Which engine is a Phase 0 research item.
- **Screens are reused as a starting point.** Current mobile screens and some
  desktop screens are the base for the new design, not a constraint on it.
- **Design comes first.** Before UI development, a design pass validates every
  screen to be built, one complete flow through the app, and the design system
  all components share. See `docs/design/`.
- **iOS: out of the initial scope.**
- **Data migration: required for agent subscriptions.** The owner's agent
  accounts and subscriptions must carry over so agents can be tested.
- **New feature: import sessions from other agents**, so work started in
  another agent tool continues here.

- **Customisation is a design principle, not a feature added later.** The
  product must let its owner adjust it to their preferences and build their own
  applications inside it, in the spirit of DeepSeek Harness, where the model
  adapter, tool registry, session log and agent loop are each replaceable
  plugins, and plugins extend tools, skills and the interface. Consequences:
  the core is built around stable extension points from Phase 1; our own
  features should use those same extension points where practical; the UI has
  defined places where extensions add panels, commands and views; the protocol
  lets extensions add capabilities without a core release. The extension model
  is decided in `docs/decisions/0001-extension-model.md` and designed in
  `docs/architecture/extensions.md`, including the seams Phase 1 must build.

## How the work is organised

- One orchestrating session plans, decides and reviews. It writes no product
  code.
- Work is done by child sessions, which in turn delegate to subagents.
- Child sessions and their subagents run at most on Opus 5.5 at medium effort.

## Open questions

1. What is in the first version? Answered by `docs/inventory/` once the
   functional inventory is reviewed.
