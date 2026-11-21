# Extensions: design

Decision and reasons: `docs/decisions/0001-extension-model.md`. This document
is the design the core is built around. Names of types and methods are
proposals the implementing session may refine, as long as the shape holds.

## 1. Overview

```
            ┌───────────── host: one core service (desktop daemon or VPS) ─────────────┐
            │  contribution registry ── owner: core | ext:<id>                         │
            │     ▲            ▲                    ▲                                  │
            │  built-in     data loader         Wasm bridge ── Wasmtime ── component    │
            │  Rust impls   (extension.json,    (WIT world      (Rust, or JS source     │
            │               views, themes...)   massorca:ext)    in bundled JS engine)  │
            │                                   process runner ── MCP server / agent CLI│
            │  permission broker ── grants per extension, per scope                    │
            └──────────────── protocol: capabilities, methods, view documents ─────────┘
                        │                    │                       │
                       CLI            GPUI desktop             React Native Android
                                (render view documents)     (render view documents)
```

- An extension is installed into a **host**. Its code runs there and nowhere
  else. Clients receive data (view documents, command lists, themes) over the
  protocol and render it natively.
- Built-in features and extensions register through the **same registry**. A
  built-in is Rust code implementing a host-side trait. An extension is a
  WebAssembly component that the bridge wraps in that same trait.

## 2. Package and manifest

A package is a directory, distributed as a zip archive (`.mxt`) and addressed
by its SHA-256:

```
extension.json        manifest (required)
views/*.json          declarative view documents
themes/*.json         themes; keybindings.json, locales/*.json, skills/*/SKILL.md, recipes/*.json
main.wasm | main.js   optional logic: a component, or JS source for the bundled engine
icon.svg, README.md
```

Manifest sketch:

```json
{
  "id": "massorca.sticky-wall",
  "name": "Sticky Wall",
  "version": "1.2.0",
  "publisher": "massorca",
  "description": "Notes on a wall, shared with your agents.",
  "api": "1.0",
  "requires": { "massorca": ">=0.6.0" },
  "views": 1,
  "surfaces": ["desktop", "android"],
  "modes": ["workspaces"],
  "main": "main.js",
  "permissions": [
    { "kind": "storage", "reason": "Keeps your notes." },
    { "kind": "net", "scope": { "hosts": ["api.example.com"] }, "reason": "Syncs notes." },
    { "kind": "secrets", "scope": { "names": ["example-token"] }, "reason": "Signs in to the sync service." }
  ],
  "contributes": {
    "commands":   [{ "id": "add-note", "title": "Add note", "handler": "addNote", "keys": "Ctrl+Alt+N" }],
    "views":      [{ "id": "wall", "slot": "canvas-page", "title": "Wall", "document": "views/wall.json" }],
    "tools":      [{ "id": "add_note", "description": "Add a note to the wall", "input": "schemas/add_note.json" }],
    "events":     [{ "on": "workspace.created", "handler": "onWorkspace" }],
    "settings":   [{ "key": "columns", "type": "integer", "default": 4, "title": "Columns" }]
  }
}
```

Rules:

- **Identity.** `id` is `publisher.name`, lowercase, and stable forever.
  `version` is semver.
- **Compatibility fields.**
  - `api` is the minimum WIT world version.
  - `requires.massorca` is the minimum host version.
  - `views` is the view-schema major.
- **Surfaces.**
  - `surfaces` restricts where the UI contributions appear.
  - `modes` follows the design (Workspaces, Bot Teams...).
  - Logic and tools are host-side and do not depend on surfaces.
- **No `main`** means a data-only extension. It cannot declare `handler`s.
- **Validation.** The manifest is validated against a published JSON Schema.
  An unknown contribution kind or permission kind is an error. It is never
  ignored, so a typo cannot silently grant or drop anything.
- **Risk level.** The manifest cannot set its own risk level. The core
  computes it (§6).

## 3. Contribution kinds

These kinds form a closed list. A new kind comes with a minor WIT/manifest
version.

