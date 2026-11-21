# Desktop app platform: functional inventory (PLAT)

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits), read with read-only git from worktree `unified-custom-profile-build-547ea6`. The earlier pass read worktree `massorca-server-language-820c84`, read-only. Paths are relative to that repository root unless marked otherwise. Version in `package.json`: 1.4.224 at both commits. The desktop is an Electron app; "main process" below means its Node main process.

## Domain summary

This domain covers how the desktop app is built, installed, updated, started, windowed, closed, and how it reports its own health. Installable artefacts exist for Windows (NSIS setup), macOS (disk image and zip, Developer ID signed and notarised in release builds) and Linux (AppImage, plus deb and rpm both named `orca-ide`), and a Homebrew cask exists. The updater checks GitHub releases once a day in packaged builds. Stable and RC come from `stablyai/orca`. Hourly, daily and adhoc dev channels come from separate repositories and exist only for macOS and Windows. Startup takes a single-instance lock and a profile lock that is shared with the headless `serve` mode, hydrates PATH from the login shell, and detects git, forge CLIs, agent CLIs and shells. Window, tray, Dock and menu behaviour is small and mostly convenience. Telemetry is opt-in. Crash dumps stay on disk. Report Crash files a public GitHub issue after the user previews the text, and diagnostic bundles leave the machine only after the user confirms, and only to an endpoint the build configures. At pv96 the main window uses one simplified shell (rail, side panel and session bar), the visible product name is MassOrca, and the main process records responsiveness breadcrumbs. Per-OS workarounds for macOS, Windows and Linux are spread across the startup and window code.

## Source directories read

- `config/electron-builder.config.cjs` (lines 1-170 and 696-770 read; other target and signing lines by search), `config/distribution-policy.cjs`, `config/nsis/orca-installer-hooks.nsh` and `orca-process-check.nsh`, `resources/linux/packaging/after-install.sh` (head), `Casks/orca.rb` (head), `package.json` (scripts and version).
- `src/main/updater/` (`distribution-policy.ts`, `rollout-flags.ts`, `updater-build-selection.ts`, `updater-scheduling.ts`, `updater-release-feed.ts`, `updater-remote-status.ts`, heads only; file names for the rest).
- `src/main/` root updater files: `updater-release-builds.ts`, `updater-release-api-token.ts`, `updater-prerelease-feed.ts`, `updater-changelog.ts`, `updater-mac-install.ts`, `updater-events.ts` (constants), `updater-fallback.ts` (names), `update-install-exit-watchdog.ts`, `updater-nudge.ts` (names).
- `src/main/startup/`: `main-process-preflight.ts`, `desktop-profile-instance-lock.ts`, `os-opened-documents.ts`, `hydrate-shell-path.ts`, `linux-keyring-backend-selection.ts`, `windows-install-dir-acl-recovery.ts`, `cli-launch-redirect.ts`, `main-process-preflight-failure.ts`, `ensure-virtual-display.ts`, `window-all-closed-quit-policy.ts` (headers and key constants).
- `src/main/startup/single-instance-lock.ts` (constants and first comments), `src/main/index.ts` (`open-url` and `open-file` handlers).
- `src/main/preflight/agent-detection.ts` (header), `src/main/startup/main-process-preflight.ts` (imports and `PreflightStatus`).
- `src/main/window/window-close-decision.ts`, `renderer-recovery-prompt.ts` (header), `main-window-close-lifecycle.ts` (search).
- `src/main/tray/system-tray.ts`, `tray-attention-icon.ts`, `src/main/dock/unread-badge.ts`, `src/main/menu/` (labels only).
- `src/main/crash-reporting/crashpad-capture.ts` (header and constants), `crash-feedback-diagnostic-bundle.ts` (header), `src/main/ipc/feedback-request.ts` (header and endpoint).
- `src/main/telemetry/consent.ts`, `client.ts` (header), `cohort-classifier.ts` (header).
- `src/main/observability/index.ts` (consent), `bundle.ts`, `diagnostic-bundle-upload.ts`, `diagnostic-upload-endpoint.ts` (headers and constants).
- `src/main/hang-watchdog/main-thread-hang-watchdog.ts`, `previous-hang-detection.ts` (heads).
- `src/main/star-nag/service.ts`, `threshold-trigger.ts`, `direct-star-attempt.ts`, `web-handoff.ts` (heads); `src/main/github/client/fetch/orca-star.ts` (search).
- `src/main/local-builds/local-build-feed-server.ts` (head); `src/main/updater/updater-build-selection.ts`.
- `src/main/agent-awake-service.ts` (head); `src/shared/computer-awake-mode.ts`; `src/main/system-resume-broadcast.ts` (head).
- `src/main/macos-press-and-hold-default.ts`, `macos-full-disk-access-status.ts`, `macos-tcc-prompt-notice.ts`, `macos-tcc-reset.ts`, `macos-update-running-instances.ts` (heads); `src/main/window/macos-tahoe-release.ts`.
- `src/main/durable-file-write.ts` (header); `src/main/quit-teardown-*.ts` (names and constants).
- `src/main/linux-package-install-command.ts`, `linux-update-package-type.ts` (heads).
- `src/main/windows-native-registry.ts`, `windows-process-tree-kill.ts`, `windows-pty-root-identity.ts` (names only); `native/windows-registry/index.d.ts`; `native/windows-cli-launcher/` (file names).
- `src/shared/release-channel.ts` and `profile-state-build-compatibility.ts` (read); `update-status-types.ts` and `skill-share-link.ts` (in part).
- Cross-domain references only: `src/main/orca-profiles/profile-cloud-auth-config.ts` and `src/main/runtime/relay/relay-auth-coordinator.ts` (see the CLD inventory).

