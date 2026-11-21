# Accounts, credentials, subscriptions, usage and rate limits (ACC)

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits), read with read-only git from worktree `unified-custom-profile-build-547ea6`. The earlier pass read worktree `massorca-server-language-820c84`.

This domain covers how MassOrca signs the user into AI agent subscriptions, stores several accounts per agent, switches the active one, keeps tokens alive, and reads usage and rate-limit windows. It also covers local token-usage analytics, credential-file protection, and the code that preserves sign-ins across restarts. The current app keeps most agent credentials in the vendor's own locations (for example `~/.claude` and `~/.codex`) and only copies them into app-owned folders for managed accounts. Two agents (Claude Code and Codex) have real multi-account management with switching. Cursor, Grok, Gemini and Kimi are read-only detections of a sign-in done in the vendor tool. OpenCode Go and MiniMax use API keys entered in the app. Automatic account rotation on limits does not exist in the current code.

Product name note: the code uses the identifier "Orca" throughout (for example the Keychain service `Orca Claude Code Managed Credentials`). This document keeps those identifiers as they appear in code. The pv96 rebrand (PLAT-29) left these identifiers unchanged.

## Source directories read

All paths are relative to the reference repository `src/` root unless stated (reference repo: `D:\projects\massorca\.claude\worktrees\massorca-server-language-820c84`, read-only).

- `src/main/claude-accounts/`: service, registration, selection, managed auth storage, managed auth path, keychain, profile paths, profile router (dormant), runtime paths, environment, login session, command process, auth capture, OAuth refresh, live PTY gate, `runtime-auth/` (file storage, sync, runtime state, keychain snapshots, readback, preparation, types).
- `src/main/codex-accounts/`: runtime home paths, launch, auth sync, selection, registration (add and import from home), login session, managed home lifecycle, config mirror (grep), identity (grep), reset-credit coordinator (head), legacy shared auth migration (grep), restart-related files (names only).
- `src/main/codex/codex-home-paths.ts`
- `src/main/cursor-accounts/status.ts`, `src/main/grok-accounts/status.ts`
- `src/main/managed-data-accounts/` (service, credential capture, launch environment)
- `src/main/credentials/encrypted-api-key-file-store.ts`, `src/main/credential-file-protection.ts`, `src/main/agent-auth-restart-preservation.ts`
- `src/main/macos-keychain/generic-password.ts`
- `src/main/rate-limits/` (Claude fetch, OAuth usage request, CLI PTY fallback, credentials, Codex RPC probe and backend client, reset-credit client, Cursor, Grok, Gemini, Kimi, MiniMax/OpenCode Go/NaN/ZCode/Antigravity fetchers by grep, `service/` constants and orchestration names, auth filesystem operation)
- `src/main/usage/` (provider contract, store lifecycle by grep), `src/main/usage-cache-snapshot-writer.ts`, `src/main/usage-worktree-metadata.ts`, `src/main/usage-worktree-canonicalizer.ts`
- `src/main/claude-usage/` (transcript discovery, store by grep), `src/main/codex-usage/` (session discovery, store by grep), `src/main/opencode-usage/` (database discovery, data directory), `src/main/muse-usage/` (session discovery)
- `src/main/stats/` (types, collector head)
- `src/main/ipc/rate-limits.ts` (channel names), `src/main/delegated-session/delegation-options.ts` and `delegation-usage-input.ts` (adjacent: agent routing by headroom)
- `src/preload/usage-provider-api.ts`
- `src/shared/` read in part: `secure-file.ts`, `claude-profile-routing.ts`, `rate-limit-types.ts`, `managed-account-types.ts` (names), `global-settings-types.ts` (names), `muse-session-log.ts`, `default-global-settings.ts` (names)
- `src/renderer/src/components/status-bar/usage-roster-formatting.ts` (thresholds only), other renderer files by name only (UI detail belongs to another session)

