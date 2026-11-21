# Browser and devices (domain BRW)

Status: draft functional inventory, 2026-11-12. Describes the current MassOrca (Electron) behaviour as a reference. Proposal marks are proposals for the owner, not decisions. Nothing here prescribes the Rust design.

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits), read with read-only git. The browser engine, emulator, computer-use and speech code did not change at pv96; the earlier pass described the same code from `massorca-server-language-820c84`.

## Domain summary

This domain covers the integrated browser pane: embedded web pages in workspace tabs, with profiles, cookie handling, localhost preview of dev servers, and the commands agents use to drive pages. It also covers desktop computer use (agents inspecting and acting on native apps), emulator control and streaming for Android and iOS simulators, previews of workspace documents and media that share the same sandboxed-view machinery, and voice dictation with local or cloud speech recognition. The reference builds the pane on Electron webview guests and Chromium debugger APIs. Agent automation runs through an external daemon attached to a per-tab DevTools-protocol proxy. Computer use uses one native helper per OS. Android uses adb, the emulator binary and scrcpy. iOS uses simctl and an external stream helper. Speech uses sherpa-onnx.

## Source directories read (repo-relative, reference worktree)

- `src/main/browser/`: 326 non-test files. Read by file names, `// Why:` comments, headers and selected bodies. Test files only by name.
- `src/main/runtime/`: grep for browser, emulator and speech handlers (128 files with "browser" in the name). Read: `runtime-browser-commands-*` names, the mobile method allowlist (`runtime-rpc/runtime-rpc-mobile-method-allowlist.ts`), the browser client-host files by header only.
- `src/main/computer/`: 34 non-test files, mostly sampled.
- `src/main/emulator/`: 48 non-test files, mostly sampled; Android backend header and bridge partly read.
- `src/main/speech/`: 24 non-test files, sampled through comments and the model catalogue.
- `src/main/media/`: both files (protocol and range responses), partly read.
- `src/main/ipc/`: channel names only (`browser*.ts`, `speech.ts`, `emulator-*.ts`).
- `src/main/ports/` and `src/main/localhost-worktree-label-proxy.ts`: headers and key lines.
- `src/preload/browser-*.ts` and `src/preload/doc-preview-link*.ts`: headers and key lines.
- `native/computer-use-macos/` (file list and the native method contract), `native/computer-use-windows/runtime.ps1` and `native/computer-use-linux/runtime.py` (headers only), `native/keyboard-layout-macos/` (file name only).
- `src/cli/browser-handler-groups.ts`, `src/cli/handler-group-manifest.ts` (command keys), `src/cli/handlers/computer.ts`, `src/cli/handlers/emulator.ts`, `src/cli/specs/computer.ts`, `src/cli/specs/emulator.ts` (command keys only).
- `src/shared/browser-user-agent-mode.ts`, `src/shared/browser-find-source.ts`, `src/shared/media-file-extensions.ts`.
- `src/renderer/src/components/browser-pane/assemble-chrome/BrowserFind.tsx`: one grep for the find-in-page call.
- `package.json` (pinned external packages), `README.md` (product wording for Design Mode and Computer Use). `docs/` has no browser or device documents.

- pv96 (`git diff --name-status custom/integration d3c6311a531`): no non-test file changed under `src/main/browser`, `src/main/computer`, `src/main/emulator`, `src/main/speech`, `src/main/media`, `src/main/ports`, the browser preload files, `src/main/ipc/browser*`, `src/main/ipc/emulator-*`, `src/cli/browser-handler-groups.ts`, or the computer and emulator CLI handlers and specs. The changed code that touches this domain is `src/main/window/runtime-window-lifecycle.ts` (the `orca file open --preview` route), `src/cli/handlers/file.ts` and `src/cli/specs/file.ts`, plus the approved side-panel spec in `odd/tasks/desktop-simplification.md`.

## Not read / low confidence

- **Tests** (`*.test.ts`, `*.electron.test.ts`): skipped, except test names and `Why:` comments that clarify a rule.
- **Most of `src/main/browser/`** (about 300 non-test files) was sampled through comments, headers and file names. No file was read end to end. Rules taken only from a single comment are medium confidence.
- **Runtime client-hosted machinery** (`browser-host-*`, `client-hosted-browser-*`, `paired-runtime-browser-*`): names and headers only. Exact protocol details are low confidence.
- **Emulator internals**: scrcpy stream session, serve-sim runtime materializer, iOS backend bodies, start-lease semantics (inferred from the file name).
- **Native code bodies**: the macOS Swift helper (file list and method contract only), the Windows PowerShell runtime and the Linux Python adapter (headers only), the macOS keyboard-layout helper (not read; purpose inferred from its name).
- **External packages** not in the repo: `agent-browser` (~0.27), `serve-sim` (0.1.47), `sherpa-onnx` (1.12.37). Their behaviour is inferred from the wrapper code only.
- **Renderer and mobile UI**: not read; one grep only.
- **Unverified details**: what `tab profile clone` copies; whether browser tabs persist across restarts (inferred from a comment about ghost tabs in persisted state); whether captured network and console logs are kept on disk (inferred as memory only); exact CLI flag sets (only command keys were extracted).
- **Outside this domain**: session import from other agent tools; the Android client app.
- **pv96 side panel and rail**: the new-tab routing and the panel's browser tabs are renderer code; this pass read them from the approved spec and its records only.
- **pv96 `orca file open --preview`**: the CLI handler and the window route were read by name and commit subject; the HTML path into the browser pane was not traced.

