# 0001: Extension model

Status: accepted, 2026-11-20. Design detail: `docs/architecture/extensions.md`.

## Context

`docs/PLAN.md` makes customisation a design principle: the owner must be able
to adjust the product and build their own applications inside it, in the
spirit of DeepSeek Harness. The core must be built around stable extension
points from Phase 1, and our own features should use those points where
practical. What an extension is, in which language, and how it is isolated
and distributed was left open (`docs/inventory/open-questions.md` items 18 to
29; `docs/inventory/customisation.md` Part C).

Facts that constrain the answer:

- **One core, one protocol.** The core always runs as a service, on the
  desktop and on the headless server. GPUI desktop, React Native Android and
  the CLI are clients. Execution may be remote (SSH, VPS), and the host that
  executes work owns it.
- **GPUI has no web view, and Android cannot download native code.** Google
  Play forbids executable code fetched outside the store
  (`customisation.md` C.1). An extension's UI must therefore reach both
  clients without shipping code to them.
- **The design is already drawn.**
  - Marketplace has Discover, Installed and Updates.
  - Installing goes through a permission review (D-EX-02). The core computes
    the risk level; the manifest cannot set it.
  - There are 14 desktop contribution points, each marked with its `modes`
    and `surfaces`, and Android has none in v1.
  - Extensions are created by an agent in a normal session in Creator mode,
    which writes `extension.json` and `views/*.json`, with a live Preview tab
    (agreements A62 to A68; `screens/teams-extensions-settings.md`).
- **Owner direction.** The workspace board, GitHub Projects and the notebook
  viewer are extensions, not core. Prototypes was moved to core on the owner's
  behalf, pending their confirmation.
- **What the inventory actually extends.** The inventory gives evidence for
  these kinds of extension:
  - agent adapters: DeepSeek Harness, Pi, Antigravity;
  - trackers as item sources: GitLab, Jira, Linear, GitHub Projects;
  - panels, commands and keybindings;
  - lifecycle hooks;
  - themes and theme importers;
  - language packs;
  - environment recipes;
  - agent-state rules;
  - skills;
  - MCP server entries;
  - notebook kernels;
  - shell providers.

  It shows **no** need to replace the session log, the store, the protocol or
  an agent loop. MassOrca orchestrates external agent CLIs and has no agent
  loop of its own.
- **Prior art** (sources below):
  - DeepSeek Harness makes every part an in-process plugin with host
    permissions and no per-plugin permissions.
  - Zed runs extension code as WebAssembly components behind named,
    user-granted capabilities, and adds no UI panels.
  - VS Code, Claude Code plugins and Raycast run extensions with the user's
    full privileges and rely on review.
  - MCP is the standard for tools that agents call.

## Decision

1. **What an extension is.** An extension is a versioned package, identified
   as `publisher.name`, and installed into one core service, the host. Its
   `extension.json` manifest declares the contributions it adds from a closed
   list of kinds. There are three tiers:
   - **Data contributions**, which run no code: commands, keybindings, themes,
     language packs, skills, environment recipes, agent-state rules, settings
     sections and declarative views. Most extensions only need these.
   - **Logic contributions**, which run sandboxed code: agent tools, item
     sources, event handlers, view data and actions, scheduled jobs, agent
     adapters and importers.
   - **Process contributions**, which declare an external program the core
     launches: MCP servers, and the agent CLI an adapter drives. These are
     not sandboxed and are always reviewed as High risk.

   Core internals are not extension kinds: the store, the protocol, the
   session log and permission checks. Building our own features on the same
   points keeps them replaceable where it matters, without making the
   security boundary itself swappable.