pv96 delta (`git diff custom/integration d3c6311a531`): `src/shared/secure-file.ts`, `secure-path-windows-acl.ts`, `secure-path-metadata-snapshot.ts` (new), `secure-path-hardening-report.ts`, `windows-user-sid.ts` (new), read for comments and the write-path logic; `src/main/observability/index.ts` (hardening span fields); `src/main/startup/main-process-observers.ts` (the desktop call that enables non-blocking writes); `src/main/marketplace/marketplace-vault-master-key.ts` (key file mode and location, for SET-04); `src/shared/default-global-settings.ts` and `global-settings-types.ts` (diffs); `src/main/codex/` (five changed files and one new file, by name); `src/cli/handlers/account.ts`, `account-usage.ts`, `account-login-terminal.ts` and `src/cli/specs/account*.ts`. No file under `src/main/rate-limits/`, `usage/`, `claude-accounts/`, `codex-accounts/` or the usage-scanning folders changed at pv96 (checked by diff stat).

## Not read / low confidence

Not read or read only by name:

- Most of `rate-limits/service/` orchestration, `rate-limits/` Antigravity, ZCode, NaN and MiniMax fetchers, and the Gemini, Cursor and Kimi fetch internals beyond the paths above. Endpoints for Cursor and MiniMax were not traced.
- `src/main/antigravity/` account service and credential backend (only the names). Antigravity account storage is unknown here.
- Where the NaN and ZCode API keys come from (the fetchers take a key as input; the caller was not traced).
- The Kimi home resolver (the fetcher receives the home as an argument; the default was not found by grep).
- Codex managed-home lifecycle, identity claim keys, config mirror details and WSL drain scripts (read by name and comments only).
- Claude WSL profile router, profile setup worker, profile history sharing, and runtime-auth readback and restore internals.
- The usage scan worker, token-usage store internals, Devin usage source path, Muse record parser, and stats collector internals.
- Renderer UI (status bar, switcher menus, usage page) beyond the thresholds and file names.
- Electron `safeStorage` key storage location per OS (not in this code).
- Whether the Codex CLI itself uses an OS keychain (the app code only reads `auth.json`).
- Tests (skipped by brief), except test names when they clarified a rule.
- pv96: `src/main/codex/codex-bot-run-mcp-overrides.ts` (new, bot-run MCP overrides) was not read.
- pv96: the rail usage rings and the status bar's usage meters are renderer code; they are described from `odd/tasks/desktop-simplification.md` (spec and records), not from code.
- pv96: the Windows ACL helpers were read for their comments and write-path logic only; the Windows behaviour was not run or tested here.

Low confidence:

- Exact Windows, macOS and Linux `userData` folder name. Code comments say `app.setName('Orca')` runs before userData is captured, which implies `Orca`. A helper for non-Electron CLI contexts uses lowercase `orca`. The two only differ on a case-sensitive file system (Linux).
- Windows protection of plain-JSON snapshot files (see ACC-21).
- Cursor CLI config path on Windows: the code builds the CLI path from the same `Cursor` folder as the IDE state, which looks unusual, so it may be a quirk rather than intent.

## Accounts and subscriptions per agent

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

## Features

### ACC-01 Claude Code sign-in and import

- What the user does: adds a Claude account by signing in (OAuth in the browser through the Claude CLI), or imports an existing Claude config folder.
- What happens: a new account id and managed folder are created. The app runs the Claude CLI login with CLAUDE_CONFIG_DIR pointing at a new temp folder (created under the OS temp directory with the prefix `orca-claude-login-`). It then reads `claude auth status --json` for identity, reads the credentials (macOS: scoped Keychain item, then the legacy item when its digest changed; other OS: the file in the temp folder), and reads `oauthAccount` from `.claude.json` or `.config.json`. The credentials are stored in the managed folder (ACC-03 shows the record) and the account is registered.
- Edge cases: an unfinished or cancelled login removes its temp folder. A newer add, reauth or select request supersedes a pending login. Windows uses a dedicated interactive login spawn. WSL adds the account inside the distro. Import from a config folder captures credentials from that folder without running the login.
- Client asks: add account (host or WSL target), add from config folder, reauth, remove, select. Core returns the account list state.
- Persisted: managed auth files (table), account record in settings, temp folder (deleted after the attempt).
- Depends on: ACC-03 (registry and duplicates), ACC-21 (file protection), ACC-07 (token refresh). Needs the Claude CLI installed on the execution host.

