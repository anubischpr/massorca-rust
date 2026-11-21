# Customisation and extensibility

MassOrca lets its owner change behaviour and add to the app in several ways. Most are declarative and live in files or settings: a repository file (`orca.yaml`) with worktree scripts, per-agent command and argument overrides, theme imports, quick commands, and a declarative rules file that classifies agent output. The app also has an experimental plugin system (manifest `orca-plugin.json`, out-of-process Node worker, sandboxed panels, consent per capability set, install from git) that is off by default. DeepSeek Harness (`dsh`) is supported as one more agent CLI, and its integration writes status hooks into that tool's own patch file. This document maps every surface by behaviour, feature rows only for surfaces no other domain owns, a research note on DeepSeek Harness's extension model, and options for our own extension model. It does not decide the model.

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits).

Ownership in this document: settings and keybindings are in `settings-and-files.md` (SET-xx); the plugin runtime, skills and marketplace are in `skills-and-plugins.md` (SKL-01 to SKL-44); automations and bot teams are in `automations.md` (not yet written); source-control AI prompts and recipes are in `git.md` (GIT-10) and `providers-and-trackers.md`. Where a surface is owned elsewhere, this document describes it only enough for the cross-cutting view and points to that owner.

## Source directories read

Paths are relative to the reference repository root (`src/...`).

- `src/main/dsh/` (all three non-test files: `dsh-home-patch.ts`, `hook-service.ts`, `hook-settings.ts`).
- `src/main/plugins/` (file list and header comments of about 90 files; full reads of `plugin-host-process.ts` headers, `plugin-discovery.ts` headers, `plugin-enablement.ts`, `plugin-install-trust.ts`, `plugin-content-pack-registry.ts` head, `plugin-artifact-validation.ts` head, `plugin-storage-store.ts` and `plugin-secrets-store.ts` headers, `plugin-kill-list-service.ts` head, `plugin-dev-watcher.ts` head, `plugin-worker-env.ts` head).
- `src/shared/plugins/` (`plugin-manifest.ts`, `plugin-capabilities.ts`, `plugin-extension-registry.ts`, `plugin-host-api.ts` (method table), `plugin-content-pack-contributions.ts`, `plugin-consent-fingerprint.ts`, `plugin-marketplace.ts` (head), `plugin-panel-shell.ts` (head), `plugin-panel-bridge.ts` (head), `plugin-install-lockfile.ts` (head)).
- `src/shared/orca-yaml.ts`, `src/shared/orca-yaml-hook-types.ts`, `src/main/hooks.ts` (grep and read of the loader), `src/main/effective-hook-config.ts` (head), `src/main/setup-hook-env-vars.ts` (grep), `src/renderer/src/lib/ensure-hooks-confirmed.ts` (head and the hash check), `src/renderer/src/components/settings/RepositoryHookPolicySettings.tsx` (labels only).
- Settings types: `src/shared/global-settings-types.ts` (field names and comments; not the defaults file).
- Agent overrides and launch: `src/shared/tui-agent-config.ts` (dsh entry), `src/shared/tui-agent-launch-command-override.ts`, `src/shared/commit-message-agent-specs-secondary.ts` (dsh entry), `src/shared/agent-resume-argv.ts` and `src/shared/agent-session-resume.ts` (dsh cases), `src/shared/agent-kind.ts`, `src/shared/agent-hook-types.ts`, `src/shared/agent-hook-listener/providers/dsh-events.ts` (header), `src/main/agent-hooks/managed-agent-hook-registry.ts`, `src/renderer/src/lib/agent-catalog-entries-primary.ts` (dsh entry), `src/shared/agent-node-package-entrypoints.ts`.
- Themes and terminal customisation: `src/main/warp-themes/` (`index.ts`, `discovery.ts`, constants), `src/main/ghostty/` (`discovery.ts`, `theme-resolution.ts`), `src/shared/terminal-custom-themes.ts`, `src/shared/terminal-quick-command-types.ts`.
- Agent state rules: `src/main/runtime/agent-state-rules/` (file list, `agent-state-rules-live-update.ts` head, `agent-state-rules-bundle.ts` head).
- Source-control AI custom agent and templates: `src/shared/source-control-ai-types.ts` (grep), `src/shared/source-control-ai-command-template.ts`, `src/shared/source-control-ai-action-variables.ts`, `src/shared/commit-message-prompt.ts` (head).
- Open-in launchers: `src/shared/ui-chrome-types.ts` (type only).
- Profiles: `src/main/orca-profiles/` (file list and `profile-ui-scope.ts`), `src/shared/orca-profiles.ts` (head).
- Automations and bot teams, names and headers only: `src/main/automations/` (file list), `src/main/bot-teams/bot-team-task-start.ts` (head).
- Bundled and official content: `resources/plugins/launch/bundled-plugins.json`, `resources/plugins/launch/orca-marketplace.json`, `examples/plugins/hello-orca/` (manifest and `main.mjs`), `examples/plugins/hostile-panel/` (manifest).
- Renderer copy only: `src/renderer/src/components/settings/PluginConsentDialog.tsx` (line 206 copy), `src/renderer/src/components/settings/PluginMarketplacePreviewDialog.tsx` (line 237 copy).
- Mobile: `mobile/src` searched for `plugin` (no plugin UI found).
- `docs/inventory/` in this repository (headings of `settings-and-files.md`, `app-platform.md`, `git.md`, `providers-and-trackers.md`, used for overlap checks only).
- External, primary: the DeepSeek Harness repository and its docs, the Cordis repository and primer, Zed's extension docs, Extism docs, the MCP specification, the WebAssembly Component Model site, Google Play policy, the GPUI README. Full list in Part B and Part C.
- pv96 additions (commit `d3c6311a531`; diffs against `custom/integration`). The Marketplace's extension-facing code: `src/main/marketplace/agent-mcp-config.ts`, `marketplace-agent-config.ts`, `codex-mcp-entry-edit.ts`, `json-member-edit.ts`, `marketplace-entry-specs.ts`, `marketplace-agent-paths.ts`, `marketplace-home-agent-paths.ts`, `marketplace-agent-instructions.ts`, `instructions-block-edit.ts`, `agent-file-write.ts`, `marketplace-file-original-backup.ts`, `marketplace-agent-env.ts`, `marketplace-app-writes.ts` (headers and key logic), and `custom/` (custom item service, store, probe, arguments and environment). `src/shared/marketplace-apps.ts` (integration levels, access policy, env and MCP names), `src/shared/marketplace-custom.ts`, `src/shared/marketplace-types.ts`, `src/shared/third-party-skill-sources.ts`. `src/main/ipc/marketplace-custom.ts`. Renderer, behaviour only: `components/marketplace/MarketplaceAgentSettings.tsx`, `components/settings/agent-settings-row-props.ts` and `AgentCatalogRow.tsx` (diffs). `src/shared/default-global-settings.ts`, `global-settings-types.ts` and `ui-chrome-types.ts` (diffs: no Marketplace setting added; `marketplace` and `calendar` join the top-level views). `src/main/plugins/` and `src/shared/plugins/` (no diff between the two commits). `docs/site/content/docs/cli/archify.mdx`. Odd docs `odd/tasks/marketplace.md` and `odd/tasks/marketplace-parity-audit.md`, read in full for the decisions behind the Marketplace's agent wiring. Owner design read for intent (`massorca-rust/docs/design/`, screen inventory and app flow): the rail, the extension browser and the extension slots.

## Not read / low confidence