2. **Runtime.** All extension logic runs as **WebAssembly components on
   Wasmtime**, against one versioned WIT world, `massorca:extension`, and only
   on the host. Clients never run extension code.
   - **Authors** can write a compiled component (Rust first), or plain
     JavaScript/TypeScript source. The source runs inside a JavaScript-engine
     component that the core bundles. Neither path needs a toolchain on the
     user's machine, which is what makes Creator mode work.
   - **UI** is a **declarative view document**: a fixed, versioned set of
     widgets in JSON. GPUI renders it natively on the desktop and React Native
     renders it natively on Android. Extension code supplies the data and
     handles the actions on the host.
   - **No web views in extensions for now.** Owner item 17 stays open, and a
     web view can be added later as a desktop-only surface without breaking
     anything.

3. **Isolation and permissions.**
   - Permissions are a closed vocabulary with scopes. Each permission is
     declared in the manifest with a reason, and the core computes the risk
     level.
   - Some data contributions count toward the risk as well. Skills, agent
     rules and recipes that contain commands are High, because agents act on
     them with the owner's rights.
   - Permissions are granted in the install review. Consent is bound to a
     fingerprint of the permission set and the agent-facing files.
   - An update that adds permissions, or changes a skill, rule or recipe,
     waits for a new review. The previous version keeps running until then.
   - Anything an extension's contribution invokes runs as the extension's
     principal, even when a user's click triggered it. That includes
     data-only commands and keybindings.
   - The user can revoke any single permission. A denied call returns
     `permission.denied`.
   - The WebAssembly components have no ambient access. Every file, network,
     process, terminal, session and secret operation goes through a host
     import that checks the grant.
   - Secrets are used by name and bound to the hosts they may be sent to. The
     host injects them into requests it performs, so extension code never
     sees their values.
   - Process contributions cannot be confined in this version, and the review
     says so plainly.
   - Each component gets fuel or epoch limits and a memory cap. An extension
     that keeps crashing is disabled.

4. **Manifest, versioning and compatibility.**
   - `extension.json` declares:
     - the WIT API version it needs (`api`, for example `"1.2"`);
     - the minimum host version;
     - the view schema version;
     - the surfaces it works on;
     - its permissions and its contributions.
   - The host keeps bindings for the current and the previous WIT major. Minor
     versions only add things.
   - Over the protocol, an installed extension adds its own capability
     (`ext/<id>@<version>`), and its methods use the `ext.<id>.` prefix.
   - A client renders only the view schema versions it knows. It declares
     them in `hello`, and the host filters views by that list. For a newer
     widget within a known major, the client shows the widget's fallback
     text, the way Adaptive Cards do.

5. **Distribution.**
   - A package is an archive addressed by its SHA-256. It is installed
     immutably under the host's data directory and recorded in a lockfile
     with its source, hash and consent.
   - Packages come from four sources:
     - the official Marketplace index, a JSON file signed with our Ed25519
       key, which pins every entry to a version and a hash and carries a
       revocation list;
     - third-party indexes the user adds, marked "Third party";
     - a local archive;
     - a development folder.
   - A private index never falls back to a public one.
   - Updates appear in Updates. Automatic updates happen only when the
     permission fingerprint is unchanged. The previous version is kept so the
     user can roll back.
   - Per-publisher signing and a "verified" mark wait until there are outside
     publishers.

6. **Creator mode.** Creator mode is an ordinary agent session working in the
   extension's development folder on the host, as the design draws it.
   - The agent works through `massorca ext` CLI commands:
     - `api` prints the WIT world, the view schema and the contribution
       reference, so the agent can discover the API;
     - `new`, `check`, `reload` and `logs` build and test the extension;
     - `pack` and `install` package and install it.
   - The Preview tab renders the development install, scoped to that session.
   - "Enable in this project" runs the same permission review as any other
     install. Code an agent wrote is third-party code.
   - If the JavaScript-engine spike fails, Creator mode writes Rust
     components and builds them on the host with `cargo`. It never moves
     logic into an unsandboxed process to avoid the sandbox.