### ACC-02 Codex sign-in and import

- What the user does: adds a Codex (ChatGPT) account by running the Codex login in a browser, or imports an existing Codex home.
- What happens: a new managed home is created. `codex login` runs with CODEX_HOME set to it. The app polls until `auth.json` appears, then ends the login CLI (on Windows the CLI can linger after writing the file). Identity is read from `auth.json`. Import copies `auth.json` from the source home into the new managed home and mirrors config into it.
- Rules: a `config.toml` that pins a custom `model_provider` blocks OAuth account add. The app shows a message and leaves the config unchanged. Reauth starts from the existing `auth.json` and only accepts new content. Quota is probed right after add, which can take 10 to 25 s on a cold home.
- Persisted: managed home (`<userData>/codex-accounts/<id>/home`, or the WSL path), settings record.
- Depends on: ACC-03, ACC-05, ACC-21.

### ACC-03 Account registry, identity and duplicate detection

- What the user sees: a list of accounts per agent with email and plan, grouped by runtime (host or WSL distro), with one selected per runtime.
- Rules: Claude duplicates are matched on normalised email plus organization UUID within the same runtime scope (WSL distros are separate buckets). Records created before the runtime fields existed count as host. Codex identity comes from `auth.json` claims; the exact claim keys were not traced.
- Persisted: `claudeManagedAccounts`, `codexManagedAccounts`, `activeClaudeManagedAccountIdsByRuntime`, `activeCodexManagedAccountIdsByRuntime` in the app settings store (legacy file name `orca-data.json`; newer profile-state storage also exists, not traced).
- Client asks: list accounts, select per target. Core pushes refreshed account state after each change.
- Depends on: none.

### ACC-04 Claude Code account selection and switching

- What the user does: picks an account for the host (or a WSL distro). New Claude sessions then use that account.
- What happens: the active credentials are the shared runtime copy (`.credentials.json` in CLAUDE_CONFIG_DIR or `~/.claude`; on macOS the two Keychain services, plus the `oauthAccount` block in `.claude.json`). On switch the app first reads back the outgoing account's runtime copy. If it still belongs to that account and identity matches, the refreshed tokens are saved to the outgoing managed copy. Unverified read-backs are skipped with a warning. Then the incoming account's credentials are written to the runtime copy.
- Deselect: when no account is selected the app restores the system-default snapshot (the credentials and Keychain values captured before the first managed switch). The snapshot file holds the user's previous credentials JSON.
- WSL: the active WSL account must still be owned (marker present) and have a valid credential object. Otherwise selection reverts to the system default with a warning.
- Launch environment: the app sets CLAUDE_CONFIG_DIR only when the user already has one (and marks it with ORCA_CLAUDE_INJECTED_CONFIG_DIR). For a pinned host account it removes inherited ANTHROPIC_API_KEY, ANTHROPIC_AUTH_TOKEN, CLAUDE_CODE_OAUTH_TOKEN and AWS_BEARER_TOKEN_BEDROCK, and auth-like ANTHROPIC_CUSTOM_HEADERS. A launch that defines any of these with a non-empty value is refused with a fixed message. Empty values are ignored.
- Concurrency: a switch in progress blocks other launches for up to 15 s (settle timeout), then refuses.
- Persisted: selection in settings, runtime copy, system-default snapshot, managed copies.
- Depends on: ACC-03, ACC-07, ACC-08, ACC-21.

### ACC-05 Codex account selection and switching

