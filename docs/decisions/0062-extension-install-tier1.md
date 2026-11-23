# 0062: Installing data-only extensions (tier 1)

Status: accepted, 2026-11-22. Builds on 0001, 0059 (agents come from the
Marketplace) and 0061 (package format).

## Context

0061 defines what a package is and how the core judges it: manifest, permissions,
computed risk, uninstall effects and the archive check. It writes nothing to
disk. Tier 1 is the part that does: install, list, enable, disable and remove
packages per profile, from the CLI and the protocol. Agent packages (0059) are
the first real packages, so tier 1 also runs a vendor's CLI installer, the only
step here that executes anything. Code (tier 2, Wasmtime) is out of scope.

## Decision

### Crate and layout

- New crate `crates/core-extensions` (`massorca-core-extensions`). It depends on
  `massorca-profile` and `massorca-ext-package`, never on `core-agents`, so
  `core-agents` can depend on it.
- Under `<profile>/extensions/`:
  - `pkgs/<id>/<version>-<hash8>/`: the unpacked package. Read-only for
    everyone but the installer.
  - `data/<id>/`: the package's own data, plus `install.log` from the vendor
    installer.
  - `lock.json`: the source of truth for what is installed.
  - `dev/` stays reserved for Creator mode (§11); tier 1 does not use it.
- `lock.json` is written atomically (temporary file, flush, rename). One
  service owns a profile, and the service serialises every change with one
  lock.

```json
{ "version": 1,
  "packages": [ {
    "id": "massorca.claude", "version": "1.0.0",
    "source": { "kind": "catalogue|dir|file", "ref": "massorca.claude" },
    "sha256": "<hex of the archive, or of the canonical file list>",
    "path": "pkgs/massorca.claude/1.0.0-1a2b3c4d",
    "enabled": true,
    "consent": "<fingerprint the person accepted>",
    "installed_at_ms": 0,
    "cli": [ { "agent": "claude", "binary": "claude",
               "detected_before_install": false,
               "installer": { "command": ["..."], "ran_at_ms": 0, "exit_code": 0 },
               "installed_by_package": true } ] } ] }
```

Unknown fields are kept on read and write, so a newer service can add fields.

### Sources

Tier 1 installs from three sources:

- `catalogue:<id>`: the built-in catalogue, embedded in the binary. Nothing is
  downloaded. It holds the four first-party agent packages under
  `packages/agents/massorca.<id>/`.
- `dir:<path>`: a folder on the service host holding `extension.json`. It is
  copied in; later edits to the folder do nothing.
- `file:<path>` with `--sha256 <hex>`: a package archive, checked with 0061's
  `archive::open`. The hash is required, and it is pinned the same way an index
  entry pins it.

A remote index (Marketplace download) arrives with the signed index at M6.

For a folder or catalogue entry, the hash is SHA-256 over the sorted list of
`path`, NUL, length (decimal), NUL, bytes. An archive's review hashes the
file as read; its install still needs the pin.

### Review and consent

- `extensions.inspect { source }` returns the review without changing anything:
  - the manifest summary;
  - `risk` (level and reasons, from 0061);
  - the `listing`;
  - for each agent entry, the CLI plan: `detected` (with its path), or
    `will_run` with the exact command for this OS, or `unsupported` when no
    command fits this OS or the entry has no `cli` block. An `unsupported`
    CLI does not stop the install: the package registers and the CLI stays
    missing;
  - a `consent` fingerprint.
- The fingerprint is SHA-256 over:
  - the id;
  - the version;
  - the package hash;
  - the sorted permissions;
  - the planned installer commands;
  - each agent CLI's `verify` command and its `uninstall` command for this OS.

  They are encoded as one JSON array tagged `massorca.consent/2`, never a map,
  so the fingerprint does not depend on key order.
- `extensions.install { source, consent, run_installer, sha256? }` refuses with
  `extension.consent_required` when `consent` is missing. It refuses with
  `extension.consent_mismatch` when the package or plan changed since the review.
  A plan that runs an installer also needs `run_installer: true`, or it is
  refused with `extension.installer_consent_required`. Accepting the permission
  review does not accept running a program.