7. **Phase 1 seams.** The Phase 1 session builds the seams listed in
   `docs/architecture/extensions.md` §9:
   - method and capability ownership;
   - a caller identity on every request;
   - an event bus;
   - a contribution registry with reversible registration;
   - settings keyed by owner;
   - reserved names, errors and directories.

   Our own features use these seams: commands, lifecycle events, agent
   adapters, item sources, themes, settings sections, skills and MCP entries.
   The workspace board, GitHub Projects and the notebook viewer ship as real
   first-party extensions at M6, to prove the model.

8. **Phasing.** M1 to M3 build only the seams; M4 builds the GPUI view
   renderer, used by our own settings sections. The runtime, Marketplace,
   permission review and Creator mode come at M6.

## Alternatives considered

- **In-process plugins with host permissions**, as in DeepSeek Harness, the
  old app's Node worker, and VS Code. This is the most flexible and the
  simplest to load. Rejected: a plugin written by an agent in Creator mode
  would run with the owner's full rights inside the service that holds their
  credentials, on every host including the VPS, and the permission review the
  design draws would be decorative. We take Harness's logic instead:
  - reversible registrations;
  - load order from declared dependencies;
  - a check before install, and a restore when an install fails;
  - pinned indexes, rejected whole on any defect;
  - a host-version requirement in the manifest.
- **Out-of-process extensions over our own protocol, in any language.** These
  fit "one core, one protocol" and are easy to author. Rejected as the
  default: the process has full machine access unless each OS gets a sandbox,
  which is months of platform work. Each extension would also cost a process
  on a small VPS. The idea is kept narrowly as process contributions (MCP
  servers, agent CLIs), which must run real binaries anyway, and they are
  labelled honestly.
- **An embedded scripting language in process (Lua, Rhai, QuickJS).** Small
  and fast. Rejected: the only boundary would be our own host API inside the
  core process, so a bridge bug becomes a core compromise. JavaScript stays
  available as an authoring language, but it runs inside a WebAssembly
  component.
- **MCP as the whole extension model.** MCP covers agent tools well, but has
  no views, events, item sources or commands. We use it for what it is for:
  extensions can declare MCP servers, and the core serves extension tools to
  agents over MCP.
- **Extism-style core WebAssembly modules.** Simpler to embed, but the
  contracts are untyped bytes or JSON. WIT gives typed, versioned interfaces
  and multi-version host bindings, which is Zed's proven route.
- **Web views (sandboxed HTML) for extension UI.** The most expressive, and
  MCP Apps standardises it. Deferred, not rejected:
  - GPUI would need the embedded engine that is planned only for the browser
    pane;
  - Android would need a WebView bridge with store-policy caution;
  - the design keeps declarative views as the v1 rule;
  - owner item 17 is open.

  The manifest's `surfaces` field already lets a later web-view contribution
  declare itself desktop-only.
- **Compiled-in Rust panels for extensions.** These suit first-party core
  features only: no sandbox and no separate distribution. Our built-ins are
  Rust implementing the same host-side traits through the same registry.
- **Replaceable session log, agent loop and store, as in Harness.** Not
  needed by anything in the inventory. They would turn the core's integrity
  and security boundary into a plugin. This can be revisited if a real need
  appears.

## Consequences

- **WebAssembly from the start.** The core takes on Wasmtime and a WIT world
  it must keep stable. Bindings for one previous major must be maintained.
- **Two spikes before M6 commits.**
  1. The bundled JavaScript-engine component: feasibility, size, start time,
     and whether async host calls need WASI 0.3.
  2. WIT async for long-running work (agent adapters, jobs), measured against
     today's Wasmtime.
- **Limited UI.** Extension UI is limited to the widget set. Sticky Wall and
  similar designs must be expressible as declarative boards, or wait for the
  web-view decision.
- **Remote hosts.** Extensions run on the host where they are installed, so
  the VPS needs its own installs. "Installs on VPS" in the design is literal.