- pv96 delta (`git diff custom/integration d3c6311a531`, read by name-status, numstat and `git show` on the commit): `src/main/window/` (`createMainWindow.ts`, `runtime-window-lifecycle.ts`, and the `renderer-request-relay.ts` header), `src/main/startup/` (`main-process-observers.ts`, `main-process-ready-runtime.ts`, `native-theme-sync.ts`), `src/main/diagnostics/main-thread-responsiveness-breadcrumbs.ts` (read in full), `src/main/observability/index.ts` and `diagnostic-upload-endpoint.ts` (diffs), `src/main/crash-reporting/` (the deleted bundle file, by name and size), `src/main/ipc/feedback.ts`, `crash-reporting-submission.ts` and `shell.ts` (diffs), `src/main/github/issue-create.ts` (header), `src/shared/crash-reporting.ts`, `feedback-submit-contract.ts`, `massorca-distribution.ts`, `product-identity.ts`, and the write-path comments of `src/shared/secure-file.ts` (for ACC-21 only).
- pv96 odd task documents: `odd/tasks/desktop-simplification.md` and `odd/tasks/rebrand-visible-massorca.md` read in full; `odd/tasks/cli-write-commands.md` read for the CLI features only.
- Unchanged at pv96 by diff stat (not re-read): `src/main/updater/`, `src/main/menu/`, `src/main/tray/`, `src/main/dock/`, `src/main/telemetry/`, `src/main/hang-watchdog/`, `src/main/macos-keychain/`.

## Not read or low confidence

- Updater internals: install execution, download and install support, package recovery, check state and failure handling, nudge campaign mechanics, menu checks, and the changelog format. Behaviour in PLAT-03 to PLAT-06 comes from constants, comments, and names.
- Window internals: window creation, bounds validation, focus and visibility, the renderer reload watchdog and launch-failure probe, and the clipboard and drop paths (those last belong to another domain).
- `src/main/windows/` (process table via CIM, PTY job, command-line recovery) and the root Windows files `windows-process-tree-kill.ts`, `windows-pty-root-identity.ts`, `windows-native-registry.ts`: names and one comment only.
- `src/main/crash-reporting/` beyond Crashpad capture: GPU fallback decisions, breadcrumbs, process-gone classification, minidump parsing, low-commit recovery gate.
- Telemetry validator, burst cap, install id, onboarding cohorts; observability local sink, tracer, redactor, logs directory, instrumentation; hang watchdog worker and detection loop; star-nag prompt context and agent value moment; local-build candidate, compatibility and switch modules; `preflight/agent-command-detection.ts` and `pi-runtime-detection.ts`.
- Startup lifecycle files not named above (main window controller, configure-process, first-window deferral, GPU lifecycle, HTTP/1 compatibility markers, disabled Chromium features, login-shell environment, serve activation, serve update handoff).
- Root files, names only: `appimage-runtime-identity.ts`, `appkit-scene-mutation.ts`, `app-relaunch.ts`, `asar-transparent-fs.ts`, `e2e-config.ts`, `electron-updater-loader.ts`, `linux-lid-sleep-assertion.ts`, `linux-package-downloaded-status.ts`, `linux-package-install-diagnostic.ts`, `linux-package-update-recovery.ts`, `macos-system-sleep-assertion.ts`, `macos-tcc-prompt-watch.ts`, `system-fonts.ts`, `system-power-lifecycle.ts`, `app-icon.ts`, `updater-lifecycle-diagnostics.ts`.
- Native helpers: `native/windows-cli-launcher/src/main.rs` (purpose inferred from its name, not read); other `native/*` directories are outside this scope.
- Electron-builder configuration between lines 170 and 695 (per-target details, signing hooks, asar unpack rules), and the `beforePack`/`afterPack` verification scripts beyond their names.
- `Casks/orca@rc.rb`; the contents of `resources/` (listed only).
- `src/main/macos-keychain/` and `src/main/diagnostics/main-thread-churn-probe.ts`.
- The desktop's user-facing copy and settings screens were not read. Settings keys are named only where the main process reads them.
- pv96 renderer shell (rail, side panel, session bar, status bar): not read in code. PLAT-30 comes from `odd/tasks/desktop-simplification.md` (specs, and the T2 to T4 records, which say the layout was verified on Windows only). The side-panel emulator entry comes from that spec and was not confirmed in code.
- pv96 rebrand: the 829 literal changes in 329 files were not reviewed one by one; PLAT-29 uses the rebrand task's own list of what was kept.
- pv96 `src/main/crash-reporting/crash-breadcrumb-store.ts`: read only as called from the new diagnostics file.
- pv96 `src/main/window/renderer-request-relay.ts`: header only; the mobile markdown relay that uses it was not read.
- pv96 Report Crash: `src/main/github/issue-create.ts` was read for its header only, so the authentication and error paths are not confirmed.