| Kind | Tier | What it adds | Inventory evidence | Built-in users of the same point |
|---|---|---|---|---|
| `commands` | data (+logic handler) | Palette command, optional key, context-menu or composer action; handler is a protocol method or extension function | SKL-21, CUS-07 quick commands | every CLI/palette command (§10) |
| `keybindings` | data | Key to command map | SET keybindings | default keymap |
| `themes`, `iconThemes` | data | Colour tokens per `docs/design/tokens.json` | CUS-06, TERM-21 | built-in themes |
| `importers` | logic | Convert a foreign file into a theme, keymap or recipe | SET-28, TERM-22 (Ghostty, Warp) | built-in Ghostty importer |
| `locales` | data | UI strings | language packs | English |
| `skills` | data | `SKILL.md` folders placed for agents | SKL-01..09 | bundled skills |
| `recipes` | data | Environment / VM recipes | WS-27, CUS-02 | built-in recipes |
| `agentRules` | data | Declarative agent-state rules | CUS-05 | bundled rules |
| `settings` | data | Typed settings in a settings section | design D-SL-06 | every core settings section |
| `views` | data (+logic data) | A view document placed in a UI slot (§7) | design's 14 points, SKL-21 | settings sections, status items |
| `events` | logic | Handler for a lifecycle event (§4 `events`) | WS-13, WS-15 | automations, agent status hooks |
| `tools` | logic | A tool agents can call, served to agents over MCP by the core | CUS-10, DSH model | core's own agent tools |
| `itemSources` | logic | Tracker items (issues, cards) for task lists and boards | WS-08, SCM-04..06, SCM-20/21, SCM-27 | GitHub issues and PRs |
| `jobs` | logic | Scheduled handler (cron-like), runs on the host | automations | automations scheduler |
| `agentAdapters` | logic + process | Launch, resume, status and transcript mapping for an agent CLI | CUS-03 (DSH), AG-19, AG-20 | Claude Code, Codex, OpenCode, Gemini |
| `mcpServers` | process | An MCP server the core launches and offers to agents | CUS-10, CUS-11 | Engram/OpenMetadata entries |

Not kinds (core owned): store, protocol, permission broker, session history,
terminal engine, workspace model. Shell providers (TERM-10) and notebook
kernels (SET-25) map onto `agentAdapters`-style process contributions plus
views, and get their own kind only when a first one is built.

## 4. Host API (WIT world `massorca:extension@1.0.0`)

The core exports these interfaces to components. Every import is checked
against the extension's grants before it does anything.

| Interface | Functions (sketch) | Permission |
|---|---|---|
| `log` | `log(level, message)` (goes to the extension log the design shows in Logs) | always |
| `storage` | `get`, `set`, `delete`, `list(prefix)` on a per-extension store, scope `global` / `project` / `workspace` | `storage` |
| `settings` | `get(key)`, `watch(key)` on the extension's own declared settings | always |
| `workspaces` | `list`, `get`, `files.read`, `files.write`, `files.list` inside a workspace | `workspaces:read` / `files:read` / `files:write` (scoped) |
| `http` | `fetch(request, auth?: secret-name)`; the host injects the secret | `net` (hosts), `secrets` (names) |
| `process` | `spawn(command, args, cwd, env)` with streamed stdio | `process` (command patterns) |
| `terminals` | `list`, `read`, `write`, `open` in a workspace | `terminals:read` / `terminals:write` |
| `agents` | `list`, `status`, `send-message`, `start` | `agents:read` / `agents:control` |
| `sessions` | read conversation history of agent sessions | `sessions:read` |
| `notify` | toast and notification card | `notifications` |
| `ui` | `refresh(view-id)`, `open(view-id)`, `dialog(spec)`, `secret-prompt(name)` | always (dialogs cannot alter core confirms) |
| `events` | subscribe at runtime beyond the manifest | as for the event's domain |

Components export the handlers the manifest names:

- `handle-command`, `on-event` and `run-job`;
- `call-tool`;
- `view-data` and `view-action`;
- `item-source.query` and `item-source.get`;
- the agent adapter functions: `launch-spec`, `parse-status` and
  `map-transcript`.

