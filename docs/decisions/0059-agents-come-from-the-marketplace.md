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
  "contributes": { "agentAdapters": [
    { "id": "claude", "kind": "agents/claude.json", "cli": { "...": "see below" } }
  ] }
  ```

  The path is always `agents/<id>.json`. That file is exactly a 0050 kind
  declaration, and its own `id` must equal the entry's `id`. `core-agents`
  validates its content with its own loader, with owner `ext:<package-id>`.
  The extension crate checks only that the file exists inside the package,
  the id rule and the id match. It never reads kind fields, so `core-agents`
  checks that the `process` permission's `programs` equal the declaration's
  `program` candidates.
- A kind id is unique among installed packages. A second package that
  declares an installed id is refused at install. The ids `claude`, `codex`,
  `antigravity` and `gentle-shell` are reserved to publisher `massorca`.
- **Permissions.** An `agentAdapters` entry is always computed High risk;
  the manifest cannot lower it. The review copy is "runs a program on this
  computer with your rights".
  - `{ "kind": "process", "scope": { "programs": [...] } }`, listing the
    declaration's `program` candidates.
  - `{ "kind": "credentials", "scope": { "agents": ["<id>"] } }` when the
    declaration has a `credentials` block. The review shows its home
    variable and stripped variables, which `core-agents` supplies.
  - `{ "kind": "secrets", "scope": { "names": [...] } }` only for vault
    secrets (0071) injected at launch, and only those names.
- **The CLI comes with the install (owner direction, decision 0061).**
  Installing an agent must end with a usable CLI. Each entry may carry a
  `cli` block: `binary`, a `verify` command (for example
  `["claude", "--version"]`), the vendor's `source` page, and `install` and
  optional `uninstall` commands per OS. Packages bundle no binaries.
  - If the program is already `detected`, the package registers at once.
  - Otherwise the install review shows the exact official command. It runs
    only after the person confirms it as a High-risk step, never
    silently. It is never elevated unless the vendor's own installer asks
    and the person accepts that prompt. On a headless host,
    `massorca extension install <id> --yes-run-installer` gives the same
    consent through the CLI. The verify command must then succeed, or the
    install fails and the package is not registered.
  - What was run is recorded in the lockfile: the command, the time, its
    exit code, and whether this package installed the CLI.
  - On remove, the agent is unregistered. A CLI the package did not install
    is left alone. If the package installed it, the person is offered its
    `uninstall` command.
  - The extension runtime runs the installer and the verify command. In
    M3 nothing runs them yet: the Marketplace entry shows the command for
    the person to copy until `core-extensions` lands. There is no separate
    `setup` field.

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
  not found" and its `cli` install command.
- **Removing a package** that has a live session of one of its kinds is
  refused with `agent.busy`. `core-extensions` calls the removal guards
  that features register (`register_remove_guard(owner, fn(package_id))`)
  before deleting a package, and the agents guard performs this check. Ended sessions keep their history, and their
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
  badge (`detected`) and the `cli` install command. Install and remove are the
  extension runtime's `extensions.install` and `extensions.remove`; the
  agents chain adds the live-session check and the credential-home choice.
- **For Settings:** installed kinds only, with the same Connection,
  Settings and Advanced Options parts as the Marketplace page below.

### The agent's Marketplace page

An installed agent is managed from its Marketplace page and from Settings.
Every Marketplace item page has these parts, in this order. The agent
parts are rendered from data, with no agent-specific UI code.

| # | Part | Source for an agent |
|---|---|---|
| 1 | Logo | manifest `icon` |
| 2 | Name | manifest `name` |
| 3 | Host | the host the package is installed in, or would be |
| 4 | Banner | the `agentAdapters` entry's `examples`: up to 8 `{ text, description }` showing how the agent is invoked in the chat composer. They are display-only, and are in the manifest so the page shows them before install. |
| 5 | Connection | accounts of the kind on this host (below), when the kind declares `accounts` |
| 6 | Information | manifest description, version, publisher, permissions, and the `cli` source link |
| 7 | Settings | the kind's `options` with `section: basic` |
| 8 | Advanced Options | the kind's `options` with `section: advanced`, when there are any |
| 9 | Danger Zone | uninstall, under the removal rules above |

**Options.** A kind declaration may list `options`:

```json
{ "key": "model", "title": "Model", "type": "enum", "choices": ["..."],
  "default": "...", "section": "basic", "args": ["--model", "{value}"] }