## Features

### PLAT-01 Installable builds per operating system

- **What the user gets.** Orca is installed from GitHub release assets: `orca-windows-setup.exe` (NSIS), `orca-macos-x64` and `orca-macos-arm64` as `.dmg` and `.zip`, `orca-linux.AppImage` (or `orca-linux-arm64.AppImage` when the arm64 release flag is set), `orca-ide_<version>_<arch>.deb` and `orca-ide-<version>.<arch>.rpm`. App id `com.stablyai.orca`.
- **Rules and per-OS behaviour.** The Windows installer checks whether Orca is running before it proceeds (PowerShell CIM query, with an image-name fallback), registers "Open with" entries for documents (PLAT-15), and removes them on uninstall. The uninstaller is signed through SignPath, and non-dev builds carry the SignPath publisher name for the installer. macOS release builds use the hardened runtime, Developer ID signing and notarisation. Dev-channel builds on macOS are notarised with the release identity so that they can replace an installed release. Dev-channel builds on Windows are unsigned and omit the publisher name. Linux builds run build-time checks (static AppImage contract, glibc floor, packaged node-pty). Release artefacts are created as GitHub drafts and are undrafted only after every asset exists. Dev channels publish as prereleases to their own repositories.
- **Persists.** Nothing user-facing beyond the installed application and, on Linux, the CLI link (PLAT-27).
- **Depends on.** None.
- **Status.** First version. Size L as a full set. An installable build per OS is the minimum for daily desktop use. Signing and notarisation can follow.

### PLAT-02 Homebrew cask and Linux package installs

- **What the user gets.** `brew install --cask orca` (and `orca@rc`). The cask sets `auto_updates true`, so Homebrew does not upgrade a self-updating app unless forced, and it links the bundled `orca` CLI onto PATH. Linux users install the deb or rpm `orca-ide`.
- **Rules.** The cask conflicts with `orca@rc` and needs macOS 11 or later. The deb and rpm declare runtime dependencies (GTK, NSS, xdotool, xclip, python3, and a virtual X server, `xvfb` or `xorg-x11-server-Xvfb`) so headless browser panes work. The after-install script links `/usr/bin/orca-ide` to the bundled CLI shim and keeps Chromium's sandbox helper usable. The after-remove script removes the link only if the package owns it.
- **Observed gap.** `Casks/orca.rb` pins version 1.3.24, while `package.json` says 1.4.224. The cask lags the app.
- **Depends on.** PLAT-01, PLAT-27.
- **Status.** Later. Size M. Distribution channels follow the first installable build.

### PLAT-03 Automatic update checks and install (stable and RC)

- **What the user sees.** A background check runs every 24 h in packaged builds. A "Check for Updates..." item exists in the menu and the tray. The renderer receives a status stream with these states: idle, checking, available, not-available, downloading (with percent), downloaded (with an optional release link), and error. An update card (a "nudge") appears when a campaign is active. Release notes are fetched for the card. The install runs through a quit-and-install step that the user requests or that is scheduled (scheduling logic not read).
- **Rules.** Dev and unpackaged builds never check; they report not-available. A failed automatic check retries with a one-hour base interval that doubles up to six hours. A check is deferred while a quit-and-install is pending, while a pinned or local build is active, or while the active feed belongs to another source. Campaign ids persist, so a dismissed card stays dismissed after a relaunch. Versions with a dash are RC; others are stable. When a stable or RC release manifest is missing, one fallback to the prerelease feed is tried once. A shutdown that stalls during an install is bounded to 20 s, so an installer can replace the bundle.
- **Exchange.** The renderer asks for a check or an install and receives status events. The main process owns the feed requests.
- **Persists.** Last-check time, pending nudge id, retry count.
- **Depends on.** PLAT-01; PLAT-04 for per-OS install rules.
- **Status.** Later. Size L. The first version can be upgraded by hand. Signed artefacts and a feed come first.

### PLAT-04 Per-OS update installation rules

- **macOS.** Before the bundle is replaced, the updater lists the running instances of the same bundle path. Squirrel.Mac waits for every main process of that bundle to exit, so the list matters.
- **Windows.** Installers are verified against the publisher name recorded in the installed app's update configuration. Unsigned dev-channel installers therefore cannot be entered from a signed build (PLAT-05).
- **Linux.** The updater classifies the installed package type: AppImage, deb, rpm, non-root, or unusable. For deb and rpm, it builds a package-manager install command string (apt or dpkg for deb; zypper, dnf, yum or rpm for rpm); how the UI presents it was not read. It reports why it cannot when sudo or a trusted manager is missing. A package manager counts as trusted only when it resolves from `/usr/bin`, `/bin`, `/usr/sbin` or `/sbin`, never from a user-writable PATH entry. An externally-managed marker changes the path, and tests cover it. AppImage updates use the standard in-app path (not read in detail).
- **Depends on.** PLAT-03.
- **Status.** Later. Size M. Needed with PLAT-03. Linux package paths differ by distribution.