- Not read: `src/main/plugins/plugin-marketplace-fetch.ts`, `plugin-marketplace-installer.ts`, `plugin-install-staging.ts`, `plugin-host-runtime.ts` and `plugin-host-entry.ts` bodies, `plugin-worker-*` bodies, `plugin-panel-*` bodies beyond headers; `src/main/skills/`, `src/shared/skills/`; automation and bot-team service bodies; `src/main/orca-profiles/` bodies; `src/main/keybindings/` bodies; the defaults file `src/shared/default-global-settings.ts`; the agent-state-rules bundle schema; `tui-agent-launch-command.ts` body; the renderer UI for these features (another session covers screens); all `*.test.ts` files except test names where a test name clarified a rule.
- Low confidence: whether plugin agent-profile contributions are consumed anywhere. I found validation and hashing for them, and no registry or consumer in `plugin-content-pack-registry.ts`, `plugin-service.ts` or `plugin-discovery.ts`. Treat "no consumer" as observed in those files only.
- Low confidence: the exact effect of `trustedOrcaHooks` persisted state (the renderer trust check was read; the write path was not traced in full).
- Low confidence: `src/shared/terminal-custom-themes.ts` import pipeline (types read; the parser was not).
- Inconsistency found, not resolved: `src/shared/agent-hook-listener/providers/dsh-events.ts` documents `SubagentStart` and `SubagentStop` as DSH events, while `src/main/dsh/hook-settings.ts` installs only five events (`SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `Stop`). The install list is what DSH receives; the normaliser handles more. Check against a running DSH before relying on either.
- Referenced but missing in this checkout: `docs/reference/dsh-harness-integration.md` is cited by `src/main/dsh/hook-service.ts`. The reference repository's `docs/` has no `reference/` folder.
- CodeGraph: at pv96 the reference worktree has a `.codegraph/` index. One read-only explore call was made (the bot-run denial path); the index was not synced and its freshness against the commit was not checked, so the other structure claims come from git reads at `d3c6311a531`.
- pv96, read by name or in part only: the bodies of the custom-item probe and store, the Engram and OpenMetadata writer tests, the bot-team code that consumes the policy (beyond the denial helper and its callers), the renderer marketplace bodies (sampled from the catalog, page and sidebar modules), the mobile marketplace screens, the skill guide bodies (diff only), and the pv96 test files (names only). The A.3 list of hideable sidebar buttons was not re-checked at pv96; only the Mobile shortcut toggle was seen.
- pv96 low confidence: Antigravity cannot be kept out of Marketplace apps on bot runs (the code's own comment and the page copy say so; not run). The MCP entries Gentle Shell's own config already ships were read from code only.

---

## Part A. Customisation surfaces in the current app

### A.1 Surface map

| Surface | Where a user changes it | Stored in | Owner in this inventory |
|---|---|---|---|
| Repository hooks (`orca.yaml`: setup and archive scripts, default tabs, shared directories, VM recipes, issue command) | Commit `orca.yaml`; per-repo local hook settings; per-repo source policy | `orca.yaml` in the repo; local hook settings in profile state; trust records | CUS-01, CUS-02 |
| Per-agent command, arguments, environment | Settings (agent overrides, default args, default env, disabled agents, default agent) | Global settings record | CUS-04 (agent launch owner not yet written) |
| DeepSeek Harness agent | Install from the agent catalog; hooks installed automatically | `$DSH_HOME/cordis.patch.yml` (managed block), `dsh-hooks.json` | CUS-03 |
| Declarative agent-state rules | Settings path to a local rules file; live update from release assets | `agent-state-rules.json` (bundled and cached) | CUS-05 |
| Terminal and app themes | Settings (system, dark, light; terminal theme for dark and light) | Global settings | SET-28 for imports; theme selection in the terminal appearance domain (not yet written) |
| Theme imports (Warp, Ghostty, files) | Settings import flow with preview | Custom terminal themes in global settings | SET-28 |
| Plugin-contributed themes | Not supported; manifest rejects `themes`, `icons`, `icon-themes`, `terminal-themes`, `skills` contributions | n/a | CUS-06 |
| Keyboard shortcuts | Settings and `~/.orca/keybindings.json`; plugin keybinding contributions | Keybinding file, legacy settings field | SET-11, SET-12 |
| Interface language | Settings; plugin language packs | Global settings; plugin locale files | SET-13 |
| External editor and "Open in" | Settings (external editor command); extra "Open in" rows | Global settings (`openInApplications`) | SET-23 (the extra rows are described in A.3 under this heading) |
| Terminal quick commands | Settings (global or per repo; terminal command or agent prompt) | Global settings | CUS-07 |
| AI text prompts and recipes (commit message, PR, branch name, fix actions) | Source-control settings: agent choice, custom agent command, per-action template with placeholders | Global settings, per-repo recipes | GIT-10, providers-and-trackers |
| Plugins (manifest, worker, panels, commands, events, keybindings, VM recipes, language packs) | Plugin manager; `pluginSystemEnabled` master switch; dev plugin paths | Install tree, lockfile, consents, per-plugin storage | skills-and-plugins (runtime); described in A.4 |
| Skills and skill sharing | Settings; agent-driven publishing gate | Global settings flag | skills-and-plugins |
| Automations and bot teams | UI and external automation managers | Automation and team records | automations (not yet written) |
| Profiles (local "Personal" and cloud-linked) | Multi-profile switcher behind an environment flag | Profile index and per-profile state | SET-05 to SET-07 |
| Marketplace custom items (tools, agents and skills added by hand) | Marketplace, Add custom (this computer only) | A list kept on this computer | CUS-09 |
| Agent MCP entries for Marketplace apps (Engram, OpenMetadata) | Marketplace app pages: Connect, Disconnect and per-agent switches | Each agent's own MCP config (Claude Code's config, Codex `config.toml`, Antigravity `mcp_config.json`, Gentle Shell `mcp.json`) | CUS-11 |
| Marked instruction sections for Marketplace apps | Marketplace app pages (Engram, OpenMetadata), written on connect | `CLAUDE.md` in the Claude config folder and `AGENTS.md` in `~/.codex` | CUS-10 |

### A.2 Feature rows (CUS-01 to CUS-11)

#### CUS-01 orca.yaml setup and archive scripts with per-script trust

- **What the user does and what happens.** A repository commits an `orca.yaml` at its root with `scripts.setup` (run after a worktree is created, for example installing dependencies) and `scripts.archive` (run before a worktree is archived). The script runs in the new worktree with `ORCA_ROOT_PATH`, `ORCA_WORKSPACE_NAME` and `ORCA_WORKTREE_PATH` set. The first time a script runs, the user confirms it. Confirmation is stored against a hash of that script's content, separately for setup, archive and issue command, so a changed script asks again.
- **Rules and edge cases.**
  - Per-repository source policy (`RepositoryHookPolicySettings.tsx`): run the committed file only ("orca.yaml only"), run only local commands from the repo's local settings and ignore the file, or run both (shared first, then local). A local-only repository may use shared tab titles and colours but must not run committed commands.
  - Local hook settings can carry their own setup and archive scripts and a setup run policy: ask, run by default, or skip by default.
  - Setup can be run or skipped from the CLI for a worktree created without the UI (`--setup run`), and archive hooks are opt-in on the runtime path (`--run-hooks`). The archive hook can be skipped on removal.
  - An unknown top-level key in `orca.yaml` is reported, so a typo is visible.
  - Hook scripts run unattended, so Git Credential Manager's interactive prompt is blocked while cached credentials still work.
  - Over WSL, Windows paths in the hook environment are translated to Linux paths.
  - A hook cut off mid-archive is not recorded as a success.
- **Client requests and core pushes.** Clients ask to check hook trust for a repository, read the issue command, and run the hook through the runtime. The core reports hook output and exit code back. The runtime also reads `orca.yaml` on behalf of clients for repository hook commands (`runtime-repository-hooks-commands.ts`).
- **Data persisted.** `orca.yaml` (committed, read-only to the app); trust records keyed by repository and script kind with content hash; local hook settings in profile state.
- **Depends on.** Worktree create and archive (git.md GIT-14, GIT-15); repository catalog; CUS-02.

#### CUS-02 orca.yaml extended keys

- **What the user does.** Adds to `orca.yaml`: `defaultTabs` (titles, colours and commands for the tabs a new worktree opens), `setupAgentStartupPolicy` (`start-immediately` or `wait-for-setup`, controlling whether an agent starts while setup is still running), `issueCommand` (a shared command for linked GitHub issues, under the same trust rule as CUS-01), `worktree.sharedDirectories` (repo-relative directories, such as `node_modules`, that are symlinked into every worktree instead of copied), and `environmentRecipes` (ephemeral VM recipes with create, suspend, resume and destroy commands and a checkout mode).
- **Rules and edge cases.** Shared directories reject absolute paths, `..` traversal and `.git`; entries needing normalisation are dropped rather than rewritten, and at most 100 are read. VM recipe ids are lower-case, 1 to 64 characters; invalid recipes are reported as diagnostics and do not stop the rest of the file. Recipes also come from enabled plugins (CUS-08 and skills-and-plugins).
- **Data persisted.** As CUS-01 (`orca.yaml` is read, not written).
- **Depends on.** CUS-01; workspace tabs; the ephemeral VM domain (not yet written).

#### CUS-03 DeepSeek Harness agent integration (`dsh`)

- **What the user does.** Chooses DeepSeek Harness from the agent catalog (labelled "DeepSeek Harness", search aliases "deepseek", "dsh", "dst"). The app launches `dsh-tui .` in a worktree, detects the `dsh-tui` launcher (with the `dst` alias) and requires the `dsh` binary on PATH. The agent's status (working, waiting, idle) reaches the app through hooks. A finished session can be resumed with `dsh-tui --resume <session id>`. DSH is also used for one-shot commit message generation (`dsh --profile headless -`, prompt on stdin).
- **How the hooks are installed (`src/main/dsh/`).** DSH composes its configuration from profiles, bundles and a patch file at `$DSH_HOME/cordis.patch.yml` (default `~/.dsh`). The app writes one marker-delimited block at the end of that file, containing a row with id `orca-agent-hooks` that inserts the bundle `@deepseek-ai/dsh-hooks-claude-code` with a `configPath` pointing at the app's own `dsh-hooks.json`. The rest of the file is copied byte for byte, and a previous block is replaced on install or stripped on remove. The app also writes a shell or `.cmd` wrapper that spools the hook payload to the app's endpoint, falling back to a spool file when the app is not reachable. Install and remove are available for local homes and over SSH (`installRemote`, POSIX shell, `~/.dsh/cordis.patch.yml`).
- **Rules and edge cases.**
  - A patch file whose body is a non-empty flow sequence (`[...]`) cannot be appended to safely; install refuses and status reports the reason. An empty `[]` is removed and restored when the block is removed.
  - The app's hook events for DSH are `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse` and `Stop`. DSH's bridge does not document `Notification`, `PermissionRequest` or `SessionEnd`. An approval pause therefore shows as "working", and a pause for a question is read from the question tool's own pre- and post-use hooks.
  - DSH's shell executor drops environment variables whose names contain `KEY`, `TOKEN`, `SECRET` or `PASSWORD`. The app mirrors the two identifiers it needs onto scrub-safe aliases and restores the canonical names inside the hook script.
  - DSH sessions are keyed by workspace path, so resume must keep the working directory.
  - The bridge always sends an empty `transcript_path`, so the session id alone identifies a resumable session.
  - Hook behaviour was measured against DSH `0.1.5-rc.1` according to comments in the code. Compatibility beyond that is not recorded.
- **Client requests and core pushes.** Install, remove and status for the DSH hook target (local and remote). Status states: installed, not installed, partial (some events missing, named in the detail), error (flow-style file, unreadable file, or the block points at another file).
- **Data persisted.** `$DSH_HOME/cordis.patch.yml` (managed block only); the app's managed `dsh-hooks.json` and wrapper script in its shared agent-hooks directory; per-session ids in the agent session record.
- **Depends on.** The agent adapter contract and agent status hooks (agents domain); CUS-04 for command overrides; SSH execution hosts.

#### CUS-04 Per-agent command, argument and environment overrides
- Source: pv96 (unmerged). The same command, argument and environment editor, with the Use this agent and default-agent controls, is also on each host's Marketplace agent page (its "Launch command" and "Environment variables" sections), so the setting can be changed per host from there.

- **What the user does.** In settings: sets a custom command or path for an agent CLI (for example a wrapper script or a non-standard install), adds default arguments that are appended after the binary and before the prompt, sets default environment variables per agent (used when an agent's yolo mode is exposed through an environment variable), disables agents so they are hidden from the picker and auto-launch, and chooses the agent that is pre-selected in the new-workspace composer (or "blank").
- **Rules and edge cases.**
  - A missing override means the catalog's default binary name is used.
  - A custom launch command for the structured chat must be a single literal executable with no shell operators. Any other value yields no structured command (`structuredAgentCommandToken` returns null).
  - When a custom launch command is set, the terminal-backed chat skips the structured catalog for that agent.
  - Yolo defaults are added once to untouched profiles by a one-time migration; later edits persist.
  - Codex's history home can point elsewhere (`CODEX_HOME`-style history path). The setting is history only and does not change which account, config or hooks are used.
  - A local agent-state rules file can replace downloaded and bundled rules for testing (CUS-05).
  - Pre-trusting the worktree folder before starting an agent (so the agent's "trust this folder" prompt is skipped) is on by default and can be turned off.
- **Client requests and core pushes.** Settings get and set (desktop, web and CLI pass one normalisation boundary). Pushes: changed settings to all windows.
- **Data persisted.** Global settings record: `agentCmdOverrides`, `agentDefaultArgs`, `agentDefaultEnv`, `disabledTuiAgents`, `defaultTuiAgent`, `agentYoloDefaultsMigrated`, `agentWorkspaceTrustEnabled`.
- **Depends on.** Agent catalog and launch (agents domain, not yet written); settings (SET-01).

#### CUS-05 Declarative agent-state rules (bundled, live-updated, local override)

- **What the user does.** Nothing by default. Agent status detection (the rules that classify terminal output into agent states) is driven by a JSON bundle, `agent-state-rules.json`, that ships with the app. The app can download a newer bundle from the app's release assets when live updates are on, and a user can point the app at a local rules file to test a change.
- **Rules and edge cases.** A downloaded bundle must parse against a schema that carries an engine version; a rejected bundle is logged and the last good copy (cached, or bundled) stays active. A cached copy older than the bundled one is ignored, and a download must be newer than the bundled rules. The live-update module reads no signature or digest for the bundle; trust rests on the release source and schema validation (observed in the module; not verified against the release pipeline).
- **Data persisted.** Global settings: `agentStateRulesPath`, `agentStateRulesLiveUpdates`; the cached bundle in the app's data directory.
- **Depends on.** Agent status detection (agents domain). Owner to confirm.

#### CUS-06 Plugin-contributed themes and icon themes (not supported)

- **What the user sees today.** Themes and icon themes cannot be contributed by a plugin. The plugin manifest rejects `themes`, `icons`, `icon-themes`, `terminal-themes` and `skills` contributions, so a plugin that declares one fails to install as a whole. The marketplace hides listings in those categories. Theme import from Warp, Ghostty and files is a settings feature (SET-28), not a plugin contribution.
- **Why this row exists.** It records a deliberate deferral that matters for the extension model. A theme is the simplest extension type and it is missing from the plugin contract.
- **Depends on.** The plugin runtime and manifest (skills-and-plugins); CUS-08.

#### CUS-07 Saved terminal quick commands

- **What the user does.** Saves one-click actions in settings. Each has a label and either a terminal command (with an option to press Enter) or an agent prompt bound to one agent. Each action is global or limited to one repository.
- **Rules and edge cases.** Mobile can read and update terminal quick commands; they are the one terminal setting on the mobile allowlist (settings-and-files SET-01).
- **Data persisted.** Global settings: `terminalQuickCommands` (typed records with scope).
- **Depends on.** Terminal sessions (terminal domain, not yet written); repository scope; SET-01. Owner to confirm.

#### CUS-08 Stable internal extension points in the core

- **What this row means.** `docs/PLAN.md` (Product decisions, 2026-11-12) makes customisation a design principle: the core is built around stable extension points from Phase 1, and the product's own features should use the same points where practical. The current app has no equivalent. Its extension points are the plugin contributions and host API (A.4), which are experimental and desktop-first, and the internal typed registry in `src/shared/plugins/plugin-extension-registry.ts`, which is runtime-only and defines one point (`command`, `PLUGIN_COMMAND_EXTENSION_POINT`).
- **Why first.** The principle is a Phase 1 requirement. Loading third-party code can come later (the runtime is an open decision, Part C), but the points must exist when the core is designed.
- **Depends on.** Protocol and core architecture (PLAN Phase 1); CUS-01, CUS-04 and CUS-05 as first consumers.

#### CUS-09 Custom Marketplace items

- Source: pv96 (unmerged). New feature: the person adds a tool, an agent or a skill to the Marketplace by hand, with a check command and an install command per platform (or a skill link), and the app runs those commands on this computer.
- **What the user does and what happens.** Add custom takes a kind, a name, a check command, install commands for Windows, macOS and Linux (a platform left out means the item does not run there) and, for a skill, a link. The check reports found, missing or unverifiable. Install runs the command and sends its log lines to the window that started it. Custom items appear in their catalog section, each with remove and install actions.
- **Rules and edge cases.**
  - Commands are parsed into an argument list on this computer and never run through a shell.
  - Commands run as the person, with no sandbox. A found item records whether MassOrca installed it or it was already there.
  - Names are unique without regard to case; a duplicate is refused.
  - Desktop and this computer only: there is no RPC or CLI surface, and the custom rows show as unavailable for other hosts.
- **Client requests and core pushes.** Desktop IPC: list, add, remove and install.
- **Data persisted.** A list of custom items on this computer: kind, name, check arguments, install arguments per platform, skill link and creation time.
- **Depends on.** SKL-27 (catalog rows), SKL-39 (skill rows), CUS-08.

#### CUS-10 Marked instruction sections in agent instruction files

- Source: pv96 (unmerged). New feature: the Engram and OpenMetadata connections add a marked section to the agent's user-level instructions file, and disconnecting removes only that section.
- **What the user does and what happens.** Connecting Engram adds a section to `CLAUDE.md` in the Claude config folder (`~/.claude`, or `CLAUDE_CONFIG_DIR` when set) and to `AGENTS.md` in `~/.codex`. The Engram section tells the agent when to use its memory tools; the OpenMetadata section tells it to consult the OpenMetadata server first for company data. Antigravity and Gentle Shell have no such file in this design, so nothing is written for them.
- **Rules and edge cases.**
  - The app edits only the text between its own start and end markers; every other byte stays as it was.
  - An Engram heading written by someone else is detected and left alone, so the section is not duplicated.
  - A write that would leave the home folder through a link is refused. Only agents present on this computer are written, and no agent folder is created.
  - Before the first write, a copy of the original file is kept as `<file>.massorca-original`.
- **Client requests and core pushes.** No call of its own; part of the connect and disconnect actions of the app pages.
- **Data persisted.** The instructions files, marked sections only. The Marketplace state records, per agent, whether a section was written.
- **Depends on.** CUS-11 (the same file rules); SKL-32; SKL-34.

#### CUS-11 MCP server entries written into agent configs

- Source: pv96 (unmerged). New feature: the app writes, switches and removes the MCP server entries of the apps it manages (Engram; OpenMetadata's reader and writer entries and its extra profiles) in each agent's own MCP config, and leaves every other entry alone.
- **What the user does and what happens.** Connect and the per-agent switches on an app page write an entry; Disconnect removes the entries MassOrca wrote for that app, and no others. The files are Claude Code's user config (`~/.claude.json`, or `.claude.json` in the Claude config folder when `CLAUDE_CONFIG_DIR` is set), Codex's `~/.codex/config.toml`, Antigravity's `~/.gemini/config/mcp_config.json`, and Gentle Shell's `mcp.json` in its agent folder.
- **Rules and edge cases.**
  - A user's own entry with the same name is left as it is.
  - An entry that Gentle Shell already ships (Engram) is treated as built in and is never added or removed.
  - A remote entry names its token by an environment variable (`OPENMETADATA_TOKEN`, or the profile's variant) and never inlines it.
  - Antigravity and Gentle Shell reach the OpenMetadata server through `npx -y mcp-remote@0.1.29`, a pinned package that npx fetches the first time the entry runs.
  - An unreadable file, or one of an unexpected shape, is reported rather than repaired. A link out of the home folder is left alone.
  - Each write keeps a copy of the original file (`<file>.massorca-original`) and a rolling backup beside it.
- **Client requests and core pushes.** No call of its own; part of the connect, disconnect and switch actions of the app pages.
- **Data persisted.** The agent config files, only the named entries. The Marketplace state records, per agent, which entries MassOrca wrote.
- **Depends on.** SKL-32; SKL-34; SKL-36 (an agent kept out of an app loses the entries MassOrca wrote for it); CUS-04 (agents read the same files at launch).

### A.3 Smaller surfaces (described, not separate rows)

- **Extra "Open in" applications.** A list of up to any number of extra entries (id, label, command) in the worktree "Open in" submenu; VS Code is always shown first. Covered by SET-23 for the external editor setting; the list is in global settings as `openInApplications`.
- **Source-control AI actions.** A custom agent command can replace the catalog agent for AI text (`agentId: 'custom'` with `customAgentCommand`). Each action has a template with `{basePrompt}` and per-action variables (for example `branch`, `stagedPatch`, `linkedIssue`, `changedFiles`, `commitSummary`, `firstPrompt`). The commit message prompt takes a custom suffix appended verbatim. Covered in GIT-10 and providers-and-trackers.
- **Keyboard shortcuts.** Per-action overrides in `~/.orca/keybindings.json`, with platform overrides, conflict diagnostics and a one-time migration of the older settings field. Covered by SET-11 and SET-12.
- **Interface language.** Built-in languages and plugin-contributed language packs (locale files under a plugin, for example the bundled Portuguese pack). Covered by SET-13.
- **Appearance details.** System, dark or light app theme; separate terminal theme for dark and light (with a switch); terminal font family, size, weight and ligature handling; editor font that follows the terminal font unless set; left sidebar tint colour and opacity; app font. Terminal appearance and its settings sit in the appearance domain (not yet written); the editor font is covered in SET-16 and SET-28 covers theme imports.
- **Pinned and visible sections.** Sidebar buttons can be hidden (Tasks, Artifacts, Skills, Orca Mobile) and the Agents and Kanban views are experimental toggles.
- **Hook status for other agents.** Each agent's status hooks are installed into that agent's own configuration by a managed installer (`managed-agent-hook-registry.ts`), with a global switch `agentStatusHooksEnabled` that, when off, stops reinstalling them. The agents domain owns this.
- **Shell environment for native chat.** A list of environment variable names (`nativeChatShellEnvironmentVariables`) passed to native chat sessions when the option that inherits the full shell environment is off. A PATH and locale baseline is always passed.

### A.4 Plugin system (cross-cutting view; detail in skills-and-plugins)

- **Gate.** The master switch `pluginSystemEnabled` is off by default. When off there is no discovery, no panels and no plugin code runs. Turning it on or changing `devPluginPaths` goes through an IPC path that reconciles activation.
- **Manifest (`orca-plugin.json`, `manifestVersion` 1).** Required: `id`, `publisher`, `name`, `version` (semver), `engines.orca` (only `>=x.y.z`), `pluginApi` (1). Optional: `main` (a Node entry run in a worker), `icon`, `repository`, `author`. `contributes` (strict object): `panels` (id, title, Lucide icon name, HTML entry), `commands` (id, title, context global or worktree, optional built-in action alias), `events` (subscriptions limited to `worktree.created`, `worktree.removed`, `agent.status.changed`), `languagePacks` (locale and path), `keybindings` (command, key, scope), `vmRecipes` (path), `agents` (path; validated and hashed, no consumer found, see Low confidence). Capabilities: a closed set of seven kinds (`workspace:read`, `terminal:send`, `notifications:show`, `storage`, `secrets`, `events:subscribe`, `settings:own`). Unknown kinds fail validation.
- **Identity.** Canonical identity is `<publisher>.<id>`. Names beginning with `stablyai.orca-` are reserved for official plugins; a reserved identity can only come from the stablyai organisation over git, never from a local path. Bundled plugins must use an official identity.
- **Consent.** Consent is recorded per plugin as a fingerprint over the capability set, whether the plugin has a trusted Node worker, and, for plugins with instructional contributions (keybindings, VM recipes, agents), the hash of the installed tree. Any change re-prompts. Declining disables the plugin. The consent copy says the permissions limit the plugin's use of the app's API, and that its worker "runs as a normal process on your computer with full access to your files, network, and other pr[ograms]" (`PluginConsentDialog.tsx`). The capabilities are therefore a boundary for the API, not for the process.
- **Worker.** Main-process code forks a plain Node process (not Electron) with an allowlisted environment. Workers activate lazily on the first trigger (command invocation or subscribed event). Shutdown is requested, then killed after a short grace period. The host API is a versioned table (`PLUGIN_HOST_API_V0`), each method with parameter and result schemas, its capability, scope and whether panels may call it. Methods: `workspace.readContext`, `terminal.sendText` (explicit terminal id only), `notifications.show`, `storage.get|set|delete|keys` (per-plugin JSON file; 256 KiB per value, 5 MiB total, 1024 keys), `secrets.get|set|delete` (OS-encrypted vault, no plaintext fallback), `settings.get|set` (plugin's own settings), `events.subscribe`. Mutations are appended to a JSONL audit log with actor `plugin:<key>`. The log does not see the worker's own Node calls, and the consent copy says so.
- **Panels.** Sandboxed frame (`sandbox="allow-scripts"`, opaque origin). The host generates the document shell first, with a Content-Security-Policy of `default-src 'none'; connect-src 'none'`, so plugin HTML cannot make network requests. Panels receive design tokens as CSS custom properties and talk to the host through a postMessage bridge that re-validates every payload. Panels can call only the methods marked for panels.
- **Install and distribution.** Sources: a git URL (HTTPS or SSH only, no remote helpers), or a marketplace entry that names a git source and a ref. Marketplace installs resolve the ref to an exact commit first. The install lockfile records source, commit, content hash and consent fingerprint; a reinstall that does not match is a visible change. Installed trees are immutable and addressed by hash, with a `current` pointer file. A remote kill list (fetched from the project site) can revoke plugins; the content-pack registry checks a revocation predicate before it admits a plugin's packs and commands.
- **Official content in this checkout.** `resources/plugins/launch/orca-marketplace.json` lists three official plugins, each pinned to tag `v1.0.0`: `stablyai.orca-portuguese` (language pack), `stablyai.orca-multipass-recipes` (VM recipes, for disposable Multipass workspaces), and `stablyai.orca-navigation-shortcuts` (keybindings and command aliases). The last one is also bundled (`bundled-plugins.json`).
- **Examples.** `examples/plugins/hello-orca` (a panel, a command, two event subscriptions, storage, notifications; `main.mjs` exports a default `activate(orca)` function) and `examples/plugins/hostile-panel` (a deliberately hostile fixture for containment tests).
- **Not in the plugin contract today.** Themes, icon themes, skills, and any UI contribution on mobile (no plugin UI was found in `mobile/src`). Desktop only.
- **Other extension mechanisms.** Hook scripts (CUS-01), declarative rules (CUS-05), and commands and keybindings contributed by plugins.

### A.5 Owned elsewhere (reference only)

- **Skills and skill sharing** (skills-and-plugins): a settings flag gates agent-driven and CLI skill publishing; manual reviewed publishing stays available (`agentSkillSharingEnabled`). The skill directory layout was not read.
- **Automations** (automations): runs of agent work per workspace, with external automation managers that can be discovered and mapped (`src/main/automations/external-manager*.ts`; a Hermes cron output path exists). Storage and triggers were not read.
- **Bot teams** (bot-teams): a team has a folder workspace, bots that each map to an agent, an access mode, and tasks that create a session per bot and send the prompt (`bot-team-task-start.ts`). Read as headers only.
- **Profiles** (settings-and-files SET-05 to SET-07): local "Personal" and cloud-linked profiles. The multi-profile switcher is behind the environment flag `ORCA_MULTI_PROFILE_UI=1`, documented in code as a product-scope toggle and not a security boundary.

---

## Part B. DeepSeek Harness: research note

### B.1 Sources and how to read the confidence labels

- Primary: the repository `https://github.com/deepseek-ai/deepseek-harness` (README), `https://github.com/deepseek-ai/deepseek-harness/blob/master/docs/architecture.md`, `https://github.com/deepseek-ai/deepseek-harness/blob/master/docs/user/develop/basic/index.md` (the "Your first plugin" tutorial), `https://github.com/deepseek-ai/deepseek-harness/blob/master/SAFETY.md`, the product page `https://www.deepseek.com/en/harness`, and the docs site `https://deepseek-harness.github.io/deepseek-harness/` (only its title was captured, so no content from it is used) and its primer page `https://deepseek-harness.github.io/deepseek-harness/reference/cordis-primer`.
- Upstream: `https://github.com/cordiverse/cordis` (README).
- Secondary, marked as such: the npm guide `https://cdn.jsdelivr.net/npm/dsh-plugin-guide@0.3.33/guide/quick-reference.md`, which states its own content and calls itself a quick reference without saying who wrote it. Its claims are used only where marked.
- Method limit: the fetch tool returns a model-generated summary of each page, not the raw text. Quotes below are the summary's wording and should be re-checked against the raw page before anyone relies on exact identifiers. The `raw.githubusercontent.com` addresses for the same files returned HTTP 404. The `blob/master` addresses worked. The `main` branch URL did not.
- Labels used below: **(primary)** stated in a primary source above; **(secondary)** stated only in the npm guide; **(UNVERIFIED)** not confirmed in any source I could open.

### B.2 What a plugin is

- Harness is built on Cordis. The Cordis repository describes itself as a "Meta-Framework of Spatiotemporal Composability" and links a paper, `arXiv 2608.25512` (primary; paper not opened). Cordis's README warns that its API "is not yet stable and may change without notice" (primary).
- Harness's architecture page says "Every part of the product is a plugin" and "there is no privileged core to patch" (primary). Plugins "contribute services, typed events, and reversible effects to a shared context built on Cordis" (primary).
- A plugin is "a TypeScript module that exports an `apply` function" that the framework calls on load with a context object (primary, tutorial). The tutorial shows `export const name = 'hello-plugin'` and `export function apply(ctx: Context)`, with `export const inject = ['tools']` for required services. Three forms are shown: function, object (`export default { name, inject, apply }`) and a class extending `Service` (primary).
- The npm guide adds a `Config` export (a Schemastery schema) and `apply(ctx, config)` (secondary). The Cordis primer does not name Schemastery (primary: absent). Treat the `Config` detail as UNVERIFIED.
- Registrations go through `ctx.effect()` or `ctx.on()` and are undone on unload (primary, tutorial and primer).
- Events have dispatch modes, declared through declaration merging: `emit`, `waterfall`, `parallel`, `serial` and `bail` (primary, primer).
- Profiles and bundles: a profile is a named composition of bundles, the profile's `cordis.patch.yml` and a home patch layer; shipped profile templates are `web`, `headless`, `sdk`, `sdk-minimal` and `acp` (primary, architecture). A bundle "distributes Cordis config rows and the code they mount" (primary). `dsh --profile web --dump-config` prints the composed tree (primary). The manifest field is `dsh` in `package.json`, with `dsh.profile` listing bundles and `dsh.bundle` pointing at a patch file (primary, architecture). The npm guide's field list (`dsh.client.inject`, `engines.dsh`, `dsh.bundle.patch`) is secondary.

### B.3 What can be replaced

Named services in the architecture page (primary), each a `ctx` key:

| Seam | Primary statement | Swap procedure |
|---|---|---|
| Model adapter | `ctx.llm`, "the adapter seam, registered by a provider" | Register a provider plugin. Procedure UNVERIFIED. |
| Tool registry | `ctx.tools`, "a scoped registry with a guarded execution pipeline"; schemas join prompt assembly | Register tools from a plugin. Official registration signature UNVERIFIED (the tutorial shows only `ctx.tools.register(/* ... */)`). The npm guide gives `defineTool({ name, description, parameters, output, execute })` (secondary). |
| Session log | `ctx.sessions`, an append-only `SessionEvent` log; new backends implement `SessionPersistence` (`create`, `open`, `stat`, `list`, `export`); `ctx.sessionProjections` folds committed events | Implement `SessionPersistence` and register it. Procedure UNVERIFIED. |
| Agent loop | `ctx.agents` (the `Agent` interface and live registry) and `ctx.agentLoop` (the default driver); turns and steps follow a fixed sequence of `agent/*`, `tools/*` and `llm/stream` events | A replacement driver is implied by the separate `Agent` interface and the default driver. Replacement procedure UNVERIFIED. |
| System prompt | `ctx.systemPrompt` assembles prompt sections | Contribute sections. UNVERIFIED. |
| Sandbox | `ctx.sandbox` is a backend; consumers wrap argv before spawning | Replace the backend. UNVERIFIED. |

Other services named (primary): `ctx.shell`, `ctx.terminals`, `ctx.fs`, `ctx.subprocess`, `ctx.commands`, `ctx.jobs`, `ctx.goals`, `ctx.webhookRuntime`, `ctx.sessionTitle`.

The website also lists official plugins: Agent teams, Auto approval reviewer, Scheduled tasks and Voice input (all "Experimental"), plus Terminal, Agent loop and Subagents (primary, website). That suggests the agent loop and terminals are themselves plugins, but the page does not say how to replace them.

### B.4 Creator mode

- The product page says users create plugins by chatting in "Creator mode". Its demo asks for a Pomodoro timer. The shown transcript loads a plugin-development skill, writes plugin files, installs them through the plugin manager and verifies them, with a stated duration of 5 minutes 24 seconds (primary, website).
- The demo is illustrative. Nothing in the repository's docs I could open describes Creator mode's mechanism, its skill, or its limits (UNVERIFIED).

### B.5 How plugins extend tools, skills and the interface

- **Tools.** Plugins register tools with `ctx.tools` (primary: the service exists; the tutorial leaves the call open). The npm guide's `defineTool` shape is secondary.
- **Skills.** The architecture page says "Skills: not documented on this page" (primary). The npm guide lists a `skills` service registry among context keys and gives no contribution API (secondary). The Creator-mode demo loads a plugin-development skill (primary, website). The skill format and how a plugin contributes one are UNVERIFIED.
- **Interface.** A Web client chat node is added by registering a `ConversationNodeDefinition` with a keyed renderer. Settings pages are added through a settings-card guide. UIs render from `session/event`. The plugin manager serves web and agents (primary, architecture). The npm guide adds that UI plugins render React inside a slot (not an iframe), that tool display cards come from pure functions `presentCall` and `presentResult`, and that `dsh.client.inject` must be a static array (secondary, all UNVERIFIED against the repository).

### B.6 Distribution and install

- Local development: a patch file passed with `pnpm dsh web --patch ./scratch-plugin/cordis.yml` (primary, tutorial). `- insert:` rows carry an `id` and a `name` path. The tutorial notes that a patch file does not change the module resolution root of the profile.
- Plugin manager: `dsh plugin` manages out-of-tree plugins (primary, architecture). The subcommands `add` and `remove` with `--profile` are shown only in the npm guide (secondary). `dsh plugin add <pkg>` for a bundle changes the profile's bundle stack on restart; for a plain Cordis plugin it installs the dependency and adds an insert row to the profile's `cordis.patch.yml`, with live config reload (secondary, UNVERIFIED).
- Git sources: `dsh plugin add "github:owner/repo#<sha>&path:<subdir>"`, recommended with a pinned commit (secondary, UNVERIFIED).
- Packages: npm packages under `@deepseek-ai/dsh-*` (secondary; the repository's own package names were not listed because the `packages/` page was not fetched).
- Discovery: community plugins are found through the GitHub topic `dsh-plugin` (primary, README). There is no registry or signing scheme in any primary source I could open (primary: not documented).
- Desktop app: its runtime ships in signed resources and keeps its packages and activation separate from the npm CLI (primary, architecture). Python SDK wheels bundle the CLI and launch `dsh --profile sdk` (primary, architecture).
- Versioning: "THERE WILL BE COMPATIBILITY-BREAKING CHANGES" in the developer-preview README (primary). The npm guide says peer ranges are enforced at install time, and then says peer ranges "are not enforced" in the same paragraph before correcting itself (secondary; the contradiction is noted, UNVERIFIED). No plugin API versioning scheme is documented in a primary source (primary: not documented).

### B.7 Licence, status, safety

- MIT licence (primary, README and website footer). Developer preview (primary). No version number on the product page (primary).
- SAFETY.md: the project is early and has had no security audit. Harness can run model-generated code and commands, change or delete files, use the network and processes, and reach credentials made available to it. Untrusted plugins are named as a risk. The notice recommends minimal privileges, a disposable VM or container, backups, and reviewing plugins and commands before running them. It says sandboxing and approval prompts reduce risk but do not guarantee isolation (primary).
- Marketing copy on the product page ("Explore the limits of intelligence", "And so much more") has no measurable content and is not used.

### B.8 What the dsh code in this reference repository reveals

Observed in `src/main/dsh/` and `src/shared/`:

1. **Integration by patch layer, not by plugin.** MassOrca does not ship a DSH plugin. It writes one marker-delimited row into the user's home patch file (`$DSH_HOME/cordis.patch.yml`) that inserts the upstream bundle `@deepseek-ai/dsh-hooks-claude-code` and points its `configPath` at a MassOrca-owned JSON file. This matches the primary architecture description of "insert rows" and "a home patch layer above every profile" (`dsh-home-patch.ts`, `hook-settings.ts`: the code comment says that layer sits above every profile so a pane the user starts themselves still reports status).
2. **The hook bridge speaks Claude Code's hook format,** a subset of Claude's events. Its payload has `session_id`, `tool_name`, `tool_input`, `tool_use_id` and `prompt` (`dsh-events.ts`). This shows DSH is compatible with Claude-style hook configuration at the bridge, not that the core has a Claude-compatible hook API.
3. **Env scrubbing is a security property of the executor.** DSH drops environment variables whose names contain `KEY`, `TOKEN`, `SECRET` or `PASSWORD` before hooks start (comment in `hook-service.ts`). Any extension design that passes secrets through environment variables inherits this.
4. **Session identity.** DSH keys sessions by workspace path and resumes with `dsh-tui --resume <id>`. The bridge sends an empty `transcript_path` because its "persistence seam exposes no artifact path" (`agent-session-resume.ts`). That suggests the session log is behind an interface that does not expose files, consistent with the primary `SessionPersistence` description.
5. **One-shot entry point.** `dsh --profile headless -` reads a prompt on stdin and prints the answer (`commit-message-agent-specs-secondary.ts`). The headless profile is the scripting seam; the interactive TUI is `dsh-tui`.
6. **Launch and detection.** The launcher is `dsh-tui` (package `@deepseek-harness-tui/dsh-tui`, which the app identifies by package entry point), and it needs the `dsh` binary (`tui-agent-config.ts`). The app's composer waits for a DSH-specific ready marker.
7. **Pinned evidence.** Comments cite DSH `0.1.5-rc.1` and say the bridge "documents no `Notification`, no `PermissionRequest` and no `SessionEnd`" (`hook-settings.ts`). The reference repository has no `docs/reference/dsh-harness-integration.md` (cited by the code), so the integration's own design notes are missing from this checkout.

### B.9 Items I could not verify (summary)

- Creator mode mechanism, its skill, its limits (UNVERIFIED).
- Skill contribution format (UNVERIFIED).
- Replacement procedures for model adapter, session log, agent loop, sandbox and system prompt (UNVERIFIED).
- `ctx.tools.register` signature beyond the tutorial placeholder (UNVERIFIED; `defineTool` shape is secondary).
- Manifest fields beyond `dsh.profile` and `dsh.bundle`, and the `dsh.client.inject` rule (secondary, UNVERIFIED).
- `dsh plugin add` and `remove` behaviour, git sources and `#sha&path:` syntax (secondary, UNVERIFIED).
- Version compatibility: peer-range enforcement (contradictory in the secondary guide, UNVERIFIED).
- Cordis `Config`/Schemastery and the full lifecycle order (not in the primer; UNVERIFIED).
- Official docs page counts and migration notes cited by the npm guide (secondary, UNVERIFIED).

---

## Part C. Options for our extension model

### C.1 Constraints from our own plans (`docs/PLAN.md`)

- One Rust core runs as a service; GPUI desktop, React Native Android and a headless server are clients of one protocol. A plugin added to the core must work on all of them, or state which surfaces it works on.
- Stable extension points must exist from Phase 1; the product's own features should use them (Product decisions).
- The protocol is versioned and negotiates capabilities from day one. An extension can add capabilities without a core release. This is a stated requirement.
- GPUI has no webview. The browser pane embeds a web engine for that pane only (`PLAN.md`; GPUI README: no webview or HTML layer, "tailwind-style API" in Rust). Plugin panels on desktop therefore need a decision: web content through the embedded engine, or declarative views.
- React Native on Android: a plugin cannot be downloaded as native code. Google Play forbids downloading executable code (`dex`, JAR, `.so`) from outside Play, but allows code that runs in a virtual machine or interpreter with indirect access to Android APIs, such as JavaScript in a WebView. Interpreted code loaded at runtime "must not allow potential violations of Google Play policies" (Google Play policy, "Device and Network Abuse", primary). Whether this applies to our mobile update path is a legal and policy read (UNVERIFIED).
- iOS is out of scope.

### C.2 Lessons from the current plugin system

- Consent bound to a fingerprint of capabilities, trusted-worker status and instructional content; any change re-prompts. Worth keeping.
- A closed capability vocabulary, so a typo fails validation rather than silently granting nothing. Worth keeping.
- Immutable installs addressed by content hash, a lockfile recording source commit and consent, and a remote kill list. Worth keeping.
- Honest scope: the worker is a normal process with full access to the machine, and the consent copy says so. The capability model limits the API, not the process. A stronger model needs a process sandbox, which the current code does not have.
- Sandboxed UI panels with a CSP that blocks all network access and a postMessage bridge that re-validates everything. This works on desktop; there is no mobile counterpart.
- Gaps: agent-profile contributions are validated but unused; themes and skills were deferred and are rejected; the plugin system is experimental and off by default.

### C.3 Precedent: Zed's WebAssembly extensions

- An extension is a Git repository with `extension.toml`. Rust code compiled to `wasm32-wasip2` is needed only for language servers, context servers (MCP-like) and debuggers. Most extensions are declarative (languages, themes, snippets, icon themes), and their categories are assigned automatically (primary: `https://zed.dev/docs/extensions/developing-extensions`).
- The Rust crate `zed_extension_api` is versioned; the extension implements an `Extension` trait and calls `register_extension!`. A compatibility table maps Zed releases to API version ranges, and newer API versions do not load in older Zed (primary: `https://github.com/zed-industries/zed/tree/main/crates/extension_api`).
- Capabilities gate host operations: `process:exec` (scoped by `command` and `args`), `download_file` (scoped by `host` and `path`), `npm:install` (scoped by `package`), with wildcard scopes. Users restrict them through a `granted_extension_capabilities` setting; denial returns an error, and there is no prompt in the page (primary: `https://zed.dev/docs/extensions/capabilities`). The page does not say where an extension declares its capabilities.
- Distribution: a central repository, `https://github.com/zed-industries/extensions`, with an `extensions.toml` and submodules (not confirmed). "Every submission is reviewed." No signing is mentioned in the primary page (primary, partial; the submission steps are UNVERIFIED).
- Development: install a dev extension locally, which overrides the published version (primary, developing-extensions page).
- Relevance: Zed's extension surface is about language intelligence, tool servers and visual themes. It does not add arbitrary UI panels. Its UI is GPUI itself. A precedent for our tools and rules, not for our panels.

### C.4 Plugin runtime and language options

| Option | What it is | Languages for authors | Isolation boundary | Typed contract | Fit with our core | Effort | Main risk |
|---|---|---|---|---|---|---|---|
| A. WebAssembly components (WIT, WASI) on Wasmtime | Plugins are components that import and export typed interfaces defined in WIT | Rust, and others with component tooling (UNVERIFIED for list) | Strong: no ambient access; capabilities are granted by the host (WASI 0.3.2 is described as the current stable release; primary: component-model.bytecodealliance.org) | Strong: WIT generates bindings | Good for tools, rules and data-only plugins; async needs WASI 0.3 | L to XL | Authoring toolchain friction; async model still evolving (the spec page says proposals "continue to evolve") |
| B. Extism-style WebAssembly modules with a plug-in development kit | Core WebAssembly modules, exports and host functions, one PDK per language | Many PDKs (primary: concepts page; list not given) | Host grants access through manifest: `allowed_hosts`, `allowed_paths`, `memory.max_pages`, `max_http_response_bytes` (primary: manifest page; `timeout_ms` not confirmed) | Weaker: byte or JSON contracts unless we define them | Simple to embed; less typed | M to L | Contract drift between host and PDKs; runtime underlying Extism not confirmed in the pages read (UNVERIFIED: Wasmtime) |
| C. Embedded Lua or Rhai in process | Scripts run inside the core process with an API we expose | Lua or Rhai | Instruction and memory limits are possible; no OS isolation; a bug in the bridge is a core bug | Our API only | Small surface; good for rules and commands | S to M | Sandbox escape through the host API; no process isolation (UNVERIFIED for library limits) |
| D. Embedded JavaScript (QuickJS-class engine, or a V8 runtime) | Scripts in an embedded JS engine | JavaScript or TypeScript | Engine-level isolation only, plus our API gates; V8 is heavy | Our API, typed if we ship types | Matches the current plugin authors (Node) and the DSH tutorial style | M | Memory and startup cost on server; security of the engine (UNVERIFIED for sizes) |
| E. Out-of-process plugins over our core protocol | A plugin is a child process (any language) speaking JSON-RPC over stdio or a socket, the same protocol family as MCP and language servers | Any | OS process boundary; the core brokers capabilities; OS-level sandboxing can be added per platform | Our protocol schemas | Matches "one core, one protocol" and remote execution; plugins become protocol clients | M | Cost of process per plugin; full host access unless sandboxed (a trusted-process model, as today) |
| F. MCP servers as the tool extension | Tools and resources served by MCP servers over stdio or Streamable HTTP | Any | Process boundary (stdio) or network (HTTP); the MCP spec says servers must validate `Origin` and bind to localhost | MCP's JSON-RPC schemas | Good for agent tools; not for UI or core hooks | S to M | Trust: the spec leaves consent to the client; HTTP servers need auth; a tool only, not a full extension model |
| G. Declarative manifests only | Keybindings, commands, recipes, rules, themes as data | None (data) | No code runs | Schemas | Fits CUS-05, CUS-06 and the current contributions | S | Cannot express behaviour; themes and rules need a separate path |
| H. Current model (baseline): trusted Node worker plus sandboxed iframe panels | Plugin `main` runs as a Node process with capability-gated API; panels in CSP-restricted frames | JavaScript or TypeScript (Node) | Process has full access; API gated; panels isolated | Host API v0 with zod schemas | Desktop only; not in the protocol | Already built | Full machine access for the worker; no mobile story |

Notes: effort uses the PLAN scale (S days, M one to two weeks, L several weeks, XL months) for a first usable version. The estimates are judgement, not measured.

### C.5 Sandboxing and permissions options

1. **Manifest-declared capabilities brokered by the core, with consent** (current model; Zed's capabilities are a similar idea but user-configured instead of prompted). Works for any runtime. Limits API use, not raw process access, unless the runtime is sandboxed.
2. **Ambient-free runtime with host-granted capabilities** (WASI preopens and host imports in option A; Extism's `allowed_hosts` and `allowed_paths` in B). Strong by construction for file and network access; needs per-capability host code.
3. **OS-level sandbox around out-of-process plugins** (process restrictions per platform). Needed for option E or H to limit raw access. Platform APIs were not verified in this session.
4. **Trusted-by-consent, no sandbox** (current worker). Honest and simple; contradicts the SAFETY stance of DSH and the security notes of MCP; acceptable only with strong disclosure and a curated registry.
5. **Scoped capabilities per use** (Zed's `process:exec` with `command` and `args` patterns; MassOrca's manifest comments name scoped kinds such as `net:fetch` hosts and `process:exec` globs as later work). Finer grain, more configuration for users.

Policy notes: a plugin that can run commands or reach the network should never run without a visible consent that names those scopes. Secrets should use the OS vault path the app already has (`secrets.*`), not environment variables (DSH drops them anyway).

### C.6 UI contribution options

GPUI (desktop):

- **G1. Declarative view description rendered by GPUI.** Plugins return a tree of known widgets (text, list, table, form fields, buttons, status items), validated by schema. No plugin code runs in the renderer. Matches the protocol; the same description can render on Android. Limits: no arbitrary layout or custom drawing.
- **G2. Embedded web engine for plugin panels.** Reuses the engine chosen for the browser pane (PLAN). Full HTML and CSS, the most flexible. Needs CSP and no-network rules like today's panels. Adds a second web surface to secure; not available on Android.
- **G3. Compiled-in Rust panels.** Plugins as Rust crates linked into the desktop build. Native GPUI quality, no sandbox; suits only first-party features, not third-party plugins.

React Native (Android):

- **R1. Same declarative description, rendered with native components.** Works with a server-driven design: the core sends the description, the client renders it. No code is downloaded. Matches the policy position in C.1 best.
- **R2. WebView panels.** Allowed by Google Play for JavaScript in a WebView, but the policy warns against untrusted web content in a WebView with a JavaScript interface. Needs strict CSP and no bridge to native beyond a fixed message set.
- **R3. Plugin JavaScript executed on the device.** Interpreted code loaded at runtime is governed by the policy quoted in C.1; this is the riskiest option, and any OTA path needs a policy read (UNVERIFIED).

Cross-cutting: the core owns plugin execution and sends views over the protocol, so G1 and R1 are the same contract. G2 and R2 are different. Plugin commands and keybindings need no UI beyond the existing command palette.

### C.7 Distribution and signing options

- **D1. Curated registry repository with review, pinned to exact commits** (Zed-style; MassOrca's marketplace index already does this with `ref` resolved to a commit). Simple; a single trust point.
- **D2. Git URLs pinned to commits, any repository** (MassOrca installs today from HTTPS or SSH git; DSH recommends `#sha`). Flexible; no discovery; the lockfile and consent carry the trust.
- **D3. Package registry (npm-style)** (DSH uses `@deepseek-ai/dsh-*`; peer ranges are the intended compatibility tool, enforcement contradictory in the secondary guide). Familiar to authors; brings a dependency tree into the app's trust.
- **D4. Signed artifacts with a verification step** (for example Sigstore-style signatures over the content hash). Not in any source in this review (UNVERIFIED for tooling). Adds key management.
- **D5. Local development install** (MassOrca dev plugin paths; Zed dev extensions; DSH patch files). Required in every option.
- **Revocation.** MassOrca's remote kill list is an implemented revocation path; keep it in any option.

### C.8 Versioning against the protocol

- Host API major in the manifest (MassOrca: `pluginApi: 1`, `engines.orca >=x.y.z`). Additive changes within a major, per its comments; experimental until frozen.
- WIT package versions, if option A is chosen (UNVERIFIED for tooling details).
- Zed: the API crate is pinned by version and a compatibility table maps Zed releases to API ranges.
- DSH: a developer-preview warning of breaking changes; peer ranges (contradictory, UNVERIFIED).
- Our protocol already plans capability negotiation. Plugins should declare the capabilities they need, and the core should refuse to load a plugin whose capabilities the connected client or server lacks, rather than failing at call time.
- Clients update independently of servers (PLAN). A plugin manifest must therefore state the minimum core version and the client surfaces it needs.

### C.9 Tradeoff summary

| Decision | Option set | What it is best at | Main cost |
|---|---|---|---|
| Runtime | A WASM components | Typed, strong isolation, many languages | Toolchain and async maturity |
| Runtime | E out-of-process protocol | Matches one core, one protocol; any language; remote-friendly | Process cost; needs OS sandbox for real isolation |
| Runtime | C or D embedded scripts | Small, fast, in-process | Sandbox depends on our host API; core risk |
| Runtime | F MCP for tools | Existing ecosystem | Tools only |
| UI desktop | G1 declarative | Safe; same contract for Android | Limited layout |
| UI desktop | G2 web panels | Flexible | Second web surface; not on Android |
| UI Android | R1 declarative native | Store-policy safe | Limited layout |
| UI Android | R2 WebView | Flexible | Policy caution; bridge surface |
| Distribution | D1 curated registry | One trust point; reviewed | Gatekeeping cost |
| Distribution | D2 git commits | No central dependency | Weak discovery; trust by consent |

No option is recommended here. Options can be combined (for example, E for process model with G1 and R1 for UI, and D1 for distribution).

---

## Feature table summary

Eleven rows in this domain (CUS-01 to CUS-11, section A.2). Proposed for the first version: CUS-01, CUS-04 and CUS-08. CUS-09 to CUS-11 are added by pv96 and are proposed as later; open decisions 14 and 15 decide them.

## Open decisions

1. Which plugin runtime (A to H in C.4), or a combination, for the first version that supports third-party code?
2. Must the first version run third-party code at all, or only declarative contributions and MCP servers?
3. Is an OS-level sandbox for out-of-process plugins required for v1, or is consent with disclosure enough?
4. Plugin UI on desktop: declarative views (G1), embedded web panels (G2), or none in v1?
5. Plugin UI on Android: declarative native (R1) only, or none in v1?
6. Distribution: curated registry only (D1), git commits (D2), or both?
7. Signing and verification of plugin artifacts (D4): required, or later?
8. Which core extension points (CUS-08) exist in v1, and which features use them (for example CUS-01 and CUS-04)?
9. Keep the current plugin manifest (`orca-plugin.json`, `publisher.id`) or define a new format in the new protocol?
10. When does the plugin API freeze, and what compatibility promise is made to plugin authors?
11. Is an agent-driven "Creator mode" (DeepSeek Harness style) wanted, and in which language should generated plugins be written?
12. Is DeepSeek Harness a first-version agent (CUS-03 as first) or a later adapter?
13. Who owns CUS-05 (agent-state rules) and CUS-07 (quick commands): the agents and terminal domains, once written?
14. Should users add catalog entries that run their own install commands (CUS-09), or should the catalog hold only curated entries?
15. Should the app write instruction sections and MCP entries into the owner's agent files (CUS-10, CUS-11) in the first version, or leave agent files untouched?

