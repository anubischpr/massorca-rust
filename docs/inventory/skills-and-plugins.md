# Skills, plugins and marketplace

Skills and plugins are the two ways MassOrca extends what agents and the app can do. A skill is a folder with a `SKILL.md` file that an agent tool reads from its own skill directory. MassOrca finds the skills already installed on a machine, places skills for the agent tools the owner uses, updates its own skills, and shares selected skills as versioned, unlisted bundles that can be installed on this machine, on paired servers, on SSH hosts and inside WSL. A plugin is a packaged extension with a manifest, consent-gated capabilities, an optional sandboxed Node worker, sidebar panels, commands, keybindings and content packs; plugins are installed from local folders or git sources, and listed in git-hosted marketplace indexes. The whole plugin system is off by default behind one setting, and the code marks its manifest and host API as experimental with no compatibility promise yet. On pv96 (unmerged), a Marketplace page becomes the one catalog for tools, agents, skills and services, with per-host installs, app connections and a skill-install CLI (SKL-27 to SKL-44); the plugin code itself is unchanged from `custom/integration`.

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits).

## Source directories read (repo-relative, reference repository)

- `src/main/skills/`: discovery and its source lists, provider roots, install planning and transactions, placement and reconciliation, update and freshness, removal, guarded deletion (`skill-delete/`), share preparation, bundle and package creation and extraction, cloud client and upload sessions, paired-runtime and SSH relay installs, client-mediated transfer, WSL detection and install filesystem, and startup recovery (headers, constants and selected bodies).
- `src/main/plugins/`: service, discovery, installer and staging, enablement, consent, kill list, marketplace service and store, worker manager, restart loop, secrets and storage stores, approved VM recipes, bundled bootstrap, dev watcher, audit log, and the private marketplace shim (test helper only).
- `src/shared/plugins/`: manifest schema, capabilities, host API table, consent fingerprint, marketplace schema, kill list schema, content-pack contributions, panel bridge limits, path safety, install lockfile.
- `src/shared/`: `skill-install-providers.ts` (provider registry), `skill-freshness.ts`, `skill-install-capability.ts`, `skill-bundle-*`, `skill-delete-contract.ts`, `agent-skill-sharing-gate.ts`, `agent-feature-install-commands.ts`, `default-global-settings.ts` (default values only).
- `src/main/startup/main-process-plugins.ts`, `src/main/ipc/plugins.ts` (channel names and the gate), and the runtime skill and plugin method names (grep only).
- Top-level skill content: `skills/*/SKILL.md` (eight generated discovery stubs), `skill-guides/*.md` and `skill-guides/*/references/` (full guides), `skill-stubs/*.md` and `skill-stubs/_shared/cli-resolution.md` (stub sources), `config/scripts/generate-bundled-skill-guides.mjs` and `skill-stub-composition.mjs` (build-time generation, read for behaviour).
- Resources: `resources/skills/` (bundle manifest, release mapping and snapshot registry, headers only), `resources/plugins/launch/` (bundled plugin list, official marketplace listing, three plugin manifests).
- `docs/agent-skill-sharing-implementation-checklist.md`: status section, definition of done, product semantics, provider registry, and the headings and open items.
- pv96 additions (commit `d3c6311a531`; diffs and file reads against `custom/integration`). Odd docs `odd/tasks/marketplace.md` and `odd/tasks/marketplace-parity-audit.md`, read in full. `src/main/marketplace/` (all 96 files by name; headers, and the catalog, app service, access and bot-run denial, agent config and instruction writers, vault and master key, host programs, Git install, services, Engram operations and custom items read in part). `src/shared/marketplace-*.ts`, `src/shared/marketplace/`, `src/shared/third-party-skill-sources.ts`, `src/shared/bot-team-skill-picker.ts`, `src/shared/skills.ts` (diff), `src/shared/rpc-contract/marketplace-*.ts` and `skill-package-install-params.ts`. `src/main/skills/skill-package-version-install.ts` and the diffs of `skill-cloud-grant-installation.ts`, `skill-remote-install-cancellation.ts` and `src/main/ipc/skill-cloud-*.ts`. `src/main/ipc/marketplace*.ts`, `src/main/runtime/rpc/methods/marketplace-*.ts` and `skill-package-install-methods.ts` (method and channel names, file headers). `src/cli/specs/marketplace*.ts`, `src/cli/handlers/marketplace*.ts`, `src/cli/handlers/skill-picker.ts`, and the diffs of `src/cli/specs/skills.ts` and `src/cli/handlers/skills.ts`. Renderer, behaviour only: `src/renderer/src/components/marketplace/` (file list, catalog data, rows, skill rows and placeholders, skill and integration pages, agent switches), `components/sidebar/MarketplaceSidebar.tsx`, `app-shell/AppRail.tsx`, `components/settings/MarketplaceSettingsPane.tsx` (and the removed `IntegrationsPane.tsx`), `components/skills/SkillsPage.tsx`. Mobile: `mobile/app/marketplace/` and `mobile/src/marketplace/` (file list, catalog model and items, and the RPC methods they call). Docs and content: `docs/site/content/docs/cli/archify.mdx`, `skill-guides/` (diff), `docs/audits/plugin-uninstall-log-retirement/README.md` (its fix is already on `custom/integration`), and the `skills/` and `resources/skills/` listings (unchanged). Confirmed no change: `src/main/plugins/` and `src/shared/plugins/` (no diff). Owner design read for intent only (`massorca-rust/docs/design/`, screen inventory and app flow), and one read-only CodeGraph query on the reference worktree for the bot-run denial path.

## Not read / low confidence