### PLAT-05 Dev channels, exact-build jumps and local builds

- **What the user sees.** In the menu, a developer can pick hourly, daily or adhoc builds (macOS and Windows only; Linux falls back to stable or RC), or jump to an exact release build. On macOS, a local build artefact can be chosen and served from a loopback feed, which allows downgrades.
- **Rules.** Dev builds live in separate repositories (`orca-hourly`, `orca-daily`, `orca-adhoc`) so their tags do not push stable entries out of the main releases feed, which exposes only the newest 10 entries. The release picker calls the GitHub API with the user's own `gh` token, so it does not share the 60-per-hour anonymous bucket; the token is never logged. The local feed serves only GET requests under a random 24-byte path token, with no-store caching. A pinned build owns the feed until it settles, so a background check cannot repoint it. A downgrade across the SQLite profile boundary is refused (PLAT-09). Local build switching is macOS only.
- **Depends on.** PLAT-03, PLAT-04.
- **Status.** Drop. Size M. Developer and test tooling. A rebuilt product should decide its own channel model.

### PLAT-06 Update policy controls: manual-only builds and remote rollout flags

- **Manual-only builds.** A build made with `ORCA_MANUAL_UPDATES=1` writes an `orcaDistribution` record into its packaged `package.json` (repository `anubischpr/massorca`, update policy `manual`). Such builds never check automatically, and remote service updates report `manual-service-update-required`. Unknown or unreadable metadata fails closed to manual.
- **Rollout flags.** The campaign payload (`nudge.json`) can carry a `rollout` block with six named flags: `pinned-relay-default`, `managed-servers-visible`, `serve-on-orcad-default`, `legacy-npm-rung-retired`, `legacy-relay-dir-sweep`, and `windows-relay-wmi-fallback-retired`. Each flag can be `default`, `off`, or `on` for a percentage of installs, within a version range. The cohort is a SHA-256 bucket of flag name and install id. All six are off in this build. An absent, invalid or out-of-range entry keeps the baked-in value.
- **Depends on.** PLAT-03.
- **Status.** Drop. Size M. The flags are migration switches for the current product, and manual-only distribution is a fork policy.

### PLAT-07 Single instance and profile lock

- **What the user sees.** A second launch focuses the running window (Electron single-instance lock). A duplicate `orca serve` start is ignored rather than promoted to a window, because a supervisor restart must not raise a desktop window. A held profile lock is reported through the preflight-failure path, which names the holder.
- **Rules.** The lock is keyed by the user-data path, so dev and packaged builds lock separately. The desktop and the headless `orca serve` process (orcad) take one shared profile lock and refuse each other. Exit code 3 means "another process owns this profile" and is a stable contract for supervisors. A lock left by a dead process is reclaimed on the next start. `ORCA_BYPASS_SINGLE_INSTANCE_LOCK=1` bypasses the packaged macOS lock, for diagnostics only.
- **Exchange.** A second process hands its argv to the running instance, which decides whether to activate a window. A serve argv never activates one.
- **Why it matters.** The runtime discovery files (RPC endpoint with token, and hook endpoint) are written once per owner. A second owner would overwrite them and leave `orca status` reporting a dead process.
- **Persists.** Lock file under the profile directory.
- **Depends on.** PLAT-26; the core service (plan Phase 1).
- **Status.** First version. Size S. The plan requires exactly one local core per profile.

### PLAT-08 Startup preflight and tool detection

- **What the user sees.** At startup and when the source picker or settings ask, Orca reports whether git, the GitHub CLI (installed and authenticated), GitLab CLI, Bitbucket, Azure DevOps, Gitea, shells and known agent CLIs (including TUI agents) are installed and reachable. The result is a single preflight status object.
- **Rules.** GUI-launched processes miss PATH entries that shell profiles add. Orca probes the profile-loading login shell once on POSIX and merges the persisted Windows PATH on Windows, so user-local tools are found. WSL distributions and SSH remote hosts are probed with the same commands, and WSL results are cached and pruned. If the profile lock is held or startup preflight fails, a dialog explains the failure.
- **Exchange.** The renderer asks for the preflight status and gets the full status object. A failed shell hydration is returned as a typed reason, not thrown.
- **Persists.** In-memory detection results; WSL preflight cache.
- **Depends on.** PLAT-07; terminals (Phase 2) and agents (Phase 4) by name.
- **Status.** First version. Size M. Git and at least one agent CLI must be found before any work starts. Forge sign-in checks can wait.

### PLAT-09 Profile storage compatibility guard

- **What the user sees.** Profile storage moved to SQLite in release 1.4.214. A build at or after that release refuses to switch to an older build that cannot read the store, and the refusal explains that each profile must first be rolled back with the CLI (`orca profile state rollback --latest-json --profile-id <id>`). The same guard blocks local build switching across the boundary (PLAT-05).
- **Depends on.** None.
- **Status.** Drop. Size S. The mechanism is specific to the Electron-era store cutover. The new core store needs its own schema versioning in Phase 1, which is not this row.

### PLAT-10 Main window lifecycle and close confirmation