## Proposal summary

- First version (3): BRW-01, BRW-02, BRW-07.
- Drop (2): BRW-04, BRW-20.
- Later (25): all other features.

## Features

### BRW-01 Load and navigate web pages in the pane

**What the user does and what happens.** The user opens a browser tab in the pane and types a URL, follows links, or uses back, forward, reload and stop. The tab shows loading state, title and current URL. A failed load shows an error page with the reason. A navigation replaced by a newer one is not reported as success. Agents reach the same behaviour through `goto`, `back`, `forward` and `reload` (see BRW-13).

**Rules and edge cases.**
- Web schemes are the normal targets. Local file URLs are allowed for previews the user opens. A remote page that redirects to a file URL is blocked.
- `goto` runs under an embedded navigation limit that is longer than the agent tool's own timeouts, so the bridge never kills a command before the page does.
- An aborted navigation is judged by its cause. If the page vetoed unload, the command does not fail and returns the page still shown. An abort that was not a veto is reported as a failure. An abort caused by a newer navigation waits for the replacement and reports the replacement's load error, if any.
- Error copy and external handoffs remove tokens carried in URLs of search providers (the reference names Kagi).
- Certificate and load-error states stay visible until a newer navigation commits. A stale error is never replayed.
- Page content cannot change the app's tab model. Only validated URL requests cross from the page to the host.
- Default-profile cookies and site storage persist across restarts (see BRW-03).

**Interface.** The UI asks for navigation, reload and stop. The core pushes load state, title and URL changes, load errors and active-tab changes.

**Data persisted.** Default partition in the app data directory (cookies, local and session storage, cache). Tab URL and title in workspace state is inferred from a comment about restored tabs (medium-low confidence).

**Depends on.** The web engine (see the engine section).

### BRW-02 Tabs, popups and page-opened windows

- Source: pv96 (unmerged). Browser tabs open in the workspace's side panel (New Browser Tab, Mod+Shift+B) instead of a tab strip above the session; the engine, popup rules and profile rules are unchanged (no change under `src/main/browser` at pv96).

**What the user does and what happens.** The user creates, switches and closes tabs in the pane. Each tab belongs to the workspace or worktree where it was opened. A link or `window.open` without window features opens a new tab in the pane. A window request with size or position (OAuth flows, for example) opens a child window whose origin bar shows the real destination. Local file URLs never open native child windows.

**Rules and edge cases.**
- Popups inherit the opener's profile and owner. A popup can never replace its opener's primary registration.
- A new-tab intent needs a fresh trusted click, and each click is consumed by one routed tab. A page cannot loop `window.open` on a single click.
- Popups share a budget per opener tree, so named child popups cannot mint new quotas.
- Closing the owning tab removes its popups' authority at once.
- If the renderer that owns the gesture disappears mid-click, the request is dropped. It never falls back to a native popup.
- Popup URLs that carry auth tokens are reduced to origin metadata before any handoff to the OS browser.
- Each worktree remembers one active tab. A command without a worktree uses the global tab. Text-changing commands never fall back to the global tab. Read-only listing does not change the active tab.
- Dead pages are skipped in lists. The active tab is recomputed when a queued command runs, because queue delay can change the tab list.

**Interface.** CLI and UI: create, list, show, current, switch and close (the `tab` group) and `open-url`. The core pushes page lifecycle and tab-list changes.

**Data persisted.** Tab records with their workspace (inferred).

**Depends on.** BRW-01. BRW-03 for profile inheritance.

### BRW-03 Browser profiles (isolated cookie sessions)

**What the user does and what happens.** Every tab uses one profile, and the default profile always exists. Users and agents can list, create, show, delete, set (move a tab to another profile), switch back to the default, and clone profiles (`tab profile` commands). Moving a tab reloads its view and returns only after the new view has registered.

**Rules and edge cases.**
- A profile is a storage partition with its own cookies and site storage. Partitions derive from the app's own profile, so each app profile gets its own default partition.
- Setting the profile a tab already uses is a no-op, so no view is torn down.
- Clients see display metadata only: no filesystem paths and no keychain identifiers.
- Deleting a profile cancels its pending passkey requests and retires its partition. A partition whose setup fails is retired so nothing is left behind.
- Readiness loops for a removed partition must stop before its storage is released.
- Clone semantics (what state is copied) were not verified.

**Interface.** The `tab profile` commands and the matching UI. Set and clone return only after the tab's new page is registered.

**Data persisted.** One directory per partition in app data. A metadata file in the app profile directory records the default profile's import source and identity notice.

**Depends on.** BRW-01. The first version needs only the default profile (see the proposal summary).

### BRW-04 Browser identity (user agent mode)