- The CLI never prompts. `massorca extension install <source>` prints the review
  and exits with `extension.consent_required`. `--yes` accepts the review it just
  printed. `--consent <FINGERPRINT>` accepts the review an earlier run printed,
  exactly. `--yes-run-installer` also accepts running the vendor command. This is
  the headless-host path from 0059. The desktop sends the same fields from its
  review dialog.

### Running a vendor installer

- It runs on the service host (the execution host, also over SSH). The argv is
  exactly the manifest's command for the host OS. There is no shell and no
  string joining. On Windows no console window opens.
- It is never elevated by MassOrca. If the vendor's installer asks for elevation
  itself, that prompt is the vendor's, and the person answers it there.
- Output goes to `data/<id>/install.log`, capped at 1 MiB. It runs with a
  15-minute limit. On timeout the child and every process it started are
  killed. It gets only an allowlisted part of the service environment.
- Afterwards the `verify` command must exit 0, and `binary` must resolve.
  Otherwise the install fails with `extension.installer_failed` or
  `extension.verify_failed`. Nothing is registered, and the unpacked folder is
  deleted. What ran is still written to `install.log`.
- On success, the lockfile records the command, the time, the exit code and
  `installed_by_package: true`. A CLI that was already detected gets
  `detected_before_install: true` and no installer entry.
- Program resolution and process running come into the crate as two small
  traits, which the service supplies:
  - `ProgramResolver`, which uses the same PATH and PATHEXT rule as
    `core-agents`;
  - `CommandRunner`.
  Tests replace both, so no test runs a real vendor installer.

### Enable, disable, remove

- `extensions.set_enabled { id, enabled }`. Disabling calls the remove guards
  first, then unregisters the package's contributions; files and data stay.
- `extensions.remove { id, delete_agent_homes?, uninstall_cli? }`:
  1. Runs every registered remove guard. A refusal (for example
     `agent.busy`) stops here.
  2. Removes the lockfile entry, then deletes `pkgs/<id>/` and `data/<id>/`.
  3. Calls every registered remove cleanup, but only when
     `delete_agent_homes` is true. A cleanup failure is reported in the result
     and not rolled back.
  4. Runs the package's `uninstall` command only when `uninstall_cli` is true,
     and only for CLIs the lockfile says this package installed. That is the
     same High-risk consent as the installer; the CLI flag is
     `--uninstall-cli`. Otherwise the result lists the command, for the
     Danger Zone to offer. Its output goes to `logs/<id>-uninstall.log`,
     because `data/<id>/` is already deleted.
- Guards and cleanups are registered with an owner and return a drop handle:
  `register_remove_guard` and `register_remove_cleanup`. Each is
  `Fn(&str package_id) -> Result<(), ErrorBody>`.
- Events, published after the lockfile commit: `extension.installed`,
  `extension.enabled`, `extension.disabled`, `extension.removed`. Each has
  `id` equal to the package id.

### Agents

- `installed_agent_kinds(&Profile) -> Result<Vec<InstalledAgentKind>, Error>`
  returns every installed package's agent kinds, enabled or not, in lockfile
  order. Each entry carries:
  - `package_id`, `package_version`, `kind_id`;
  - `path`: the absolute path of `agents/<id>.json`;
  - `enabled`;
  - `cli_installed_by_package`.
  - `error`: set, with `path` empty, when the package's files no longer match the
    lockfile (`extension.tampered`). Its kinds are still listed, from the
    `agent_kinds` ids the lockfile records at install, so one changed package
    never hides the others. The kind-conflict check on install uses the same
    recorded ids. Only an unreadable lockfile fails the whole call.
- `extensions.list` and `extensions.get` follow the same rule: a package whose files no longer match the lockfile
  comes back with `problem` (`extension.tampered`) and only its lockfile record, instead of failing the call.