- Source: pv96 (unmerged). The separate title bar is gone: window controls and the window drag area sit on the workspace's session bar, so the main window has no title row above the chat (verified on Windows only; macOS traffic lights and fullscreen were not checked).
- **What the user sees.** One main window. Restored window bounds are validated before use. Closing the window asks for confirmation when unsaved editor files, running processes or several sessions are at risk.
- **Rules.** A window whose renderer is gone or crashed can close without confirmation. A renderer that is alive but unresponsive still gets the confirmation, so a hung UI cannot discard sessions. On macOS, closing the last window keeps the app running until it is quit. On Windows and Linux, closing the last window quits, except in serve mode, where the process keeps running until it is quit. The "minimise to tray on close" setting (PLAT-12) changes the close path when enabled; it is off by default.
- **Exchange.** On a close request the main process asks the renderer to confirm. The window closes only after the renderer replies, or when the renderer is gone.
- **Persists.** Window bounds and visibility in UI settings (not read in detail).
- **Depends on.** PLAT-07; PLAT-12.
- **Status.** First version. Size M. Work must not be discarded on close, and the core keeps running regardless of the window.

### PLAT-11 Renderer recovery and GPU fallback

- **What the user sees.** If the UI process crashes or fails to launch repeatedly, a recovery prompt explains the failure. For a low-commit (memory) failure the prompt shows the available commit size. After a GPU-related failure, the app offers to restart with GPU acceleration disabled, and a marker survives the restart. Repeated automatic reloads are bounded by a watchdog and a circuit breaker (logic not read). A Windows install directory whose permissions were damaged is detected and gets its own recovery.
- **Depends on.** PLAT-10.
- **Status.** Drop. Size M. These are Electron renderer failure modes. A GPUI window needs its own crash policy.

### PLAT-12 Tray and Dock attention indicators

- **Tray.** An icon in the tray (menu bar on macOS, using a template glyph; a 16 px icon on Windows). The menu offers Open, Settings, Check for Updates and Quit. An amber dot appears while something needs attention. Dev builds show a DEV tooltip with the worktree or branch label, so several dev instances can be told apart.
- **Dock (macOS).** An unread-count badge that shows `99+` above 99.
- **Setting.** "Minimise to tray on close" (default off). When on, closing the window hides it to the tray.
- **Depends on.** PLAT-10.
- **Status.** Later. Size S. A convenience. The core runs with or without a tray.

### PLAT-13 Application menu

- **What the user sees.** Localised menus. On macOS the app menu offers About, Hide and Quit. Other menus cover File, Edit (Copy, Paste, Select All), Appearance (toggle left and right sidebars, with shortcut hints), Settings (`Ctrl+,` or `Cmd+,`), Check for Updates..., Explore MassOrca, Getting Started, and Report Crash... (PLAT-16).
- **Depends on.** PLAT-10.
- **Status.** Later. Size S. Standard menus are cheap, but daily work does not need them. The first window needs only Edit and Quit.

### PLAT-14 Safe quit and durable state writes

- **Quit.** Quitting runs named teardown steps, and they share one deadline (`WILL_QUIT_TEARDOWN_DEADLINE_MS`), so one stuck step cannot block exit. Teardown starts once (a start gate). During a quit-and-install, an exit watchdog gives the shutdown 20 s, so a wedged teardown cannot keep the old process alive and block the update. Quit paths that would drop pending durable writes are blocked until those writes finish.
- **Durable writes.** Files are written to a temporary file, fsynced, renamed into place, and the directory is fsynced. A ring of `.bak` copies recovers a file that came back empty after power loss, at a cost of up to about one hour of changes, according to the code comment.
- **Depends on.** PLAT-07; persistence (plan Phase 1).
- **Status.** First version. Size M. Crash-safe state and a bounded quit are data-safety requirements for the core store and the UI.

### PLAT-15 OS file opening, Open-with entries and `orca://` links

- **Documents.** Markdown (`.md`, `.markdown`, `.mdx`) and tabular files (`.csv`, `.tsv`) opened from the OS are queued until the window is ready, up to 32 paths. macOS delivers them as open-file events, Windows as a command-line argument, and Linux as a path or file URI. Other file types are not claimed.
- **Windows.** The installer adds "Open with Orca" entries for these extensions without changing default handlers, and it removes them on uninstall.
- **Links.** The `orca://` scheme is registered. The app claims only skill-share links (`parseSkillShareId`). Other `orca://` URLs are not claimed. Skill-share resolution needs the cloud API (CLD-04).
- **Depends on.** PLAT-10, PLAT-01.
- **Status.** Later. Size S. File association is a convenience. Skill sharing sits outside the first version.

### PLAT-16 Crash dumps and Report Crash feedback

- Source: pv96 (unmerged). Report Crash now creates a public GitHub issue in the MassOrca repository instead of posting to onorca.dev, and the three-day log bundle, image attachments, the anonymous option and the GitHub identity fields were removed.