- Not read: the renderer (skills page, settings sections, plugin panels), per the brief. The Orca Cloud service and its API (the `cloud/` directory is outside this scope), and the relay package under `src/relay`.
- Not read in detail (headers, constants and names only): the skill transaction and placement internals, WSL provider detection, the SSH relay protocol, the client-mediated transfer protocol, the package tar and gzip code, the discovery cache, the plugin host process and worker spawn, the panel bridge, the marketplace store and lockfile file formats, the audit log format, language-pack loading, and VM recipe application.
- Checklist: the phase sections (1 to 15) were read by heading and open items only.
- Tests were not read (names only).
- Root-level `src/main` files: checked by name; none is a loose skill or plugin file (the code lives in `src/main/skills/` and `src/main/plugins/`, read above).
- Low confidence: limits and timeouts are taken from constants and comments, not from end-to-end runs. The provider list is what the registry declares; the checklist still lists the real-install verification of each provider path as open. The count of eight bundled skills comes from the generator's topic list.
- Community package pin: the install and update command lines for MassOrca's own skills run the `skills` npm package through `npx --yes` with no version in the arguments. Whether a pin exists elsewhere was not verified.
- The plugin panel frame policy (no network, opaque origin) is taken from the customisation domain's reading; it was not confirmed in code here.
- pv96, read by name or in part only: most `src/main/marketplace/` bodies (vault and key store internals, host-program executors and plans, Git install steps, Engram cloud and import bodies, custom-item probes, the Codex and JSON edit helpers, the access-enforcement and bot-run tests); the renderer marketplace pages beyond the catalog, the skill pages and the agent switches (read in part); the bot-team code that consumes the access policy (through the denial helper and its callers only); the skill guide bodies (diff only); the pv96 test files (names only, except where a test name clarified a rule); the mobile marketplace screens (names and the catalog model only); the Skill Cloud server and the relay code outside `src/main/skills/`; the external tools the Marketplace drives (Engram, OpenMetadata, the vendor installers, `gh`, `glab`, `npx skills`), none of which was run.
- pv96 low confidence: whether removing a skill from the Marketplace keeps modified copies (SKL-11). The page's Danger zone says it deletes every copy on the host; the removal call was not traced. Board-skill rows match folders by name; only `computer-use` and `orchestration` have a bundled folder in this repository, so the other board matches are unverified (open item T2 of the parity audit). The archify install runs `npx skills add`, which is not version-pinned; only the archify source is pinned. Platform behaviour (installers, keychain sealing, Windows paths) comes from code and comments; nothing was run.
- pv96 commit hashes: the ODD document's first-slice commit `10b0f263b15` is not an ancestor of pv96, although the code it describes is present. The other 13 commit hashes the odd docs cite were checked and are ancestors of pv96.
- CodeGraph: the reference worktree has a `.codegraph/` index. One read-only explore call was made (bot-run denial); the index was not synced and its freshness against the commit was not checked, so structure claims come from git reads at `d3c6311a531`.

## Overview

### Agent tools and where their skills live

The install registry lists eleven agent tools that MassOrca can place a skill for. The canonical copy of every installed skill lives in `.agents/skills` (global: under the home folder; workspace: under the workspace folder). Agents that read the canonical root need no placement of their own.

| Agent tool | Global placement | Workspace placement |
|---|---|---|
| codex | canonical root (reads `.agents/skills`) | canonical root |
| claude | `~/.claude/skills` | `<workspace>/.claude/skills` |
| cursor | `~/.cursor/skills` | canonical root |
| gemini | `~/.gemini/skills` | canonical root |
| droid | `~/.factory/skills` | `<workspace>/.factory/skills` |
| continue | `~/.continue/skills` | `<workspace>/.continue/skills` |
| trae | `~/.trae-cn/skills` | `<workspace>/.trae/skills` |
| grok | `~/.grok/skills` | `<workspace>/.grok/skills` |
| aug (Augment) | `~/.augment/skills` | `<workspace>/.augment/skills` |
| muse | canonical root | canonical root |
| zcode | `~/.zcode/skills` | canonical root |

