# Agent integrations: launching and driving agent CLIs

Domain prefix: `AG`. Reference product: the current MassOrca (Electron) worktree. This document inventories behaviour only; it does not prescribe the Rust design.

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits).

## Summary

MassOrca runs third-party AI coding agent CLIs inside workspaces. It knows 45 agent ids (`TuiAgent`), each with a binary name, a launch command, a way of injecting the first prompt, and optional extras (workspace-trust preset, resume command, permission-bypass flag, draft-prompt mechanism). All 45 can be launched as a **terminal TUI** in a PTY. A small subset (7 agents) can additionally run as a **structured (native) chat**: MassOrca spawns the CLI headless as a child process, speaks the CLI's machine protocol (Claude stream-json via the Claude Agent SDK, Codex `app-server` JSON-RPC, Pi `--mode rpc`, Antigravity stream-json, and the Agent Client Protocol (ACP) for Grok, OpenCode and OMP) and renders a chat UI from a host-owned journal. Structured chat is off by default (`experimentalNativeChat`), is local-only (not SSH, not WSL), and silently falls back to a terminal when any feasibility check fails, always recording why in a launch receipt. On top of these sit: install detection, version gating, workspace-trust pre-writes, resume/fork command construction, model/effort option catalogs, delegated child sessions, and one-shot "text generation" calls to agent CLIs (commit message, PR text, branch names, chat names).

Hooks and status plugins that report agent state (27 agents), account/subscription handling, usage polling and on-disk session history are separate domains and are only referenced here.

On pv96 (unmerged), the structured chat gains a host-kept checklist (AG-32), a question card that any chat agent can raise (AG-34), and Results-card rows for environment URLs and opened files (AG-35). Delegated child sessions gain a bring-back of a finished child's git work into the parent worktree (AG-31), a delegation card that can ask for a prototype count and design system, and an optional Jev decision service that can pick a delegated task's tier (AG-33). Several adapters also get the visuals folder, the checklist instructions and a design-system folder grant (AG-36). Each changed section carries a `Source: pv96` line.

## Source directories read (repo-relative, under `src/`)

- `main/agent-launch/` (all), `main/acp/` (launch specs, session setup, structured definitions, dialect interfaces, prompt content, recovery history), `main/provider-process/` (supervisor, teardown, missing-executable), `main/providers/` (agent-foreground-process*, ssh-agent-session-* headers only), `main/native-chat/` (structured-agent-command-resolution, create-support, attachments store, model-catalog service, session-wire registry/idle sweep/restart-resume headers, journal database header), `main/delegated-session/` (all), `main/text-generation/` (requests, launch, headers), `main/runtime/structured-agent-*` (registrations, launch support/check).
- Per-agent: `main/claude/` (launch resolution, definition, flag support, trust file), `main/codex/` (definition, permission policy, app-server session), `main/codex-cli/`, `main/pi/` (definition, launch, registration), `main/antigravity/` (registration, launch, account binding), `main/opencode/` (model startup plan, startup prompt source, file list), `main/opencode2/`, `main/qoder/workspace-trust.ts`, `main/jcode/daemon-prewarm.ts`, `main/kimi/kimi-runtime-home.ts`, `main/zcode/interactive-capability.ts`, directory listings of amp, codebuddy, command-code, copilot, cursor, devin, droid, gemini, grok, hermes, mimo, minimax, muse, openclaude, qwen-code, dsh.
- Root files: `main/agent-launch*`, `main/agent-cli-version-probe.ts`, `main/agent-trust-presets.ts`, `main/agent-trust-write-deadline.ts`, `main/agent-workspace-trust.ts`, `main/agent-workspace-trust-spawn.ts`, `main/execution-host-workspace-trust.ts`, `main/agent-auth-restart-preservation.ts`, `main/preflight/agent-detection.ts`, `main/preflight/agent-command-detection.ts`.
- Shared (to learn the rules): `shared/tui-agent*.ts` (config, startup, permissions, launch defaults, selection, display names, detection commands, resume startup), `shared/agent-resume-argv.ts`, `shared/agent-session-resume.ts`, `shared/agent-launch-intent.ts`, `shared/structured-native-chat-launch-route.ts`, `shared/native-chat-agent-support.ts`, `shared/agent-session-*` (capabilities, access mode, sign-in, availability, registered agents, option catalogs), `shared/commit-message-agent-spec*.ts`, `shared/agent-image-paste.ts`, `shared/agent-prompt-injection.ts`, `shared/pi-runtime-selection.ts`, `shared/node-cli-command-resolution.ts` (headers), `shared/system-cli-install-dirs.ts`, `shared/agent-hook-relay.ts` (source list), `renderer/src/lib/agent-catalog*.ts` and settings pane copy (to learn what the UI asks the core).
- pv96 additions (commit `d3c6311a531`; diffs and file reads against `custom/integration` where a file existed on both): odd docs `odd/tasks/delegated-child-sessions.md` (pv96 diff: task T8 only), `odd/tasks/pv57-child-session-qa.md`, `odd/tasks/child-bring-back.md`, `odd/tasks/checklist-card.md`, and `odd/tasks/remote-session-recorded-file-read.md` (chat side: objective and scope; no pv96 change). `main/jev/` (decision client and API key store). `main/delegated-session/` (headers of the bring-back, ask-user, Jev tier, read-only classifier, person-message and usage-refresh modules; the placement router in full). `main/native-chat/` (checklist store, design-system grant and reported-file row by header; visuals delivery). `main/runtime/rpc/methods/structured-agent-session-checklist.ts`, `structured-agent-session-status-stream.ts` and `structured-agent-session-status-delegated-parent.ts`. `shared/rpc-contract/agent-session-checklist-params.ts`, `shared/rpc-contract/orchestration-ask-user-params.ts` and `shared/jev-decisions-status.ts`. `cli/handlers/jev.ts` and `cli/handlers/orchestration/ask-user-handler.ts`. Diffs of `main/agent-launch/agent-launch-mode.ts`, `main/text-generation/source-control-text-generation-requests.ts` and `shared/agent-status-freshness.ts`. Changed file lists of `main/claude/`, `main/codex/`, `main/pi/`, `main/antigravity/` and `main/acp/`. Subjects of the 80 non-merge commits that touch the owned paths.

## Not read / low confidence

- Hook and status-plugin installers in every per-agent directory (`hook-service.ts`, `hook-config*.ts`, plugin sources for amp, opencode, opencode2, pi, hermes, grok, etc.): covered by the agent status/hooks domain; only their existence per agent is recorded.
- Internals of `claude-accounts/`, `codex-accounts/`, `managed-data-accounts/`, account-home binding in `antigravity/native-*`, `opencode/*credential*`, `minimax/`, `zcode-plan-api-key-store`: accounts domain.
- Transcript decoders, watchers and WSL transcript filesystem (`native-chat/transcript-*`, `wsl-transcript-fs-*`), journal row schemas and reducers, ACP generated protocol types, ACP timeline translators, Codex item translation, Claude background-task trackers: read for intent only, not for rules.
- OpenCode TUI status plugin sources (`opencode/status-plugin-*`, `opencode2/status-plugin-*`) and the OpenCode 2 startup-prompt plugin: understood in outline only.
- Orchestration worker start (`orchestration.workerStart`) beyond how it reuses the launch router and delegation.
- All renderer UI (agent pickers, chat rendering).
- Concrete model names found in static option catalogs (e.g. GPT/Grok/Gemini/Claude ids) are seeds that go stale quickly; treat them as examples, not requirements.
- Platform behaviour is inferred from code branches and comments; nothing was run.
- pv96 items owned by other domains, referenced only: prototypes (design-system files and versions, merge preview, set titles; see `prototypes.md`, PROTO), bot teams and bot runs (`claude-structured-bot-run.ts`, `codex-bot-run-mcp-overrides.ts`, orchestration worker starts; see AUTO), the Marketplace vault and its relay environment (`relay-marketplace-*`), Antigravity account identity from `id_token_email` (accounts domain), and the automated-run marking of text-generation transcripts (session-history domain).
- pv96 not traced: visual annotate (`shared/native-chat-visual-annotate*.ts`, element picks in the prototype viewer), Grok ACP request handling (`acp/acp-dialects/grok-requests.ts`), checklist and Results rendering in the renderer, and the Codex and Claude child-environment and launch-flag changes beyond the design-system grants. Those behaviours come from commit subjects only.
- Jev: rules were read from the decision client, the key-store head, the tier advisor, the CLI and the RPC names. No request was sent to the service, and the Settings screen was not read.
- Removed behaviour: none found in the reviewed paths.
- CodeGraph (pv96 pass): the reference worktree now has a `.codegraph` index, but its semantic answers for the pv96 Jev and delegation modules were unrelated, so it looked stale. It was not synced, because that writes into the read-only worktree. Claims were checked with `git show d3c6311a531`, and with Read and Grep on the worktree, which matched the commit for these paths.

---

## Agent reference table

Legend. **Std detect** = the agent counts as installed when its detect command (or an alias) is found on PATH, else (POSIX only) in a fixed list of install directories (Homebrew, `/usr/local/bin`, snap, Linuxbrew, nix, `~/.opencode/bin`, `~/.vite-plus/bin`, version-manager dirs); PATH is first re-read from the user's login shell; WSL distros are probed inside the distro; SSH hosts are probed by the relay (`preflight.detectAgents`). No `--version` call is made for a TUI-only agent. **Surfaces**: T = terminal TUI, C = structured chat (own protocol), ACP = structured chat over Agent Client Protocol. **Hooks** = MassOrca ships a status hook/plugin installer for the agent (other domain). **Yolo** = permission-bypass argument that is the shipped default of the "Arguments" setting. **Resume** = resume command MassOrca can build once a provider session id is known. **Prompt** = first-prompt injection mode (see AG-05).