**What the user does and what happens.** The user chooses a clean identity (embedded-browser markers removed) or a native identity (Chromium's own user agent), in settings or with `browser identity get` and `set`. Status shows the applied mode, the configured mode, and whether a restart is needed. The new mode applies at next launch.

**Rules and edge cases.**
- The applied mode never changes while the process runs. Only the next launch takes the configured mode.
- One writer owns the process-wide record.
- Mobile emulation sets identity per tab. Identity follows the viewport that Chromium holds, so a phone identity never stands on a desktop viewport.
- The reference uses the debugger, not the plain user-agent setter, to change identity during a redirect, because the plain setter cancels the in-flight navigation.
- Google sign-in uses an adjusted identity, because Google blocks some embedded browsers.
- Detaching the debugger clears its overrides, so the next page must reapply its identity.

**Interface.** Settings UI and `browser identity get|set`.

**Data persisted.** A versioned identity record in the user data root.

**Depends on.** BRW-01, BRW-11.

**Proposal note.** Drop as a user setting. The setting exists because Electron leaks markers. A new engine can set one identity policy internally, which is an engine requirement (see the engine section).

### BRW-05 Import cookies from other installed browsers

**What the user does and what happens.** The user sees installed browsers (Google Chrome, Microsoft Edge, Arc, Brave, Comet, Helium and Firefox; Safari on macOS), picks a source profile, and imports its cookies into a chosen app profile. The result reports imported, skipped and partition-skipped counts with reasons.

**Rules and edge cases.**
- The import replaces cookies for the domains it imports rather than merging. Stale cookies for those domains are cleared first, under a per-partition lock.
- Different partitions import in parallel. A failed import releases its lock.
- Google source-bound cookies are never transplanted, and google.com is excluded. YouTube is deliberately not excluded, because it re-issues its session cookies.
- Chromium values are encrypted with OS-specific keys and format versions. Rows that cannot be decrypted are dropped and counted. Chrome and Edge 140+ on Windows use app-bound encryption, so their rows may be unreadable.
- If the live cookie store rejects some cookies, the import keeps a staged database and replays it on the next cold start. An older staged replay must never overwrite a later import.
- The source database is copied with retries against a running browser's write-ahead log. It is never opened in place.
- Firefox and Safari have their own readers. The Firefox reader selects only columns the schema has.
- Diagnostics go to the user data directory with a capped preview of payload text.

**Interface.** Settings import action and browser detection. The CLI `cookie` group does not import.

**Data persisted.** A temporary staged copy with plaintext cookie values, removed on every exit path. A bounded diagnostics log.

**Depends on.** BRW-03, BRW-06. This is not the new feature "import sessions from other agent tools".

### BRW-06 Read, set and clear cookies and site storage

**What the user does and what happens.** Agents read, set and delete cookies (`cookie get`, `set`, `delete`) and read, set and clear local or session storage (the `storage` group) in a tab's profile. The settings UI clears default-profile cookies while preserving some domains.

**Rules and edge cases.**
- Cookies are grouped by registrable domain. An IP address has no registrable domain and is handled as its own group.
- A rejected cookie write is reported in the reply, never swallowed.
- Removals for the same cookie coordinate are ordered. A rejected removal is restored before the error is reported.
- A clear snapshots its whole removal plan before removing anything.
- Preserve rules are enforced per coordinate, not by a later filter.

**Interface.** CLI `cookie` and `storage` groups and the UI clear action.

**Data persisted.** In the profile's partition.

**Depends on.** BRW-03. BRW-13 for agent access.

### BRW-07 Localhost preview of dev servers

**What the user does and what happens.** A dev server started in a workspace (by a terminal or an agent) appears as a candidate URL. The user opens it in a pane tab. Each worktree gets a stable host name of the form `<label>.orca.localhost`, so two worktrees that run the same port do not collide.

**Discovery.**
- Listening TCP ports are attributed to a workspace by the process working directory and command line.
- Ports 80, 3000, 3001, 4200, 5000, 5173, 5174, 8000, 8080 and 8888 are HTTP candidates. Ports 443 and 8443 are HTTPS candidates.
- URLs printed in terminal output are collected after terminal control sequences are stripped. Per-terminal buffers are bounded.
- Remote (SSH) workspaces add their advertised remote ports.
- Scans run in a worker with timeouts and backoff after failures.

**Proxy rules.**
- Serves HTTP and HTTPS targets and forwards WebSocket upgrades, which hot module reload needs.
- The same worktree and target keep their label. Colliding labels are made unique.
- A worktree's labels are removed when it is torn down. The shared server stays up for the session.

**Interface.** The core pushes port and advertised-URL changes. The pane opens the labelled URL.

**Data persisted.** In memory only, recomputed after restart (inferred).

**Depends on.** BRW-01, BRW-02, terminal output (terminal domain), workspace identity (workspace domain). BRW-19 for SSH targets.

### BRW-08 Find in page

**What the user does and what happens.** Cmd or Ctrl+F while a pane page has focus opens a find bar for that page. Matches update as the user types. Next and previous move through them. Closing the bar clears the selection.

**Rules and edge cases.**
- The page is a separate process, so the shortcut is forwarded from the page to the app.
- The find target carries a page id and, when known, a workspace id. Page ids are unique, so an unscoped target still resolves.
- Subscriptions are kept per page and per workspace. They must be removed when the pane closes. The reference counts them to catch leaks.

**Interface.** The renderer's find bar calls the engine's in-page find and stop. No CLI command.

**Data persisted.** None.

**Depends on.** BRW-01, BRW-17.

### BRW-09 Screenshots and PDF export

**What the user does and what happens.** Agents capture the visible viewport (`screenshot`), the full page (`full-screenshot`), or print the page to PDF (`pdf`). Results come back as base64 data or as a file path.

**Rules and edge cases.**
- Background pages produce empty captures unless throttling is disabled for them. The reference disables it.
- Screenshot payloads can exceed a default RPC buffer, so the transport must accept large base64 bodies.
- The automation tool's PDF path hangs in embedded views, so the reference prints through the engine's own call.
- The automation tool writes screenshots to a temporary file. The bridge reads it back and removes it.

**Interface.** CLI navigation group.

**Data persisted.** None. Temporary files are removed.

**Depends on.** BRW-01 (engine capture and print calls), BRW-13.

### BRW-10 DevTools window and debugger endpoint

**What the user does and what happens.** The user opens DevTools for a pane page as a detached window. The main window has its own toggle. Automation tools connect to a per-tab DevTools-protocol WebSocket proxy that presents the page as a target with synthetic sessions.

**Rules and edge cases.**
- An open DevTools window blocks debugger attach. The app reports that DevTools may be open instead of hanging.
- Offscreen pages do not open a visible DevTools window.
- The debugger attaches only when automation or a screencast needs it. Plain browsing tabs never attach, because the reference notes that Cloudflare treats the DevTools protocol as a bot signal.
- Detach clears every override the debugger holds, including viewport and identity. Code that changes one override must not detach.
- The proxy keeps focus-replay and synthetic-session state across command boundaries.

**Interface.** UI action to open DevTools. External tools connect to the proxy.

**Data persisted.** None.

**Depends on.** BRW-01. Required by BRW-12, BRW-13, BRW-14 and BRW-15.

### BRW-11 Viewport, device and environment emulation

**What the user does and what happens.** Agents and the pane set a viewport size, a device preset, mobile emulation (touch and a mobile identity), media preferences such as colour scheme, geolocation, offline mode, HTTP credentials, extra request headers, clipboard access, and dialog accept or dismiss. The commands are `viewport`, `set device`, `set media`, `geolocation`, `set offline`, `set credentials`, `set headers`, `clipboard` and `dialog`.

**Rules and edge cases.**
- Viewport overrides are serialized per tab, so the last requested state wins.
- A metrics-only resize can leave the compositor at the old size and crop screencast clients. The reference works around this.
- The automation tool's viewport command has no mobile flag. The bridge applies mobile emulation directly.
- Touch-point counts outside the engine's accepted range are rejected by Chromium. The reference keeps values in range.
- Overrides are gated on the direct page id, because popups have no device override of their own.
- A stale page clears all its per-tab entries.

**Interface.** CLI environment group.

**Data persisted.** None (inferred).

**Depends on.** BRW-10 (the reference uses the debugger for most of these), BRW-04.

### BRW-12 Live screencast of a page with remote input

**What the user does and what happens.** The pane shows a live image of the page. Remote clients, such as the mobile app, can subscribe to a page's frames and send pointer, wheel, key and text input back. The mobile method allowlist includes screencast, mouse, key, text, viewport and tab-creation methods.

**Rules and edge cases.**
- Frames are paced to each subscriber's minimum interval, read live.
- A page that never paints triggers a first-frame timeout, so frameless viewers learn quickly.
- The stream ends with an error when the page stops compositing.
- Dialogs the page raises are reported to the stream owner, who must settle them. Only the debugger session that reported a dialog may answer it.
- A subscription owned by a disconnected viewer is evicted. A reconnecting viewer keeps its subscription only within a retention window.
- A renderer throttle lease keeps frames flowing for a visible pane.

**Interface.** Subscribe and unsubscribe, input methods, and frames pushed by the core.

**Data persisted.** None.

**Depends on.** BRW-01, BRW-10. Remote clients are outside the first version.

### BRW-13 Agent browser automation commands

**What the user does and what happens.** An agent or the CLI drives a pane tab through about 80 command keys in the browser groups. They cover navigation (goto, back, forward, reload, eval, scroll, wait, pdf and screenshots), interaction (click, dblclick, fill, type, select, check, uncheck, focus, clear, select-all, keypress, hover, drag, upload, scrollintoview, get, is, inserttext, mouse move, down, up and wheel, find, download, highlight), tabs (open-url, tab list, show, current, switch, create, close, exec), profiles, cookies, capture (intercept, capture start and stop, console, network), environment, storage and identity.

**Mechanism.** An external automation daemon (agent-browser, pinned around 0.27) runs one named session per tab and attaches to the pane's DevTools proxy (BRW-10). The bridge queues commands per tab, checks that the tab still exists when the command runs, and maps failures to stable error codes.

**Rules and edge cases.**
- A stale element reference produces its own code, which tells the agent to take a new snapshot (BRW-14).
- Closed tabs, unknown references and timeouts have separate codes. The bridge enforces a per-command timeout on selector waits, which the external tool lacks.
- Text insertion goes through the browser's input pipeline. The external tool's direct insertion loses focus in embedded views.
- Each tab's daemon closes with its tab. Daemons left behind by a crash are swept at startup, one per open tab.
- A daemon retires itself when idle. The bridge notices the gap and recreates the session.
- A restarted daemon must receive the CDP owner on every invocation, or it launches its own browser.
- Passthrough arguments cannot override the selected page or the proxy target.
- Commands for a worktree never fall back to another worktree's tab for text changes.

**Interface.** CLI (about 80 keys). The core returns structured results: navigation returns the final URL and title, and interactions return the element acted on.

**Data persisted.** Interception patterns are held per session and restored after a process swap.

**Depends on.** BRW-01, BRW-02, BRW-03, BRW-06, BRW-09, BRW-10, BRW-11, BRW-14, BRW-15, BRW-16.

**Owner decision.** This is the largest feature in the domain. It is needed in the first version only if agents must verify web UIs.

### BRW-14 Element snapshot with refs for agents

**What the user does and what happens.** `snapshot` returns the page's interactive elements with short references. Later commands (click, fill, hover and others) use those references. Frames and shadow roots are included, and so are clickable elements that are not semantic controls.

**Rules and edge cases.**
- Each snapshot creates fresh references and bypasses the stale-reference guard.
- After a navigation or a re-render, old references are stale. Commands on them fail with a stale-element code that asks for a new snapshot.
- Click and fill resolve references directly, so iframe and unfocusable-field behaviour stays intact.
- Fill replaces values through the browser's own edit transaction, so rich editors update their model. Contenteditable targets follow a separate path.
- Select-all is emulated as focus plus the select-all key chord, because the external tool has no select-all.

**Interface.** CLI `snapshot`. References are returned to the agent.

**Data persisted.** In memory only.

**Depends on.** BRW-10.

### BRW-15 Request interception, network and console capture

**What the user does and what happens.** Agents enable interception rules that block or route requests matching patterns (`intercept enable|disable|list`), start and stop captures (`capture start|stop`), and read console and network logs (`console`, `network`).

**Rules and edge cases.**
- Each pattern is routed on its own.
- Interception patterns survive a daemon restart or a process swap, unless the caller's first command already reconfigured routing.
- A failed restore is best-effort and does not fail the user's command.
- Identity changes during redirects use the debugger rather than a plain setter, so the navigation is not cancelled.

**Interface.** CLI intercept, capture, console and network commands.

**Data persisted.** Capture buffers in memory (inferred). Interception patterns are held per session.

**Depends on.** BRW-10, BRW-13.

### BRW-16 File uploads and downloads

**What the user does and what happens.** Agents upload files into a page's file input (`upload`), and a page's own file chooser receives staged copies. Downloads from pages land in the OS downloads folder under a unique name. A download can be cancelled from the UI or by an agent (`download`). For remote workspaces, uploads are staged on the remote side and downloads are relayed back to it.

**Rules and edge cases.**
- Each download's save path is decided synchronously when the engine announces it. A failed path cancels the download and moves the UI to a terminal state.
- Duplicate names get a reserved unique name in the downloads folder.
- A staged upload stays alive as long as the page may open it, because the engine opens files lazily. It is removed when the page is released or recovered.
- Each staged file has its own directory, so the page sees the original base name. Separators and unsafe characters are removed.
- On Windows, antivirus or the engine can lock a staged file briefly, so deletes retry.
- A zero-byte download still sends a final empty chunk, so the remote side commits a file.
- A cancel and a stream failure can both abort the same transfer. Cleanup is idempotent.
- Document previews cannot start downloads. The user sees one notice per document.
- Staging directories left by an abnormal exit are swept at startup.

**Interface.** CLI `upload` and `download`, and the UI cancel action.

**Data persisted.** Temporary staging directories. The final file in the downloads folder.

**Depends on.** BRW-01, BRW-13. Remote relay exists only in the client-hosted model (BRW-20, proposed drop).

### BRW-17 Keyboard shortcuts, context menu and zoom in the pane

**What the user does and what happens.** App shortcuts keep working while a page has focus. Right-click shows a context menu for links, selections, images and editable fields (Open Link, Copy and similar). Ctrl or Cmd with plus and minus, and Ctrl with the wheel, zoom the page. A held tab switcher commits on key release.

**Rules and edge cases.**
- The page runs in a separate process, so the host intercepts key events before the page and forwards only allowlisted chords. Cmd or Ctrl plus Alt plus Arrow is the only allowlisted chord that carries Alt.
- A key-down must not be prevented, or the matching key-up is lost.
- Some layouts turn Ctrl or Cmd plus and minus into native zoom before the host sees them, so the host handles zoom explicitly.
- Copy is forwarded only when no text field or selection is active, so native copy still works in inputs.
- The context menu forwards the native selection, so Copy works on pages that block copy handlers.
- Right-click on a non-link does not offer "Open Link".
- Grab shortcuts (BRW-22) use the same path.

**Interface.** UI only.

**Data persisted.** None.

**Depends on.** BRW-01.

### BRW-18 Certificate prompts and passkeys

**What the user does and what happens.** A site with an invalid certificate shows a warning page. The reference lets the user trust one certificate for one endpoint for the session. A page's passkey (WebAuthn) request shows an account picker owned by the app. The user picks an account or cancels.

**Rules and edge cases.**
- Certificate decisions fail closed. The challenge is recorded before a decision is applied, and an error while recording still denies.
- The engine caches certificate decisions for the session. The reference therefore pins the trusted certificate at grant time and re-checks identity on later requests.
- A trust decision covers one endpoint, not the whole profile.
- Passkey requests are scoped to a profile and are cancelled when that profile is deleted.
- Passkey access is checked against the request origin before the picker appears.

**Interface.** UI warning page and account picker. No CLI.

**Data persisted.** Trust decisions are held by the controller for the session. Whether they persist is not verified.

**Depends on.** BRW-01, BRW-03.

**First version.** Fail closed with the warning and offer no trust decisions. Passkeys come later.

### BRW-19 Pane traffic through SSH or WSL hosts

**What the user does and what happens.** For a worktree on an SSH host or inside WSL, the pane's requests leave from that host. A dev server or intranet site reachable only there can then be opened. The user configures nothing per page.

**Rules and edge cases.**
- Each profile partition is proxied through a loopback SOCKS listener whose port stays stable for the partition's lifetime.
- Each connection resolves the execution route at dial time, under the target's current authority.
- While the target is disconnected or rotating, dials fail. The reference never falls back to a direct connection.
- A WSL relay starts on demand per distribution and can be aborted.
- The execution host's runtime identity and revision must match, or the route is refused.
- Memory per session is bounded by frame budgets.
- Partition identities include the probe intent, so a "try anyway" success cannot be reused silently.

**Interface.** None directly. The pane follows the worktree.

**Data persisted.** Partition identity and proxy configuration per profile.

**Depends on.** BRW-01, BRW-03, and the SSH and WSL execution hosts (workspace and remote domains). Plain port forwarding may replace this design; decide together with BRW-07.

### BRW-20 Client-hosted and headless browser pages

**What the user does and what happens.** When the core runs on a headless server or a remote runtime, the browser pages of its worktrees are hosted by a paired desktop client that owns the windows. The server issues commands and tracks page inventory. The client executes them and reports back. A headless core can also run pages with offscreen rendering.

**Rules and edge cases (reference).**
- Each host holds a lease with an authority and a generation.
- Commands are recorded in a ledger and replayed after reconnects.
- Pages are placed and reconciled onto valid hosts. Stale hosts are fenced out.
- Uploads and downloads cross a file channel. A download with no resolvable remote destination is cancelled.

**Assessment.** The model exists because the Electron core and remote runtimes split page hosting. The Rust core is a service that does not render pages, so this design should be replaced, not ported. Proposed drop. Reopen only if headless automation needs pages on a server without a desktop client.

**Interface.** Internal runtime protocol. Not a CLI feature.

**Data persisted.** Lease records and a command ledger (reference).

**Depends on.** BRW-12, BRW-16, remote pairing (client domain).

### BRW-21 Preview workspace files (documents, images, media)

- Source: pv96 (unmerged). `orca file open --preview` renders an HTML file in the browser pane, and the approved side-panel spec adds a New Markdown tab; the document sandbox code did not change (no change under `src/main/browser`).

**What the user does and what happens.** The user opens Markdown, images and PDFs from the repository in a preview. The user also plays audio and video files from the workspace. Audio types: mp3, wav, m4a, aac, ogg, oga, flac, opus. Video types: mp4, m4v, mov, webm.

**Documents.**
- Rendered in a sandboxed view bound to a grant for one document root. Closing the tab revokes the grant.
- The view has no outbound network: no fetches, no WebRTC and no socket connections. The reference states that previewed documents are agent-authored, so any outbound request is a possible exfiltration channel.
- The response carries a content security policy that the document cannot exceed. A reload re-reads the workspace file.
- Links are intercepted and reported to the host only from a live, grant-bound view. Subframes cannot change the grant.
- File reads are scoped to the worktree. For SSH workspaces, only images and PDFs are served as bytes, and fonts are refused.

**Media.**
- Served through a custom scheme that supports HTTP range requests, so seeking works.
- Grants belong to a renderer and disappear with it. Only the listed extensions are served.
- SSH-hosted media are read through the SSH file provider.

**Interface.** Opened from the file explorer. No CLI.

**Data persisted.** None. Grants are held in memory.

**Depends on.** BRW-01 (sandbox policies), workspace file access (workspace and SSH domains).

### BRW-22 Element grab for agent prompts (Design Mode)

**What the user does and what happens.** In Design Mode the user hovers over page elements (a highlight follows the pointer) and clicks one. The element's HTML, a summary of its styles and a cropped screenshot are attached to the agent's prompt. The README calls this Design Mode.

**Rules and edge cases.**
- The page payload is untrusted. The host re-validates every field, removes query strings from URLs, removes attributes that look like secrets, and re-checks URL schemes.
- The highlight overlay is removed before the screenshot, so the highlight is not captured.
- Crop rectangles are in physical pixels and are validated against the bitmap size.
- One grab operation per page. A main-frame navigation cancels it. A subframe navigation does not.
- Teardown mid-grab cancels the grab, and the renderer gets a signal instead of a hanging promise.
- A timeout clears stale grab state.

**Interface.** UI only.

**Data persisted.** None.

**Depends on.** BRW-01, BRW-09, BRW-17.

### BRW-23 Desktop computer use: inspect apps and UI

**What the user does and what happens.** Agents list apps and windows, read an app's state (a screenshot plus an accessibility tree with element indexes), and read the provider's capabilities (`computer capabilities`, `list-apps`, `list-windows`, `get-app-state`).

**Per-OS providers (reference).**
- macOS: a native helper application with a stable bundle identity, speaking JSON lines over a Unix socket. It needs the Accessibility and Screen Recording grants. The socket is owned from the helper's birth, and late errors from a replaced helper cannot close the new one.
- Windows: a PowerShell runtime that uses UI Automation and GDI screenshots through platform calls. A serve mode keeps one process alive. After repeated failures the app falls back to one process per operation for a cooldown.
- Linux: a Python adapter that uses AT-SPI accessibility through GObject introspection and GdkPixbuf screenshots.

**Shared rules.**
- A Node sidecar owns the public contract. The providers are interchangeable behind it.
- Snapshots are cached by element alias and serve only as hints for follow-up actions. Element frames and provider ids are not durable.
- Large screenshots can be downscaled to cap IPC size.
- Capability flags say which actions each provider supports.
- Requests reject promptly when a platform API hangs.

**Interface.** CLI `computer` group.

**Data persisted.** Snapshot cache in memory.

**Depends on.** BRW-25.

### BRW-24 Desktop computer use: act on apps

**What the user does and what happens.** Agents click, perform secondary actions, scroll, drag, type text, press keys and hotkeys, paste text and set values in an app (`click`, `perform-secondary-action`, `scroll`, `drag`, `type-text`, `press-key`, `hotkey`, `paste-text`, `set-value`).

**Rules and edge cases.**
- Actions refer to element indexes from the latest snapshot. App re-renders can reassign indexes, so an action re-checks the element.
- Pasted text is size-checked before it reaches the clipboard.
- Typing and keys go through the platform input path. The macOS helper has keyboard-safety rules. A separate helper reports the keyboard layout so key names map correctly (behaviour inferred from its name).
- Actions are checked against shared argument rules before a provider runs them.
- The macOS contract includes a terminate method for the helper.

**Interface.** CLI `computer` group.

**Data persisted.** None.

**Depends on.** BRW-23, BRW-25.

### BRW-25 Desktop computer use: permissions and diagnostics

**What the user does and what happens.** `computer permissions` reports what the current OS provider needs. On macOS the app can reset the Accessibility and Screen Recording grants of its helper to clear stale denials, and it reads status through the helper's identity. On Windows it checks the PowerShell execution policy. On Linux it checks AT-SPI availability.

**Rules and edge cases.**
- Permission status probes are not killed by the timeouts that apply to helper launches.
- macOS keeps privacy records after an uninstall, so an explicit reset path is needed.
- Unavailable providers return a specific message rather than a generic failure.
- Sidecar diagnostics strip raw paths.

**Interface.** CLI `computer permissions` and `computer capabilities`.

**Data persisted.** None. Grants live in the operating system.

**Depends on.** None.

### BRW-26 Emulator device control (Android AVDs and iOS simulators)

- Source: pv96 (unmerged). The side panel offers a New Mobile Emulator tab per the approved spec (marked done in its record), so emulator tabs can sit beside terminals and browser tabs; the device commands and backends did not change.

**What the user does and what happens.** Agents list emulators and devices, attach to one, boot one, install and launch apps, grant permissions, run shell commands, read logcat, and shut down or kill a device (`emulator list`, `devices`, `attach`, `install`, `launch`, `permissions`, `exec`, `logcat`, `kill`, `shutdown`).

**Android.**
- Discovered through the Android SDK tools: adb, the emulator binary and the AVD manager.
- Running devices and AVDs are listed together. With none present, the message tells the user to create one in Android Studio.
- A start lease prevents two boots of the same device (inferred from the file name).

**iOS (macOS only).**
- Simulators are listed through simctl. Setup problems from the developer tools are reported as plain text.
- The stream helper needs Simulator.app visible for display and rotation, so the app hides it while it embeds the stream.

**Rules.** The backend registry is always populated, so explicit targets resolve. The default backend depends on host support. Error codes are stable for agents.

**Interface.** CLI `emulator` group.

**Data persisted.** None kept.

**Depends on.** Android SDK and Xcode tooling (external).
- Owner direction for the new product (2026-11-12, design rows D-SH-17 and D-SH-18): the Work panel offers a Mobile emulator tab as one of its four actions. Its details are assumed in the design (A37): a device frame showing a mobile app or mobile web view from a URL or the workspace's dev server. This inventory assumes the first version shows a mobile web view in a device frame through the browser pane (BRW-01, BRW-07); full emulator control (this feature) and screen streaming (BRW-27) stay later.

### BRW-27 Emulator screen streaming and input

**What the user does and what happens.** The app shows a live device image. Android frames arrive as H.264 video from a scrcpy server that is pushed to the device. iOS frames come from an external helper that also streams accessibility data. Agents and users tap, gesture, type, press hardware buttons, rotate, and read the accessibility tree (`emulator tap`, `gesture`, `type`, `button`, `rotate`, `ax`).

**Rules and edge cases.**
- Android input goes through the device shell's input command, not through the scrcpy control socket.
- The iOS helper reports taps in normalized coordinates (0 to 1, top-left origin). The app maps them to device pixels.
- The scrcpy server jar is downloaded, checked, and deployed per device.
- Stream sessions are registered per device and stopped on exit.
- Helper processes run under node. A helper binary from before an update is still reaped.

**Interface.** CLI `emulator` tap, gesture, type, button, rotate and ax commands. The UI stream.

**Data persisted.** The downloaded scrcpy server jar and helper binaries, in app data.

**Depends on.** BRW-26.

### BRW-28 Voice dictation

**What the user does and what happens.** The user starts dictation in the app, speaks, and the transcript is typed into the focused field. The user stops dictation explicitly. With streaming models, text arrives incrementally. With offline models, it arrives after chunked decoding.

**Rules and edge cases.**
- A dictation session belongs to the window that started it. Another window cannot stop it.
- Stop waits for in-flight decoding, and that wait never blocks forever.
- A recogniser failure in one window does not disturb other windows.
- Audio is resampled to the model's sample rate.
- Long captures are split into bounded chunks at points chosen to avoid cutting words, so decoding tensors stay small.
- Recognition runs in a worker process with the native sherpa-onnx runtime. The WebAssembly build cannot read host files, so the reference does not use it.
- Warm workers are reused, but each offline stream is single-use.

**Interface.** Renderer IPC to start and stop dictation. No CLI.

**Data persisted.** None kept. Transcripts go into the focused field.

**Depends on.** BRW-29 (local models) or BRW-30 (cloud).

### BRW-29 Local speech models

**What the user does and what happens.** The user picks a speech model from a catalogue, downloads it with progress, cancels, deletes, and sees each model's state. The reference catalogue includes, among others, Parakeet TDT v3 and v2, Zipformer bilingual (Chinese and English), Paraformer bilingual, streaming Zipformer for English, Chinese and Korean, Parakeet TDT-CTC for Japanese, Whisper Tiny and SenseVoice.

**Rules and edge cases.**
- Revisions are pinned and every file has a hash. A direct download is equivalent to the pinned archive.
- Downloads resume with range requests across dropped connections. Completion is checked by hash, never by byte count.
- A server that restarts a range from byte zero overwrites the partial file. A partial file left by a crashed process is not resumed.
- A download gives up after a no-progress limit. A Retry-After wait is capped, and longer waits are shown for a manual retry.
- Model files move with the cache. Non-ASCII Windows paths get a safe copy with an atomic rename. A pre-migration copy is removed so the model does not come back.
- Downloads use the app's network proxy settings.

**Interface.** Renderer IPC for the catalogue, model states, download, cancel and delete.

**Data persisted.** Model files in the app cache. The reference notes that the large models are about 1 GB.

**Depends on.** None.

### BRW-30 Cloud transcription with an OpenAI key

**What the user does and what happens.** The user saves an OpenAI API key in settings. The app shows whether a key exists and can clear it. Dictation can then send audio to OpenAI's transcription endpoint with a gpt-4o transcription model (mini or full).

**Rules and edge cases.**
- Key-status checks report presence only. The key is not shown back.
- The key lives in its own store, not in workspace data.
- The transcription request sends the audio as a multipart file.
- No consent notice was found in the files read (the renderer was not read). Check this before reuse.

**Interface.** Renderer IPC to save, clear and read key status.

**Data persisted.** The API key in a separate store. Encryption at rest was not verified.

**Depends on.** BRW-28.

## Web engine requirements (input for the GPUI embedding decision)

The reference pane is built on Electron webview guests (one process per page) and Chromium APIs. The list below states what the pane needs from an engine, by feature. Items 1 to 10 are needed for the first version (BRW-01, BRW-02, BRW-07). Items 11 to 24 are needed later.

### Needed in the first version

1. Several embedded views inside one GPUI window, with focus, resize, show and hide, and moving a view between panes without losing its page.
2. Load, back, forward, reload and stop, with navigation events: start, commit, title and URL change, and failure with an error code and text. The host must tell an aborted or superseded navigation apart from a failure.
3. A navigation-policy hook before commit: deny by scheme, distinguish a user link, a popup and a redirect, block remote-to-file redirects, and allow the blob URLs that challenge pages need.
4. Named storage partitions with persistent cookies, local and session storage, and cache on disk, plus a supported way to clear one partition.
5. An event for window-open and target-blank requests, with their window features, so the host decides between a new tab and a child window in the same partition.
6. A certificate-error callback that can deny by default and show an error page.
7. A download event that receives the save path synchronously, with progress and cancel. The first version can use the engine's default folder or deny downloads.
8. A per-view user agent setting with a stable default. The user agent must agree with the client-hint headers the engine sends.
9. Survival of a crashed or swapped renderer process, while the host keeps the page's identity.
10. Key events visible to the host before the page sees them, or a supported way for the page to forward shortcuts. Focus inside a view must not swallow app shortcuts.

### Needed later

11. A DevTools-compatible protocol with attach and detach for a page target. It must cover mouse, key and text input, device metrics, touch, user agent, media, geolocation and offline emulation, capture and print, cookie and request events, request interception and modification, script evaluation, the DOM and accessibility tree, and screencast frames with acknowledgements. Attaching a debugger must not mark ordinary browsing, because bot protection treats protocol attach as a signal.
12. Windowless rendering for headless or background use, with frame delivery for remote streaming.
13. Find in page with match counting, next, previous and clear.
14. Screenshots (viewport, full page, clipped regions) with physical-pixel handling, and PDF printing.
15. Per-view zoom. Context-menu events with link URL, selection, editable state and image source. Clipboard read and write under host control.
16. Per-partition proxy settings, including SOCKS5 to a loopback address, with no silent fallback to a direct connection.
17. Per-request header injection, HTTP authentication callbacks, and request interception for agents.
18. Permission prompts (camera, microphone, geolocation, notifications) decided by the host. The ability to disable WebRTC for previews.
19. A passkey (WebAuthn) hook that passes requests to a host-owned account picker and returns the response.
20. A cookie store API that accepts raw values and partition keys, reports rejected writes, and allows writing the store before the engine starts (for cold-start replay of imported cookies).
21. A custom URL scheme with HTTP range support and per-view grants. Response policy headers that page scripts cannot exceed.
22. Media playback for the reference's file types (mp3, wav, m4a, aac, ogg, opus, flac, mp4, m4v, mov, webm). Codec coverage for AAC and H.264 (m4a, mp4, mov) depends on the engine build.
23. Dialog events (alert, confirm, prompt, beforeunload) that the host can accept or dismiss, and a file-chooser event that the host can answer with files.
24. Consistent client hints when the user agent is overridden. The reference notes that an incomplete override breaks them.

### Constraints that follow from the reference

- Items 11 to 20 are shaped by Chromium. A non-Chromium engine would need its own automation protocol and its own cookie-import mapping (BRW-05). This is an inference from the reference design, not a tested finding.
- The reference pins Electron 43.7.5, which bundles Chromium. The engine's packaging size and update cadence are part of the choice.
- The reference's cold-start cookie replay (BRW-05) depends on engine internals. A new engine may not expose the same hooks.