- **Mixed versions.** A client older than a host still works, and shows
  fallback text for newer widgets.
- **Process contributions stay High risk.** They remain unconfined until a
  per-OS sandbox is built. This is stated in the review copy.
- **What Phase 1 must build.** The Phase 1 seams cost little: ownership
  fields, a principal enum, an event bus, a registry and reserved names. They
  must exist before features pile up, so features never need a refactor to
  become extension-shaped.

## Challenge

An independent review challenged this decision once, before it was accepted.
The weakest assumption it found was that data contributions are harmless
because they run no code. A skills-only extension could instruct agents to
leak credentials, show no permissions, and then update silently.

We revised the decision for that and for four other findings:

- **Agent-facing data counts toward risk.** Skills, agent rules and recipes
  with commands carry risk, and changing them is part of the consent
  fingerprint.
- **Contribution calls run as the extension.** A call made on behalf of a
  contribution runs as the extension's principal.
- **Secrets are tied to hosts.** Each secret is bound to the hosts it may be
  sent to.
- **Creator mode has a stated fallback** if the JavaScript-engine spike fails.
- **Clients declare their view-schema majors**, so the host can filter views
  for older clients.

The review also found the Phase 1 command seam too heavy. It is now a static
descriptor table with a test.

## Sources

- DeepSeek Harness (read 2026-11-20 via the GitHub API; docs and design notes,
  not source):
  - `https://github.com/deepseek-ai/deepseek-harness/blob/master/docs/architecture.md`
  - `.../docs/cordis-primer.md`
  - `.../docs/capability-seams.md`
  - `.../.agents/notes/implemented/architecture/2026-09-10-public-package-manifest.md`
  - `.../2026-09-16-creator-persistent-plugin-management.md`
  - `.../2026-09-15-guided-plugin-installation.md`
  - `.../.agents/notes/implemented/feature/2026-10-02-plugin-market-index-format.md`
  - `.../packages/boot/plugin-manager/README.md`
  - `.../SAFETY.md`
- Zed:
  - `https://zed.dev/docs/extensions/developing-extensions`
  - `https://zed.dev/docs/extensions/capabilities`
  - `https://zed.dev/blog/zed-decoded-extensions`
  - `https://github.com/zed-industries/zed/tree/main/crates/extension_api`
  - `https://github.com/zed-industries/extensions/blob/main/CONTRIBUTING.md`
- VS Code:
  - `https://code.visualstudio.com/api/advanced-topics/extension-host`
  - `https://code.visualstudio.com/api/references/extension-manifest`
  - `https://code.visualstudio.com/api/extension-guides/webview`
  - `https://code.visualstudio.com/docs/editor/workspace-trust`
- Claude Code:
  - `https://code.claude.com/docs/en/plugins-reference`
  - `https://code.claude.com/docs/en/plugins/security.md`
  - `https://code.claude.com/docs/en/plugin-marketplaces`
  - `https://code.claude.com/docs/en/skills`
- MCP:
  - `https://modelcontextprotocol.io/specification/2025-11-25/server/tools`
  - `https://modelcontextprotocol.io/specification/2025-11-25/basic/transports`
  - `https://modelcontextprotocol.io/seps/1865-mcp-apps-interactive-user-interfaces-for-mcp`
- Declarative UI:
  - `https://learn.microsoft.com/en-us/adaptive-cards/rendering-cards/implement-a-renderer`
  - `https://developers.raycast.com/information/security`

Not verified:

- How a JavaScript engine is packaged as a component for runtime source
  loading. Candidates are ComponentizeJS/StarlingMonkey and Javy-style QuickJS
  builds; this is the first spike.
- The current maturity of WASI 0.3 async in Wasmtime.
- Zed's current position on UI extensions. The statement that it has none is
  from an older blog post.
- Where Zed declares capabilities.
- The DeepSeek Harness loader source. Only its docs were read.