- `catalogue_agent_kinds() -> Vec<CatalogueAgentKind>` returns, for every
  catalogue entry, the declaration bytes and its `cli` block. Each
  declaration's content is checked by `core-agents`, not here.
- An install whose kind id is already installed by another package is refused
  with `extension.kind_conflict`.
- The first-party packages copy the kind files from `crates/core-agents/kinds/`
  at main commit `801d6866`. Until the agents chain switches its loader, its
  compiled-in copies stay and the catalogue is not read by it.

### Item page

`extensions.catalogue` lists catalogue entries. `extensions.get { id }` returns
one entry, installed or not. Each entry carries the data for the owner's
nine-part item page. There is no item-specific code:

1. `logo` (package path)
2. `name`
3. host (the client knows which service it asked)
4. `listing.banner`
5. `listing.connections`
6. information: description, developer, category, version, publisher,
   permissions with risk reasons, CLI source link
7. Settings `basic`
8. Advanced Options
9. Danger Zone: the uninstall effects

The state is one of `available`, `enabled` or `disabled`. For an agent entry it
also carries the `detected` flag. `extensions.asset { id, path }` returns a
listed image (logo, banner) as base64, at most 1 MiB, and only for files the
listing names.

### Protocol and CLI

- Capability `extensions/1`. Methods, owner `core`:
  - `extensions.list`, `get`, `catalogue` and `asset` require `Read`;
  - `extensions.inspect` requires `Read` for a `catalogue:` source and
    `HostAdmin` for `dir:` and `file:`, which read a host path;
  - `extensions.install`, `set_enabled` and `remove` require `HostAdmin`,
    because they can run programs on the host.
- Error codes are `extension.*` in `protocol::codes`. The format codes from
  0061 move there as well. `core-extensions` defines them first:
  `invalid_source`, `not_found` (no catalogue entry), `not_installed`,
  `already_installed`, `kind_conflict`, `hash_required`, `consent_required`,
  `consent_mismatch`, `installer_consent_required`, `installer_failed`,
  `verify_failed`, `asset_not_listed`, `asset_too_large`, `invalid_lockfile`,
  `tampered` and `io`. A guard's refusal keeps the guard's own code, such as
  `agent.busy`.
- The CLI group is `massorca extension`, with the reserved `massorca ext` as a
  visible alias:
  - `list`;
  - `show <id>`;
  - `catalogue`;
  - `inspect <source>`;
  - `install <source> [--sha256] [--yes | --consent <fingerprint>] [--yes-run-installer]`;
  - `enable <id>`;
  - `disable <id>`;
  - `remove <id> [--delete-agent-homes] [--uninstall-cli]`.

  Each command has text and `--json` output. Every flow is tested end to end
  through the CLI.

### As built

- The wire types in `protocol::methods::extensions` are built from the service's own views. A result carries every
  field of the view that the protocol type names; unknown fields are ignored by decoders.
- `extensions.get` and `extensions.list` carry `installed.cli`: the lockfile's record of each CLI, with the vendor
  command that ran. The desktop and `extension show` read what ran from there.
- `--yes-run-installer` needs `--yes` or `--consent`. A script that wants the vendor command to run passes both.
- Without `--yes` the CLI prints the review on stderr and fails with `extension.consent_required`. In `--json` mode
  the review is the error's `data`. A relative `dir:` or `file:` path is made absolute by the CLI for the local
  service; for a paired host the path is read on that host as typed.
- The runner stops waiting for output two seconds after the child ends, then closes the log, so a process the
  installer left behind cannot keep writing to it.

### Security review

The independent review of the path that runs a vendor command (2026-11-22) found two blockers and several
hardening points. All were fixed before merge:

- **Path aliases (blocker).** Windows drops a trailing dot or space, resolves 8.3 short names and maps device
  names, so `extension.json.` could overwrite the reviewed manifest at unpack. Package paths now refuse a
  trailing dot or space, `~` followed by a digit, and reserved device names on every OS. Unpacking creates
  each file new, the unpacked folder must hash to the reviewed files before `lock.json` is written, and
  the lockfile keeps that hash as `files_sha256`. Every manifest read back from disk (agent kinds, item
  pages, removal) is checked against it first; a mismatch is `extension.tampered` and nothing it names runs.