- What the user does: picks a Codex account per runtime. New Codex panes use it.
- What happens (host, managed account): the selected account runs with its own CODEX_HOME. Switching changes no files; new panes get that home.
- What happens (system default): the shared runtime home (`<userData>/codex-runtime-home/home`) is synced with `~/.codex` (resources linked, config mirrored) and its `auth.json` is replaced with the selected account's copy. A provenance record says whether the shared auth is owned by the app or by the system default. Deselecting restores the system default `auth.json`. If the user logged out of the system default, a logout marker stops it from coming back.
- What happens (WSL): selection is per distro. WSL homes live inside the distro. Legacy shared WSL auth is drained into the matching account home by a script that verifies the copy.
- Legacy migration: an old shared `auth.json` is matched to a managed account by identity. With no unique match, neither side claims it.
- Renderer (not traced): Codex panes are marked stale after a switch and can be restarted.
- Persisted: selection in settings, runtime home files, `system-default-auth.json`, `shared-runtime-auth-provenance.json`, `system-default-runtime-logout.json`, `migration-v1.json`, `migration-diagnostics.jsonl`, all under `<userData>/codex-runtime-home/`.
- Depends on: ACC-02, ACC-03, ACC-21.

### ACC-06 Re-authentication, removal and rollback

- What the user does: reauth an account whose sign-in expired, or remove an account.
- Reauth: the same account record gets a new login. Claude keeps a snapshot of the old credentials and restores it if the new login fails. Codex accepts only new `auth.json` content.
- Removal: Claude removes the managed folder (only after an ownership check; untrusted paths are refused with a log entry) and the macOS Keychain item. Codex removes its managed home and cleans up the reset-credit ledger (a failed cleanup only warns). Removing the active account falls back to the system default.
- Failure: a failed add removes its new folder and restores settings.
- Depends on: ACC-01, ACC-02, ACC-21.

### ACC-07 Claude token refresh and refresh ownership

- What the user does: nothing; tokens stay valid while the account is used.
- What happens: for managed Claude accounts the app refreshes the access token ahead of expiry, using the refresh-token grant against the public Claude token endpoint and the public Claude Code client id. The rotated refresh token is written atomically into the managed copy. Refresh is refused while live Claude terminals hold the credentials (ACC-08). Throttling (HTTP 429) and failure statuses are surfaced without the token.
- Codex: MassOrca does not refresh. It reads `auth.json`, which Codex rotates in place.
- Rules: after migration each refresh token needs one owner (see the cross-agent rules above).
- Depends on: ACC-04, ACC-08.

### ACC-08 Live-session protection during account changes

- What the user sees: switching Claude accounts while Claude terminals are running does not break those terminals. New terminals start on the new account.
- Rules: live Claude terminal ids are tracked and persisted across restart. Ids restored from storage stay unconfirmed until the daemon confirms them, and an unconfirmed id keeps the refresh gate closed so a refresh cannot rotate a token out from under a surviving CLI. Verified refreshed tokens of the outgoing account are kept. Usage fetches for the active account can be deferred while a live session holds the credentials (the result is marked deferred).
- Depends on: ACC-04, ACC-07, ACC-09.

### ACC-09 Claude usage and limits

- Source: pv96 (unmerged). Claude usage is shown as rings on the activity rail (one ring for weekly-only use, two rings for weekly and five-hour use) instead of bars in the status bar.
- What the user sees: the 5-hour session window, the 7-day weekly window, a model-specific weekly window (named "Fable" in code), extra usage (spend and balance), and per-account usage for inactive managed accounts.
- Sources, in order: (1) OAuth usage endpoint with the active access token; (2) on specific failures, a retry with the legacy Keychain item, a deferral while a live session holds credentials, or a repair by delegated refresh; (3) if allowed, the CLI fallback: a hidden terminal runs `claude`, waits about 2 s, sends `/usage`, auto-accepts the workspace trust prompt and parses percentages and reset text. Times out with a "usage panel did not render" message.
- Failure classes: missing credentials, stale token, missing scope, no subscription, network, server, parse, rate limited (with `Retry-After` stored as a retry time), CLI unavailable, usage unavailable. The source that produced the result is recorded in metadata.
- Network: the app's proxy settings apply. On macOS a Tailscale DNS hint is added to network failures.
- Depends on: ACC-04, ACC-07, ACC-15.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): the activity rail shows a usage ring per agent with a detail popover (D-SH-15). Proposal: first version (was later).

### ACC-10 Codex usage and limits