**Lifecycle hooks.** Every export is optional. The only lifecycle exports are
`activate` and `deactivate`. Everything a component registers at runtime
returns a handle that the host drops on deactivate, so an unload leaves
nothing behind (Harness's reversible effects).

**Long-running work.** Agent adapters and jobs use streams and host-driven
calls rather than long-blocking component calls. This holds until the WASI
0.3 async spike (decision §Consequences) shows that async exports are
dependable.

**Limits** (defaults; set by the core, not by the manifest):

- Each call gets an epoch deadline: 5 s for view data and commands, 60 s for
  jobs and tools.
- Memory is capped at 128 MiB per instance.
- Three crashes in 10 minutes disable the extension and show "Stopped after
  repeated errors" on its card.

## 5. Lifecycle

```
source ─► fetch ─► verify ─► review ─► install ─► enabled ─► activate (lazy)
                     │          │                    │  ▲
                     ▼          ▼                    ▼  │
                  refused    cancelled            disabled ─► removed
```

1. **Fetch.** The archive comes from an index entry, a URL, a file or a dev
   folder. Index entries pin `version` plus `sha256`, and a mismatch refuses
   the install.
2. **Verify.**
   - The manifest schema is valid and `api` is supported by this host.
   - `requires.massorca` is satisfied.
   - Every referenced file is present.
   - The view documents validate.
   - The component's imports are a subset of the WIT world.
   - The package is not on the revocation list.

   Failure leaves nothing behind ("Not installed: \<reason\>. Nothing was
   changed.").
3. **Review.** The permission review (D-EX-02) shows each permission with its
   reason and the computed risk. High risk needs the checkbox.
4. **Install.**
   - The package is unpacked to `<profile>/extensions/pkgs/<id>/<version>-<hash8>/`,
     read-only after unpacking.
   - The lockfile `<profile>/extensions/lock.json` records the source, hash,
     granted permission fingerprint, enablement scope (all projects or the
     listed projects) and the time.
5. **Enable and activate.**
   - Data contributions register at once.
   - A component is instantiated lazily, on the first call to one of its
     handlers, and dropped after it has been idle (default 10 min).
6. **Update.**
   - The new version is verified.
   - If its consent fingerprint (§6) is unchanged, it may update
     automatically (setting, off by default). A changed skill, rule or recipe
     therefore always goes back through review.
   - Otherwise it waits in Updates with the review listing the new
     permissions first.
   - The previous version stays on disk for Roll back.
7. **Disable.** Unregisters everything. Files and data are kept.
8. **Remove.** Deletes the package, its storage
   (`<profile>/extensions/data/<id>/`) and its settings, after X-DLG-01.
9. **Reload.** For dev installs only: re-reads the dev folder. If the new
   build fails verification, the last good version keeps running.

Host restarts re-register enabled extensions from the lockfile without
contacting any index.

## 6. Permission model

Closed vocabulary. Scopes narrow a kind. A permission without a scope means
"any". Risk is computed by the core from kind and scope.

| Kind | Scope | Risk |
|---|---|---|
| `storage` | none | Low |
| `notifications` | none | Low |
| `workspaces:read` | none | Low |
| `files:read` | `paths`: globs relative to a workspace root | Medium; whole disk or home is High |
| `files:write` | `paths` | Medium; whole disk or home is High |
| `net` | `hosts`: exact or `*.domain` | Medium; any host is High |
| `secrets` | `names` | Medium |
| `terminals:read` | none | Medium |
| `terminals:write` | none | High |
| `agents:read` | none | Low |
| `sessions:read` | none | Medium |
| `agents:control` | none | High |
| `process` | `commands`: program plus argument patterns | High |
| any process contribution (`mcpServers`, adapter binaries) | the declared command | High, with the copy "runs a program on this computer with your rights" |

Some contribution kinds carry risk even though they run no code. They count
toward the computed risk like permissions:

| Contribution | Risk | Review copy |
|---|---|---|
| `skills`, `agentRules` | High | "Instructs your agents, which act with your rights." |
| `recipes` that contain commands | High | "Runs setup commands in your workspaces." |
| `commands` whose handler is a core method | the risk of the permission that method's class maps to | — |
| `themes`, `iconThemes`, `locales`, `keybindings`, `settings`, `views` | Low | — |

- **Enforcement.**
  - Components get no WASI preopens, sockets or environment. The only path
    to the outside is the host imports above, and each import checks the
    grant first.
  - Process contributions are launched by the core with only the declared
    command, a scrubbed environment and the granted secrets. Nothing confines
    them beyond that, and the review says so.
- **Grant.** All declared permissions are granted together at review. Consent
  is stored as a fingerprint over two things: the sorted permission set, and
  the hash of the agent-facing files (skills, agent rules, recipes). The
  package hash is recorded in the lockfile separately.
- **Revoke.**
  - In Marketplace > Installed > Settings, each permission has a switch.
  - A revoked permission makes its calls fail with `permission.denied`. The
    extension sees the error and may degrade.
  - Turning a permission back on needs no new review.
- **Enablement scope.**
  - "Enable in this project" limits the extension to one project. File and
    workspace scopes then resolve only inside that project's workspaces.
  - "All projects" is the other choice.
- **Secrets.**
  - Secrets live in the OS vault the core already uses.
  - An extension names a secret. It never reads the value: `http.fetch` takes
    `auth: <name>`, and process contributions receive granted secrets as
    environment variables at launch.
  - Each secret is bound to the hosts it may be sent to, for example
    `{ "name": "example-token", "hosts": ["api.example.com"] }`. The broker
    refuses to inject it into a request for any other host, so a second `net`
    host cannot receive it. The review shows each name with its hosts.
  - A process contribution that receives a secret cannot be held to hosts,
    which is one more reason it is High.
  - The Secret modal is the only way to create one for an extension.
- **Calls into the core.** Any core method that an extension's contribution
  invokes runs as principal `Extension(id)`. This covers calls from its code
  and data-only commands and keybindings, **even when the user's click or
  keypress triggered it**. The principal comes from the contribution's owner,
  never from the client connection.
  - Each method permission class (`0002-protocol.md`) maps to a manifest
    permission. A command whose handler is a core method needs that
    permission declared, and the permission feeds the risk.
  - `host-admin` maps to nothing, so no extension can reach it.

## 7. UI contribution model

**View documents.** A view document is JSON with a fixed widget set,
versioned as a whole (`views: 1`):

- layout: `stack`, `row`, `grid`, `section`;
- content: `text`, `markdown`, `badge`, `icon`, `image` (package or data
  URL only), `list`, `table`, `board` (columns of cards, for the workspace
  board, GitHub Projects and Sticky Wall);
- form: `field` (text, number, select, toggle, date), `form`;
- actions: `button`, `menu`;
- feedback: `empty`, `progress`.

Each widget can carry `fallback`, a text that an older client shows in place
of a widget it does not know. That covers new widgets within one major. Across
majors, the client lists the `views` majors it renders in `hello`. The host
filters the `contributions` stream by that list, and replaces a view the
client cannot render with one placeholder, "Update MassOrca to see this".

Styling comes only from design tokens: `tone`, `size` and `emphasis`
variants. There is no raw colour, font or pixel layout, so every extension
matches `docs/design/` on both clients.

**Data and actions.**

- A document has `{{bindings}}`, which the host fills from the extension's
  `view-data(view-id, context)` export. The context carries the project,
  workspace and selection.
- Each action is a `{ handler, args }` pair. A click sends
  `ext.<id>.view-action` to the host, which calls the component and then
  pushes the updated data on the view's stream.
- Clients never run extension code and never fetch from the network for an
  extension.

**Slots** (the design's 14 points, `design-system.md` "Extension contribution
points"):

| Slot | Design id |
|---|---|
| `project-sidebar-tab` | D-SL-01 |
| `sidebar-section` | D-SL-05 |
| `work-panel-tab` | D-SL-08 |
| `canvas-page` | D-SL-04 |
| `session-summary-card` | (design table) |
| `conversation-card` | (design table) |
| `palette-command` | D-SL-07 |
| `context-menu` | (design table) |
| `composer-action` | (design table) |
| `model-picker-entry` | D-SL-10 |
| `rail-icon` | D-SL-09 |
| `status-item` | (design table) |
| `settings-section` | D-SL-06 |
| `notification` | X-NT-01 |

The per-slot caps in the design (two sidebar tabs, label lengths, five menu
items, the card height) are enforced by the core when it registers a view, not
left to clients.

**Clients.**

- **GPUI** renders documents from M4. Core settings sections and status items
  use the same renderer, so it is exercised before any third-party extension
  exists.
- **Android** gets the React Native renderer in an M6 batch. The design shows
  no extension UI on Android in v1. The protocol already carries documents,
  so Android needs no change to extensions, only a renderer.

**Web views.** Not available. If owner item 17 approves them, they become a
`webview` slot variant with `surfaces: ["desktop"]` and a strict no-network
CSP. Nothing above changes.

## 8. Protocol

- **Capability `extensions/1`.** The host offers it from M6. Its methods:
  - `extensions.list`, `extensions.get`;
  - `extensions.inspect(source)`: verify and return the review data, install
    nothing;
  - `extensions.install(source, consent-fingerprint, scope)`;
  - `extensions.update`, `extensions.rollback`;
  - `extensions.set-enabled`, `extensions.set-permission`;
  - `extensions.remove`;
  - `extensions.logs` (a stream);
  - `extensions.dev.reload`;
  - `marketplace.search`, `marketplace.sources.*`.
- **Per extension.** Each enabled extension adds the capability
  `ext/<id>@<major>` and the methods `ext.<id>.*`, such as `view-data`,
  `view-action` and `command`.
- **`contributions` stream.** It pushes contribution lists per slot when
  something is enabled, disabled or updated, so clients never poll.
- **Older hosts.** A client that meets a host without `extensions/1` hides
  Marketplace for that host. A `method.not_found` from an extension method
  means "disabled or removed since"; the client drops the contribution.
- **Several hosts.** A desktop connected to its local service and to a VPS
  shows each host's contributions under that host's projects. Personal
  contributions (themes, keybindings, locales) come only from the local
  service. On Android, they come from the paired server.

## 9. Phase 1 extension points (act on these now)

These are seams, not features. Each is small and is cheaper now than as a
refactor later.

1. **Method ownership.** Each method registry entry (decision 0002) carries
   an `owner`: `core` or `ext:<id>`, next to its permission class. The prefix
   `ext.` is reserved for extension methods, and the core never defines a
   method under it.
2. **Dynamic capability registry.**
   - Capabilities are registered with an owner through the same registry
     that core features use. Phase 1 registers `service/1` and
     `workspaces/1` there, with no hard-wired list in the dispatcher.
   - Registration returns a handle that unregisters on drop.
   - `welcome` reads from the registry.
3. **Caller identity.**
   - Each request context carries a `Principal`. In Phase 1 the only value
     is `Owner`.
   - The type is a non-exhaustive enum. `Device(id)` comes in Phase 3 and
     `Extension(id)` at M6.
   - The permission check takes (principal, permission class) in one
     function.
   - Handlers receive the principal from the request context. A later
     dispatch on behalf of a contribution sets it from the contribution's
     owner, so this must not be read from the connection inside handlers.
4. **Event bus.**
   - A typed, in-process bus. Phase 1 has `workspace.created`,
     `workspace.removed` and `workspace.removing`, emitted after the store
     commit.
   - Subscriptions return a drop handle.
   - The `events/1` protocol stream reads from this bus, not from handler
     code, so extension handlers, automations and clients share one source.
5. **Contribution registry.**
   - A generic `Registry<K>` keyed by kind, with entries carrying an owner and
     a drop handle.
   - Phase 1 uses it for **commands**: a static table of command descriptors
     (id, title, method, argument schema), registered with owner `core`. A
     test checks that every CLI subcommand has a descriptor. The CLI stays an
     ordinary argument parser; it is not generated from the table. The
     palette and extension commands later read the same registry.
6. **Settings keyed by owner.** The settings table key is `(owner, key)`, and
   each setting is declared with a type and default in a settings registry.
   `massorca config get/set` validates against the declaration. Core uses
   owner `core`.
7. **Reserved names.**
   - Error codes `permission.denied` and `extension.*`.
   - The CLI group `massorca ext`, which is unimplemented until M6.
   - The data directory `<profile>/extensions/` (decision 0004), with
     `pkgs/`, `dev/`, `data/` and `lock.json`.
8. **Additive evolution of the manifest-facing types.** Protocol types that
   extensions will see (workspace, project, event payloads) follow 0002's
   additive rule from the start, and have no fields that leak core-internal
   identifiers.

Later seams, by milestone:

- **M2.** The capability set can change on a live connection. Add a
  `service.capabilities-changed` event.
- **M3.**
  - An `AgentAdapter` trait plus registry. Built-in agents register through
    it; none is special-cased in the launcher.
  - A `Tool` registry served to agents over MCP by the core.
  - A `Skill` placement registry.
- **M4.**
  - The view-document schema `views: 1` and the GPUI renderer.
  - The `contributions` stream.
  - A `Theme` registry fed by data files.
- **M6.** Everything else in this document.

## 10. Our own features on the same points

| Feature | Point | Form |
|---|---|---|
| CLI commands, palette, quick commands (CUS-07) | `commands` | built-in descriptors; quick commands are data contributions owned by `user` |
| Automations, agent status hooks, Bot Teams triggers | event bus, `jobs` | built-in Rust |
| Claude Code, Codex, OpenCode, Gemini | `agentAdapters` | built-in Rust |
| DeepSeek Harness, Pi, Antigravity | `agentAdapters` | first-party extensions (M6+) |
| GitHub issues and PRs | `itemSources` | built-in Rust |
| GitLab, Jira, Linear | `itemSources` | built-in or first-party extension, decided per batch |
| Built-in themes, Ghostty/Warp import | `themes`, `importers` | data files; importer built-in |
| Core settings sections | `settings`, `views` | built-in, rendered from view documents |
| Bundled skills, MCP entries (Engram, OpenMetadata) | `skills`, `mcpServers` | data contributions owned by `core` |
| Workspace board, GitHub Projects, notebook viewer | `views` (`board`, `canvas-page`), `itemSources`, process | **first-party extensions** at M6 |
| Prototypes | core (pending owner confirmation; if reversed, it becomes an extension) | — |

## 11. Creator mode

The design is `screens/teams-extensions-settings.md` (Creator mode). The
mechanism:

1. **Start.** The user starts Creator mode from a Composer, or from "Add
   extension" > "Create with chat". The core creates
   `<profile>/extensions/dev/<id>/` on the host from a template and opens an
   ordinary agent session there. The agent is any installed agent.
2. **Skill.** The session gets a bundled `extension-author` skill. It tells
   the agent to use:
   - `massorca ext api`: the WIT world, the manifest and view schemas, and
     the contribution and permission references, as text (API discovery, like
     Harness's `cordis_inspect`);
   - `massorca ext check`: validate the manifest, views and component imports.
     For JS source, the host itself compiles the snapshot; the agent needs no
     tool;
   - `massorca ext reload`: re-register the dev install;
   - `massorca ext logs`;
   - `massorca ext pack`.
3. **Preview.** The dev install is visible only to the Preview tab of that
   session, in the app skeleton with the extension "in its real place".
   - The dev install runs with its declared permissions. The first reload
     that declares anything above Low shows the permission review in the
     session.
   - The Select, Note and Draw tools and "Send to agent" are client features.
     The notes go into the next message.
4. **Hand edits.** Files are editable in the Files tab. The agent sees changes
   on its next turn.
5. **Enable.** "Enable in this project" packs the folder, runs the normal
   verify and review, and installs it with the project scope. The lockfile
   records the source as `dev:<path>`.
6. **Share.** `massorca ext pack` produces an archive. Publishing to an index
   is outside the app in v1.

**Fallback if the JS spike fails.** Creator mode writes Rust components. It
builds them on the host with `cargo` and the `wasm32-wasip2` target;
`massorca ext new` checks for that toolchain and offers to install it. Creator
mode never turns logic into a process contribution to avoid the sandbox. A
process contribution it writes goes through the High review like any other.

## 12. Delivery

| Milestone | Extension work |
|---|---|
| M1 | §9 items 1 to 8 |
| M2 | capability changes on a live connection; per-host contribution scoping in the client library; client-declared `views` majors in `hello` (an optional field) |
| M3 | `AgentAdapter`, `Tool` (served over MCP), `Skill` registries; built-ins use them |
| M4 | view schema v1, GPUI renderer, `contributions` stream, theme registry; core settings via views |
| M5 | none (Android shows no extension UI in v1) |
| M6 (batch 1) | Spikes first: the JS-engine component and WASI 0.3 async. Then Wasmtime bridge, manifest and verify, lockfile, permission broker and review, Marketplace with the signed official index, Installed, Updates, Creator mode |
| M6 (batch 2) | First-party extensions: workspace board, GitHub Projects, notebook viewer; DeepSeek Harness adapter |
| M6 (later) | React Native renderer; third-party indexes; per-publisher signing; web views if approved |

## 13. Open items

- **Owner item 17:** web views for extensions on desktop.
- **Owner item 1:** prototypes, core or extension.
- **Spike:** a JS-engine component that loads source at runtime (size, start
  time, async host calls).
- **Spike:** WASI 0.3 async in Wasmtime for adapters and jobs.
- **Process sandboxing per OS** for process contributions. There is no plan
  yet.
- **Design conflicts to resolve in the design docs, not here.**
  - Roll back as review versus confirm.
  - The Android Marketplace rows.
  - Sticky Wall's web view; it is redrawn as a `board` view in this design.