- **Inspect exposure (blocker).** `inspect` with `Read` let a read-only device probe any host path,
  including devices, FIFOs and UNC shares. Host paths now need `HostAdmin`; `dir:` and `file:` refuse UNC
  and device namespace paths; each file is opened once, must be a regular file on the opened handle, and is
  read with its cap enforced while reading. Errors name only the path the caller sent.
- **Process tree.** The installer runs in its own process group on Unix and in a kill-on-close job object
  on Windows, so a timeout ends what it started.
- **Environment.** Vendor commands get only `PATH`, `PATHEXT`, the home, profile and temporary folders,
  `SystemRoot`, `SYSTEMDRIVE`, `ComSpec`, locale and proxy variables, never a token the service holds.
  Relative `PATH` entries are skipped when the host resolves a program.
- **Review fidelity.** Manifest validation refuses control characters in every CLI argv, and the CLI quotes
  any argument with more than plain characters using Rust escaping.
- **Consent.** The fingerprint now covers `verify` and `uninstall` (`massorca.consent/2`), the CLI prints
  it, and `--consent` approves exactly a review read in an earlier run instead of re-inspecting.
- **Smaller points.** Lockfile entries are validated on read (an extension id, a path inside `pkgs/<id>/`)
  before any path is built from them, and the folder is synced after the rename on Unix. A failed verify
  after a successful installer says, in the error data and in `install.log`, that the CLI may remain. The
  catalogue's piped `bash -c` installs start with `set -o pipefail`.

### Second review

The second independent review of the install path (2026-11-22) found one blocker (B1) and required items. Each is
fixed with a test, except where a row says otherwise.

- **First-party identity (B1).** Publisher `massorca` and any `massorca.*` id are refused at review and at install
  from every source except the built-in catalogue, with `extension.reserved_publisher`. A lock entry under such an id
  counts as installed only when it is the catalogue's own install with the embedded content hash; any other entry is
  shown with that problem, never as a built-in, never runs its kinds and never blocks a built-in install, which
  replaces it. The reviews show the source (`catalogue`, `dir` or `file` with its reference) first.
- **Review shows what runs.** The verify and uninstall commands for this system are listed with the install command. Each agent kind's program, arguments, environment names, credential home and files, and
  each MCP server's command are listed. Values are not shown. Every installer step states that it installs the vendor's
  current release and that the version is not pinned.
- **Allowlist.** Install and uninstall commands start with `winget`, `brew`, `npm`, `powershell` or `bash`, bare. The
  manifest refuses anything else, and the install refuses it again before it runs. Verify commands start with the
  binary, as before.
- **Verify is the gate.** `verify` is optional; without it the install succeeds when the binary resolves. A
  PowerShell `irm | iex` can exit 0 after a failed download, so the gate is the verify command where the vendor documents
  one. Tests with the fake runner cover an installer that exits 0 with a failing verify (Antigravity) and one that exits 0
  with no binary (Codex). A real `irm | iex` cannot run in CI: it downloads and installs vendor software from the network
  on a shared runner, so the behaviour is tested through the fake runner only.
- **Lock released around the vendor command.** The change lock covers check, unpack and commit. The vendor command runs
  between them with the package busy (`extension.busy`). Other packages can be enabled, disabled and removed meanwhile,
  and a removal runs its uninstall commands outside the lock too. A test with a gated runner proves it.
- **Vendor processes.** They start in a fresh private folder, removed after the run, never in the service's working
  folder. They start suspended inside a kill-on-close job (the helper is `massorca_core_remote::windows_job`, moved from
  core-git: core-git gains one edge to core-remote, which the service already depends on and which holds the other
  process helpers; the alternative, a new crate, adds two edges and a crate for one function). Output is capped at 1 MiB
  per run; the argv and the exit status or timeout are always written.
