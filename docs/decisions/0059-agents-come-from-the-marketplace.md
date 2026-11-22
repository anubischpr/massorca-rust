# 0059: Agents are installed from the Marketplace

Status: accepted, 2026-11-22. Amends 0050 (where kinds come from) and 0056
(what Settings shows).

## Context

0050 made an agent kind a JSON declaration and shipped four of them
(Claude, Codex, Antigravity, Gentle Shell) compiled into `core-agents`,
plus profile files under `<profile>/agents/kinds/`. A kind counted as
"installed" when its program resolved on the host.

The owner has since set the rule for every addition to MassOrca: agents,
plugins and the like are installed from the Marketplace. Once installed, an
agent appears in Settings, where its accounts, login, credentials and
defaults are managed. Settings never installs anything.

The extension model already has a place for this. Tier 3 of
`docs/architecture/extensions.md` is a declared external program, and the
`agentAdapters` contribution kind (§3) exists for agent CLIs. The extension
runtime owns the manifest, permissions, risk, the catalogue and package
install and remove. The agents chain owns kinds, launch, status, credentials
and what the agent states mean. The two sessions agreed the format below.

## Decision

### A kind is a package contribution

- An agent package is a data-only extension (no `main`). Its
  `extension.json` contributes its kinds:

  ```json
  "contributes": { "agentAdapters": [ { "kind": "agents/claude.json" } ] }
  ```

  Each `kind` file is exactly a 0050 kind declaration. `core-agents`
  validates it with its own loader, with owner `ext:<package-id>`. The
  extension crate only checks that the path exists inside the package, and
  never parses kind fields.
- A kind id is unique among installed packages. A second package that
  declares an installed id is refused at install. The ids `claude`, `codex`,
  `antigravity` and `gentle-shell` are reserved to publisher `massorca`.
- **Permissions**, required by the core and shown in the review, are always
  High risk:
  - `{ "kind": "process", "scope": { "programs": [...] } }`. The list must
    equal the kind's `program` candidates; the validator checks this.
  - `{ "kind": "credentials", "scope": { "agents": ["<id>"] } }` when the
    kind declares a `credentials` block. The review also shows the home
    variable and the stripped variables.
  - Vault secrets (0071) reach a run only as the names granted, as
    environment variables at launch.
- **Setup text.** A package may carry `setup` entries per kind
  (`{ "os": "windows|linux|macos|any", "text": "...", "command": [...] }`).
  They are display-only. Installing a package never installs the vendor's
  CLI, and MassOrca never runs a `setup` command in this phase. The
  Marketplace entry shows the text and lets the person copy the command.
  Running it would need its own decision.

### What the states mean

| Word | Meaning | Who owns it |
|---|---|---|
| available | The kind is in a catalogue this host knows, and its package is not installed. | extension runtime |
| installed | Its package is in `<profile>/extensions/lock.json`. | extension runtime |
| enabled / disabled | Whether an installed package's contributions are registered. | extension runtime |
| detected | The kind's program resolves on this host (absolute path, or PATH plus PATHEXT; no version probe, as in 0050). | agents |
| removed | The package was removed with `extensions.remove`; its kinds are unregistered. | extension runtime, with the agents rules below |

- **Detected never means installed.** A CLI found on the PC is information
  for the Marketplace entry ("found on this PC": installing then only
  registers the package). It never makes an agent appear in Settings or
  startable.
- **Only kinds that are installed and enabled can be started.** Any other
  kind gets `agent.not_installed`. A kind that is installed but not detected
  is refused with `agent.not_installed` as today, with the reason "program
  not found" and its setup text.
- **Removing a package** that has a live session of one of its kinds is
  refused with `agent.busy`. Ended sessions keep their history, and their
  kind shows as unavailable. The kind's credential home
  (`<profile>/agents/homes/<id>/`) is deleted only when the person confirms
  it in the remove dialog; otherwise it is kept.
- **Disabling** a package works like removing it for starting runs. It
  keeps the credential home.

### What the core exposes

- **`agent.kinds`** (read) lists every kind the host knows: available,
  installed or both. Each entry adds two fields, additively per 0002:
  - `detected`: true when its program resolves; `program_path` is set then.
  - `package`: `{ state, id?, version? }`, where `state` is `available`
    (listed, not installed), `enabled` or `disabled` (installed), with
    `#[serde(other)] unknown`.

  The existing `installed` field keeps its `agents/1` meaning ("the program
  resolves", now the same value as `detected`) so no client breaks. It is
  deprecated and documented as such, and `agents/2` drops it. New clients
  read `package.state` and `detected`; the CLI shows them as separate
  columns. In this decision "installed" always means the package state.
- **For the Marketplace:** `agent.kinds` supplies the "found on this PC"
  badge (`detected`) and the setup text. Install and remove are the
  extension runtime's `extensions.install` and `extensions.remove`; the
  agents chain adds the live-session check and the credential-home choice.
- **For Settings:** installed kinds only, each with:
  - its credentials state (`agent.credentials.show`) and its import and
    remove actions (0056);
  - its login (the next piece: a `login` mechanism per kind, specified
    with the desktop session, which runs the vendor's own login inside the
    profile's credential home);
  - its defaults (launch args, model) as typed `settings` contributions of
    the package.

### How the built-in declarations move

1. **Now**, until the extension package crate and catalogue are on main:
   the four declarations stay compiled into `core-agents`, owner `core`.
   They count as installed, so the existing flows and M3 keep working.
   `agent.kinds` already reports `detected` separately from `installed`.
2. **When the package crate lands**, the four become first-party packages
   in the repository (`packages/agents/massorca.<id>/` with `extension.json`
   and `agents/<id>.json`), listed in the built-in catalogue as `available`.
   Installing one copies it from the host and downloads nothing. The
   compiled-in copies are removed from `core-agents` in the same change,
   and `core-agents` reads installed kinds through one call into the
   extension crate: the enabled packages' kind files, with package id,
   version and path.
3. **Profile files** under `<profile>/agents/kinds/*.json` (owner `user`)
   stay as the local and development path until the extension runtime's
   dev-folder install exists. Then they fold into it.

## Alternatives

- **Keep built-ins as a fixed list and let Settings add agents.** Settings
  would become a second install path, against the owner's rule.
- **Treat a detected CLI as installed.** Every CLI on the PATH would appear
  in Settings without the person choosing it, and would skip the
  permission review that a process contribution needs.
- **Put the kind fields inline in `extension.json`.** The extension crate
  would have to parse agent fields it does not own; a separate file keeps
  the 0050 schema in one place.

## Consequences

- `agent.kinds` gains `detected` and `package` now. Until step 2, every
  compiled-in kind and profile file reports `package.state: enabled`, with
  no package id, and no kind is `available` only.
- Step 2 touches `core-agents` (catalog) and the extension crate together,
  and needs the extension runtime's lockfile reader.
- Removing a package needs a hook from the extension runtime into `Agents`
  for the live-session check.
- Login from Settings needs its own decision once the desktop session has
  named the flow it needs.