- Source: pv96 (unmerged). Codex usage is shown as rings on the activity rail (one ring for weekly-only use, two rings for weekly and five-hour use) instead of bars in the status bar.
- What the user sees: 5-hour and weekly windows (classified by window length), plan type, extra credits, and reset credits (ACC-11).
- Sources: host homes use the read-only `codex app-server` probe over JSON-RPC (`account/rateLimits/read`), serialized per home. On failure, the HTTP backend (`wham/usage`) with `auth.json` tokens. WSL homes try the HTTP backend first. Auth-type errors are returned without fallback.
- Edge cases: while Codex rebuilds its session index, usage is "unavailable" and a background recovery starts. Inactive accounts are probed with a 2 s stagger.
- Depends on: ACC-02, ACC-05, ACC-15.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): the activity rail shows a usage ring per agent (D-SH-15). Proposal: first version (was later).

### ACC-11 Codex reset credits

- Source: pv96 (unmerged). Spending a reset credit is also a CLI command, `orca account codex-reset-credit`, which is owner-only (CLI-24).
- What the user does: sees how many earned rate-limit reset credits an account has (with expiry), and can consume one.
- What happens: consuming calls the backend reset-credit endpoint for the chosen account. A coordinator checks that the expected account scope still matches before consuming. Each attempt is recorded in a ledger keyed by account scope. Results are returned to the caller.
- Depends on: ACC-10, ACC-03.

### ACC-12 Read-only sign-in detection for Cursor, Grok, Gemini and Kimi

- What the user sees: whether the vendor tool is signed in, with email, plan (where known) and whether the token is fresh. Usage is shown when the vendor endpoint answers.
- Rules: MassOrca never signs in or switches these accounts. Cursor reads the macOS Keychain first, then the CLI file, then the IDE database. Grok honours `GROK_HOME`. Gemini refreshes its own token using client credentials from the installed Gemini CLI. Kimi uses its credential file and a base URL that can be overridden.
- Persisted: nothing new. Reads are from vendor files.
- Depends on: ACC-21 (secret handling). No account switching.

### ACC-13 Stored API keys and cookies for key-based providers

- What the user does: enters an API key (OpenCode Go, MiniMax) or a console session cookie (MiniMax), and the app uses it for usage reads.
- Rules: stored in `~/.orca` in an envelope with a provider prefix, encrypted by Electron safeStorage when available. If encryption is unavailable the key is stored in plaintext in the envelope and a warning is logged. Errors have the key replaced by a placeholder. Settings can show "sealed" or "plaintext" without decrypting (ACC-21).
- Usage: OpenCode Go reads its usage endpoint. MiniMax reads platform endpoints. NaN and ZCode fetchers take a key from their caller (source not traced).
- Depends on: ACC-21.

### ACC-14 Managed OpenCode and Devin accounts

- What the user does: adds an OpenCode or Devin account by logging in inside an isolated data home that MassOrca provides, then selects it for launches of that tool.
- What happens: the login runs with its own XDG data and state folders under `<userData>/managed-data-accounts/<provider>/<id>/`. After login MassOrca copies only the credential part: for OpenCode the `credential` rows of `opencode.db` (the import is refused if conversation tables contain data, and the user is asked to use an isolated login folder); for Devin `credentials.toml` with a `windsurf_api_key` (written with restricted permissions).
- Launch: when opencode, opencode2 or devin starts in a terminal (host only, not WSL), the environment is replaced with the selected account's XDG folders, OPENCODE_DB is set to `opencode.db`, and OPENCODE_AUTH_CONTENT is cleared so inline auth cannot bypass the account. The original environment is captured and restored when the account is cleared.
- Rules: up to 64 accounts per provider. Removal uses a retry-safe cleanup of leftover folders.
- Depends on: ACC-21.

### ACC-15 Usage refresh cadence, caching and staleness