| Agent id (display name) | Binary / detect command | Surfaces | Install detection and version probe | Special features | Per-OS limits and notes |
|---|---|---|---|---|---|
| `claude` (Claude) | `claude` | T, C | Std detect. For C: command resolves to a file (or the Command setting is runnable), plus per-launch `--version` probes for flag support (`--plugin-dir` from 2.0.25, `--thinking-display` from 2.1.94, prompt suggestions 2.1.0). C always runs the user's installed CLI, never a bundled binary. | Prompt argv; draft via `--prefill`; Yolo `--dangerously-skip-permissions`; trust preset `claude`; resume `claude --resume <id>` (selector guard strips stale `--resume/--continue`); fork `--resume <id> --fork-session`; hooks; model aliases + effort + fast mode catalog with live model list; C presets default/full-access, auto, accept-edits, plan; C: rewind off (fork instead), compact, images, steering inject. | C: local host only, not WSL, and refused when a WSL-bound managed account is active. Trust writes work for native, WSL guest and SSH hosts. Windows: nothing special beyond shell quoting. |
| `claude-agent-teams` (Claude Agent Teams) | detect `orca` (aliases `orca-dev`, `orca-ide`) and requires `claude` | T | Std detect of the Orca CLI shim and of `claude`; hidden as unavailable on Windows and WSL. | Launch `orca claude-teams` (Linux: `orca-ide claude-teams`, Windows: `orca.cmd claude-teams`); prompt stdin-after-start; Yolo as Claude; trust preset `claude`; teammates open as native splits. Not resumable. | Not offered on Windows or WSL (Claude's in-process fallback is used instead). SSH remotes always use the plain `orca` shim name. |
| `codebuddy` (CodeBuddy) | `codebuddy` (alias `cbc`) | T | Std detect. | Prompt argv; Yolo `--dangerously-skip-permissions`; resume `--resume <id>`; hooks; option catalog (effort, model tiers). | - |
| `openclaude` (OpenClaude) | `openclaude` | T | Std detect. | Prompt argv; draft `--prefill`; Yolo as Claude; hooks reuse the Claude hook service; transcripts use Claude format so the terminal-backed chat view works. Not resumable by id. | - |
| `codex` (Codex) | `codex` | T, C | Std detect. For C: command resolves to a file; `codex app-server` capability is learned from the child's stderr; read-only probes use `app-server` with `-s read-only -a never` and plugins disabled. | Prompt argv (delivered after shell is ready); Yolo `--dangerously-bypass-approvals-and-sandbox`; trust preset `codex` (keyed by start folder, writes `~/.codex/config.toml`); resume `codex resume <id>` (adds `-c tui.resume_cwd=current` when launching in a chosen cwd); fork `codex fork <id>`; hooks; model + reasoning effort catalog (`-m`, `-c model_reasoning_effort=`); C presets default/full-access, auto, read-only; C: rewind, compact, goals, provider commands (`/usage /skills /plugins /memories /rename /fork /side /review /approve /stop`), images, steering inject, sandbox/approval enforced by Codex. | C: local, not WSL (the receipt vocabulary also has a "Codex on Windows" reason, but no current code produces it). Trust is never pre-written for a WSL guest launch. Windows TUI: paste newline uses Alt+Enter encoding; submit retry after 1.2 s. |
| `autohand` (Autohand Code) | `autohand` | T | Std detect. | Prompt stdin-after-start; Yolo `--unrestricted`. | - |
| `opencode` (OpenCode) | `opencode` | T, ACP | Std detect. ACP: `--version` must be a stable 1.x >= 1.18.31 or 2.x >= 2.0.14. | Prompt `--prompt`; paste waits for bracketed-paste + cursor signal (20 s budget on Windows hosts); resume `opencode --session <id>`; hooks via generated plugin; Yolo none (flag removed from launch args); model chosen with `--model` only after the host verifies the model is available (refused otherwise); ACP via `opencode acp` with `OPENCODE_CLIENT=acp`, question tool disabled, images, compaction, model list via `opencode models`. | Model-pinned terminal launches refuse WSL, remote hosts and a platform different from the host. ACP: local, not WSL; on Windows only when a process start time is available. |
| `opencode2` (OpenCode 2 beta) | `opencode2` | T | Std detect. | Launch `opencode2 --standalone` (private server inherits the pane's hook identity); prompt `--prompt` via an Orca-installed TUI plugin that claims the prompt through a local endpoint; resume `opencode2 --session <id>`; hooks via plugin. | Prompt injection works only on known plugin API versions (native prompt versions 2.0.12 and 2.0.16). |
| `mimo-code` (MiMo Code) | `mimo` | T | Std detect. | Prompt `--prompt`; paste ready signal like OpenCode; resume `mimo --session <id>`; hooks. | - |
| `pi` (Pi) | `pi` (alias `gentle-shell`) | T, C | Std detect; Pi and Gentle Shell are detected separately and the host reports which runtime resolves. For C: `--version` must be stable >= 0.84.0 (0.x) or >= 1.0.0; Gentle Shell is probed through its own version probe. | Prompt argv; draft via env `ORCA_PI_PREFILL` + Pi extension; resume `pi --session <transcript file>` (Gentle Shell: `gentle-shell -- --session <file>`); hooks via Pi extension; C is `pi --mode rpc`; C capabilities: compact, images, steering inject, provider commands `/name /export`, full-access only for stock Pi, `default`/`full-access`/`auto` for Gentle Shell; slash command catalog cached to disk. | Windows: Shift+Enter encoded as CSI-u. C needs Windows process start-time proof. The `--mode/--session/--fork/-p` flags are refused in saved arguments for C. If only Gentle Shell is installed a chat silently uses it. |
| `omp` (OMP) | `omp` | T, ACP | Std detect. ACP: stable >= 17.0.5. | Prompt argv (fresh-session wrapper; resume uses transcript path); draft env `ORCA_OMP_PREFILL`; resume `omp --resume <file or id>`; hooks; model catalog (`--model`, `omp models --json` for terminal picker); ACP via `omp acp` with plugin dir; no CLI sign-in command. | Windows: Shift+Enter CSI-u. ACP: local, not WSL, Windows needs process start time. Path mentions containing both quote kinds are sent as file URLs. |
| `prime-agent` (Prime Agent) | `prime-agent` | T | Std detect. | Prompt argv after `--`; embeds Pi's TUI (CSI-u on Windows); resume `prime-agent --resume <transcript file>`; hooks. | - |
| `qoder` (Qoder CLI) | `qodercli` (alias `qoder`) | T | Std detect. | Prompt `--prompt-interactive`; Yolo `--dangerously-skip-permissions`; trust preset `qoder` (`~/.qoder/settings.json` `permissions.trustDirectories`); resume `qodercli --resume <id>`; hooks. | - |
| `qoder-cn` (Qoder CLI China) | `qoderclicn` (alias `qodercn`) | T | Std detect. | As `qoder`, config dir `.qoder-cn`. | - |
| `gemini` (Gemini) | `gemini` | T | Std detect. | Prompt `--prompt-interactive`; Yolo `--yolo`; resume `gemini --resume <id>`; model `-m` with `/model` picker; image attachment by pasted path; hooks. | - |
| `antigravity` (Antigravity) | `agy` | T, C | Std detect. For C: `agy --version` must be stable 1.x >= 1.3.5; `--add-dir` (child-session instructions) only from 1.4.9. | Prompt `--prompt-interactive`; Yolo `--dangerously-skip-permissions`; trust preset `antigravity` (exact folder, `~/.gemini/antigravity-cli/settings.json` `trustedWorkspaces`, one entry per worktree); resume `agy --conversation <id>`; submit settle 45 ms per prompt line; hooks; C is `agy --input-format stream-json --output-format stream-json`; C capabilities: no compact, no images, steering queue, presets default/full-access/auto/accept-edits/plan; model name + effort variants grouped in picker. | C is refused on Windows (`antigravityWindowsUnsupported`) and needs the host credential home to equal the child's; alternate model-provider auth cannot be bound to a Google account. Brain transcripts exist only on the owning host. |
| `aider` (Aider) | `aider` | T | Std detect. | Prompt stdin-after-start; Yolo `--yes-always`. Not resumable. | - |
| `goose` (Goose) | `goose` | T | Std detect. | Prompt stdin-after-start; Yolo is env `GOOSE_MODE=auto`. | - |
| `amp` (Amp) | `amp` | T | Std detect. | Prompt stdin-after-start; Yolo `--dangerously-allow-all`; hooks via plugin; no provider session id (not resumable by id). | - |
| `kilo` (Kilocode) | `kilo` | T | Std detect. | Prompt stdin-after-start; `--dangerously-skip-permissions` is stripped from saved arguments. | - |
| `kiro` (Kiro) | `kiro-cli` | T | Std detect (id stays `kiro` for stored prefs). | Launch `kiro-cli chat --tui`; Yolo `--trust-all-tools`; prompt stdin-after-start. | - |
| `crush` (Charm) | `crush` | T | Std detect. | Prompt stdin-after-start; Yolo `--yolo`. | - |
| `aug` (Auggie) | `auggie` | T | Std detect (id `aug`). | Prompt stdin-after-start. | - |
| `cline` (Cline) | `cline` | T | Std detect. | Prompt stdin-after-start; Yolo `--auto-approve true`. | - |
| `codebuff` (Codebuff) | `codebuff` | T | Std detect. | Prompt stdin-after-start. | - |
| `freebuff` (Freebuff) | `freebuff` | T | Std detect. | Prompt stdin-after-start. | - |
| `command-code` (Command Code) | `command-code` (not `cmd`, to avoid the Windows shell) | T | Std detect. | Launch `command-code --trust` (skips first-run trust menu); prompt argv; Yolo `--yolo`; hooks; no provider session id. | Detect name chosen to avoid collision with `cmd.exe`. |
| `continue` (Continue) | `cn` | T | Std detect. | Prompt stdin-after-start; Yolo `--allow "*"`. | Binary is `cn` because `continue` is a shell keyword. |
| `cursor` (Cursor) | `cursor-agent` | T | Std detect. | Prompt argv; Yolo `--yolo`; trust preset `cursor` (marker `~/.cursor/projects/<slug>/.workspace-trusted`); resume `cursor-agent --resume <conversation id>`; model `--model`, models from `cursor-agent models`; image attachment by path; hooks. | Slug of the path replaces `:` and separators on Windows. |
| `droid` (Droid) | `droid` | T | Std detect. | Prompt argv; Yolo `--auto high`; resume `droid --resume <id>`; image attachment; CSI-u for Shift/Ctrl+Enter on Windows; hooks. | - |
| `kimi` (Kimi) | `kimi` (alias `kimi-code`) | T | Std detect (alias needed for foreground recognition). | Prompt stdin-after-start; Yolo `--yolo`; resume `kimi --session <id>` (sessions are work-dir scoped); hooks via TOML config; home `KIMI_CODE_HOME` or `~/.kimi-code`. | WSL runtime probing for Kimi's home happens on Windows only. |
| `mistral-vibe` (Mistral Vibe) | `vibe` (alias `mistral-vibe`) | T | Std detect. | Prompt stdin-after-start; Yolo `--agent auto-approve`. | - |
| `qwen-code` (Qwen Code) | `qwen` | T | Std detect. | Prompt stdin-after-start; Yolo `--approval-mode yolo`; resume `qwen --resume <id>`; hooks; 1.2 s submit retry (Enter is ignored shortly after paste on Windows/old Node). | Windows paste workaround. |
| `rovo` (Rovo Dev) | `acli` | T | Std detect of `acli`. | Launch `acli rovodev run`; Yolo `--yolo`; prompt stdin-after-start. | Detection cannot distinguish Rovo Dev from other `acli` uses. |
| `hermes` (Hermes) | `hermes` | T | Std detect. | Launch `hermes --tui`; prompt via Hermes's startup-query contract (agent submits once its composer is ready); Yolo `--yolo`; hooks via managed plugin and YAML config edits. Not resumable by id. | - |
| `openclaw` (OpenClaw) | `openclaw` | T | Std detect. | Prompt stdin-after-start. | - |
| `copilot` (GitHub Copilot) | `copilot` | T | Std detect. | Prompt `-i` (interactive, because `--prompt` exits); Yolo `--yolo`; trust preset `copilot` (`~/.copilot/config.json` `trustedFolders`, file kept mode 0600); resume `copilot --resume=<id>`; image attachment; hooks. | Trust writer inherits from any trusted ancestor, so MassOrca refuses to pre-trust a home or disk root. |
| `grok` (Grok) | `grok` | T, ACP | Std detect. ACP: no version floor; `grok agent stdio`. | Prompt argv after `--`; Yolo `--permission-mode bypassPermissions`; resume `grok --resume <id>`; image attachment; Ctrl+Enter CSI-u; composer-prompt paste signal; hooks; model `-m`, `--reasoning-effort`, models via `grok models`; ACP: `grok agent [--always-approve] [--plugin-dir] stdio`, auth via `XAI_API_KEY` or cached token, models read from `initialize`, compaction, subagents and background tasks mapped. | ACP: local, not WSL, Windows needs process start time. Hook script has a Windows launcher chain. |
| `devin` (Devin CLI) | `devin` | T | Std detect. | Prompt stdin-after-start (`devin -- <prompt>` would auto-submit); Yolo `--permission-mode bypass --respect-workspace-trust false`; resume `devin --resume <id>`; hooks. | - |
| `ante` (Ante) | `ante` | T | Std detect. | Prompt stdin-after-start (its `--prompt` is headless); Yolo `--yolo`. | - |
| `trae` (Trae CLI) | `traecli` | T | Std detect (name avoids another project's `trae-cli`). | Prompt argv after `--`; Yolo `--yolo`. | - |
| `muse` (Muse) | `muse` | T | Std detect. | Launch `muse --trust-workspace`; prompt stdin-after-start; Yolo `--yolo`; resume `muse resume <id>`; model + effort flags; hooks. | - |
| `zcode` (ZCode) | `zcode` (process name `zcode-cli`) | T | Std detect plus an interactive-capability probe (run once, no TTY) that tells a build with a TUI from one without before a pane is opened. | Prompt stdin-after-start (argv would be parsed as a subcommand); Yolo `--mode yolo`; resume `zcode --resume <id>`; composer-prompt paste signal; hooks. | - |
| `dsh` (DeepSeek Harness) | `dsh-tui` (alias `dst`), requires `dsh` | T | Std detect of both commands. | Launch `dsh-tui .`; prompt stdin-after-start; composer-prompt paste signal; resume `dsh-tui --resume <id>` (sessions keyed by workspace path); hooks; home patching on install. | - |
| `jcode` (Jcode) | `jcode` | T | Std detect. | Prompt stdin-after-start; resume `jcode --resume <id>`; hooks; per-pane `JCODE_RUNTIME_DIR` plus a background daemon pre-warm so the TUI finds a live socket within its 5 s wait. | Pre-warm and runtime dir are non-Windows only. |

`minimax` and the `zcode` plan key store in the reference tree are API-key/cookie stores for usage and accounts, not launchable agents (see accounts domain).

---

## Features

### AG-01 Supported agent catalog, defaults and enable/disable
**What the user does:** picks an agent from a list (new workspace, new agent tab, settings); sets a default agent; hides agents they never use.
**Behaviour and rules:**
- One definition per agent id (45): display name, detect command and aliases, launch command (some differ from the detect command, e.g. `kiro-cli chat --tui`, `hermes --tui`, `dsh-tui .`, `acli rovodev run`, `opencode2 --standalone`), expected foreground process name (for example `zcode-cli`, `dsh`), prompt-injection mode, optional extras (see AG-04 to AG-13). Ids are kept stable even when the binary has a different name (`kiro`, `aug`, `continue`) because ids are stored in user settings.
- A homepage/docs URL per agent is shown as installation help (see AG-02); there is no built-in installer or updater for any agent CLI.
- Default agent setting `defaultTuiAgent` (or a "blank" choice meaning no agent). When no default is chosen, or the default is not installed/enabled, the first installed and enabled agent in a fixed priority order is auto-picked (claude, claude-agent-teams, openclaude, codex, grok, copilot, opencode2, opencode, ... jcode).
- `disabledTuiAgents` hides agents from pickers and from auto-pick; unknown ids in stored lists are dropped.
- Offering "Claude Agent Teams" requires both the Orca CLI shim and `claude` (see AG-28).
**Persists:** `defaultTuiAgent`, `disabledTuiAgents` in settings.
**Depends on:** AG-02. Settings domain for storage.

### AG-02 Install detection, refresh and installation help
**What the user does:** opens the Agents settings or an agent picker and sees which agent CLIs are installed on the workspace's host; after installing one, presses Refresh instead of restarting.
**Behaviour and rules:**
- Detection asks "is command X runnable on the host that will run the agent": the local machine, a WSL distro (probed with the distro's own PATH), or an SSH host (the relay answers `preflight.detectAgents`; a disconnected host reports nothing without error).
- Locally: command on PATH first; for commands not found, a bulk check of install directories (POSIX only; Windows has no install-dir fallback). On a cold GUI launch PATH is first re-read from the user's login shell (shell hydration); Refresh forces that re-read and returns the new agent list plus which PATH segments were added and why hydration failed. For WSL, Refresh clears the cached guest PATH before probing.
- Aliases map several binaries to one agent (e.g. `pi` and `gentle-shell` both mean Pi, the host reports which Pi runtime resolved). An agent can require additional commands (`dsh-tui` needs `dsh`; Agent Teams needs `claude`) and can be marked unsupported on some runtimes (Agent Teams on Windows and WSL).
- The guest PATH of a WSL distro is cached for the process lifetime (hence the explicit invalidation on Refresh). The neighbouring git/gh/hosting preflight caches its result for the session, with WSL entries expiring after 30 s so an unreachable distro can recover; that cache is not the agent list.
- Installation/update help is a link per agent (docs or repo URL) and, for agents MassOrca can manage hooks for, install/repair controls (other domain). MassOrca never downloads or updates agent binaries.
- When the Command setting (AG-11) names a file, that file is what is used for launch; for structured chats an unrunnable Command setting refuses the start instead of silently running the stock binary.
**Clients ask the core:** detect agents (local, per host), refresh agents, detect Pi runtimes.
**Core pushes back:** list of installed agent ids per host; for structured chats a per-agent "unavailable" verdict (`cliMissing`, `notSignedIn` with account kind).
**Depends on:** remote-hosts (SSH/WSL), terminals (login shell PATH).

### AG-03 CLI version probing and capability gating
**What the user sees:** a structured chat only starts when the installed CLI is a release MassOrca has verified; otherwise the launch falls back to a terminal and says why.
**Behaviour and rules:**
- The probe runs `<cli> --version` with a 5 s timeout and 4 KB output cap, parses the version, and returns one of: supported, did-not-start (CLI missing or not runnable), unsupported (non-zero exit, timeout, no version, or not an allowed release). Outcomes are logged by program name only.
- Rules are "stable release only" (prerelease versions are refused) plus a floor and sometimes a major line: Pi >= 0.84.0 on 0.x or >= 1.0.0; OMP >= 17.0.5; OpenCode 1.x >= 1.18.31 or 2.x >= 2.0.14; Antigravity 1.x >= 1.3.5. Codex and Claude have no floor but Claude flags are probed individually (`--plugin-dir`, `--thinking-display`, prompt suggestions) and cached per binary and workspace; a flag the CLI does not know is simply not passed.
- OpenCode 2's startup-prompt plugin works only on specific known versions.
- TUI-only agents get no version probe.
**Depends on:** AG-02.

### AG-04 Terminal launch command construction
**What the user does:** starts an agent in a workspace terminal; MassOrca types the right command in the right shell.
**Behaviour and rules:**
- Command = Command override (AG-11) or the agent's launch command for the platform (Linux Orca shim is renamed `orca-ide`, Windows `orca.cmd`; SSH remotes always plain `orca`), then picked model/effort flags (AG-14), then the user's saved Arguments, then the prompt as per AG-05. Flags the user's Arguments already set for the same option replace the picker's, because a repeated model flag can stop some CLIs.
- Quoting follows the shell the command is typed into: POSIX family (bash/zsh/fish), PowerShell, or cmd on Windows (chosen from the configured Windows terminal shell; remote targets are not assumed to be Windows). Arguments that cannot be tokenised for the shell make the launch fail with a message rather than guessing.
- Per-agent environment variables are applied to the process environment (AG-11).
- The agent's "expected foreground process" is recorded so later actions (reuse a running agent, inject text) can verify an agent really runs in the pane.
- Some launches are delivered only after the shell reports ready (Codex), so rc-file startup cannot eat the command.
- Windows `.cmd`/`.bat` shims and Node-script agents are resolved to something spawnable; a CLI resolved from a version-manager directory is paired with the `node` next to it on PATH.
**Depends on:** terminals (PTY, shell-ready), AG-01, AG-11.

### AG-05 Prompt delivery to terminal agents
**What the user does:** types a task when creating a workspace/agent (or an API caller supplies one) and the agent starts working on it.
**Behaviour and rules:** six injection modes by agent:
- `argv`: prompt is a positional argument (Claude, Codex, OpenClaude, CodeBuddy, OMP, Pi, Prime Agent, Cursor, Droid, Grok, Trae, Command Code). Some need `--` first so a prompt like `help` is not parsed as a subcommand (Grok, Trae, Prime Agent).
- `flag-prompt`: `--prompt <text>` (OpenCode, OpenCode 2, MiMo). For OpenCode `run` commands the positional form is used.
- `flag-prompt-interactive`: `--prompt-interactive <text>` (Gemini, Qoder, Antigravity).
- `flag-interactive`: `-i <text>` (Copilot, because `--prompt` makes it exit after one turn).
- `hermes-query`: Hermes's own startup-query contract.
- `stdin-after-start`: launch the bare TUI, wait until the composer is ready, then bracketed-paste the text and press Enter (all remaining agents, including those whose argv would be treated as a headless run or a subcommand: Ante, ZCode, Muse, Devin, DSH, Kiro).
- Readiness for paste: default is a quiet-render window (8 s hard timeout); agents with a better signal use it: Codex composer prompt (20 s), Grok/DSH/ZCode composer prompts, OpenCode bracketed-paste-enabled + cursor (20 s) and an "agent row" signal before Enter. Enter is sent on a later turn (50 ms) after paste end; Codex and Qwen get one extra blind Enter after 1.2 s; Antigravity waits 45 ms per prompt line because it expands pastes slowly; OMP needs Enter inside the paste write; Claude-style agents need a short typed lead line before a pasted brief.
- Windows specifics: ConPTY ingests pasted bytes slowly (about 64 bytes/ms is assumed), so the submit delay scales with size; some TUIs need Shift+Enter or paste newlines encoded as CSI-u or Alt+Enter.
- Only a prompt that fits on the typed command line rides the command (long or multi-line prompts fall back to paste); the receipt reports `handed-to-terminal`, never `journaled`, because a terminal keeps no transcript.
**Receipt outcomes:** journaled (structured chat), handed-to-terminal, not-delivered, unconfirmed (replay after a host stop mid-delivery; the caller must not resend).
**Depends on:** AG-04, terminals.

### AG-06 Draft (unsent) prompts
**What the user does:** drops text (for example a URL or an issue brief) into an agent's input without submitting it, so they can edit first.
**Behaviour and rules:**
- Preferred: the agent's own seeding option: Claude and OpenClaude `--prefill <text>`; Pi and OMP via an environment variable read by an Orca extension (`ORCA_PI_PREFILL`, `ORCA_OMP_PREFILL`, with cleanup after launch).
- Otherwise paste after ready (AG-05) without Enter.
- A draft is refused if the resulting command line would exceed the platform limit. A structured chat accepts a draft directly; a terminal draft reports `not-delivered` because the host cannot confirm the composer accepted it.
**Depends on:** AG-05.

### AG-07 Image and file attachments for terminal agents
**What the user does:** pastes or drags an image or file into an agent terminal.
**Behaviour and rules:**
- Agents with verified paste-attachment behaviour (Claude, OpenClaude, Codex, Gemini, Cursor, Copilot, Droid, Grok) get the raw file path pasted so the CLI creates an attachment; every other agent gets an `@path` file mention (quoted when it contains spaces, `@`, or quotes; OMP gets a `file://` URL when the path contains both quote kinds).
- Pastes are framed in bracketed paste; image paste followed by text is separated so the TUI does not merge them.
**Depends on:** terminals (paste), AG-05.

### AG-08 Launch intent API (`agent.launch`) and launch receipts
**What the user (or CLI, phone, automation) does:** asks "run agent X in workspace Y with this prompt" without caring about the surface; the core decides and reports what it did.
**Behaviour and rules:**
- Intent: agent, target (existing workspace, a worktree to create, or a folder workspace to create), optional prompt with delivery `submit` or `draft`, session options (model, effort, mode), reuse of a running terminal, per-call Arguments override (terminal only), start `cwd`, launch source (telemetry label), caller-minted `paneKey` or `sessionId`, placement hints (group / after tab) and presentation (focused or background).
- Result: outcome (`terminal` with handle and pane key, or `structured` with session id, handle and tab id), the workspace id, an optional warning, a mode **receipt** (mode, the user's preferred mode, a reason code and one sentence, always present so a downgrade is never silent), a prompt receipt, and a placement receipt (which group was used and why a fallback happened).
- Refusals: pane or session already exists, tab closed during launch, target forbidden, replay refused. Launches are idempotent and replayable (`agent.launchReplay`); a terminal whose spawn failed before the request left is reported as definitively failed.
- A launch into a workspace that does not exist yet creates the workspace first, without a startup agent, then asks the host about a chat, so the route can be decided against the real workspace.
- A launch that carries a start `cwd` outside the workspace root is always a terminal.
- Telemetry `agent_started` is derived by the host (agent kind, request kind new) except the caller's launch source.
**Depends on:** AG-04, AG-05, AG-15 (mode decision), workspaces, terminals.

### AG-09 Creating a workspace with a startup agent
**What the user does:** creates a worktree or folder workspace and chooses an agent and prompt in the same step.
**Behaviour and rules:** worktree creation with an agent keeps its historical contract: the agent is the workspace's startup terminal (never a chat), and the create delivers the prompt itself ("legacy host" prompt policy). The create request carries the agent fields (`startupAgent`, command, prompt, draft, launch config, env); the new launch router strips them from creates it fronts so a stale field cannot start a second agent. If the startup terminal cannot be spawned the workspace still exists and the result carries a warning. Worktree names/branches can be renamed after the agent's first work (see AG-27).
**Depends on:** AG-08, workspaces/git domains.

### AG-10 Permission posture (Yolo / Manual) and access presets
**What the user does:** chooses whether agents run with approvals off ("Yolo") or on ("Manual"), globally or per agent.
**Behaviour and rules:**
- There is no separate storage: the toggle writes or removes each agent's bypass argument inside its saved Arguments (AG-11). Shipped default is Yolo for every agent that has a bypass flag (list in the table); Goose uses an environment variable. A global summary shows Yolo, Manual or Mixed and a global switch rewrites only values that are empty or exactly the default.
- OpenCode and Kilo have the bypass flag removed from saved args because they never exposed it on the TUI.
- For structured chats the posture is read from the same resolved arguments (flags after `--` or inside quoted prompt text do not count) and mapped to the chat's access mode: `default` (approvals on) or `full-access`. Per-agent declared presets: Claude auto / accept-edits / plan, Codex auto / read-only, Antigravity auto / accept-edits / plan, Pi full-access (and Gentle Shell default/full-access/auto), ACP agents declare none (MassOrca answers every permission request itself; agents still decide when to ask, so this is not a sandbox).
- Who enforces: `provider` (Claude, Codex, Pi, Antigravity: the CLI gets launch flags/thread policy) or `orca` (ACP: MassOrca answers permission requests by the setting).
- Codex policies: Yolo = never ask + no sandbox; Manual = ask on request + workspace-write sandbox (stated explicitly so a user config of `approval_policy=never` cannot leak in); read-only preset = ask on request + read-only; auto = auto-review of approvals.
**Persists:** `agentDefaultArgs`, `agentDefaultEnv`.
**Depends on:** AG-11, AG-15.

### AG-11 Per-agent Command, Arguments and Environment settings
**What the user does:** overrides the binary or wrapper for an agent, adds standing arguments, sets extra environment variables (for example to point an agent at another provider or proxy).
**Behaviour and rules:**
- Source: pv96 (unmerged). Chat-mode agents, and agents that an SSH relay launches, now also receive the Marketplace app environment from their own host's vault, so the relay path works without a connected desktop.
- `agentCmdOverrides[agent]`: replaces the launch command. For structured chats it must be a single literal executable (no shell operators, expansion, newline, `$` or backtick); it is verified on the execution host, and on Windows must end in `.exe/.com/.cmd/.bat`. A non-runnable value refuses the start with a message that never echoes the value (it may hold a secret). A customised command does not by itself force a terminal.
- `agentDefaultArgs[agent]`: appended after picker flags; a per-call override (`agentArgs`) applies only to terminal launches. Stored values are sanitised (unsupported flags removed).
- `agentDefaultEnv[agent]`: merged into the child's environment; an empty record owns the key (explicit "none").
- Structured chats additionally build the child environment from the login-shell environment according to chat settings (`nativeChatInheritShellEnvironment`, `nativeChatShellEnvironmentVariables`).
- The same override and args feed commit-message generation (AG-27).
**Persists:** `agentCmdOverrides`, `agentDefaultArgs`, `agentDefaultEnv`.
**Depends on:** AG-01.

### AG-12 Workspace trust pre-writes
**What the user does:** nothing; with the setting on, agents MassOrca starts do not stop at "Do you trust this folder?".
**Behaviour and rules:**
- Setting `agentWorkspaceTrustEnabled` (default on). Off: agents ask themselves; unattended agents (orchestration workers, automations, agents started from a phone) would wait at that prompt.
- Applied only to fresh launches (not reattach or restore), just before the PTY spawns, for agents with a preset: `claude`, `codex`, `cursor`, `copilot`, `antigravity`, `qoder`, `qoder-cn`. Also applied to a structured Codex chat (there is no PTY) so that a read-only chat still reads project `.codex` config.
- What is written, per agent: Claude (`.claude.json` / `CLAUDE_CONFIG_DIR` config, under a file lock that MassOrca never breaks; skipped if held or unreadable), Codex (`[projects."<realpath>"] trust_level = "trusted"` in every `config.toml` the launched Codex may read, in the hook installer's lock order, keyed by the start folder), Cursor (marker file under `~/.cursor/projects/<slug>/`), Copilot (`trustedFolders` array), Antigravity (`trustedWorkspaces`, exact path, one per new worktree), Qoder (`permissions.trustDirectories`). Unrelated keys survive; corrupt config files are never overwritten.
- Safety: for agents that inherit trust from ancestors (Claude, Copilot, Qoder) MassOrca refuses to write trust for a home directory or disk root or anything above them.
- Where: written on the host that runs the agent. SSH: the relay does it (the spawn request carries `agentWorkspaceTrust`). WSL guest: only Claude is written (to the guest's config); other presets write nothing. Paths are canonicalised (realpath) because agents compare resolved paths.
- Deadlines: Codex 20 s (queues behind hook-install writers of the same file), others 1.5 s. On timeout the launch proceeds untrusted and the agent prompts; trust is never granted that was not written (fails closed).
**Persists:** edits agent-owned config files.
**Depends on:** AG-04, remote-hosts.

### AG-13 Resume, continue and fork of terminal sessions
**What the user does:** reopens a workspace/tab and the agent continues the same conversation; chooses to fork a Claude or Codex conversation.
**Behaviour and rules:**
- 24 agents are resumable (see table). The core stores a provider session record per pane (agent, session id or conversation id, optional transcript path, launch command/args/env, prompt, last state, interrupted flag, whether captured on sleep, quit or live) learned from agent hooks (other domain); the resume command is rebuilt with the agent's own flag shape (`--resume <id>`, `resume <id>`, `--session <id>`, `--conversation <id>`, `--resume=<id>`, or a transcript path for Pi/Prime Agent/OMP).
- Session ids are validated (non-empty, <= 512 chars, no control characters, cannot start with `-`).
- Claude cold restore removes any stale `--resume`/`--continue` selector from the user's saved command and appends exactly one authoritative selector (fail-open if the command cannot be parsed). Picker flags (model etc.) are not replayed on resume because the resumed session restores its own state.
- Codex: resume can pass `-c tui.resume_cwd=current` so it does not prompt for a deleted recorded folder. Fork: Claude `--resume <id> --fork-session`, Codex `fork <id>`.
- Pi resumes with the runtime recorded for that session (stock Pi vs Gentle Shell), decided from the host record, the transcript's home folder, the launch seed, then the Command setting.
- Kimi, Cursor and DSH sessions are scoped to the working directory; callers must keep the cwd.
- Before an app restart or update, active Codex and Claude auth selections are synced to disk within a 2 s budget (WSL Codex separately) so a relaunch does not lose a login; failures only log.
**Depends on:** agent status/hooks domain (session id capture), session-history domain, AG-04.

### AG-14 Model, effort and mode selection for launches
**What the user does:** picks a model, reasoning effort or fast mode when starting an agent (or as a worker preference for orchestration), and can change it mid-session.
**Behaviour and rules:**
- Option catalogs exist for 10 agents (Antigravity, Claude, CodeBuddy, Codex, Gemini, Cursor, Grok, Muse, OMP, OpenCode). Each catalog has a seeded model list, per-model options (effort levels, fast mode, thinking toggle) and how each option applies: launch flags (`--model`, `-m`, `--effort`, `-c model_reasoning_effort=`, `--reasoning-effort`), or mid-session commands (`/model X`, `/effort X`, `/fast` toggle, the agent's own `/model` picker typed into the terminal, `/orca-model` for OMP). Claude's mid-session model switch may raise a confirmation prompt that is detected so the user answers it in the terminal.
- Live model discovery overlays the seed for Claude (a `list_models` control request over stream-json), Cursor (`cursor-agent models`), Grok (`grok models`), OMP (`omp models --json`); unknown persisted model ids are passed through, because the seeds are not complete.
- Options that conflict with the user's saved Arguments or Command override cause a clear error (`conflicts with the requested launch preferences`).
- OpenCode terminal launches with a chosen model are only allowed when the host can verify the model: stable legacy versions check availability; 2.0.16 builds a one-off config and verifies agent and model before and after; anything unverifiable is refused, not guessed.
- Delegation can pick model tiers (AG-26).
- "Provider selection" is mostly the CLI's own: Codex model-provider config is read; OpenClaude targets other providers; Pi requires `--model` with `--provider`; environment overrides (AG-11) are the user's route to other endpoints. MassOrca has no provider abstraction of its own across agents.
**Depends on:** AG-04, AG-23 for chats.

**Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`):** one model picker (agent, model, reasoning effort and access mode) sits in every chat composer (X-AG-02); the access mode is its footer. Proposal: first version (was later).

### AG-15 Structured (native) chat platform
**What the user does:** turns on the Chat setting; new agent tabs for supported agents open as a chat view instead of a terminal; chats survive app restarts, can be reattached from other clients (desktop, phone, CLI), and keep running in the core.
**Behaviour and rules:**
- Source: pv96 (unmerged). The launch-mode check judges a restricted-access request against the Pi runtime that will run (stock Pi or Gentle Shell). Chats also gain a Results card with environment URLs and opened files (AG-35), a shared checklist (AG-32) and a question card (AG-34).
- **Mode decision** (per launch, receipt in AG-08): the user's default (`experimentalNativeChat`) chooses; a caller may ask for a chat over a terminal default. A chat is downgraded to a terminal, with a named reason, when: the host is remote (SSH or a different execution host), an existing terminal is being reused, the agent has no structured adapter, the launch asks for a start cwd outside its workspace, the host or workspace is WSL, the host lacks the structured-session capability or has not answered yet, the caller's client cannot render chats, the agent's CLI is missing/unsupported/not runnable, or the access posture cannot be applied. Support is asked of the host that runs the agent (`agentSession.createSupport`) before anything is created, so a refusal becomes a terminal rather than a failed launch.
- **Registered agents:** a host advertises which structured agents it runs and each agent's capability record (AG-16) via `agentSession.agents`; older hosts are assumed to run only Claude and Codex.
- **Client API** (`agentSession.*`): agents, createSupport, create, ensure, attach, subscribe / unsubscribe, subscribeStatus, subscribeTurnCompletions, send, cancel, close, hold / release, history, options / setOption, modelCatalog, commands, providerCommand, conversationCommand, conversationOutline, rewind, threadGoal, respondTo / respondToApproval / respondToQuestion, queuedMessageSend / Delete / Resume, restartResumable / restartResume / restartContinue / restartResumableDismiss, continueInterrupted, handoffStatus, acknowledgeAttention, dismissPromptSuggestion, readPlanDocument, readRecordedFile, readVisual, reveal.
- **Persistence:** a host-wide SQLite journal (WAL, fsync on every commit, bounded ~32 MB WAL, schema versioned; a database from a newer schema is opened read-only and never modified) holds session records, tabs, an append-only journal of turns, items, tool calls, prompts/answers, queued messages, command receipts and attachment claims. Provider session ids are stored as a handle chain so a replaced provider session keeps lineage. Each chat pins an account home so history and models come from the same account.
- **Lifecycle:** a provider child is started on demand; an idle sweep every 5 minutes stops the child of a chat quiet for 30 minutes that owes no work (the conversation, tab and history stay; the next send starts a new child). Closing a chat tears down its child, route and sink. Stop shows "Stopping" until the stopped turn ends. After an app/host restart, interrupted chats are offered for resume and a cut reply can be continued (opt-in); the continuation asks the agent to verify its last action before repeating it.
- **Child-session environment:** each chat child gets its own session id and the Orca CLI path so the agent can call `orca orchestration delegate` (AG-26); pane identity and hook-endpoint variables are removed so the agent's own status hooks cannot double-report.
- Transcripts, rewinds and recovery after crash use the journal; an unconfirmed send after a crash is resolved by checking the agent's own store where possible (ACP) and otherwise left for the user to resend or discard.
**Depends on:** AG-08, AG-16 to AG-21, AG-23, AG-25; terminals are not used.

**Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`):** the Conference room (AUTO-13) and the model picker composer are chat surfaces, and current bot tasks already run as structured chat sessions (AUTO-14). Proposal: first version (was later). This adds an XL item; see open-questions.md.

### AG-16 Chat capabilities (what a chat can do per agent)
**What the user does:** sends messages, queues messages while a turn runs, stops a turn, answers approvals/questions, rewinds, compacts, sets goals, runs provider commands, watches subagents and background tasks, sees context usage.
**Behaviour and rules:** each agent declares a capability record before any session exists; running sessions can narrow it but never widen it.

| Capability | Claude | Codex | Pi | Antigravity | Grok / OpenCode / OMP (ACP) |
|---|---|---|---|---|---|
| Rewind | no (fork instead) | yes (not for legacy-history threads) | no | no | no |
| Compact (`/compact`) | yes | yes | yes | no | yes (as a prompt) |
| Thread goal | no | yes | no | no | no |
| Provider commands | built-in slash set | usage, skills, plugins, memories, rename, fork, side, review, approve, stop | name, export | CLI command list | `/` commands from agent |
| Image prompts | yes | yes | yes | no | only if the agent advertises images (OpenCode yes) |
| Steering while running | inject | inject | inject | queue | queue |
| Approval enforcement | provider | provider | provider | provider | MassOrca |

- Source: pv96 (unmerged). Gentle Shell and Antigravity chats now also get the visuals folder and instructions, and two capabilities join the chat: a shared checklist (AG-32) and a question card that any chat agent can raise (AG-34).
- Messages sent while a turn runs: `inject` hands them to the running agent; `queue` holds them in MassOrca and sends as the next prompt when the turn ends (queued cards can be sent now, deleted, paused or resumed).
- Questions the agent asks (multiple-choice) and approvals are surfaced as cards; the answer returns to the same agent request. Background tasks and subagents appear as rows with their own state, tokens and result.
- Context usage is reported per turn where the agent provides it.
- Slash commands typed into a chat are classified: those MassOrca implements itself vs those the agent expands from text (Claude, OpenCode, Antigravity expand; Codex has text-driven commands such as `/goal`, `/init`, `/recap`, `/plan`).
- Inline visuals: a chat can be launched with a private writable folder and a skill that teaches the agent a reply line for rich visuals; best effort, a chat without it still starts. Reply suggestions (AG-27) and prototype/design requests use the same folder.
**Depends on:** AG-15.

### AG-17 Claude structured adapter
**Behaviour and rules:** drives the user's installed `claude` through the Claude Agent SDK using the CLI binary found by Orca (the SDK's bundled binaries are excluded). Setting sources user, project and local; partial messages streamed; `--replay-user-messages` kept because send acknowledgement depends on it; the CLI's own `claude_code` system prompt; permission requests answered through a `canUseTool` callback; bypass is passed as the `--dangerously-skip-permissions` flag rather than the SDK's newer option so older binaries still start. Sessions use a stable session id per chat and resume a Claude transcript (`--resume`) when one exists. Model aliases (family names) with live listing; effort levels; fast mode; prompt suggestions only when the user allows reply suggestions (they cost a cached call per turn). Managed accounts: the chat pins `CLAUDE_CONFIG_DIR`; start is refused while an account switch is in progress, when the environment carries conflicting auth variables, or when a WSL-bound managed account is active. Sign-in help command `claude auth login`. Child env also enables todo tools and session-state events so plans and state show in the chat.
**Local only:** not SSH, not WSL.
**Depends on:** AG-15, accounts domain.

- Source: pv96 (unmerged). The chat's instructions include the checklist format (AG-32). A chat whose launch directory is a linked worktree gets the main checkout's design-system folder as an additional directory (AG-36), and a Claude startup frame is no longer shown as an error card.

**Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`):** a first-version chat surface needs at least one structured agent, and this is the reference adapter. Proposal: first version (was later).

### AG-18 Codex structured adapter
**Behaviour and rules:** spawns `codex app-server` (stdio JSONL JSON-RPC) per chat with a managed account home (`CODEX_HOME`); the app-server enforces approval and sandbox policy carried on the thread (AG-10). Account home preparation, session index healing and rollout backfill keep Codex's own history consistent across accounts (accounts/session-history domains). Supports rewind, goals, compaction, forks, side threads, reviews, guardian "auto review" approvals, images, collab/subagent items and background commands as chat rows. Short-lived read-only app-server sessions are used for model catalog and rate-limit probes (plugins disabled so marketplace clones do not outlive the probe). A missing `app-server` subcommand on an old CLI is detected from the error and treated as unsupported. Workspace trust is pre-written for the chat (AG-12). Sign-in help `codex login`.
**Local only:** not SSH, not WSL.

- Source: pv96 (unmerged). The chat's instructions include the checklist format (AG-32), and a chat in a linked worktree gets the main checkout's design-system folder as a writable root in the Codex config (AG-36).

### AG-19 Pi RPC adapter and Gentle Shell
**Behaviour and rules:** runs `pi --mode rpc` (JSONL) with the chat's account directory `PI_CODING_AGENT_DIR` (default `~/.pi/agent`, or `~/.gentle-shell/agent` for Gentle Shell). Gentle Shell is an alternative Pi-protocol runtime with its own binary, home and permission modes (`/gentle:mode`, `/gentle:yolo`); the runtime for a chat is chosen from host record, transcript home, launch seed, host fallback, then the Command setting, and MassOrca falls back to Gentle Shell when only it is installed. Stock Pi supports full-access only (no restricted mode), so a restricted request is refused for stock Pi. Version floor in AG-03. MassOrca appends the child-session delegation instruction through `--append-system-prompt`. Dialogs raised by Pi extensions (select, confirm, input, toggle) are answered through the chat. Slash command catalog is cached on disk. Supports saved sessions, forks via `--fork`, background jobs, worker details and todo views from Gentle Shell.

- Source: pv96 (unmerged). Gentle Shell chats now get the visuals folder and instructions, and the checklist format (AG-32). The launch-mode check judges a restricted-access request against the Pi runtime actually chosen, stock Pi or Gentle Shell.

### AG-20 Antigravity stream-json adapter
**Behaviour and rules:** runs `agy` with stream-json input and output, one prompt serialised at a time at result boundaries, resuming only the pinned conversation via `--conversation` (never the CLI's global continue). Model plus effort are sent as a combined variant id; the picker groups variants. Permission mode maps to agy flags. Child-session instructions are given through an `AGENTS.md` folder passed with `--add-dir` (skipped on CLIs older than 1.4.9 and never allowed to stop a chat from starting). Account binding requires the child to run under the same credential home and authority as the host (file/keychain credential backends differ by OS). Not supported on Windows. Reads artifacts, plan documents and configured connectors for display.

- Source: pv96 (unmerged). A resumed chat stays alive when agy settles its interrupted turn first. Antigravity chats get the visuals folder and the design-system folder as extra `--add-dir` grants (AG-36), and the checklist format (AG-32) rides with the AGENTS.md instructions.

### AG-21 ACP client (Grok, OpenCode, OMP)
**What the user gets:** chats for agents that speak the Agent Client Protocol over stdio, added as data rather than new code per agent.
**Behaviour and rules:**
- Per-agent launch spec: command and args (Grok `agent stdio`, OpenCode `acp`, OMP `acp`), fixed environment, login command, how the account is pinned (`GROK_HOME`, OpenCode data dirs, `PI_CODING_AGENT_DIR`), where the agent installs itself (searched after PATH), version rule (AG-03), image and compaction support, how models are discovered (from `initialize`, from a listing command, or unavailable for OMP), and a "dialect" for protocol extensions (Grok subagents/background tasks, OMP, OpenCode).
- Protocol use: `initialize` with the agent's capabilities; `authenticate` when the agent answers auth-required and an advertised method can be used non-interactively (Grok: `XAI_API_KEY` else cached token; otherwise the person is told the sign-in command); `session/new`, or `session/load`/`session/resume` to reattach a session this chat proved; `session/prompt`, `session/cancel`; `session/set_mode`, `session/set_model`, `session/set_config_option` for model and effort; incoming `session/request_permission` answered by MassOrca by the session's access setting (cancelled answers on shutdown); session updates mapped into the shared timeline (messages, thoughts, tool calls with locations, plans, usage, mode and config changes, background tasks, subagents).
- No client-side file system or terminal capabilities are offered to the agent (the agent works in the workspace itself).
- Hardening: JSON-RPC line size limits (prompts with images are budgeted against the line limit), write queue, stdio error boundary, oversized-line handling, abort/stop proofs, recovery history for unconfirmed prompts.
- Failure words from the agent's error answers are surfaced.
- OpenCode runs with `OPENCODE_ENABLE_QUESTION_TOOL=false`, because ACP cannot answer it.
**Local only:** not SSH, not WSL; on Windows only where a process start time can be read (so a stored record can prove which process it names).
**Depends on:** AG-15, AG-25.

- Source: pv96 (unmerged). ACP chats get the checklist format (AG-32) in their instructions. Checklist screenshots reach ACP agents as host paths in the update text, because ACP refuses image blocks for agents without image prompts. Grok request handling was also changed (not traced).

### AG-22 Chat attachments and image prompts
**What the user does:** attaches files or images to a chat message from desktop or phone.
**Behaviour and rules:** attachments are uploaded in chunks (384 KB chunks, 8 concurrent uploads, 5 minute idle expiry, 50 MB maximum, file name kept intact and truncated to 200 bytes) into a per-upload folder in a host-owned store, committed by rename, and claimed by the message that names them (claims recorded in the journal; unreferenced uploads are swept). Images are previewable. Agents receive attachments as file references or, for ACP agents that advertise images, as image blocks (PNG, JPEG, GIF, WebP; checked and read before sending; an agent that did not advertise images has a message with images refused). An agent without image support (Antigravity) gets no image prompts.
**Persists:** attachment store under the state directory; claims in the journal.

### AG-23 Model catalog service for chats
**What the user does:** sees the list of models and effort levels the signed-in account can use for a chat before starting it.
**Behaviour and rules:** per-agent probes list models without starting a session (Claude `list_models` control request, Codex app-server model list, Pi `--list-models`, Antigravity `models`, ACP per spec). Results are cached and persisted per (agent, account fingerprint) so one account's list never answers for another; listings that mark the configured default are honoured only when the workspace's own config cannot pick another model. Probe failures report `cliMissing` / `notSignedIn` (managed or system account) so the picker can offer the sign-in command. Catalogs refresh when an agent is in use or the account changes.
**Persists:** model catalog store in the state directory.

- Source: pv96 (unmerged). The chat model picker shows one featured set per agent and folds every other model under "More models" (inline on desktop, a drill-in on the phone), keeping each live description. Desktop and phone share the fold.

**Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`):** the model picker lists models per agent, with favourites and search (X-AG-02). Proposal: first version (was later).

### AG-24 Terminal-backed chat view (transcript chat)
**What the user does:** reads a terminal agent session as a chat (messages, tool calls, plan, approvals) while the real TUI keeps running; can answer approvals and send text from the chat view into the PTY.
**Behaviour and rules:** supported for Claude/OpenClaude (Claude transcript format), Codex, Grok, OMP, OpenCode and OpenCode 2 (SQLite), Antigravity. The transcript file is located from the hook-reported path or by id; incremental readers and watchers tail it. Transcript access rules: OMP, Antigravity, Grok and OpenCode transcripts must be read on the owning host (not over the SSH "Model A" transcript reader); WSL transcripts go through a dedicated reader process; selector-style TUIs (Claude, Codex, OpenCode) need key steps rather than pasted option labels; OpenCode approval accepts with Enter, others with `1`.
**Depends on:** hooks (path), session-history domain, terminals.

### AG-25 Provider process supervision
**Behaviour and rules:** every headless agent child (structured chats, one-shot generation, probes) is spawned through a supervisor so an Orca crash or quit does not leave orphaned agent process trees. POSIX: a small supervisor process in its own group closes the child's stdin, waits 1 s, SIGTERM, waits 3 s, SIGKILL, waits up to 1.5 s for reap, and relays remaining output for up to 1 s; it exits if its owner disappears (parent change). Windows: no supervisor wrapper; the process tree is terminated with `taskkill` and the root with a kill fallback, and the result is reported "unverifiable" unless taskkill exits cleanly. A missing executable (ENOENT for the command itself) is distinguished from other start failures so the UI says "not installed". Spawn failures before the provider runs are reported with a marked stderr line (exit 127). Child environments are scrubbed of MassOrca pane/launch identity. Process identity is checked with start times (required on Windows) so stale records never kill an unrelated process.

### AG-26 Delegated (child) sessions
**What the user does:** tells a chat agent "delegate this to another session"; a card appears asking which agent should run it; the child runs in a new or the current workspace and its result returns to the parent chat.
**Behaviour and rules:**
- Source: pv96 (unmerged). Placement follows a stricter read-only rule, the card asks a prototype count and design system, a host-side Jev advice can set the tier, a failed start is rolled back, and a finished child's git work can be brought back (AG-31).
- The agent's only sanctioned route is `orca orchestration delegate --summary <abstract> --prompt <task> --title <name> [--writes] --json`; the chat's system instruction (given per agent: system prompt, `--append-system-prompt`, `AGENTS.md` folder) forbids the agent's own sub-agent tools for this. The request returns at once and the agent ends its turn.
- A request is filed in the orchestration database and a question card is posted to the parent chat with options: Automatic plus every usable agent, each with usage text ("37% used", "No usage data", "near its limit", "Limit reached") and the model/effort it would get.
- Routing: classify the summary by keyword into dimensions (risk, complexity, ...) and a tier 1-5 (high risk raises the floor to 4); rank candidate agents by usage from the subscription rate-limit windows (5-hour, 7-day, monthly), excluding agents near limit (90%) or at limit, with 15-minute freshness, a 5-point tie tolerance, parent-agent preference, then fixed order claude, codex, antigravity, pi; map tier to model and effort (Claude haiku/sonnet/opus ladder, Codex tiers; other agents run their default); clamp to what the catalog accepts. Before ranking, the host refreshes its usage meter and waits up to 8 seconds for it. When Jev advises the tier (AG-33), that advice replaces the classifier's tier, except that a task the classifier marks high-risk keeps at least tier 4.
- Placement: in a git workspace a code-writing task runs in a new child worktree, and so does any task that does not clearly only read (agents differ on when they set writes); only a clearly read-only task stays in the current worktree. In a folder workspace a read-only task stays in the current folder, and writes also run there with a warning. A child worktree takes its name from the chat name unless the person named it, and the child is told that its parent location is the parent worktree.
- Access: the child inherits the parent's access mode, never widened; an agent that cannot be held to a restriction is refused rather than started wider.
- Lifecycle: on answer, the child worker is started once; its start, finish or failure are posted to the parent as notices; a host restart fails half-started requests and finishes ones whose dispatch ended meanwhile. A failed start releases its dispatch, removes only a worktree that the start itself created, closes the task as failed, and still tells the parent the reason.
- Prototype tasks: when a design the person asked for in a child session is recognised from the person's latest message in the parent chat, the card also asks how many prototypes to make (1 to 4) and which design system to use (use it, review it first, propose one first, or skip). The answers are stored with the request, and the child's first prompt follows them. The card's Cancel works without a running turn.
- A child that creates the design system says in its report whether it reused or created it (AG-36).
**Persists:** delegation requests in the orchestration database; journal rows in the parent chat.
**Depends on:** AG-15, usage/subscription domain, orchestration domain.

**Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`):** orchestration is in the first version (open question 7 in the design inventory, answered); coordinator agents launch child sessions through the same engine as AUTO-20, shown as a tree in the parent session (X-DL-01). Usage-based agent ranking can follow. Proposal: first version (was later).

**Owner direction for the new product (2026-11-12, child placement rules from the design session):** (1) a delegated child session gets its own worktree by default; (2) it may use the current worktree only when the child only reads (research, review, explain), when the workspace is a plain folder without git, or when the user picks it by hand on the delegation card; (3) two sessions never write in the same worktree at the same time, so if a child writes in the current worktree the parent waits until it finishes (sequential delegation, not parallel); (4) parallel launches always give each child its own worktree, with no option. Compared with the observed pv96 behaviour: pv96 decides "only reads" from the agent's write flag and offers no manual pick of the current worktree on the card (rule 2 adds it); in a folder workspace pv96 lets a writing child run in place with a warning while the parent keeps going, which rule 3 replaces with sequential delegation. Folder workspaces keep delegation (rule 2). Proposal unchanged: first version.

### AG-27 AI text generation through agent CLIs
**What the user does:** presses "generate" for a commit message, pull-request title/description or branch name; chats get auto-names; the next-message suggestion appears; a workspace/branch is renamed after the agent's first work.
**Behaviour and rules:**
- Source: pv96 (unmerged). Each local generation run is recorded as an internal agent run when it starts, so Session History hides its transcript as an automated run. A remote run's transcript lives on its host and is not recorded here.
- Uses the agent in non-interactive print mode. Agents with a generation spec: Claude (`-p`, stdin, plan permission mode), Codex, OpenCode, OpenCode 2, Pi, Amp, Cursor, Kimi, Muse, DSH, Copilot, Antigravity, Jcode, plus a user-defined custom command. Each spec states binary, argv or stdin delivery (large diffs go through stdin), models (static or discovered), thinking levels, and options that may appear once.
- Same Command override and Arguments as AG-11, with a launch-time check of conflicting model/effort flags; a custom command template is tokenised with the right backslash rule (literal for native Windows).
- Runs locally, in a WSL distro through a login shell, or on an SSH host through the relay (the plan is computed in shared code, the process runs on the host holding the repo). Output is cleaned (reasoning preambles and prefilled text stripped) and parsed; failures carry the agent's own output.
- Reply suggestion picks a cheap model per agent (Claude fast alias, Codex smallest tier) or inherits the chat's model (Pi, Gentle Shell, Antigravity); agents without a print mode produce none.
**Depends on:** AG-11, AG-25, git/source-control domain.

### AG-28 Claude Agent Teams mode
**What the user does:** starts "Claude Agent Teams"; Claude's teammates open as native split panes inside MassOrca.
**Behaviour and rules:** launched as `orca claude-teams [claude args]` from inside a MassOrca terminal (it refuses to run elsewhere); the wrapper enables Claude's teams feature with a tmux-style shim so teammate panes become MassOrca splits. Detection needs the Orca CLI shim and `claude`. Windows and WSL are excluded (Claude's in-process fallback is used). Remote (SSH) launches use the relay-deployed `orca`. Passes all arguments (for example `--resume <id>`) through to Claude. Trust preset `claude`, prompt stdin-after-start.
**Depends on:** CLI domain, terminals, AG-04.

### AG-29 Agent process recognition in panes
**What the user gets:** MassOrca knows which agent runs in a terminal pane (to reuse it, inject text, or show it) even when the agent is a Node script or runs behind a wrapper.
**Behaviour and rules:** the foreground process of a PTY is matched against each agent's expected process name and detect aliases (Kimi's `kimi-code`, ZCode's `zcode-cli`, DSH's `dsh`); Node entrypoint identities and package entrypoints are recognised; Windows uses process tables, console-attached process lists, job membership and cached revalidation; POSIX uses a pane foreground fingerprint; SSH panes use remote evidence from the relay. A pane whose agent cannot be proven is not treated as an agent (dispatch refuses with `no_agent_detected`).
**Depends on:** terminals, agent status domain.

### AG-30 Per-agent startup quirks
Small, agent-specific behaviours that each need an owner when the agent is supported:
- **OpenCode 2:** standalone mode so its private server inherits the pane's hook identity; prompt claimed by an installed TUI plugin through a loopback endpoint with nonce and digest checks.
- **Jcode:** per-pane runtime dir and daemon pre-warm (non-Windows).
- **ZCode:** interactive-capability probe before opening a pane; animated banner defeats quiet-render detection, so the composer corner is the ready signal.
- **Kimi:** home resolution (`KIMI_CODE_HOME` or `~/.kimi-code`), hooks in TOML.
- **Hermes:** YAML config edits that preserve comments and flow style; managed plugin; startup-query prompt contract.
- **DSH:** launches the `dsh-tui` profile with `.`, home patch on install.
- **Qwen Code / Codex:** delayed second Enter.
- **Antigravity:** per-worktree trust entry; slow paste expansion.
- **OMP/Pi/Prime Agent:** CSI-u key encodings, extension-based draft seeding, fresh-session wrapper for OMP.
- **Command Code / Muse / Kiro:** launch flags that skip first-run trust or choose the TUI.
- **Claude Code:** `--prefill` draft; managed-account auth preparation per launch (accounts domain).

### AG-31 Bring back a child session's git work into the parent worktree
**What the user does:** when a delegated child finishes with commits, its notice offers "Bring back", which merges the child's branch into the parent's worktree in place. Under automatic or full access the merge can happen without asking.
**Behaviour and rules:**
- Source: pv96 (unmerged). New feature: a finished child's git work can come back into the parent's worktree, on desktop and phone.
- The notice shows the child's commit and file counts and the parent branch, in place, with no dialog. The host runs `git merge --no-ff` in the parent worktree over local, WSL and SSH routes. Nothing is pushed, nothing is stashed, and neither side is committed automatically.
- States are read from git, so a manual merge also reads as brought back. Each blocked state says why: child still running, child with uncommitted work, parent moved ahead, parent with uncommitted work, parent session working, a conflict predicted by `merge-tree --write-tree` (Git 2.38 or later), worktree removed with its branch kept, or parent without a branch or with an operation running.
- Conflicts show the unresolved count, "Resolve with AI" and "Abort merge"; desktop also offers "Review conflicts".
- The parent's access decides. Ask for approval, Accept edits, Plan, Read only, and any access the host cannot resolve always show the manual notice. Auto merges by itself only when the child finished successfully with commits, the parent has no active turn and no uncommitted changes, and no conflict is predicted; otherwise it asks. Full access never asks: it waits for the parent's turn to end, asks the parent agent to commit uncommitted work first, and leaves conflicts for the parent agent to resolve.
- The host runs automatic merges one at a time per parent worktree, and the parent agent is told what entered its tree. Without `merge-tree --write-tree`, Auto tries the merge and aborts it on conflict. A host too old for the status call shows only the manual notice.
**Depends on:** AG-26, AG-10 (access posture), AG-15, Remote hosts (SSH and WSL routes), Git.

**Owner direction (consistency pass, 2026-11-12):** Proposal unchanged. Covered in the first version by AUTO-32 (bring-back flow) and GIT-30 (merge into the parent branch); count once.

### AG-32 Shared checklist card in agent chats
**What the user does:** sees a Checklist card in a chat's summary and a row in the transcript where the agent wrote its list. Ticks items, adds notes, screenshots and items of their own; the agent reads those marks on its next turn.
**Behaviour and rules:**
- Source: pv96 (unmerged). New feature: a checklist that the agent and the person both update, with its state kept on the host that owns the chat.
- The agent writes its list in a fenced `orca-checklist` block in its reply (`- [ ]` and `- [x]` items, with an optional title line). The format is part of the instructions every structured agent receives, Gentle Shell included, so it does not depend on the visuals setting.
- One checklist per chat, and the latest block's title wins. Items are checked or unchecked, with no failed state. The person can add and remove items.
- Each item shows who changed it last and when; the latest change, by the agent or the person, wins.
- Marks, notes and screenshots live on the host, so desktop, phone and paired or SSH clients read the same record. A host without the checklist methods shows the card read-only.
- Changes the agent has not seen go with the person's next message as one text block, which the transcript shows as a chip. "Send now" sends them alone. A refused send releases its claim, and a retried send carries the same update. Screenshots travel as host paths inside that text, not as image blocks.
- Desktop: the card sits under Plan in the chat summary, and each item offers note and screenshot actions on hover. Phone: an inline row, a Checklist sheet from the chat's menu (shown from session start, with a hint such as "3 of 7" or "No items") and an item sheet.
**Depends on:** AG-15, AG-16, AG-17 to AG-21 (instructions per adapter), Remote hosts (host-kept state).

### AG-33 Jev decision for delegated task tier
**What the user does:** optionally enables Jev in Settings or with `orca jev enable`, stores a Jev API key, and then has Jev pick the tier of each delegated task. Off by default.
**Behaviour and rules:**
- Source: pv96 (unmerged). New feature: an optional third-party decision service that can set a delegated task's tier.
- Jev is an HTTP decision service (host `api.typesafe.ai`, model alias `jev-latest`). Each call asks one multiple-choice question: which of five tiers, from trivial to exceptional, fits the task. The setting and the key belong to the host the chats run on.
- What leaves the machine: a one-line abstract summary of the delegated task, whether it writes code, and fixed tier criteria. A summary is refused when it is longer than 160 characters, contains a secret-like or URL-like word, a run of five or more digits, or characters other than letters, digits and basic punctuation. The task prompt never leaves the host.
- Fallback: when the feature is off, no key is stored, the call fails or times out (8 seconds by default), or the reply is malformed or names no known tier, the classifier's tier stands. There is one call per decision and no retry. Redirects are refused, so the key cannot reach another host.
- The key is stored in `~/.orca/jev-api-key.enc` through the app's secret store. Where sealing is unavailable it is kept as plaintext, and the status says which. The status never shows the key, and a key that cannot be read on this host is reported as unreadable.
- Settings and CLI: status, enable, disable, set key and clear key.
**Depends on:** AG-26 (tier), Settings, Secret storage.

### AG-34 Ask the person a question from a chat agent
**What the user does:** sees a question card in a chat when its agent calls `orca orchestration ask-user`, answers it, and the answer returns to that command.
**Behaviour and rules:**
- Source: pv96 (unmerged). New feature: any MassOrca chat agent can ask the person a question through a card and get the answer in its own command. The command did not exist on the base.
- One question with options (`--question`, `--options`, optional `--multi-select`), or several as JSON (`--questions`), with an optional `--timeout-ms`. It is refused outside a MassOrca chat session.
- The card is a question row that the host writes into the calling chat. It can be answered only where that chat is held, and the answer goes back to the same request.
- A pending question blocks conversation commands until it is answered, as a question the agent raises itself does. Host-owned delegation questions do not block.
**Depends on:** AG-15, AG-16.

### AG-35 Results card: environment URLs and opened files
**What the user does:** sees, in a chat's summary Results card, the addresses where the work can be tested and the files the agent opened for preview.
**Behaviour and rules:**
- Source: pv96 (unmerged). New feature: the chat summary's Results card lists environment URLs and files the agent opened for preview.
- Environment URLs come from the journal: an address in a command's or deploy tool's output, or in the main agent's reply naming it as the place to test.
- Files come from `orca file open --preview`, which the agent runs. The host writes one row per file into the calling chat when the file lies in that chat's workspace. Opening the same file again revises its row, so a page that a shell command builds shows in the Results card.
**Depends on:** AG-15, AG-16, Workspaces.

### AG-36 Design-system folder access for chat agents in linked worktrees
**What the user does:** nothing; a chat started in a linked worktree can still write the project's shared design system, which lives in the main checkout.
**Behaviour and rules:**
- Source: pv96 (unmerged). New feature: a chat in a linked worktree is granted write access to the main checkout's design-system folder.
- When a chat's launch directory is a linked worktree whose design system lives in the main checkout, MassOrca creates the `design/system` folder if it is missing and grants it: as an additional directory for Claude, as a second `--add-dir` for Antigravity, and as a writable root in the Codex config. Gentle Shell and Pi have no sandbox to grant. ACP agents and the other adapters are not granted.
- The grant is made at launch. A running chat keeps its old grants until it is relaunched.
- A delegated child that finds no design system creates one and continues, and its report says whether it reused or created the system.
**Depends on:** AG-15, AG-17, AG-18, AG-20, Prototypes (PROTO-10, PROTO-11).

---

## Accounts and subscriptions per agent

_Copied from [accounts-and-usage.md](accounts-and-usage.md); that file is the maintained source and describes the related features ACC-01 to ACC-26._


Paths: `<userData>` means the Electron userData folder for the app: `%APPDATA%\Orca` on Windows, `~/Library/Application Support/Orca` on macOS, `~/.config/Orca` on Linux (see low-confidence note). `~/.orca` is a fixed folder used by the API-key stores. "Vendor login" means the sign-in is done by the vendor tool, not by MassOrca.

| Agent | How the user logs in | Where credentials live (per OS) | How MassOrca stores multiple accounts and switches | How usage and limits are read |
|---|---|---|---|---|
| Claude Code | OAuth (claude.ai account). MassOrca runs `claude auth login --claudeai` with CLAUDE_CONFIG_DIR set to a fresh temp folder. It can also import an existing config folder (the user's own CLAUDE_CONFIG_DIR or `~/.claude`). API keys are not managed; MassOrca only strips inherited Anthropic auth env vars for a pinned managed account. | Runtime copy Claude reads: `$CLAUDE_CONFIG_DIR/.credentials.json`, or `~/.claude/.credentials.json` when unset (all OS), plus the `oauthAccount` block in `.claude.json` (next to CLAUDE_CONFIG_DIR, else `~/.claude.json`). macOS also has a Keychain generic password, service `Claude Code-credentials` (account = $USER, or `claude-code-user` if $USER has unsafe characters). A config-dir scoped service `Claude Code-credentials-<8 hex>` uses the first 8 hex of sha256 of the NFC config-dir path. MassOrca copies: non-mac `<userData>/claude-accounts/<id>/auth/` with `.credentials.json`, `oauth-account.json` and marker `.orca-managed-claude-auth`. macOS: credentials in Keychain service `Orca Claude Code Managed Credentials` with account = account id; `oauth-account.json` still on disk. WSL: `~/.local/share/orca/claude-accounts/<id>/auth` inside the distro. System-default snapshot: `<userData>/claude-runtime-auth/system-default-auth.json`. | Registry in app settings (`claudeManagedAccounts`, with email, organization UUID, managed path and runtime). Selection per runtime (`activeClaudeManagedAccountIdsByRuntime`; host or a WSL distro). Switching rewrites the runtime copy (file, and on macOS the two Keychain services) with the chosen account's credentials, after reading back the outgoing account's refreshed tokens. No selection restores the system-default snapshot. Per-account folders (`<userData>/claude-profiles/<id>/home` with its own CLAUDE_CONFIG_DIR) are built but disabled (`claudeProfileRoutingEnabled()` returns false). | Active account: OAuth usage endpoint (`api.anthropic.com/api/oauth/usage`) with the access token from Keychain (macOS) or the runtime file. On specific failures: legacy Keychain retry, delegated refresh repair, then CLI fallback that runs `claude` in a hidden terminal, sends `/usage` and parses the panel. Inactive managed accounts use their own stored credentials. Refresh: MassOrca calls the Claude token endpoint (`platform.claude.com/v1/oauth/token`, refresh-token grant). Usage fetch can be deferred while a live Claude session holds the credentials. |
| Codex (OpenAI) | OAuth ChatGPT account. MassOrca runs `codex login` with CODEX_HOME set to a new per-account home, and waits for `auth.json` to appear. It can also import `auth.json` from an existing home. OpenAI API-key-only setups have no `auth.json` identity and are not managed. | `auth.json` inside CODEX_HOME (all OS). System default: `~/.codex/auth.json` (on Windows `%USERPROFILE%\.codex\auth.json`). Managed host account: `<userData>/codex-accounts/<id>/home/auth.json`. Shared runtime mirror: `<userData>/codex-runtime-home/home/auth.json`. Snapshot of the user's system default: `<userData>/codex-runtime-home/system-default-auth.json` (plain JSON, POSIX mode 0600). WSL managed homes: `~/.local/share/orca/codex-accounts/<id>/home` inside the distro. No Keychain or credential-manager access is in app code (Codex CLI behaviour unverified). | Registry in app settings (`codexManagedAccounts`). Selection per runtime (`activeCodexManagedAccountIdsByRuntime`; host or per WSL distro). A selected managed host account launches with its own CODEX_HOME (no file copy at switch time). Otherwise the shared runtime home is synced with `~/.codex` and its `auth.json` is replaced with the selected account's copy. Deselecting restores the system default from the snapshot, unless the user logged out (logout marker). Config (`config.toml`) and shared resources (skills, hooks, plugins, prompts, `AGENTS.md`) are mirrored or linked into managed homes. | Primary: short-lived read-only `codex app-server` probe over JSON-RPC (`account/rateLimits/read`), serialized per home by a process lock. Fallback: HTTP `chatgpt.com/backend-api/wham/usage` with tokens from `auth.json`. WSL homes try HTTP first. Reset credits: `wham/rate-limit-reset-credits` (read) and `.../consume` (write). |
| Cursor | Vendor login in Cursor IDE or `cursor-agent`. MassOrca only detects the sign-in. | macOS: Keychain generic password service `cursor-access-token`, account `cursor-user`; CLI file `~/.cursor/auth.json`; CLI identity `cli-config.json` (no token). Windows: `%APPDATA%\Cursor\auth.json` (CLI path uses the same folder as the IDE; looks unusual, unverified). Linux: `~/.config/cursor/auth.json` (CLI) and `~/.config/Cursor` (IDE). IDE session: `User/globalStorage/state.vscdb` in the IDE config folder (cursorAuth keys). | None. One detected identity, read-only (status shows email, plan, token freshness). | Usage read from the cursor.com dashboard origin with the session token (endpoints not traced). Plan pools appear as named buckets. |
| Grok | Vendor login with the Grok CLI (`auth.json`). MassOrca only detects it. | `$GROK_HOME/auth.json`, or `~/.grok/auth.json` (all OS). | None. One detected identity, read-only; token freshness checked. | Usage via `cli-chat-proxy.grok.com/v1` with the access token (fetcher details not traced). Weekly and monthly windows exist in the shared type. |
| Gemini CLI | Vendor login with the Gemini CLI (Google OAuth). MassOrca reads the CLI's files and can refresh. | `~/.gemini/oauth_creds.json` (all OS). Also reads the Google entry from OpenCode's `auth.json` (`$XDG_DATA_HOME/opencode/auth.json`, `%APPDATA%\opencode\auth.json`, or `~/Library/Application Support/opencode/auth.json`). | None. Single account. | Quota from `cloudcode-pa.googleapis.com` (`retrieveUserQuota`, after `loadCodeAssist`). Refresh via Google's token endpoint with client id and secret taken from the installed Gemini CLI package (not bundled in MassOrca). |
| Kimi Code | Vendor login with the Kimi Code CLI. | `<kimi home>/credentials/kimi-code.json` (home default not traced; typically `~/.kimi`). | None. Single account. | `GET /usages` on `api.kimi.com/coding/v1` (base URL overridable by `KIMI_CODE_BASE_URL`). |
| OpenCode Go (subscription API key) | API key entered in MassOrca, or `OPENCODE_API_KEY` in the environment, or the OpenCode credential store. | Encrypted file `~/.orca/opencode-go-api-key.enc` (Electron safeStorage ciphertext; if encryption is unavailable, a plaintext envelope with a warning). Vendor store: `$XDG_DATA_HOME/opencode/auth.json` (default `~/.local/share/opencode`). | Single key in the app. Managed OpenCode accounts are in ACC-14. | `opencode.ai/zen/go/v1/usage` and the `opencode.ai/_server` function (details not traced). Monthly window. |
| OpenCode (managed login, token usage) | Vendor OpenCode login run in an isolated data home by MassOrca. | Per account: `<userData>/managed-data-accounts/opencode/<id>/data/opencode/opencode.db` (credential table only is kept). Vendor default: `$XDG_DATA_HOME/opencode/opencode.db`. | Registry `<userData>/managed-data-accounts/opencode/accounts.json`. At launch, XDG data and state homes are pointed at the selected account; `OPENCODE_DB` is set and inline auth content cleared. | Token usage from local `opencode.db` (ACC-19). No limits fetch found for managed accounts. |
| Devin (Windsurf login) | Vendor login run in an isolated data home by MassOrca. | Per account: `<userData>/managed-data-accounts/devin/<id>/data/devin/credentials.toml` (must contain a `windsurf_api_key` value). Vendor default: `$XDG_DATA_HOME/devin/credentials.toml`. | Same managed-data registry pattern with `devin/accounts.json`; XDG env applied at launch. | No limits fetch found. A token-usage provider id `devin` exists; its source path was not traced. |
| MiniMax | API key entered in MassOrca, plus an optional console session cookie. | API key: `~/.orca/minimax-api-key.enc` (safeStorage, same pattern as OpenCode Go). Cookie: a file in `~/.orca/` (name in the cookie store). | Single account in the app. | Fetched from MiniMax platform endpoints (`platform.minimax.io` or `minimaxi.com`) with the key or cookie (endpoints not traced). |
| NaN, ZCode | API key used by the usage fetcher. | Not traced. ZCode reads a JSON config file at a path passed in. | Single account (assumed). | NaN: `cloud-api.nan.builders/api/usage/quota` with a bearer key. ZCode: fetcher not traced. |
| Antigravity | Vendor login (not traced). | Not traced. Credential files are written atomically via the integration credential helper. | Native account service exists (`src/main/antigravity/`), not read. | Usage via an Antigravity CLI command and response parser (not traced). |
| Muse | No login or credentials in scope. | None. Token usage comes from local logs at `$XDG_DATA_HOME/muse/sessions/YYYY/MM/DD/<id>/session.jsonl` (plus `subagents/`). | Not applicable. | No limits fetch found. Local token usage only. |

Cross-agent rules that matter for the migration requirement:

- Claude refresh tokens are single use. Code comments say the app owns the refresh so the rotated token is persisted atomically, because a CLI that rotates the token after MassOrca copied it leaves stale copies. Two holders of one login can invalidate each other. After migration each login needs exactly one owner.
- Codex rotates `auth.json` in place. Several copies of one login (system default, shared runtime mirror, managed home) can diverge. The app's identity checks are the only guard.
- Encrypted API-key files (`.enc`) use Electron safeStorage. Decrypting them depends on the same OS user and the safeStorage key. Whether the new core can read them has not been checked.
- macOS Keychain items are named by app and by config-dir hash (Claude). Reading them can trigger OS prompts (3 s timeout on the `security` command).
- WSL-side account folders live inside the distro, not on the Windows host.


## Session history on disk per agent

_Copied from [session-history.md](session-history.md); that file is the maintained source and describes the related features HIST-01 to HIST-18._


Notes for reading the table:
- `%USERPROFILE%` is the Windows home (Node `os.homedir()`). `~` is the home on macOS and Linux.
- "Read" describes how the scanner finds files. The scanner also reads the same layouts under each running WSL distro's home on Windows (`\\wsl$\<distro>\home\<user>\...`, Windows only, running distros only).
- Environment overrides, where the code shows them: `CODEX_HOME`, `CLAUDE_CONFIG_DIR`, `COPILOT_HOME`, `GROK_HOME`, `KIMI_CODE_HOME`, `JCODE_HOME`, `DEVIN_HOME`, `CLINE_SESSION_DATA_DIR`, `PI_CODING_AGENT_DIR`, `OPENCLAW_STATE_DIR`, `XDG_DATA_HOME`, `OPENCODE_DB`. Oh My Pi and Prime Agent have their own overrides (see their roots helpers). There is no in-app setting for these roots in the IPC layer that was read.
- The "Resume" column is the command MassOrca builds. It is the vendor CLI doing the resume, against the file where it already lives. See the low-confidence list.

| Agent | Format on disk | Windows | macOS / Linux | How MassOrca reads it | Can MassOrca resume a session found there |
|---|---|---|---|---|---|
| Claude Code | JSONL, one file per session | `%USERPROFILE%\.claude\projects\<cwd>\<sessionId>.jsonl`. `<cwd>` is the working directory with every non-alphanumeric character replaced by `-` | `~/.claude/projects/<cwd>/<sessionId>.jsonl` | Scanner walks the root; `subagents/` directories are skipped (read on demand under the parent). Native chat reader parses records. Managed accounts add their own `projects` dirs | Yes: `claude --resume <sessionId>` in the session's cwd. Also adoptable into a structured chat (HIST-08) |
| CodeBuddy | JSONL | `%USERPROFILE%\.codebuddy\projects\...` | `~/.codebuddy/projects/...` | Scanner walks the root; subagent dirs skipped; message roles and `input_text`/`output_text` blocks parsed | Yes: `codebuddy --resume <id>` (constructed) |
| Qoder | JSONL | `%USERPROFILE%\.qoder\projects\...` | `~/.qoder/projects/...` | Scanner walks the root; subagent dirs skipped | Yes: `qodercli --resume <id>` (constructed; the binary name is `qodercli`) |
| Codex | JSONL rollouts: `sessions\YYYY\MM\DD\rollout-<timestamp>-<uuid>.jsonl`; thread names in `session_index.jsonl` at the Codex home | `%USERPROFILE%\.codex\sessions\...` and `%USERPROFILE%\.codex\session_index.jsonl` | `~/.codex/sessions/...` | Scanner walks the root; hardlinked aliases across roots are deduplicated; titles come from `session_index.jsonl`. Forks hold only post-fork records and inherit the parent prefix | Yes: `codex resume <id>`. A `CODEX_HOME` prefix is added for MassOrca-managed account homes |
| Gemini CLI | JSON (legacy single document with a `messages` array) and JSONL logs | `%USERPROFILE%\.gemini\tmp\...` (recursive) | `~/.gemini/tmp/...` | Parsed per file type. Only the JSONL log format is marked resumable; legacy JSON is rewritten in place and read whole | Yes for JSONL: `gemini --resume <id>` (constructed). Legacy JSON: not resumable per the parser comment |
| GitHub Copilot CLI | JSONL (`<id>.jsonl` in the fixture) | `%USERPROFILE%\.copilot\session-state\` (`COPILOT_HOME`) | `~/.copilot/session-state/` | Scanner walks the root for `.jsonl`. Real nesting unverified (low confidence) | Yes: `copilot --resume=<id>` (constructed) |
| Cursor (agent CLI) | JSONL transcripts under `agent-transcripts\`; JSON metadata (`meta.json`) under `chats\<md5 of cwd>\<uuid>\` | `%USERPROFILE%\.cursor\projects\<slug>\agent-transcripts\**\*.jsonl` and `%USERPROFILE%\.cursor\chats\...\meta.json` | `~/.cursor/projects/...` and `~/.cursor/chats/...` | Transcripts are listed; cwd, title and times come from `meta.json`, joined by chat id through an index of the chats directories (the md5 hashes the cwd, so the join needs the index). The Cursor IDE's `state.vscdb` is separate (see HIST-13) | Yes: `cursor-agent --resume <id>` (constructed) |
| Grok | JSON `summary.json` per session; JSONL `chat_history.jsonl` beside it | `%USERPROFILE%\.grok\sessions\<encodeURIComponent(cwd)>\<id>\summary.json` (`GROK_HOME`). Long cwd names use a slug-plus-hash directory | `~/.grok/sessions/...` | `summary.json` is the discovered file; native chat reads `chat_history.jsonl`. Native chat requires a local transcript, so it is not available over SSH | Yes: `grok --resume <id>` (constructed) |
| Devin | ATIF JSON per session in `transcripts\`; SQLite `sessions.db` beside the transcripts' parent; `agent_logs\` as an extra root | `%APPDATA%\devin\cli\transcripts\*.json` and `%APPDATA%\devin\cli\sessions.db` (`DEVIN_HOME`) | `$XDG_DATA_HOME/devin/cli/transcripts` or `~/.local/share/devin/cli/transcripts` | Transcript JSON is discovered; `sessions.db` is read read-only for index fields, and a change to it re-merges sessions without re-reading transcripts. Path of `sessions.db` is derived (low confidence) | Yes: `devin --resume <id>` (constructed) |
| Hermes | JSON, one file per session named `session_*.json` | `%USERPROFILE%\.hermes\sessions\` | `~/.hermes/sessions/` | Files filtered by the `session_` prefix | Yes: `hermes --resume <id>` (constructed; base is `hermes`) |
| Jcode | JSON, `session_*.json`; `.journal.jsonl` appends are ignored | `%USERPROFILE%\.jcode\sessions\` (`JCODE_HOME`) | `~/.jcode/sessions/` | Files filtered by prefix; internal display roles are not counted as messages | Yes: `jcode --resume <id>` (constructed) |
| Rovo Dev | JSON: `metadata.json` plus `session_context.json` per `<id>\` directory | `%USERPROFILE%\.rovodev\sessions\<id>\` | `~/.rovodev/sessions/<id>/` | `metadata.json` is the discovered file; `session_context.json` gives the opening request | Yes: `acli rovodev run --restore <id>` (constructed; base is the full launch command) |
| Pi | JSONL message graph (session, model_change and message records) | `%USERPROFILE%\.pi\agent\sessions\` (`PI_CODING_AGENT_DIR`). Gentle Shell: `%USERPROFILE%\.gentle-shell\agent\sessions\` | `~/.pi/agent/sessions/`; `~/.gentle-shell/agent/sessions/` | Graph JSONL parser; title read for Pi (title agents list includes Pi) | Yes: `pi --session <id>`. Gentle Shell sessions resume through `gentle-shell -- --session <file>` with the file path |
| Oh My Pi (OMP) | JSONL message graph; Task subagent transcripts inside `<stamp>_<uuid>\` artifact directories | `%USERPROFILE%\.omp\agent\sessions\` (overridable; see OMP roots helper) | `~/.omp/agent/sessions/` | Graph parser; artifact directories pruned from top-level listing. Subagents listed on demand | Yes: `omp --resume <absolute transcript path>` (resumes by file path, not id) |
| Prime Agent | JSONL message graph (Pi fork) | `%USERPROFILE%\.prime\agent\sessions\` (own override variable) | `~/.prime/agent/sessions/` | Same graph parser as Pi and OMP | Yes: `prime-agent --resume <absolute transcript path>` |
| OpenClaw | JSONL under `agents\<agent>\sessions\` | `%USERPROFILE%\.openclaw\agents\...` (legacy `%USERPROFILE%\.clawdbot`); `OPENCLAW_STATE_DIR` | `~/.openclaw/agents/...` (legacy `~/.clawdbot`) | Files must sit under a `sessions` path segment; current and legacy state dirs merge | Yes: `openclaw --resume <id>` (constructed) |
| Droid (Factory) | JSONL | `%USERPROFILE%\.factory\sessions\` and `%USERPROFILE%\.factory\projects\` | `~/.factory/sessions/` and `~/.factory/projects/` | Scanner walks both roots | Yes: `droid --resume <id>` (constructed) |
| Cline | JSON manifest per session (`<id>.json`) plus `<id>.messages.json` | `%USERPROFILE%\.cline\data\sessions\<id>\` (`CLINE_SESSION_DATA_DIR`) | `~/.cline/data/sessions/<id>/` | Only the manifest directly under each session directory is a session; the messages file is a content dependency | Yes: `cline --id <id>` (constructed) |
| Kimi Code | JSON `state.json` per session plus JSONL `agents\main\wire.jsonl`; `session_index.jsonl` at the Kimi home maps the work directory | `%USERPROFILE%\.kimi-code\sessions\wd_<name>_<hash>\session_<uuid>\` (`KIMI_CODE_HOME`) | `~/.kimi-code/sessions/...` | Only `state.json` under `session_*` directories is a session. The index supplies the real work directory | Yes: `kimi --session <session_id>`, run from the work directory (the CLI rejects other directories) |
| Muse | JSONL, one `session.jsonl` per session | `%USERPROFILE%\.local\share\muse\sessions\YYYY\MM\DD\<uuid>\` (per the code default; low confidence on Windows) | `~/.local/share/muse/sessions/YYYY/MM/DD/<uuid>/` (`XDG_DATA_HOME`) | Only `session.jsonl` counts; sibling `.log`, `.sqlite3` and `.msp-view` files are ignored | Yes: `muse resume <uuid>` in the session's cwd |
| Antigravity | JSONL `transcript.jsonl` at `brain\<conversationId>\.system_generated\logs\`; `history.jsonl` beside `brain` | `%USERPROFILE%\.gemini\antigravity-cli\brain\...` (CLI). IDE origins, opt-in: `%USERPROFILE%\.gemini\antigravity-ide\brain\...` and `%USERPROFILE%\.gemini\antigravity\brain\...` | `~/.gemini/antigravity-cli/brain/...` (and IDE origins) | Transcripts are paired with the `history.jsonl` beside `brain` (pairing rule not read); the opening prompt is the join key for IDE history. Title read for Antigravity | CLI origin: `agy --conversation <id>`. IDE origin: not the same conversation; opened with a reference prompt (inferred). Adoptable into a structured chat |
| OpenCode | SQLite `opencode.db`; legacy JSON under `storage\` (`session\<project>\ses_*.json`, `message\<session>\msg_*.json`) | `%USERPROFILE%\.local\share\opencode\opencode.db` (no `%APPDATA%` branch in code) unless `XDG_DATA_HOME` is set. `OPENCODE_DB` overrides the file | `~/.local/share/opencode/opencode.db` (`XDG_DATA_HOME`) | Read-only SQLite on a worker process; legacy JSON read from `storage\`. A synthetic `<db>#<sessionId>` path identifies each SQLite session | Yes: `opencode --session <id>` (constructed) |
| OpenCode 2 | Same `opencode.db`, v2 tables (`session_v2`, `session_message`). Beta files `opencode-next.db` and `opencode-local.db` are excluded | As OpenCode | As OpenCode | Worker checks for v2 tables in the shared file; schema called unstable in the code | Yes: `opencode2 --standalone --session <id>` (constructed; base is the launch command) |
| ZCode | SQLite `db.sqlite` (OpenCode-shaped schema, per code comment) | `%USERPROFILE%\.zcode\cli\db\db.sqlite` | `~/.zcode/cli/db/db.sqlite` | Read-only SQLite on a worker; kept separate from OpenCode so one cannot absorb the other's rows | Yes: `zcode --resume <id>` (constructed) |

Summary for the import requirement:
- Nothing converts a foreign transcript into MassOrca's own store, except structured adoption (HIST-08), which imports a Claude, Codex or Antigravity transcript into a MassOrca journal.
- Terminal resume re-launches the vendor CLI against its own file. It works only where that CLI is installed on the host that holds the file, and it does not move the conversation.
- Every reader opens foreign stores read-only. SQLite files are opened read-only, and the one write (deletion, HIST-09) is limited to 14 agents.
- Discovery covers 24 agent identifiers. The search compatibility list frozen to an older parser covers 20 of them; Codebuddy, Qoder, ZCode and Jcode are absent from it, so search sends those four only to hosts that negotiate the newer list.