Discovery looks further: about twenty fixed home-level roots (the same agents plus opencode, pi, omp, hermes, prime, and Antigravity's CLI and built-in skill folders), the Codex and Claude plugin caches, and, per local repository and the current directory, repository-level roots.

### Skill states and freshness vocabulary

Destination states when installing or updating: missing, unchanged, clean update (Orca-owned and unmodified), modified (Orca-owned but edited), unowned (present without an Orca receipt), external link, and name collision (same name in another letter case). Freshness of installed MassOrca skills against the bundled registry: current, outdated, newer than known, unrecognized, inaccessible.

### Size limits named in the code

Package: 40 MiB compressed, 1 MiB manifest, 256 KiB upload chunks, skill files searched to depth 5 (10 inside plugin caches). Discovery: 128 repository skill roots, 10-second shared scan cache. Deletion: 512 skills per batch. Plugin scan: 16,384 entries, depth 9, 64 skill candidates.

## Features

### SKL-01 Find skills already installed in agent tools
- Source: pv96 (unmerged). Skills MassOrca installed from the Marketplace are labelled as such (`source: marketplace`, matched by folder name and scope), and a skill found in several agent folders is listed once, as installed if any copy is. The Skills page, which the Marketplace's Skills section also mounts, and the chat composer's `/` menu use the same discovery, filtered by agent.

- **Audience:** owner (sees the list in the desktop skills page); the agents read their own folders directly.
- **What happens:** The app scans the home-level skill folders of about twenty agent tools, the canonical folder, the Codex and Claude plugin caches, Antigravity's built-in skills, each local repository's skill roots and the current directory. Each root is reported with a label, its owning agent, its kind (home, plugin, repository or workspace), whether it exists, and a skip reason when it is not read. Each discovered skill shows the root it came from.
- **Rules and edge cases:** Plugin and built-in roots are never deletable. Scans are shared for about 10 seconds so several panes do not walk the same folders again. Hermes honours its profile home variable. Repository roots are capped at 128. Remote and WSL targets are scanned on their own host through their own path rules (SKL-15 to SKL-17).
- **Core and data:** Scan results live in memory only; nothing is persisted.
- **Depends on:** the CLI skills commands (command-line interface domain) for the CLI listing.

### SKL-02 Place skills for agent tools
- Source: pv96 (unmerged). The Marketplace's Skills page switches each agent the picked host reports on or off for a copy MassOrca placed; each switch reinstalls the installed catalog version with the new agent list. A copy MassOrca did not place is locked. The CLI does the same with `orca marketplace skills agents`.

- **Audience:** owner (installs); agents read the result.
- **What happens:** An install writes one canonical copy at `.agents/skills/<name>` for the chosen scope, then makes each selected agent see it. Agents that read the canonical root need nothing more; others receive a placement in their own folder (table above). By default only agents detected on the host get placements; an explicit selection is authoritative, including agents not yet installed.
- **Rules and edge cases:** On macOS and Linux a placement is a relative directory alias to the canonical copy; on Windows it is a directory junction with an absolute target. When aliases are unavailable or denied and the policy allows, a verified independent copy is made instead. Each placement's topology is recorded. Broken aliases owned by Orca are repaired; unowned or modified placements are left alone unless replacement is approved; a failed placement does not undo the canonical copy and can be retried. Two agents may not claim the same root. Reconciliation is idempotent after installs and updates.
- **Core and data:** The canonical copy, the placements, and one install receipt per copy with its provenance (bundle, version, digest).
- **Depends on:** SKL-11 (protection of local content), SKL-12 (transactions).

### SKL-03 Check freshness and update installed MassOrca skills

- **Audience:** owner.
- **What happens:** The app compares installed MassOrca skills with the bundled registry of known releases and reports each as current, outdated, newer than known, unrecognized or inaccessible. Updating runs the update command of the community `skills` package for the chosen skills, at global scope by default (the freshness rule covers global installs only), and then checks the result against the lock the command writes.
- **Rules and edge cases:** Only the canonical-copy and provider-alias topologies are eligible for a global update; other copies are not authorized. Bytes whose git tree hash equals the lock's recorded hash are treated as the tool's own install, not as a user edit. The command's colour and progress output is shown verbatim and never parsed. The command line has no version pin (see the Not read list). A reimplementation would need its own reviewed update path rather than running an unpinned package at update time.
- **Core and data:** Reads the update lock and the registry; the community tool writes the lock.
- **Depends on:** SKL-04; CLI skills commands (name); SKL-12.

### SKL-04 MassOrca's own skills and version-matched guides
- Source: pv96 (unmerged). The eight bundled skills are unchanged. The bundled guides for `orca-cli` and `orchestration` add worktree session commands and recovery notes. The Marketplace lists `computer-use` and `orchestration` among the board skills (SKL-39).

- **Audience:** agents (they load the guides); owner (installs).
- **What happens:** MassOrca ships eight skills for its own features: `orca-cli`, `orchestration`, `computer-use`, `orca-per-workspace-env`, `orca-linear`, `linear-tickets`, `orca-emulator` and `orca-emulator-android`. An installed skill is a short discovery stub. It tells the agent to resolve the CLI executable for its session and to run `skills get <topic>`, which prints the full guide embedded in that CLI build. Guides can have reference files that are fetched on demand.
- **Rules and edge cases:** The stubs share one resolver block: use `ORCA_CLI_COMMAND` when set (WSL sessions), `orca-dev` in a dev checkout, `orca-ide` on Linux outside Orca terminals, otherwise `orca`. The build refuses empty shared blocks and topic lists that do not match. Renamed topics keep an alias ledger so that old installs converge. Stubs keep the guide's frontmatter byte for byte, so discovery does not change between versions.
- **Core and data:** Text is kept in the repository and embedded in the CLI build; installed stubs live in agent folders.
- **Depends on:** CLI skills commands (command-line interface domain); SKL-02; SKL-03.

### SKL-05 Share selected skills as an unlisted link

- **Audience:** owner; agents and the CLI may publish only when the owner allows it.
- **What happens:** The owner selects one or more installed skills (multi-select on the skills page, or `skills share` in the CLI). The app packages them into one immutable bundle version and creates one durable unlisted link. Settings → Share Skills lists the owner's active links, with copy and revoke actions.
- **Rules and edge cases:** Agent and CLI publishing needs the device setting that allows agents and the CLI to publish skill links; the setting defaults to off and only a literal true enables it. Manual sharing in the desktop app is not affected by that setting. The link is unlisted: there is no search, browse, organization library or index, and the owner's inventory is never exposed anonymously. The link carries an unpredictable, revocable bearer credential; it is not end-to-end encryption against the cloud operator. A selection that matches a managed bundle exactly publishes a new version; a partial or ambiguous selection creates a new package. Hidden selections survive rescans.
- **Core and data:** Cloud package, version and link records; the local manifest of the bundle. Local installs are not changed by sharing.
- **Depends on:** SKL-09 (package), SKL-10 (cloud), SKL-08 (revocation), CLI skills commands (command-line interface domain).

### SKL-06 Install a shared skill bundle
- Source: pv96 (unmerged). A catalog package can also be installed by id and version (`skills.installPackageVersion`, a new method name, so an older host answers method-not-found and clients ask for an update). The cloud download grant comes from the desktop app connected to the host, so SSH hosts work too; a headless host without that sign-in answers that Skill Cloud is unavailable. A local package file can be installed instead (`--file`). Install dialogs open on the picked host's target: this computer, a paired server, an SSH host or a WSL distro.

- **Audience:** recipients, who do not sign in.
- **What happens:** Anyone with an active link can open the bundle, see its skills, choose all or some, choose the agents and the scope (global, or a Git worktree or plain-folder workspace), and install. The result is shown per skill: installed, conflict or failed. Installs can target this machine, a paired server, an SSH host or a WSL distribution (SKL-15 to SKL-17).
- **Rules and edge cases:** The package is verified and checked for unsafe content before any destination is touched. Modified or unowned local content is kept unless the recipient approves replacement. A cancelled install releases its locks and removes its staged bytes. Clients and hosts of different versions negotiate capabilities; a host without the capability shows an update notice instead of failing on an unknown field.
- **Core and data:** Local receipts and provenance for each installed skill.
- **Depends on:** SKL-09; SKL-10; SKL-11; SKL-12; SKL-15 to SKL-17.

### SKL-07 Update and roll back a shared bundle
- Source: pv96 (unmerged). An earlier catalog version can be chosen by id from the CLI and from connected hosts (SKL-06), which gives the same result as a rollback on those hosts. Publishing and the shared link are unchanged.

- **Audience:** publisher and recipients.
- **What happens:** The publisher updates a bundle by publishing a new immutable version under the same link. Recipients can preview the latest version or a pinned one, and install either. A rollback installs a selected earlier version as a complete version. Existing installs keep their version until someone updates them.
- **Rules and edge cases:** Versions are never changed in place and persist until deleted; there is no age-based expiry for published versions. Rollback reinstalls a complete prior version, not a partial patch.
- **Core and data:** Cloud version records; local receipts record the installed version.
- **Depends on:** SKL-05; SKL-06; SKL-08.

### SKL-08 Revoke and delete shared links and versions

- **Audience:** publisher.
- **What happens:** The owner can revoke a link, which stops new resolution and download grants, or delete a version or a whole package. The longest lag before a revoked link stops working is the five-minute lifetime of a download grant.
- **Rules and edge cases:** Revoking or deleting never removes skills already installed on any machine. Deleted package bytes are removed and their storage objects cleaned up afterwards.
- **Core and data:** Cloud authorization and package records; local installs are untouched.
- **Depends on:** SKL-05; SKL-10.

### SKL-09 Skill bundle package format and limits

- **Audience:** the publisher (indirectly); anyone who inspects a bundle.
- **What happens:** A bundle is an archive whose root follows the portable Agent Plugins 1.0.0 layout for skills-only packages, with Orca's integrity manifest in its own folder. Version one carries skills only. MCP servers, hooks, processes, connectors and permissions are excluded. Archives are built deterministically, so identical input yields the same digest; the reference validated the same digest on macOS, native Windows and Linux.
- **Rules and edge cases:** Limits: 40 MiB compressed, 1 MiB manifest, 5 directory levels for skill files, 256 KiB upload chunks. Unsafe archives (path escape, links, oversize, invalid gzip) are rejected before any destination is touched. Package names that are Windows reserved device names are rejected everywhere. Bundles from git, npm or community registries are out of scope; those stay with existing tools.
- **Core and data:** Package bytes and manifests in cloud storage and in local staging during an operation.
- **Depends on:** none.

### SKL-10 Cloud storage and transfer for bundles

- **Audience:** publisher (upload); recipients (download).
- **What happens:** Publishing uploads the archive in chunks through an upload session. Uploads resume from the offset the server acknowledged, under one stable transfer id, and abandoned uploads expire. Finalization creates the package version. Downloads use signed grants that last five minutes. Unfinished upload grants expire after 15 minutes, and quarantined objects are deleted after one day.
- **Rules and edge cases:** Three kill switches (upload, download, remote install) can be disabled independently, while unlisted preview stays available. Logs carry no principals, credentials, signed URLs or object paths. Cloud requests have a 60-second deadline; cancellation reaches queued and in-flight transfers. Remote installs have a five-minute request timeout. Identical archives are deduplicated only within one owner's tenant, so existence cannot leak across tenants.
- **Core and data:** Cloud metadata store, private object storage and grant records in the reference implementation; the app keeps staging only for the duration of an operation.
- **Depends on:** SKL-09; cloud domain (name only).

### SKL-11 Protect local edits during install and update

- **Audience:** owner.
- **What happens:** Before writing, each destination is classified (missing, unchanged, clean update, modified, unowned, external link, name collision). Missing, unchanged and clean-update destinations are written. Modified, unowned, external-link and collision destinations stop with a conflict result for that skill.
- **Rules and edge cases:** Replacing such content needs an explicit approval for each skill. Removal follows the same rule and keeps modified content. Digests are observed from disk and compared with the receipt, never taken from a list alone.
- **Core and data:** Receipts (ownership and digest) per installed skill.
- **Depends on:** SKL-02; SKL-06; SKL-13.

### SKL-12 Crash-safe install, update and removal

- **Audience:** owner (visible only when something goes wrong).
- **What happens:** Each install, update, rollback and removal runs as a journaled transaction under a lock on its destination. If the process dies mid-operation, the next start recovers the destination to a complete previous or requested version and removes partial files. Recovery runs before any install state is read.
- **Rules and edge cases:** Locks serialize the desktop app, headless runtime, CLI and SSH callers that touch the same destination. Interrupted extraction is cancelled and its partial files removed. A lost final response is resolved by bounded recovery rather than blind retries. The reference was tested with a real process kill and with out-of-space and permission-denied failures.
- **Core and data:** A durable journal and staging directories under the profile folder, kept until recovery completes.
- **Depends on:** SKL-02; SKL-13; SKL-14.

### SKL-13 Remove installed skills
- Source: pv96 (unmerged). The Marketplace skill page's Danger zone removes a skill from the picked host and says it deletes every copy there; it is offered on this computer and on paired servers, and shows a why-line for WSL and SSH hosts. The CLI `orca marketplace skills uninstall` also removes on WSL and SSH hosts, with `--conflict` (`replace-and-discard-local` or `cancel`), and exits 1 unless the host reports the skill removed. Whether modified copies are kept on the page is not traced (see Not read).

- **Audience:** owner.
- **What happens:** Removing a skill deletes the canonical copy and the placements that MassOrca owns, for the chosen scope. Modified or unowned content stays unless removal is approved explicitly. The result lists what was removed and what was kept.
- **Rules and edge cases:** Removal still works when an agent is no longer detected, because allowed roots come from the install registry, so aliases remain removable.
- **Core and data:** Updates the receipts and the install registry.
- **Depends on:** SKL-02; SKL-11; SKL-12.

### SKL-14 Delete local skill folders safely

- **Audience:** owner.
- **What happens:** The owner can delete discovered local skills. The app builds the deletion plan itself from a fresh enumeration (a client-supplied plan is not trusted), then stages each skill's directories and aliases, removes them and cleans the receipts. A failure stays inside one skill, and the rest of the batch continues.
- **Rules and edge cases:** Built-in and plugin roots are never deletable. A request without a last-modified guard fails closed, so a stale plan cannot delete changed content. Overlapping roots that reach the same directory are staged once. Batches are capped at 512 skills. Staging order on rollback keeps restored aliases from pointing at missing directories.
- **Core and data:** Per-skill staging directories and a journal, recovered at startup.
- **Depends on:** SKL-12; SKL-01.

### SKL-15 Install and manage skills on a paired server
- Source: pv96 (unmerged). A paired server answers the new `skills.installPackageVersion` method (SKL-06), and the Marketplace Skills page manages its skills and agent switches, with a why-line when the server cannot answer. An older server shows an update notice.

- **Audience:** owner (acts from the desktop); the paired server does the work.
- **What happens:** The desktop can preview, install, list and remove skills on a paired MassOrca server. The server owns global and Git-worktree installs and applies the same placement rules on its own filesystem. When the server cannot reach cloud storage, the desktop sends the package through the client instead, in resumable chunks.
- **Rules and edge cases:** Both sides need the managed-install capability; older hosts show an update notice. Cancellation reaches queued calls, probes and open sockets. Wire changes for mobile clients are additive, so mobile clients see no new required fields.
- **Core and data:** Installs and receipts are on the server's filesystem.
- **Depends on:** server-and-pairing domain (name); SKL-06; SKL-12.

### SKL-16 Install and manage skills on SSH hosts
- Source: pv96 (unmerged). The Marketplace and the CLI (`--host ssh:<target-id>`) install and list skills on SSH hosts through the relay. The Marketplace does not offer removal on SSH hosts yet, while the CLI sends it. `orca marketplace skills show` does not read an SSH host's skill folders yet.

- **Audience:** owner.
- **What happens:** Skills can be previewed, installed, listed and removed on SSH targets through the SSH relay, in global, remote Git-worktree and remote plain-folder scopes. The controls appear only for hosts that report support.
- **Rules and edge cases:** Transfers time out after five minutes. A connection lost during commit is recovered on the next attempt. The reference validated the relay against a Docker SSH host.
- **Core and data:** Installs and receipts are on the remote host.
- **Depends on:** remote-hosts domain (name); SKL-12.

### SKL-17 Skills in WSL distributions
- Source: pv96 (unmerged). The Marketplace host picker lists each WSL distro as a host, and installs there use the same install dialog with the distro as target. The CLI takes `--host wsl:<distro>` for show, list, install, agents and uninstall. The Marketplace does not offer removal on WSL yet.

- **Audience:** owner.
- **What happens:** Skills can be installed into a WSL distribution's global skill folders, into a workspace inside the distribution, or into a Windows folder mounted under `/mnt/c`. Agents detected inside the distribution receive their placements there, with Linux rules for names and executable modes.
- **Rules and edge cases:** The checklist lists a stopped, removed, re-homed or default-user-changed distribution as a test case; the outcome of those tests is not confirmed here. Aliases use absolute guest paths.
- **Core and data:** Files and receipts inside the distribution.
- **Depends on:** WSL environment support (name only); SKL-02; CLI WSL commands (command-line interface domain).

### SKL-18 Plugin system switch, package format and installation

- **Audience:** owner; plugin authors.
- **What happens:** One setting turns the plugin system on; it defaults to off. When on, the app discovers plugins under the user's plugin folder and development plugins listed in a second setting. A plugin is a folder with `orca-plugin.json` (manifest version 1): identity as publisher and id, name, semantic version, description, an engine range that accepts only the `>=` form, plugin API version 1, an optional Node entry, `contributes` (panels, commands, events, language packs, keybindings, VM recipes, agent profiles) and `capabilities`. Installs come from a local folder or from a git URL with a ref (HTTPS or SSH only). The installer copies files and never runs scripts.
- **Rules and edge cases:** Installed trees are immutable and addressed by content hash; an atomic pointer selects the current one, and the previous version is kept for one-step rollback. A lockfile records source, commit, content hash and consent fingerprint; a reinstall that does not match is shown as a change. Identities that begin with the reserved official prefix can only come from the official organization over git, never from a local path. Contribution caps include 64 panels, 256 commands, 3 event subscriptions, 16 language packs and 32 capabilities.
- **Core and data:** One folder per plugin with hash-named versions, a current pointer, the lockfile and provenance; a per-plugin data folder.
- **Depends on:** SKL-19; SKL-20; git (name).

### SKL-19 Plugin consent, enablement and audit

- **Audience:** owner.
- **What happens:** Before consent, each plugin's declared capabilities are shown in plain language. The seven capability kinds are: read the focused worktree's name, branch and terminal list; type text into a terminal the user can see; show notifications labelled with the plugin name; use the plugin's own storage folder; use the plugin's encrypted secret vault; receive worktree-created, worktree-removed and agent-status events; and read and change the plugin's own settings.
- **Rules and edge cases:** Consent is recorded against a fingerprint of the capability set and of whether the plugin has a trusted Node entry, so any change re-prompts. A panel-only update that adds a Node entry crosses the trust boundary and re-prompts even when the capabilities are unchanged. Declining disables the plugin, which is not asked again on later launches. Plugins that contribute keybindings, VM recipes or agent profiles also tie consent to the installed content (per the customisation domain's reading). Activation checks approval before a worker starts. Lifecycle events are written to an audit log.
- **Core and data:** Consent and enablement state per plugin, the audit log, and the secret vault.
- **Depends on:** SKL-18; SKL-20.

### SKL-20 Plugin runtime and host API

- **Audience:** plugin authors; owner (indirectly, through trust decisions).
- **What happens:** Plugin Node code runs in its own worker process, separate from the app, with a restricted environment, and starts on demand when a command is invoked or a subscribed event fires (per the customisation domain's reading). The plugin calls a versioned host API of 13 methods: read the focused worktree context, send text to a terminal, show a notification, get, set, delete and list plugin storage keys, get, set and delete secrets, get and set plugin settings, and subscribe to events. Logs are kept per plugin for display.
- **Rules and edge cases:** The host checks every call against the plugin's granted capabilities at the boundary. A crashing worker restarts with backoff. Stopping is requested first and enforced after a short grace period. The internal runtime RPC surface is never exposed to plugins. Secrets require operating-system encryption: without it, writes fail and nothing is stored in plaintext.
- **Core and data:** Per-plugin JSON key-value storage and encrypted secrets.
- **Depends on:** SKL-19; the app's process model (name).

### SKL-21 Plugin contributions: panels, commands and keybindings

- **Audience:** plugin users and authors.
- **What happens:** A panel appears in the right sidebar with an icon and a title; its HTML entry runs in a sandboxed frame and reaches the host only through a message bridge. A command has a title and a context (global or worktree), can be run from the UI, and can be an alias for a built-in action. A keybinding maps a normalized key to a command in a context.
- **Rules and edge cases:** The panel bridge limits action text to 4,096 characters and, per the customisation domain's reading, re-validates every payload. Keybindings are checked with the app's own key normalization. The frame has an opaque origin and no network access, according to the customisation domain's reading (not confirmed in code here). No plugin UI was found on mobile (customisation domain's reading).
- **Core and data:** Panel sessions and the command registry are in memory; keybinding contributions are declarative.
- **Depends on:** SKL-20; desktop UI (name).

### SKL-22 Plugin marketplace sources and listings

- **Audience:** owner.
- **What happens:** A marketplace is a git-hosted JSON index named `orca-marketplace.json`. The official source (`stablyai/orca-plugins`, branch main) is added when the plugin system is on and stays available offline. The owner can add and remove sources, refresh one or all, browse listings, preview an install or an update, install, and roll back a plugin to its previous version.
- **Rules and edge cases:** Each listing names a plugin id and a git source with a ref; the ref is resolved to an exact commit before installation, so an install is reproducible. Listings with categories this build does not support (themes, icons, icon themes, terminal themes, skills) are hidden. Indexes are capped at 2,048 entries and 16 categories per entry. Only HTTPS or SSH git sources are accepted.
- **Core and data:** The source list, cached index snapshots and the install lockfile.
- **Depends on:** SKL-18; SKL-19; git (name).

### SKL-23 Plugin kill list

- **Audience:** owner (passive).
- **What happens:** The app fetches a remote kill list of plugin keys, each with a reason and an optional HTTPS advisory link, caches it, and blocks the listed plugins. An unusable cache never stops the app from starting. Clock skew is tolerated up to 24 hours ahead.
- **Rules and edge cases:** The download is capped at 4 MiB and 4,096 entries. A list dated far in the future is rejected. The reference fetches the list from a fixed vendor address.
- **Core and data:** The cached list in the plugins data folder.
- **Depends on:** SKL-18; a trust policy that the new product must own (decision).

### SKL-24 Plugin language packs, VM recipes and agent profiles

- **Audience:** plugin authors; owner.
- **What happens:** A plugin can ship translations as locale files (locale ids in a bounded BCP-47 subset, used as bundle keys), VM recipes for per-workspace environments (offered only from enabled plugins), and agent profiles (declarative files that add agent definitions).
- **Rules and edge cases:** Locale ids are checked against a grammar that rejects path syntax. Contributions that change behaviour (VM recipes, agent profiles, keybindings) count as instructional for consent.
- **Core and data:** Pack registries derived from the enabled plugins; nothing persisted separately.
- **Depends on:** SKL-18; per-workspace environments (name); agents domain (name).

### SKL-25 Bundled launch plugins

- **Audience:** owner.
- **What happens:** The app package carries three official plugins: navigation shortcuts (command aliases and keybindings for frequent views, also listed in the app's bundled-plugin index), a Brazilian Portuguese language pack, and a Multipass VM recipe for disposable workspaces. A bootstrap step installs the bundled set and checks each one against the content hash in the bundled index.
- **Rules and edge cases:** Bundled plugins must use official identities. Their marketplace listings pin each repository to tag 1.0.0.
- **Core and data:** Bundled content inside the app package; installed copies in the plugins folder.
- **Depends on:** SKL-18; SKL-22; SKL-24.

### SKL-26 Plugin development loop

- **Audience:** plugin authors.
- **What happens:** Development plugins listed in the setting are watched. A change to the manifest or to a panel file triggers a debounced refresh, so an author sees the edit without reinstalling. The repository ships a sample plugin and a deliberately hostile panel used to test containment.
- **Rules and edge cases:** Only mutable development plugins are watched; installed plugins are immutable.
- **Core and data:** Reads local folders only; nothing persisted.
- **Depends on:** SKL-18.

### SKL-27 Marketplace page and catalog
- Source: pv96 (unmerged). New feature: a Marketplace page of its own, opened from the activity rail, is where tools, agents, skills and services are seen and changed on a host (plugins excepted).
- **Audience:** owner.
- **What happens:** The rail's Marketplace button (store icon, no badge or count) opens the page and turns the left sidebar into the Marketplace list: a search row, All, Tools, Agents, Skills and Services, then "Your apps" with a count and up to six installed or connected entries. The rows are Tools (Git, GitHub CLI, Node.js, Engram and custom tools), Agents (Claude Code, Codex, Antigravity, Gentle Shell and custom agents), Skills (installed skills, custom skills, board skills and built-in commands) and Services (OpenMetadata, GitHub, GitLab, Bitbucket, Azure DevOps, Gitea, Linear and Jira). Each row shows its state on the picked host and opens its item page (SKL-29). Settings has a Marketplace pane with the same catalog, in place of the former Integrations pane.
- **Rules and edge cases:** Rows show what the picked host reported; a host that has not answered shows a loading, failed or update-required state, not an empty list. Plugin listings are not rows: plugin marketplaces stay under Settings > Plugins (SKL-22). The owner's design (D-EX-01) plans to absorb the plugin marketplace browser into this browser; pv96 has not done that. The owner's design hides the sidebar on full pages; pv96 keeps the Marketplace list in the sidebar. Custom items (CUS-09) exist on this computer only.
- **Core and data:** Rows come from live reads of each host, not from a catalog cache. The Marketplace state on each host holds app settings, access policies and which agent files MassOrca wrote; it never holds a token.
- **Depends on:** SKL-28; SKL-29; SKL-39; CUS-09; providers-and-trackers (service rows); the agents domain (agent rows).
- Owner direction (consistency pass, 2026-11-12): Proposal: first version. Owner direction (2026-11-12): the activity rail has a Marketplace entry that opens a full page (D-SH-13); assumed first-version content: the catalog with skills and agents, installs as each feature allows.

### SKL-28 Marketplace host picker
- Source: pv96 (unmerged). New feature: the same catalog is managed on this computer, a WSL distro, a connected SSH host or a paired server, each host with its own installs and connections.
- **Audience:** owner.
- **What happens:** A host picker lists this computer, each WSL distro, each SSH host that is connected, and each paired server. Picking a host reads and changes that host's programs, agents, skills, apps and services. Apps, programs and skills work on all four host kinds; remote apps go through the relay for SSH hosts and through the server for paired servers. Custom items, Engram cloud and Git's one-click install are this computer only.
- **Rules and edge cases:** A host that predates the Marketplace methods shows "Update MassOrca on <host> first" instead of failing. A disconnected SSH host is not offered. Where an agent's settings file is a link leading out of its home, MassOrca leaves it alone and says so.
- **Core and data:** Each host keeps its own vault, agent files and installs. Remote tokens are written to the host's vault and never read back (SKL-37).
- **Depends on:** remote-hosts domain (SSH, WSL); server-and-pairing domain (paired servers); SKL-15; SKL-16; SKL-17.

### SKL-29 Standard item pages
- Source: pv96 (unmerged). New feature: every item opens a page with the same sections, and a section that cannot apply to an item says why.
- **Audience:** owner.
- **What happens:** A page has a banner with the host picker, then Connection (only what the item needs to connect), Information (description, developer, category, version, website, privacy policy, terms of service and, for agents, "How each agent uses it" with level badges), Advanced options (including who can use it, SKL-36), and a Danger zone with the item's disconnect, uninstall or remove action.
- **Rules and edge cases:** The same structure applies to tools, agents, skills and services. A function that cannot exist for an item is explained in place rather than hidden; for example, Antigravity's bot runs cannot be kept out of an app, and the page says so.
- **Core and data:** The page structure is computed by the same code the audit uses (SKL-43); no data of its own.
- **Depends on:** SKL-27; SKL-36; SKL-43.

### SKL-30 Install and uninstall host programs
- Source: pv96 (unmerged). New feature: the Marketplace installs, reinstalls and uninstalls Git, GitHub CLI, Node.js, Claude Code, Codex, Antigravity and Gentle Shell on the picked host, through one of the listed vendor options for that OS (winget, Homebrew, apt or dnf, npm, or the vendor's own script), and keeps the user's data on uninstall.
- **Audience:** owner.
- **What happens:** Status reads whether each program is on the host and where. Uninstall removes the program and leaves its data in place (for example `~/.claude` and `~/.claude.json`, `~/.codex`, `~/.gentle-shell` and `~/.config/gh`). Reinstall replaces the program. Git on this computer also has a one-click install that can adopt an existing Git (desktop only).
- **Rules and edge cases:** A step that needs administrator rights does not ask for a password; the page returns the exact command to run. Node.js is refused when it comes from a version manager (remove it with that tool) and on SSH hosts. Codex and Antigravity standalone copies are removed only at the vendor installer's default path. Known leftovers: Codex's release files under `~/.codex`, the PATH entry from the Codex Windows installer, and Antigravity's shell changes stay.
- **Core and data:** Installer commands are fixed in the app for each OS and point at each vendor's documented install; nothing else is stored.
- **Depends on:** SKL-28; SKL-16; SKL-17; the agents domain (agent CLIs); the git domain (Git).

### SKL-31 GitHub accounts on the Marketplace page
- Source: pv96 (unmerged). New feature: the GitHub page lists every gh login on the picked host, marks the main one, makes another one main, removes an account, and copies the sign-in command for a new account.
- **Audience:** owner.
- **What happens:** The card lists the accounts gh knows on the host and the projects bound to each. Make main runs `gh auth switch`. Add account copies the existing `gh auth login` command; the app has no sign-in flow of its own. Remove works on this computer, on paired servers, and on SSH and WSL hosts through the desktop app. CLI: `orca marketplace github accounts list|switch|rm`, which reaches this computer and paired servers.
- **Rules and edge cases:** Disconnect hides GitHub from agents only; gh keeps its accounts. A host that predates the methods shows "Update MassOrca on <host> first".
- **Core and data:** gh's own account store on each host; MassOrca keeps only the hidden flag.
- **Depends on:** the git domain (account binding); providers-and-trackers (GitHub); SKL-28.

### SKL-32 Engram memory connection for agents
- Source: pv96 (unmerged). New feature: connects Engram memory to Claude Code and Codex (MCP entry plus an instructions section), to Antigravity (MCP entry only), and shows Gentle Shell's own Engram as built in.
- **Audience:** owner.
- **What happens:** Connect checks Engram on the host (the program on PATH and its version). For Claude Code and Codex it writes the `engram` MCP entry (`engram mcp`; Codex runs it with `--tools=agent`) and adds the Engram section to the agent's instructions file. Antigravity gets the MCP entry only. Per-agent switches turn each entry on or off. Gentle Shell is at the full level, but its own Engram entry is treated as built in and is never added or removed. The app detects Engram; it has no installer for it.
- **Rules and edge cases:** A hand-written Engram heading in an instructions file is detected and left alone. An agent that is not on the host is not written to. Disconnect removes the entries and the section MassOrca wrote, and deletes the stored credentials (SKL-37).
- **Core and data:** The agent's MCP config and instructions file (CUS-10, CUS-11); what MassOrca wrote, in the Marketplace state; the Engram cloud token in the vault.
- **Depends on:** CUS-10; CUS-11; SKL-36; SKL-37.

### SKL-33 Engram cloud sync and memory import
- Source: pv96 (unmerged). New feature: on this computer, the Marketplace shows Engram's version and update advice, lists and enrols Engram cloud projects, syncs them, and imports an Engram memory file of up to 256 MB.
- **Audience:** owner.
- **What happens:** The cloud server and token come from the vault (SKL-37). Sync records the time of each project's last sync. Import picks one file and reports the counts that Engram's import prints. Update runs Engram's own update when the version check finds one.
- **Rules and edge cases:** Desktop only: these actions are not on the RPC surface, so paired servers and SSH hosts cannot use them. Time limits: 60 s for listing, status and enrolling, 180 s for sync, 120 s for import.
- **Core and data:** Engram's own store and cloud; sync times in the Marketplace state.
- **Depends on:** SKL-32; SKL-37.

### SKL-34 OpenMetadata connection for agents
- Source: pv96 (unmerged). New feature: connects an OpenMetadata server to Claude Code and Codex (full level) and to Antigravity and Gentle Shell (tools only), with a reader entry, a writer entry, extra named profiles, and a company-data rule in the instructions.
- **Audience:** owner.
- **What happens:** Connect takes the server address and a token. Claude Code gets an HTTP MCP entry and Codex the same in its TOML config. Antigravity and Gentle Shell reach the server through `npx -y mcp-remote@0.1.29`, a pinned package, because they cannot expand a header from the environment. Each agent gets the `openmetadata` reader entry and the `openmetadata-escritura` writer entry; extra profiles add their own entries. The instructions section (Claude Code and Codex only) tells the agent to consult OpenMetadata first for company data.
- **Rules and edge cases:** Tokens reach agents as environment variables (`OPENMETADATA_TOKEN`, or `OPENMETADATA_TOKEN_<PROFILE>`) and never appear in config files. Profiles are existing server profiles; MassOrca never creates or edits them on the server. A profile known only on this computer must be added on the host first.
- **Core and data:** Agent MCP entries and instructions (CUS-10, CUS-11); profile names and addresses in the Marketplace state; tokens in the vault.
- **Depends on:** CUS-10; CUS-11; SKL-36; SKL-37.

### SKL-35 Service connections from the Marketplace
- Source: pv96 (unmerged). New feature: GitLab, Bitbucket, Azure DevOps, Gitea, Linear and Jira connect and disconnect from the Services section, which takes over the connection cards of Settings > Integrations.
- **Audience:** owner.
- **What happens:** Each service shows its connection state on the picked host. GitLab signs the `glab` CLI in with a token. Bitbucket, Azure DevOps and Gitea keep their token in the host's vault (Azure DevOps and Gitea also keep a base URL), and MassOrca's own source-control calls read it, never an agent. Linear takes an API key; Jira needs a site and, for Jira Cloud, an email. Tokens are write-only over RPC.
- **Rules and edge cases:** On a paired server, Linear and Jira show status and disconnect (per workspace or site); SSH and WSL keep a notice. The rules for each connection follow the providers-and-trackers domain.
- **Core and data:** Vault entries for the token services; the other services keep their existing stores.
- **Depends on:** providers-and-trackers; SKL-28; SKL-37.

### SKL-36 Who can use each app
- Source: pv96 (unmerged). New feature: each app has an access policy for agents (everyone, only the picked agents, or everyone except them) and for bots (picked bots or whole teams), enforced when an agent launches and when a bot runs.
- **Audience:** owner.
- **What happens:** "Who can use it" on an app page sets the agent half for Claude Code, Codex, Antigravity and Gentle Shell (the CLI sets it with `orca marketplace access`) and the bot half in Bot teams. An agent kept out of an app loses the app's MCP entries that MassOrca wrote for it, and its variables. A bot kept out of an app runs without that app's MCP servers and tokens. An empty "only" list lets no agent use the app.
- **Rules and edge cases:** A bot run fails closed: if the app list cannot be read, the bot gets none of the apps. Antigravity loads every Marketplace entry on each run, so a bot on Antigravity cannot be kept out; the page says so and asks for another agent. The denial helper is imported on the Claude, Codex, Pi (Gentle Shell) and Antigravity launch paths and in bot-team queries. The pv85 ODD record listed delegated teammate children and one-shot replies as not enforced; the code at pv96 imports the helper in both paths, and their enforcement tests were not read.
- **Core and data:** The policy per app in the Marketplace state; no secret.
- **Depends on:** SKL-27; SKL-29; SKL-37; CUS-11; the bot-teams domain; the agents domain.

### SKL-37 Credential vault for app and service tokens
- Source: pv96 (unmerged). New feature: each host that runs agents keeps an encrypted vault for app and service tokens. Clients write tokens and never read them back, and the host injects them into the agents it launches.
- **Audience:** owner.
- **What happens:** Tokens are sealed with AES-256-GCM, with a fresh IV on each write, in the host's Orca data folder. The master key is sealed by the OS keychain when the keychain offers real protection; otherwise it is a separate key file, mode 0600, created on first use. The page shows "set" or "not set", the time of the last change, and where the credential lives ("keychain" or "key file", with the host's label). Remote hosts take writes only through the vault methods (`marketplace.vault.set`, `clear`, `setProfile`, `clearProfile` and `status`); a host that predates them says to update it first.
- **Rules and edge cases:** Agents launched by MassOrca get the variables of the apps they may use (SKL-36); terminal panes get the variables for every agent, because the launching agent is unknown at spawn. Azure DevOps and Gitea tokens are read only by MassOrca's own calls. Clearing a credential or disconnecting deletes the stored value. No token is written into an agent config; configs name only the variable.
- **Core and data:** The sealed token store and the master key (keychain-sealed or key file). Agent configs hold variable names only.
- **Depends on:** SKL-28; SKL-36; the agents domain (launch paths); server-and-pairing domain.

### SKL-38 Archify skill from a pinned release
- Source: pv96 (unmerged). New feature: Archify 3.2.0 (MIT) is offered on the Skills list and as `orca skills install --skill archify`. Every host installs the same pinned release from its repository.
- **Audience:** owner.
- **What happens:** The Marketplace shows the install command for the picked host, to run in a terminal, and the CLI form for hosts where the MassOrca CLI runs. The install runs `npx skills add` on the Archify tree pinned to commit `208acba` (tag v3.2.0). `orca skills update --skill archify` reinstalls the pinned release. The source carries the upstream copyright and licence notice that MassOrca shows wherever it offers the skill.
- **Rules and edge cases:** MassOrca does not download Archify itself; the terminal command does. Agents MassOrca launches on this computer and on paired hosts get `ARCHIFY_UPDATE_CHECK_DISABLED=1`; SSH and WSL terminals need the person to set it. Later Archify releases are Apache-2.0 with a NOTICE file, so a newer pin must carry that notice. The `skills` runner is not version-pinned (the open point of SKL-03).
- **Core and data:** Pinned source and notice text in the app; the installed copy is an ordinary skill folder (SKL-02).
- **Depends on:** SKL-02; SKL-03; SKL-39.

### SKL-39 MassOrca board skills and built-in commands
- Source: pv96 (unmerged). New feature: the Skills list shows MassOrca's board skills (Browser use, Computer use, Linear, Orchestration, Price check) with their state on each host, and two built-in commands (Prototype and Orchestration delegate) that have no install.
- **Audience:** owner.
- **What happens:** A board skill shows as installed when a folder with its name is found on the host; installing it opens the skill install dialog for the picked host. Built-in commands show "built in" and open their own page. Installed board skills count as owned and appear under Your apps; built-in commands do not.
- **Rules and edge cases:** Names match ignoring case, spaces and punctuation. Only `computer-use` and `orchestration` have a folder in this repository; the other board names match only folders already on a host (open item T2 of the parity audit).
- **Core and data:** Folder names on the host; nothing stored.
- **Depends on:** SKL-02; SKL-04; SKL-27.

### SKL-40 Skill copies, agent switches and team access
- Source: pv96 (unmerged). New feature: a skill's page lists each copy on the picked host, switches agents for a copy MassOrca placed, and turns the skill on for bots in chosen teams.
- **Audience:** owner.
- **What happens:** The copies list shows the agents and folders that hold the skill, including the shared `.agents/skills` root. Agent switches appear for a global install MassOrca placed; they show "checking" while the install is read and stay locked for a hand-made copy. Team access reads "Turn it on for every bot in these teams", or each bot chooses the skill in its own profile; the page counts the bots that use it. The bot-team skill picker lists skills added to the bot first, then installed skills, then marketplace skills.
- **Rules and edge cases:** Switching an agent reinstalls the same catalog version with the new agent list. Origin labels are pool (bundled with this MassOrca version), folder, plugin or custom.
- **Core and data:** Placements and receipts (SKL-02); team access in the bot-teams domain.
- **Depends on:** SKL-02; SKL-13; SKL-27; the bot-teams domain.

### SKL-41 Skill commands in the CLI for any host
- Source: pv96 (unmerged). New feature: `orca marketplace skills list, show, install, agents and uninstall` manage skills on this computer, a WSL distro, an SSH host or a paired server, chosen with `--host`.
- **Audience:** owner, CLI users and agents.
- **What happens:** `install` takes a catalog `--package` with `--version`, or a local `--file`; `--agents` takes a list or `none` (the shared folder only); `--conflict` is `replace-unmodified`, `replace-and-discard-local` or `cancel`. `agents` sets the full agent list for an installed skill. `uninstall` removes it and exits 1 unless the host reports the skill removed. `show` reads the copies on the host, except on SSH hosts, whose folders it cannot read yet.
- **Rules and edge cases:** Without `--host` the connected host is used. A catalog install gets its download grant from the desktop app connected to the host, so SSH hosts work; a headless host without that sign-in answers that Skill Cloud is unavailable. The `orca skills` commands have no host option and stay on the local host.
- **Core and data:** The same receipts and placements as the desktop (SKL-02, SKL-12).
- **Depends on:** SKL-06; SKL-15; SKL-16; SKL-17.

### SKL-42 Marketplace actions in the CLI
- Source: pv96 (unmerged). New feature: each Marketplace action has an `orca marketplace` command: apps (list, connect, disconnect, access), services (list, connect, disconnect), programs (status, install, uninstall, reinstall), GitHub accounts (list, switch, rm), skills (SKL-41) and audit (SKL-43).
- **Audience:** owner, CLI users and agents.
- **What happens:** Tokens come only from stdin (`--token-stdin`) or a named environment variable (`--token-env`); a `--token` on the command line is refused, and no output prints a token. Destructive program and account actions ask for `--yes` unless `--json` is set. `--host` takes `local`, `wsl:<distro>`, `ssh:<target-id>` or `runtime:<environment-id>`; `--environment` reads a paired server from this computer.
- **Rules and edge cases:** GitHub account commands reach this computer and paired servers; WSL and SSH hosts are reachable only from the desktop app for now.
- **Core and data:** None of its own; each command maps to the feature it drives.
- **Depends on:** SKL-27 to SKL-41.

### SKL-43 Marketplace audit
- Source: pv96 (unmerged). New feature: `orca marketplace audit` checks every catalog row on a chosen host: whether its page opens and, if not, why; its page sections; and whether the row is native or depends on something outside MassOrca.
- **Audience:** developers and QA.
- **What happens:** The RPC `marketplace.catalog.audit` answers from the same row code the desktop page uses. The CLI prints the rows; `--strict` exits 1 when any page is blocked. Each row names its provenance, such as a `gh` login, `glab`, Engram's data folder or a hand-edited agent config.
- **Rules and edge cases:** Some reads on a remote host stay at "checking" (Gentle Shell on an SSH-owned workspace, open item T4). The audit reads only; it writes nothing.
- **Core and data:** None.
- **Depends on:** SKL-27; SKL-29; the CLI domain.

### SKL-44 Marketplace on the phone
- Source: pv96 (unmerged). New feature: the phone has a Marketplace screen under Settings, after Notifications, with the catalog sections, Your apps, and pages for Engram, OpenMetadata, Git, GitHub and each agent.
- **Audience:** owner (phone).
- **What happens:** The phone reads the connected host's catalog and manages apps: connect, disconnect, Engram's server, OpenMetadata's token and access policy, through the same app methods the desktop uses. A row the host did not answer for shows Install or Connect in a neutral tone with no tap target.
- **Rules and edge cases:** Programs, services, custom items and skills are not managed from the phone in pv96. The owner's pending review (item 10) says there is no Android Marketplace entry in v1.
- **Core and data:** None on the phone beyond the host's state.
- **Depends on:** SKL-27; SKL-36; SKL-37; server-and-pairing domain.