- **Host paths.** A detected program's path reaches only a host administrator, in the review and the item views.
- **Agent runs.** `massorca extension install --yes-run-installer` and `remove --uninstall-cli` refuse with
  `extension.needs_person` when `MASSORCA_AGENT_SESSION` or `MASSORCA_AGENT_TOKEN` is set, before connecting. The CLI
  forwards the run token as `agent_token`, and the service refuses a vendor installer or uninstaller for any request that
  carries one. Limit: an agent that strips its environment is not detected by the CLI, and a direct call without the
  token is not detected by the service. The real boundary is per-principal authentication, which is not built yet.
- **Consent.** `--consent` sends the fingerprint the person read; a folder that changed after the review is refused with
  `extension.consent_mismatch`. The CLI cannot re-inspect and resend a new fingerprint silently. A client that does
  re-inspect and sends the new fingerprint is accepted: the server cannot know what the person saw, so the review is the
  protection.
- **Not covered by the fingerprint.** The program a bare name resolves to on the host's `PATH` (host state), and the
  proxy variables (`HTTPS_PROXY` and the rest), which are passed through to vendor commands and may hold credentials.
- **Fake vendor.** It is the example `massorca-fake-vendor`, not a binary: packaging copies `massorca` and the
  `[[bin]]` targets only. `cargo test` does not build examples by default, so the extension tests build it once with
  cargo into the same target folder.
- **Forged data.** A lockfile entry whose path leaves its package folder, a publisher or id segment that names a
  Windows device (`COM¹`, `LPT²`, superscript digits included), and a folder changed after review are each refused.
  The lockfile is validated before it is written.
- **Uninstall confirmation (V2).** `remove --uninstall-cli` runs the uninstall only after `--yes`, or `--uninstall-consent` with the fingerprint the refusal prints. A refusal changes nothing. The built-in review names no internal variable (V3); a third-party kind keeps naming its credential variables.
- **Gentle Shell (closed 2026-11-22).** The vendor's reference names the standalone npm install as recommended, and the
  package `gentle-pi` exposes the binary `gentle-shell` (https://github.com/Gentleman-Programming/gentle-shell/blob/main/docs/readme-reference.md,
  and `bin` in its package.json). The package declares `npm install -g gentle-pi` on every OS and verifies with
  `gentle-shell --version`, which the reference documents. The Pi runtime is an optional peer dependency and the
  package runs a `postinstall` script, so a host without Pi installs the package but fails verify, and nothing is
  registered on that attempt. npm leaves the launcher on `PATH`, so a later install finds it and registers it without
  running anything: verify runs only after a vendor installer, because running a found program would break the review's
  "nothing runs" promise. A session started with that CLI then fails when the program runs. The same holds for every
  built-in that declares verify. The uninstall is `npm uninstall -g gentle-pi`, the reverse of the npm install, offered
  only for a CLI the package installed. The vendor's own `gentle-shell self-uninstall` is not on the program allowlist,
  and the vendor says npm removal on Windows is not verified yet. The npm name appears only inside the quoted
  install command; every user-visible name is "Gentle Shell".
  A Gentle Shell the vendor's bundle installer put on the machine (a generated `gentle-shell.cmd` launcher in a bundle
  folder on `PATH`, with its own runtime) resolves like any program, so the install registers it and runs nothing.
- **Package scripts.** When the planned command is `npm install`, the review's plan carries `runs_package_scripts` (an
  optional wire field older peers omit), and the CLI says that npm also runs the install scripts of the package and
  its dependencies, which the review cannot show. It applies to every built-in that installs with npm, not only Gentle Shell.

#### Third-party vendor commands (refused in tier 1)

Only a built-in catalogue package may declare a vendor command (`cli.install`, `cli.verify` or `cli.uninstall`). A package
from a folder or an archive that declares one is refused at review and at install with
`extension.vendor_command_refused`, before anything runs. The reasons: the argv allowlist still admits
`powershell -Command <any script>` and `bash -c`, `verify` is not allowlisted, and the verify and uninstall commands
were not shown in the review. Third-party vendor commands come back in a later tier, with a design of their own.