- **Dumps.** Electron Crashpad writes minidumps locally. Upload stays off, because dumps contain process memory. A text signature is read from each dump and paired with a child-process or render-process-gone event that happened within 30 s. The store keeps at most 64 dumps and 128 MiB, pruned after each crash.
- **Breadcrumbs.** Crash breadcrumbs are written durably, and process-gone events are classified (renderer, GPU, child process, sibling correlation).
- **Report Crash.** The application menu opens a feedback form. Submission runs through the main process, which creates a public issue in the MassOrca repository (`MASSORCA_GITHUB_REPO`) with `gh api`, so the GitHub CLI must be installed and signed in (inferred from the helper's header). The issue carries only the text the dialog previews. The three-day log bundle, image attachments, anonymous submission and the GitHub login fields are gone, and the onorca.dev endpoint is no longer used.
- **Exchange.** The renderer hands the feedback form to the main process, which creates the issue and returns a typed result: the issue URL on success, or a status and an error on failure.
- **Depends on.** PLAT-18; GitHub CLI sign-in (name).
- **Status.** Later. Size M. Useful once there are users. Local logs (PLAT-18) come first.

### PLAT-17 Opt-in telemetry

- **What the user sees.** A first-run banner asks whether to send usage events. Until the user answers, nothing is sent (existing users are shown the banner). Only official stable and RC builds can transmit, because CI must inject both a build identity and a write key. Dev builds never transmit. Events go to PostHog (`us.i.posthog.com`).
- **Rules.** `DO_NOT_TRACK`, `ORCA_TELEMETRY_DISABLED`, and any CI environment variable disable transmission at runtime without changing the stored choice. A misspelt truthy value is warned about once and treated as unset. Common properties are app version, platform, architecture, OS release, a random install id, a session id and the channel. There is no PostHog person profile per install. Events are schema-validated and burst-capped, and the cap runs before the consent read. Cohort values (for example the number of repositories when the event fires) are attached. Diagnostic bundles never carry the install id.
- **Depends on.** None.
- **Status.** Drop. Size M. Product analytics are not in the plan. If wanted later, make it opt-in with a new schema.

### PLAT-18 Local trace log and redaction

- **What the user gets.** A local newline-delimited JSON trace of spans, redacted before writing and rotated as a family of files. A daemon lifecycle log is kept beside it for headless `serve`. Nothing leaves the machine by itself.
- **Rules.** On by default. Off in CI, and off when `ORCA_DIAGNOSTICS_DISABLED=1`. `DO_NOT_TRACK` or `ORCA_TELEMETRY_DISABLED` keep the local file on but disable bundle upload (PLAT-19).
- **Persists.** Trace and daemon logs under the logs directory of the user-data path.
- **Depends on.** PLAT-14.
- **Status.** First version. Size S. The core and the CLI need local logs for debugging and bug reports. The redaction rules must be designed fresh.

### PLAT-19 Diagnostic bundle upload

- Source: pv96 (unmerged). A build-time endpoint on the upstream onorca.dev host is now refused, so a MassOrca build uploads diagnostic bundles only when it configures its own endpoint.
- **What the user does.** From the diagnostics UI, Orca collects the last 30 minutes of redacted spans (plus the rotated daemon log), shows a preview, and uploads only after confirmation.
- **Rules.** The bundle is capped at 4 MiB. Upload is two-step: a short-lived token is requested from the configured endpoint, then the bundle is posted. The token endpoint is rate-limited per IP at the edge (10 per hour, per the code comment). The submission id is random and never the install id. The endpoint is substituted at build time; official builds pin it, and a developer environment variable can redirect only unofficial builds. At pv96 an endpoint on the upstream onorca.dev host is refused even when a build injects it, because MassOrca logs must not reach that service. The endpoint host was not read. Main keeps the exact preview bytes, so the renderer cannot swap content after preview. A delete call with the ticket id exists.
- **Depends on.** PLAT-18.
- **Status.** Later. Size M. A support tool. Manual export of the PLAT-18 logs covers the first version.

### PLAT-20 Main-thread hang watchdog

- **Behaviour.** On packaged macOS builds with local logging enabled, a worker thread watches heartbeats from the main thread. The default timeout is 45 s, tunable by environment variables. When the main thread stops answering, a marker file records the hang. On the next launch, the marker is consumed and recorded as a crash breadcrumb.
- **Rules.** Off in unpackaged builds unless `ORCA_HANG_WATCHDOG_FORCE=1`. Off when local logging is disabled.
- **Depends on.** PLAT-18.
- **Status.** Drop. Size S. The watchdog is macOS and Electron main-thread specific. The Rust core can add its own stall detection later.

### PLAT-21 Star prompt

- **Behaviour.** After a user has started 35 agents since the baseline of the current app version (the baseline resets on each version), Orca shows a card asking them to star the repository. "Later" defers it with a three-day cooldown. The star is sent through the GitHub CLI when the user is signed in (a PUT to the starred endpoint). Otherwise a web page opens. Users who completed the star are not asked again. Outcomes are tracked as telemetry.
- **Status.** Drop. Size S. A growth prompt, not product behaviour.

### PLAT-22 Keep the computer awake while agents run

- **What the user sets.** A computer-awake mode: `on`, `off` or `auto`. With no explicit mode, the default is `off`, unless a legacy auto flag is set. While agents run, Orca holds Electron's app-suspension blocker and a per-OS assertion: a macOS system-sleep assertion and a Linux lid-sleep assertion (name-based reading). Holds are re-acquired after resume. System suspend and resume events are broadcast to windows as `system:resumed`.
- **Depends on.** Agents (plan Phase 4), by name.
- **Status.** Later. Size S. Helps long agent runs, but the first build does not need it.

### PLAT-23 macOS platform integration

- **Key repeat.** Orca writes `ApplePressAndHoldEnabled=false` to its own preferences domain once, so holding a key repeats it in terminals instead of opening the accent picker. The recorded decision prevents later overwrites. Users can revert it with `defaults`.
- **Privacy prompts.** A Full Disk Access probe reads protected locations. Orca shows Full Disk Access guidance only after a macOS privacy dialog attributed to Orca has been detected. Stale privacy grants for helper tools can be reset with `tccutil`.
- **macOS 26 (Tahoe, Darwin 25 or later).** AppKit scene-backed windows can deadlock on re-entrant frame updates, so Orca defers AppKit scene mutations on that release.
- **Identity.** Dev and release builds share the bundle id and signing identity, so an update replaces the installed app.
- **Depends on.** PLAT-01, PLAT-04.
- **Status.** Later. Size M. Polish. Press-and-hold and Tahoe behaviour must be checked in the GPUI app (plan Phase 5).

### PLAT-24 Windows shell, process and job handling

- **Shells.** Git Bash and PowerShell (`pwsh`) are detected. PowerShell gets an OSC 133 shell-integration bootstrap. The persisted Windows PATH is merged into the environment, and PowerShell's constrained-language mode is handled (test names).
- **Processes.** Process tables are read through CIM with a timeout and a dedicated timeout error. PTY processes are checked for root identity and terminated as a tree (`windows-pty-root-identity`, `windows-process-tree-kill`, and a job-ownership check in the packaging scripts; logic not read).
- **Launchers.** JetBrains GUI launcher handling exists (not read).
- **Depends on.** PLAT-08; terminals (plan Phase 2).
- **Status.** First version. Size M. Windows terminals and agents need shell detection and reliable tree termination. The detailed mechanisms belong to the terminal design.

### PLAT-25 Linux platform integration

- **Credential store.** Chromium cannot recognise several tiling compositors (Hyprland, sway, river, niri). It would then pick `basic_text`, which silently falls back to plaintext. Orca names `gnome-libsecret` only when the secret service is present and its default collection is already unlocked. An explicit `--password-store` always wins, and a desktop that Chromium already resolves is never overridden.
- **Headless display.** `orca serve` starts a virtual X display (Xvfb) when none exists, because Chromium needs a display for browser panes, even offscreen.
- **Other.** A `/dev/shm` usage policy, a glibc floor check at build time, and no dev channels (PLAT-05).
- **Depends on.** PLAT-01; credential storage (security domain).
- **Status.** Later. Size M. Electron and Chromium specific. A Rust core still needs a Linux secret-store choice as part of credential storage.

### PLAT-26 Headless serve startup and supervisor handoff

- **What the operator does.** Runs `orca serve`, the same binary, as a headless host. A documented systemd unit runs `<binary> serve --port ...`. Serve keeps running when windows close and quits when its supervisor disconnects. Updates for a headless host are handed to a supervisor (`serve-update-handoff`) rather than replacing the process. Remote server updates report whether they are automatic. Under manual-only builds (PLAT-06) they report `manual-service-update-required`.
- **Rules.** A duplicate serve start does not raise a window. Desktop activation is gated by serve state (`serve-desktop-activation`).
- **Depends on.** PLAT-07; headless server (plan Phase 3).
- **Status.** Later. Size L. The headless start is needed for the first version but belongs to the server domain. The update handoff and supervisor coupling can wait.

### PLAT-27 CLI on PATH and launch redirect

- **What the user gets.** After install, the `orca` command is available. The Homebrew cask and the deb and rpm packages link it. The in-app Install CLI action (GUI only, so it cannot run on a server) puts it on PATH on desktops. One binary serves as desktop, server and CLI: a launch with a CLI command name is redirected to the bundled CLI, and value-taking flags are honoured.
- **Depends on.** PLAT-01; the CLI (plan Phase 1).
- **Status.** First version. Size S. The CLI is the first client and the test harness in the plan.

### PLAT-28 Main-thread responsiveness breadcrumbs

- Source: pv96 (unmerged). New at pv96: the main process samples its event loop and records lag and unresponsive-window breadcrumbs, which the custom/integration app did not have.
- **What the user does.** Nothing. Every 500 ms the main process checks its own timer; when a sample fires at least 1 s late, it records a `main_event_loop_lag` breadcrumb with the lag in milliseconds. The main window's `unresponsive` and `responsive` events record `window_unresponsive` and `window_responsive`; the second one carries how long the window was unresponsive.
- **Rules and edge cases.** Each breadcrumb goes to the crash breadcrumb store (PLAT-16) and as a zero-length trace span (PLAT-18), so it survives a hang the process never recovers from. Only the main window is wired. Breadcrumb data is limited to a name, a duration or lag, and the surface name.
- **Persists.** The crash breadcrumb store and trace spans.
- **Depends on.** PLAT-16, PLAT-18.
- **Status.** Later. Size S. Electron main-thread sampling. The Rust core needs its own stall detection; PLAT-20 (dropped) does not provide it.

### PLAT-29 Visible product name MassOrca

- Source: pv96 (unmerged). The rebrand (approved 2026-10-29) changes every human-visible product name to MassOrca in main-process text, renderer copy, CLI help, and mobile and store text, and leaves identifiers unchanged.
- **What the user sees.** Dialogs, notifications, error messages, menus, renderer copy, CLI help, the mobile display name, permission prompts and store text say MassOrca. The product constant `PRODUCT_DISPLAY_NAME` is `MassOrca`. The relay label reads `MassOrca Relay` on the mobile host card and in desktop pairing guidance.
- **Rules and edge cases.** Kept on purpose: `productName` and `executableName` (`Orca`), the app id `com.stablyai.orca`, the install and userData folders, the `orca` command, the `orca://` scheme, `ORCA_*` variables, Keychain service names, config keys, and wire names that paired older clients parse. Some product names stay Orca (Orca Cloud, the Orca Computer Use helper, the Windows mobile firewall rule). Older-host error classifiers still match the old wording.
- **Depends on.** None.
- **Status.** First version. Size S. The owner approved the name, and the plan already calls the product MassOrca; one constant carries it. The install identity and its migration are a separate open decision (see Observations).

### PLAT-30 Desktop shell: activity rail, side panel and session bar

- Source: pv96 (unmerged). The desktop shell is simplified to an approved layout: a fixed rail on the left, one session bar with no tabs or title bar, and a per-workspace side panel that holds terminals, browser tabs and other tools (approved prototype, 2026-10-31; spec in `odd/tasks/desktop-simplification.md`).
- **What the user sees.** A narrow rail (52 px) stays on the far left. It holds the workspace entry, Tasks, Calendar, Settings and the theme toggle. At its bottom it shows one usage ring per agent: one ring when the agent has weekly usage only, and two rings (outer weekly, inner five-hour) when it has both. The top row of the main area shows the session name on the left and the window buttons on the right; sessions no longer show a tab strip there. A right side panel per workspace holds tabs for a new terminal, a new browser tab, a new Markdown tab and a new mobile emulator, and it does not list agents. The status bar runs under the main area and the side panel, with keep-awake, resource usage, ports and remote hosts, and no usage meters.
- **Rules and edge cases.**
  - Mod+T and Mod+Shift+B open the side panel and add a terminal or browser tab. Mod+W closes the focused side-panel tab and never a session.
  - The side panel's width and its open or closed choice per workspace are remembered (SET-02). Without a choice, it opens when the workspace has tabs.
  - Bot-team mode waits for work that is not merged, so it is hidden. The calendar button is shown although it depends on that work (overnight decision, 2026-11-03).
  - The chat summary can be hidden and shown with a button (`nativeChatSummaryVisible`, on by default; chat domain).
  - The QA records show the layout checked on Windows only. Not checked: SSH, folder workspaces, macOS, Linux, window dragging, populated split groups, and macOS traffic lights and fullscreen.
- **Depends on.** SET-10 (remembered view), SET-11 (shortcuts), SET-30 (theme toggle), TERM-02 (terminal tabs), BRW-02 (browser tabs), ACC-09 and ACC-10 (usage rings); bot teams and calendar (names only).
- **Status.** First version. Size M. The approved simplified desktop is the first desktop surface. The detailed design lives in `docs/design/`, which this inventory does not restate.
- Owner direction for the new product (2026-11-12, `docs/design/screen-inventory.md` and `pending-owner-review.md` items 12 to 14, decided on the owner's behalf and pending review): the Title bar stays as a band of the frame and carries what pv96 moved into its session bar; pv96's removal of the title bar is not followed. In Workspaces mode, Automations and Prototypes are entries at the top of the Sidebar, not the rail. Tasks, Calendar, Marketplace, Settings, Automations and the Bot Teams pages open as full pages over the canvas, and diffs open as Work panel file tabs. The mode switch first returns to the current mode's home, then switches mode.

## Observations for the owner

- The build marker for manual-only distributions names `anubischpr/massorca`. Builds made with it never auto-update (PLAT-06). Confirm whether that is the current distribution model.
- The Homebrew cask version (1.3.24) lags `package.json` (1.4.224) (PLAT-02).
- Telemetry is opt-in and goes to PostHog. Crash dumps never leave the machine. Report Crash now opens a public GitHub issue on the MassOrca repository after the user previews the text, so anyone can read what the user sent; confirm that this is the intended privacy model (PLAT-16). Diagnostic bundles go only to a non-upstream endpoint that a MassOrca build configures (PLAT-19).
- The rebrand kept the install identity: `productName` `Orca`, `appId` `com.stablyai.orca`, and the install and userData folders are unchanged, so a separate MassOrca install and its data move remain an open decision (PLAT-29).
- Cloud sign-in is described as dev-only in the settings copy, but the production endpoints are compiled in (see CLD-04).