- What the user sees: usage updates on a timer and on demand, and stale numbers are dropped.
- Rules: default poll 15 min. The user can change the interval (minimum 30 s). Manual and resume refreshes are debounced to 5 min. A failing active account retries every 30 s, backing off to 15 min. Inactive-account fetches are debounced to 60 s. Cached data older than 30 min is dropped, except rate-limited data, which is kept up to 24 h. Live Claude ingestion is de-duplicated within 30 s. A credential change triggers a usage refresh.
- Persisted: per-provider usage cache files `orca-claude-usage.json`, `orca-codex-usage.json`, `orca-opencode-usage.json`, `orca-muse-usage.json`, written through a serialized snapshot writer.
- Client asks: refresh all, refresh Claude or Codex for a target, set polling interval, fetch inactive accounts, refresh MiniMax or Grok, consume a reset credit. Core pushes provider state with status (idle, fetching, ok, error, unavailable).
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md`): the rail usage rings need a refresh cadence and caching from the start. Proposal: first version (was later).

### ACC-16 Usage and limits dashboard

- Source: pv96 (unmerged). Usage moved off the status bar: the status bar now shows keep-awake, resource usage, ports and remote hosts, and per-provider usage is on the activity rail (PLAT-30).
- What the user sees: per-provider usage rings on the activity rail (PLAT-30); account switcher menus for Claude and Codex with their usage; an accounts settings area; and a usage page with account cards. The token-usage side (ACC-19) has its own summary, daily, breakdown and recent-sessions views exposed by `src/preload/usage-provider-api.ts`.
- Rules: colour cues at 60% (warning) and 80% (critical) for usage bars and numbers. Windows show labels (session, weekly, monthly, named buckets) and reset times.
- Renderer detail was not read (another session covers screens).
- Depends on: ACC-09, ACC-10, ACC-15, ACC-19.

### ACC-17 Limit warnings

- What the user sees: nothing beyond colour cues (ACC-16). No notification or toast for crossing a threshold was found in the main process.
- Adjacent: delegated sessions treat 100% as "limit reached" for routing (ACC-26).
- Depends on: ACC-16.

### ACC-18 Automatic account rotation on limits

- Not found. No code path changes the selected Claude or Codex account because of usage. Account changes come from explicit user calls (select account, per target). The only automatic choice is at agent level in delegated sessions (ACC-26), not between accounts.

### ACC-19 Local token-usage analytics and cost estimates

- What the user sees: token counts and estimated cost per day, per session and per breakdown, for Claude, Codex, OpenCode and Muse. The scan is opt-in (`setEnabled`) and runs in the background.
- Sources: Claude transcripts in `~/.claude/projects` and `transcripts`, plus profile folders; Codex rollout files in the managed Codex home and `~/.codex/sessions` (and per-account homes); OpenCode databases under the OpenCode data folder (`opencode.db` preferred); Muse session logs under `$XDG_DATA_HOME/muse/sessions`.
- Pricing: model price tables (Claude and Codex), with Codex long-context billing rules. Estimates are not billing data.
- Rules: scans run in a worker and re-read only appended lines (per-file offsets and file identity). Each provider's cache has a schema version; a version change discards it. Scans do not run `git worktree list` (it can touch protected folders on macOS).
- Persisted: `orca-<provider>-usage.json` caches, `<name>-token-usage.json`, `<name>-analytics-session-ids.json`.
- Depends on: ACC-20.

### ACC-20 Usage attribution to workspaces, worktrees and sessions

- What the user sees: usage broken down by repository, worktree and session. Automation runs are attributed separately.
- Rules: each record is matched to a known worktree by path, after realpath canonicalisation with a fan-out limit of 8. Worktree metadata is indexed by repo once per scan. Scope filters pick a project or worktree.
- Depends on: ACC-19, workspace model (other domain).

### ACC-21 Credential at-rest protection

- Source: pv96 (unmerged). On Windows the desktop app may now publish a credential file before its owner-only ACL lands: its write path applies the ACL synchronously only while the user's SID is cached and no synchronous ACL call has stalled for at least 1 s in the last 30 minutes, and otherwise publishes the file and protects it asynchronously, whereas orcad and the CLI still apply the ACL before publishing.
- What happens: credential files are written atomically, with owner-only access. POSIX: files mode 0600, folders 0700. Windows: an owner-only ACL is applied before the file is published in orcad and the CLI; the desktop app does so only while its synchronous ACL lane is open (see the Source line), and hardening results are cached per process. The current user's SID comes from `whoami` and is cached; a failed lookup is retried after 60 seconds. Electron safeStorage encrypts the API-key envelopes (DPAPI on Windows, Keychain on macOS, the OS secret store on Linux; not verified in this code). macOS Claude credentials go into Keychain generic passwords through the `security` command.
- Ownership checks: managed Claude folders need a marker file that matches the account id. Symbolic links are refused, paths must stay inside the managed root, and WSL paths are resolved inside the distro.
- Settings UI: `credential-file-protection.ts` classifies a stored file as sealed or plaintext without decrypting, so opening settings never triggers a Keychain prompt.
- Gaps: the Claude and Codex system-default snapshots hold credential JSON as plain text under `<userData>` (POSIX 0600). Protection on Windows for these files depends on the writer (not verified).
- Depends on: none (platform primitive).

### ACC-22 Keep refreshed sign-ins across app restart

- What the user sees: nothing; sessions still work after the app restarts.
- Rules: before a restart, Codex runtime auth is synced for the current selection, then Claude runtime auth is synced with a 2 s budget (refreshed tokens are written back), then the store is flushed. The WSL Codex drain keeps its own timing. Startup syncs runtime auth for the current selection.
- Depends on: ACC-04, ACC-05, ACC-08.

### ACC-23 Per-account Claude profile folders (dormant)

- What exists: a design where each Claude account gets its own CLAUDE_CONFIG_DIR folder (`<userData>/claude-profiles/<id>/home`) with an ownership marker (`profile.json`), a pointer file for the `claude` shell function, history sharing and a WSL variant.
- Current state: disabled. `claudeProfileRoutingEnabled()` returns false, and its comment says routing is enabled only after the credential writers are removed. Users do not see this behaviour today.
- Depends on: ACC-04 (would replace it).

### ACC-24 Carry existing subscriptions into the new core

- What the user needs: agent accounts and subscriptions from the current app work in the new core, so agents can be tested (product decision in the plan).
- Inputs that the current app keeps (inventory only, no values read):
  - Claude: managed copies (files, or Keychain items keyed by account id on macOS), oauth-account JSON, the shared runtime copy (file and Keychain), the system-default snapshot, and the registry in settings.
  - Codex: `auth.json` in each managed home, the system default `~/.codex/auth.json`, the shared runtime copy and snapshot, WSL homes inside the distro, and the registry in settings.
  - OpenCode Go and MiniMax: encrypted key envelopes in `~/.orca`, and the MiniMax cookie file.
  - OpenCode and Devin: managed data homes (`opencode.db` credential rows, `credentials.toml`) and their `accounts.json` registries.
- Constraints for the migration design: refresh tokens are single use, so each login must end with one owner; the same login copied in several places can diverge; encrypted envelopes depend on the OS user and the safeStorage key (check whether the new core can read them, or require re-sign-in); macOS Keychain reads can prompt; ownership markers must be recognised or re-stamped; WSL homes need the distro; duplicate identities must not be imported twice.
- Proposed scope: first version (explicit requirement). Size M for Claude and Codex files. It would become L if safeStorage decryption must be reproduced.

### ACC-25 App activity counters

- What the user sees: totals of agents started, agent working time and PRs created (not usage limits).
- Rules: agent start and stop events and PR-created events are counted. PRs are deduplicated by URL, the list is capped at 2000, and the event log at 10,000 events. Saves are debounced by 5 s.
- Persisted: `orca-stats.json` in `<userData>`.
- Depends on: agent status events (other domain).

### ACC-26 Delegated sessions choose an agent by usage headroom

- What the user sees: a delegation card with an "Automatic" option that picks the agent with the most headroom, or says that no agent has headroom right now.
- Rules: an agent is marked limit-reached at 100% usage. The exact weighting of session, weekly and monthly percentages was not traced.
- Routes between agents, not accounts.
- Depends on: ACC-09, ACC-10, ACC-16.
- Owner direction (consistency pass, 2026-11-12): Proposal unchanged. The owner's delegation picks children by competence or by the coordinator; usage-headroom routing is not required.