#### Follow-ups (not code)

- Opaque fetches: `npm` and `winget` install whatever the registry serves under a name, and `powershell -EncodedCommand`
  hides its script. Neither is refused by the allowlist; a later tier must pin or show the content.
- `bash` on Windows may be the WSL launcher, which runs a Linux distribution. Refusing `bash` on Windows is the
  preferred fix; until then the built-in Antigravity command is the only user of it.
- An installed built-in whose embedded catalogue hash changes in a later build. Options: keep it recognised as the
  built-in while the id and the hash recorded at install match the catalogue's history, or show "update available".
  Undecided; today it is shown as not built-in until it is reinstalled.
- The `dir:` consent reading is confirmed: a folder changed after its review is refused for the reviewed consent, and
  the CLI cannot re-inspect and resend a new fingerprint silently.

#### Vendor commands

Checked against the vendor pages on 2026-11-22. Each package's `cli` block records its source and the date.

| Package | OS | Command | Source | Checked |
|---|---|---|---|---|
| `massorca.claude` | Windows | `winget install Anthropic.ClaudeCode` | https://code.claude.com/docs/en/setup | 2026-11-22 |
| `massorca.claude` | Linux | `npm install -g @anthropic-ai/claude-code@latest` | https://code.claude.com/docs/en/setup | 2026-11-22 |
| `massorca.claude` | macOS | `brew install --cask claude-code` | https://code.claude.com/docs/en/setup | 2026-11-22 |
| `massorca.codex` | all | `npm install -g @openai/codex`, uninstall `npm uninstall -g @openai/codex` (no verify: not documented) | https://github.com/openai/codex | 2026-11-22 |
| `massorca.antigravity` | Windows | `powershell -NoProfile -Command "irm https://antigravity.google/cli/install.ps1 \| iex"` | https://antigravity.google/docs/getting-started?tab=cli | 2026-11-22 |
| `massorca.antigravity` | Linux, macOS | `bash -c "set -o pipefail; curl -fsSL https://antigravity.google/cli/install.sh \| bash"` | https://antigravity.google/docs/getting-started?tab=cli | 2026-11-22 |
| `massorca.gentle-shell` | all | `npm install -g gentle-pi`, verify `gentle-shell --version`, uninstall `npm uninstall -g gentle-pi` (see above) | https://github.com/Gentleman-Programming/gentle-shell/blob/main/docs/readme-reference.md | 2026-11-22 |

Uninstall commands, where documented: Claude `winget uninstall Anthropic.ClaudeCode`, `npm uninstall -g
@anthropic-ai/claude-code`, `brew uninstall --cask claude-code`. Codex and Gentle Shell declare the npm uninstall of the
package their npm install added, on every OS. Antigravity declares none: its install is a vendor script with no
documented uninstall, so removing it never offers one.

## Alternatives

- **Prompt in the CLI.** An interactive yes/no reads well, but it hangs on a
  headless host and in tests. Instead the CLI prints the review and needs an
  explicit flag, which also leaves a record in shell history of what was
  accepted.
- **One consent for permissions and the installer.** Simpler, but it would let
  accepting a package's permissions also run a program. 0059 asks for running
  a program to be its own confirmed step.
- **Run the installer through a shell.** Vendor documentation often shows a
  piped shell line. A shell would accept any string, so the manifest holds argv
  only. A vendor whose official route is a script is declared as the
  interpreter plus its arguments.
- **Store state in SQLite (`core-store`).** The lockfile is per profile, small,
  and must be readable without a running service (`installed_agent_kinds`). A
  JSON file fits, and §5 already names it.
- **Detect the CLI in `core-agents` and inject it.** That is what the resolver
  trait does, without a crate dependency loop.

## Consequences

- The agents chain switches its loader to `installed_agent_kinds` and
  `catalogue_agent_kinds`, and drops its compiled-in kinds.
- The path that runs a vendor command gets an independent security review
  before it merges.
- Updates, rollback, per-permission toggles, logs and dev reload are later
  `extensions/1` additions.