```

- `type` is `enum`, `string`, `bool` or `integer`.
- Each option says how it reaches a run: `args` with `{value}`, or `env`
  with a variable name. A `bool` uses `args` only when true.
- `core-agents` stores values per profile, with an optional per-project
  value, and applies them at start before the caller's `args` and `env`,
  which still win.
- `agent.options { kind, project? }` returns the declared options with
  their current values. `agent.options.set { kind, key, value, project? }`
  changes one value.
- Option values are never secrets. A secret is a vault name (0071).

**Accounts (Connection).** A kind can have several accounts on a host, and
one is active.

- An account is a credential home under
  `<profile>/agents/accounts/<kind>/<account-id>/`. All the rules of 0056
  apply: owner-only access, copy-only import, and names and sizes only in
  any output. Its record holds `id`, `label` (chosen by the person),
  `created_at_ms`, and an optional `identity` (for example an email) taken
  from the kind's `accounts.check` command output, never from credential
  file contents.
- Each kind has one **active** account per host. A project can choose a
  different one; `agent.accounts.list` shows which projects use each
  account. A run uses the project's choice, else the active account, else
  the agent's usual home, as in 0056 today.
- Methods (permission `workspace`, except `list`, which is `read`):
  - `agent.accounts.list { kind }`.
  - `agent.accounts.add { kind, label? }`: this is the login. It creates
    an empty account home and opens a service-owned terminal that runs
    the kind's `accounts.login` command with the home variable pointing
    at it. The client attaches to that terminal, so the vendor's own flow
    (a URL, a code) works unchanged. Once the declared credential files
    exist, or `accounts.check` succeeds, the account is kept; if the
    terminal ends first, it is discarded.
  - `agent.accounts.import { kind, from, label? }`: 0056's import, into a
    new account.
  - `agent.accounts.activate { kind, account, project? }`.
  - `agent.accounts.remove { kind, account }`: refused with `agent.busy`
    while a live session uses it. Runs the kind's `accounts.logout` when
    it is declared, then deletes the home.
  - Event `agent.accounts_changed` (id = kind) on every change.
- The kind declaration gains
  `accounts: { "login": [...], "check"?: [...], "logout"?: [...] }`.
- **Migration from #121.** The single home `<profile>/agents/homes/<kind>/`
  becomes the kind's first account, labelled "default" and active. The
  `agent.credentials.*` methods keep working on the active account and are
  deprecated in favour of `agent.accounts.*`.
- The login flow is specified with the desktop session; this part is the
  shape it plugs into.

### How the built-in declarations move

1. **Now**, until the extension package crate and catalogue are on main:
   the four declarations stay compiled into `core-agents`, owner `core`.
   They count as installed, so the existing flows and M3 keep working.
   `agent.kinds` already reports `detected` separately from `installed`.
2. **When the package crate lands**, the four become first-party packages
   in the repository (`packages/agents/massorca.<id>/` with `extension.json`
   and `agents/<id>.json`), listed in the built-in catalogue as `available`.
   Installing one copies it from the host and downloads nothing. The
   extension runtime embeds them in its catalogue, and the
   compiled-in copies are removed from `core-agents` in the same change.
   `core-agents` reads installed kinds through
   `massorca_core_extensions::installed_agent_kinds(&Profile)`, which
   returns the enabled packages' kinds with `package_id`,
   `package_version`, `kind_id` and the absolute `path` of each
   declaration.
3. **Profile files** under `<profile>/agents/kinds/*.json` (owner `user`)
   stay as the local and development path until the extension runtime's
   dev-folder install exists. Then they fold into it.

## Alternatives

- **One credential home per kind** (as 0056 ships). It is enough to run an
  agent, but the owner's Connection part needs several accounts with one
  active and per-project choices. 0056's home becomes the first account,
  so nothing is lost.
- **Agent options as the package's generic `settings` contributions.**
  Their launch mapping (args, env) is agent-specific. Keeping options in
  the kind file keeps the 0050 schema in one place, and the page renders
  them with the same typed-setting widgets.

- **Show the vendor's install command and never run it (display-only).**
  Installing an agent from the Marketplace would then end with a command to
  copy into another terminal, not with an agent the person can use. That
  breaks the owner's rule that the Marketplace is where agents get
  installed. Running the official command only after an explicit High-risk
  confirmation keeps the person in control without that detour.

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
