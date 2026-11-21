# Protocol surface: what clients ask and what the server streams back

Reference: pv96, commit d3c6311a531 (custom/integration plus 852 unmerged commits)

Companion to `server-and-pairing.md` (SRV). It sizes the request and event surface a new core would need to serve the desktop UI, the local CLI, phones, paired CLIs and remote desktops. Behaviour and counts only; no feature rows. Reference repository, read-only: `D:\projects\massorca\.claude\worktrees\unified-custom-profile-build-547ea6`, checked with `git grep`, `git show`, `git ls-tree` and `git diff` against commit `d3c6311a531` only (the working tree was not used). Paths are relative to its root.

Figures are for pv96 (`d3c6311a531`, unmerged). The custom/integration column (`f23a48ca8ca`) is recounted with the same method; it reproduces the earlier figures, except where a correction is noted.

The reference has three separate surfaces that are easy to confuse:

1. **Electron IPC**: the desktop renderer calls the main process over IPC channels. This is internal to the desktop app and is not reachable by phones or CLIs.
2. **Runtime RPC**: JSON requests with a method name, sent to the core over the local channel, the network WebSocket or the SSH bridge. Desktop, CLI, paired clients, remote desktops and phones all use it. Phones see only an allowlisted subset.
3. **Relay control protocol**: messages between a host and the cloud relay cell that lets phones reach the host when there is no LAN route. Local clients never see it directly.

## How the numbers were counted

All counts are grep or git-grep counts over non-test files at the two commits. Test files, UI-only behaviour and generated code are excluded.

| Figure | pv96 | Custom/integration | Method |
|---|---|---|---|
| RPC registrations | 782 | 699 | `defineMethod(` (765) plus `defineStreamingMethod(` (17) calls under `src/main/runtime/rpc/methods/` (347 files; 316 at custom/integration), including its `orchestration/` and `terminal/` subfolders. The permission field occurs once per registration. |
| Literal-named registrations | 755 (753 distinct names) | 694 (692 distinct) | Dotted names matched with `name: '<domain>.<action>'`. `markdown.readTab` and `markdown.saveTab` are each registered twice at both commits, so distinct names are fewer than registrations. |
| Constant-named registrations | 27 | 5 | `name: CONSTANT`, with the value taken from `src/shared/rpc-contract/`. pv96 adds 21 marketplace constants and `SKILL_PACKAGE_VERSION_INSTALL_METHOD`. |
| Per-domain counts | see table | see table | Name prefix (first dotted segment) mapped by hand to the domain rows below. Bot teams, prototypes, marketplace and Jev decisions now have their own rows. |
| Phone allowlist | 373 | 320 | pv96: 294 literal entries in `src/main/runtime/runtime-rpc/runtime-rpc-mobile-method-allowlist.ts`, plus the spread modules `runtime-rpc-mobile-agent-session-methods.ts` (40) and `runtime-rpc-mobile-bot-team-methods.ts` (39, new at pv96). Custom/integration: 289 literal entries plus 31. No duplicate names in either set. |
| Streaming names | 17 (17 registrations) | 17 (17 registrations) | Names of `defineStreamingMethod` registrations. The earlier figure of 15 left out `terminal.subscribe` and `terminal.multiplex`, which are streaming registrations at both commits. |
| Permission classes | workspace 679, accounts-admin 44, host-admin 24, desktop-control 13, skills-admin 11, settings-write 6, pairing-admin 5 | workspace 625, accounts-admin 20, host-admin 21, desktop-control 13, skills-admin 10, settings-write 5, pairing-admin 5 | Count of `permission:` values. |
| Electron invoke channels | 837 (848 call sites) | 790 (801) | Distinct string literals in `ipcRenderer.invoke(` under `src/preload/`. Preload literals are used because main-side handler names are often constants. |
| Electron listen channels | 187 (199 call sites) | 185 (195) | Distinct literals in `ipcRenderer.on(` under `src/preload/`. `ipcRenderer.send(` has 55 call sites (54) and 52 distinct literals at both commits. |
| Main-side IPC | 827 handle and 61 `on` call sites; 420 handle literals on the call line | 780 and 61; 376 | `ipcMain.handle(` and `ipcMain.on(` under `src/main/`. Only literals on the same line as the call are reported as names. |
| Main-side push | 98 channels (141 call-site lines) | 99 (141) | Distinct literals in `webContents.send(` under `src/main/`. The earlier "216 call sites" could not be reproduced: 141 lines match at both commits. |
| Relay messages | 9 out, 11 in | 9 out, 11 in | Message type literals in `relay-control-requests.ts` and `relay-control-protocol.ts`. Both files are identical at the two commits. |
| Client event kinds, terminal opcodes | 9 kinds, 18 opcodes | 9 kinds, 18 opcodes | Unchanged: `runtime-client-events.ts` and `terminal-stream-protocol.ts` are identical at the two commits. Carried over, not recounted. |

Known gap: the handler bodies under `src/main/ipc/` and `src/preload/api/` were not read. Those trees were counted through channel literals only. They hold 680 and 155 non-test files at pv96 (675 and 147 at custom/integration; the earlier figure of 472 for `src/main/ipc/` could not be reproduced).

## Totals

| Surface | Who calls it | pv96 | Custom/integration | Notes |
|---|---|---|---|---|
| Electron invoke channels | Desktop renderer only | 837 | 790 (+47) | Request/response. Grouped by prefix below. |
| Electron listen channels | Desktop renderer only | 187 | 185 (+2) | Main to renderer push. |
| Electron push channels (main side) | Desktop main to renderer | 98 literal names | 99 (-1) | Some are constants; count is a floor. |
| Runtime RPC registrations | Desktop, local CLI, paired CLI or web, remote desktop, SSH bridge, phone (subset) | 782 (753 distinct literal names, plus 27 constant-named) | 699 (692 distinct literal names, plus 5 constant-named) (+83) | Permission classes: workspace 679 (625), host-admin 24 (21), accounts-admin 44 (20), desktop-control 13 (13), skills-admin 11 (10), settings-write 6 (5), pairing-admin 5 (5). |
| Phone-reachable RPC methods | Paired phone | 373 | 320 (+53) | Allowlist, not permission classes (see SRV-09 in `server-and-pairing.md`). The +53: bot teams +29, marketplace +8, prototypes +7, delegation +4, agent session checklist +2, and three existing methods that are now allowlisted (`files.pathsExist`, `files.writeBase64`, `repo.initGit`). |
| RPC streaming methods | Any authenticated client allowed the method | 17 names (17 registrations) | 17 names (the earlier text said 15) | Each opens a stream that the server keeps pushing into until unsubscribed or the socket closes. The name set is unchanged. |
| Runtime client event kinds | Paired clients via `runtime.clientEvents.subscribe` | 9 kinds plus `ready` and `end` | Same | Desktop-to-phone state broadcasts. |
| Relay control messages | Host and relay cell | 9 host-to-relay, 11 relay-to-host | Same | Strict schemas; new fields need a compatible deployment order. |
| Relay RPC methods | Local clients to the host runtime | 2 (`pairing.getEndpoints`, `pairing.provisionRelay`) | Same | Both declared `pairing-admin`. |
| Terminal stream opcodes | Clients attached to terminal streams | 18 | Same | Binary frames, negotiated per stream. |
| JSON-lines agent-provider protocol | Core to agent provider processes | not a client surface | Same | Internal. Listed only so it is not confused with the client protocol. |

## Runtime RPC by domain

Counts are for the domain groups used in the feature inventory. "Phone" is the number of distinct allowlisted methods in that domain. The custom/integration column uses the same name-to-domain mapping. Bot teams, prototypes, marketplace and Jev decisions were inside larger rows in the earlier version and are now separate rows.

| Domain | Registered (pv96) | Custom/integration | Delta | Phone (pv96) | Phone (custom/integration) | Streaming names (pv96) | Representative methods |
|---|---|---|---|---|---|---|---|
| Workspaces (worktrees, repos, folder workspaces, project groups, host setups, ports, projects) | 59 | 59 | 0 | 23 | 22 | 0 | `worktree.list`, `worktree.create`, `worktree.rm`, `worktree.activate`, `repo.add`, `repo.clone`, `repo.initGit`, `folderWorkspace.create`, `projectGroup.importNested`, `project.list`, `workspacePorts.scan` |
| Terminals and session tabs | 51 | 51 | 0 | 34 | 34 | 4 (`session.tabs.subscribe`, `session.tabs.subscribeAll`, `terminal.subscribe`, `terminal.multiplex`) | `terminal.create`, `terminal.send`, `terminal.read`, `terminal.multiplex`, `terminal.resize*`, `session.tabs.*` |
| Agents, sessions, orchestration, history (not bot teams or prototypes) | 117 | 104 | +13 | 45 | 39 | 4 (`agentSession.subscribe`, `agentSession.subscribeStatus`, `agentSession.subscribeTurnCompletions`, `nativeChat.subscribe`) | `agentSession.create`, `agentSession.send`, `agentSession.cancel`, `agentSession.respondToApproval`, `agentSession.history`, `agentSessionChecklist.read`, `orchestration.*` (51), `delegation.bringBack*` (4), `chat.results`, `aiVault.searchSessions` |
| Bot teams | 43 | 10 | +33 | 39 | 10 | 0 | `botTeams.list`, `botTeams.create`, `botTeams.startTask`, `botTeams.listRoom`, `botTeams.listRoutines`, `botTeams.versions` |
| Prototypes | 12 | 5 | +7 | 12 | 5 | 0 | `prototypes.createRequest`, `prototypes.list`, `prototypes.readFile`, `prototypes.restoreVersion`, `prototypes.readDesignSystem` |
| Git (local source control) | 35 | 35 | 0 | 29 | 29 | 0 | `git.status`, `git.diff`, `git.stage`, `git.commit`, `git.push`, `git.pull`, `git.checkout`, `git.rebaseFromBase`, `git.generateCommitMessage` |
| Files and documents | 31 | 31 | 0 | 18 | 16 | 1 (`files.watch`) | `files.read`, `files.write`, `files.readDir`, `files.search`, `files.watch`, `files.unwatch`, `files.writeBase64Chunk`, `files.pathsExist` |
| Hosted review and work items (GitHub, GitLab, Linear, Jira, hosted review) | 149 | 147 | +2 | 86 | 86 | 2 (`jira.getIssueStream`, `jira.issueCommentsStream`) | `github.*` (59), `gitlab.*` (21), `linear.*` (41), `jira.*` (24), `hostedReview.*` (4) |
| Browser pane | 91 | 91 | 0 | 17 | 17 | 2 (`browser.screencast`, `browser.clientHost.attach`) | `browser.tabCreate`, `browser.goto`, `browser.click`, `browser.screenshot`, `browser.profileCreate`, `browser.network*` |
| Emulator | 19 | 19 | 0 | 0 | 0 | 0 | `emulator.*` (not read) |
| Computer use (desktop control) | 15 | 15 | 0 | 0 | 0 | 0 | `computer.*`; requires the `desktop-control` grant |
| Accounts (agent and integration accounts) | 19 | 19 | 0 | 7 | 7 | 1 (`accounts.subscribe`) | `accounts.list`, `accounts.selectClaude`, `accounts.selectCodex`, `accounts.consumeCodexResetCredit` |
| Marketplace | 21 | 0 | +21 | 8 | 0 | 0 | `marketplace.apps.list`, `marketplace.apps.connect`, `marketplace.vault.set`, `marketplace.program.install`, `marketplace.service.connect`, `marketplace.catalog.audit` |
| Skills and plugins | 22 | 21 | +1 | 0 | 0 | 0 | `skills.*` (16), `plugins.*` (6) |
| Automations | 7 | 7 | 0 | 0 | 0 | 0 | `automation.*` |
| Settings and UI state | 11 | 9 | +2 | 8 | 8 | 0 | `settings.get`, `settings.update`, `ui.get`, `ui.set`, `appearance.getTheme`, `appearance.setTheme` |
| Jev decisions | 4 | 0 | +4 | 0 | 0 | 0 | `jevDecisions.getStatus`, `jevDecisions.setEnabled`, `jevDecisions.saveApiKey`, `jevDecisions.clearApiKey` |
| Status, notifications, diagnostics, client events | 12 | 12 | 0 | 12 | 12 | 2 (`runtime.clientEvents.subscribe`, `notifications.subscribe`) | `status.get`, `stats.summary`, `diagnostics.*`, `runtime.clientCapabilities.update` |
| Remote hosts, server, updates, pairing | 30 | 30 | 0 | 12 | 12 | 1 (`network.browserTunnel`) | `ssh.connect`, `ssh.listTargets`, `host.platform`, `host.wsl.listDistros`, `managedServer.update`, `managedServer.rollback`, `updater.check`, `updater.install`, `orcad.*` (6), `pairing.getEndpoints`, `pairing.provisionRelay` |
| Speech, clipboard, artifacts, preflight, markdown | 31 | 31 | 0 | 20 | 20 | 0 | `speech.dictation.start`, `speech.models.download`, `clipboard.*`, `artifacts.publish`, `preflight.*`, `markdown.*` |
| Mobile web bundle (named by constant) | 3 | 3 | 0 | 3 | 3 | 0 | `mobileWeb.bundle.manifest`, `mobileWeb.bundle.chunk`, `mobileWeb.bundle.range` |
| **Total** | **782** | **699** | **+83** | **373** | **320** | **17** | |

The registered column sums to 782 (699 at custom/integration); the phone column sums to 373 (320) over distinct names. The markdown duplicates are counted in the registered column only.

What the phone cannot reach: emulator, computer use, skills and plugins, automations, and most of workspace ports. The phone reaches hosted review and work items (86 methods) more than any other domain; agents and sessions (45) and bot teams (39) follow. Among marketplace methods, the phone reaches only the app and vault calls (8); program install and service calls are not allowlisted.

## Electron IPC by prefix (desktop renderer only)

Channel prefixes, counted from the 837 distinct invoke literals in `src/preload/` at pv96 (790 at custom/integration). Top 25 prefixes by pv96 count; the rest are grouped.

| Prefix | pv96 | Custom/integration | Delta |
|---|---|---|---|
| `gh` | 71 | 64 | +7 |
| `git` | 37 | 37 | 0 |
| `fs` | 35 | 35 | 0 |
| `browser` | 31 | 31 | 0 |
| `pty` | 31 | 31 | 0 |
| `runtimeEnvironments` | 29 | 29 | 0 |
| `gitlab` | 28 | 28 | 0 |
| `skills` | 28 | 28 | 0 |
| `jira` | 25 | 25 | 0 |
| `linear` | 24 | 24 | 0 |
| `repos` | 22 | 22 | 0 |
| `ssh` | 22 | 22 | 0 |
| `worktrees` | 22 | 22 | 0 |
| `app` | 21 | 21 | 0 |
| `plugins` | 20 | 20 | 0 |
| `marketplaceApps` | 19 | 0 | +19 |
| `orcaProfiles` | 16 | 16 | 0 |
| `shell` | 15 | 15 | 0 |
| `mobile` | 13 | 13 | 0 |
| `clipboard` | 12 | 12 | 0 |
| `ephemeralVm` | 12 | 12 | 0 |
| `aiVault` | 11 | 11 | 0 |
| `runtime` | 11 | 11 | 0 |
| `automations` | 10 | 10 | 0 |
| `codexAccounts` | 10 | 10 | 0 |
| other (65 groups at pv96; 59 at custom/integration) | 262 | 241 | +21 |

New prefixes at pv96 besides those above: `marketplaceEngramOps` (6), `marketplaceCustom` (4), `marketplaceInstall` (3), `marketplaceServices` (3), `marketplace` (2) and `jevDecisions` (3). The listen side adds `gh:signInEvent` and `marketplaceCustom:installEvent`. The push side removes `ui:mobileMarkdownRequest`.

Notes on the desktop-only surface that mirror the runtime surface: `mobile:*` (13 channels) is the desktop Mobile screen for pairing, devices and network interfaces; `runtime:*` (11) and `runtimeEnvironments:*` (29) are the desktop's view of its own runtime and of saved remote environments, including managed server actions. They forward to the runtime RPC; they are not a second protocol.

## Relay control protocol (host to relay cell)

Unchanged at pv96: the relay source files are identical at the two commits.

Host to relay (9 types): `host-hello` (sent in the opening hello, with capabilities carried in upgrade headers because deployed cells refuse unknown hello keys), `host-challenge-ack`, `pong`, `auth-refresh`, `invite-create`, `device-credential-install`, `device-credential-install-status`, `device-revoke`, `device-resume-confirm`.

Relay to host (11 types): `host-challenge`, `host-hello-ack`, `conn-open`, `drain`, `ping`, `invite-created`, `device-revoked`, `device-credential-installed`, `device-credential-install-status-result`, `device-resume-confirmed`, `control-error`.

Behavioural notes: most relay-to-host messages are parsed with strict schemas, so unknown keys are refused. Connection tickets, challenge ids and resume secrets are opaque strings with length and encoding limits. The host-hello acknowledgement carries at most 8 active connection ids, and the relay keeps a phone waiting 10 s for the host's data socket to attach.

## Transport shapes (behaviour only)

Transport details and limits below were not re-read for pv96 and are carried over from custom/integration.

**Local channel (same machine).** One JSON request per message, a reply per request, runtime id in the envelope metadata. Owner token required. Framing details were not read.

**Network channel (phones, paired CLIs, remote desktops).** WebSocket. Before authentication, messages are an encrypted-session handshake (SRV-10, SRV-11). After authentication, every JSON message is an encrypted text payload, and terminal data travels as encrypted binary payloads. Inbound WebSocket messages are capped at 1 MiB. Outbound JSON replies have a size limit enforced before encryption; an oversize reply closes the socket rather than truncating it.

**Request/response envelope.** Success: `{id, ok: true, result, _meta: {runtimeId}}`, with `streaming: true` on stream-opening replies. Failure: `{id, ok: false, error: {code, message, data?}, _meta}`. Codes seen in the dispatcher: `bad_request` (6 sites), `unauthorized` (5), `forbidden` (20 sites), `method_not_found` (4), `timeout` (3), `runtime_busy`, `request_too_large`, `internal_error`, plus `runtime_error` as the shared fallback classifier. Unknown envelope fields are stripped on read, so new fields do not break old clients. Some pv96 additions (prototypes, chat results) are additive: an older host answers `method_not_found`, which clients read as "unsupported".

**Subscriptions.** A subscribe call returns a subscription id, then the server pushes messages that carry it: an initial `ready` (with a snapshot where the stream has one), zero or more updates, and an `end`. Unsubscribe is by id. Subscriptions belong to the socket and are removed when it closes. After a reconnect the client replays its subscriptions (SRV-17).

**Terminal binary stream.** A 16-byte frame header with a fixed kind byte (0x74), a version (1), an opcode, a stream id and a sequence number. Opcodes:

| Code | Opcode | Purpose (direction where the source states it) |
|---|---|---|
| 1 | Output | terminal output bytes, host to client |
| 2-4 | SnapshotStart, SnapshotChunk, SnapshotEnd | scrollback snapshot in chunks |
| 5 | Resized | size change notification |
| 6 | Error | error notification |
| 7 | Input | keystrokes and pastes, client to host |
| 8 | Resize | size request kept for compatibility, client to host |
| 9 | Subscribe | client to host |
| 10 | Unsubscribe | client to host |
| 11 | SnapshotRequest | client to host |
| 12 | Metadata | terminal metadata |
| 13 | Ack | flow-control acknowledgement, client to host |
| 14 | ClaimViewport | viewport ownership claim |
| 15 | OutputSpan | output with source ranges, host to client |
| 16 | SetOutputPaused | pause output for a stream |
| 17 | WriteUnavailable | input cannot be written right now, host to client |
| 18 | InputAck | host to client; the payload is `applied`, `delivery-unknown` or `resend` |

Opcodes 13-18 are negotiated per stream by capability, because older clients reject unknown opcodes. Input acknowledgement has three outcomes; `delivery-unknown` tells the client not to resend that input. Output is cut into 48 KiB chunks, and multiple terminals on one connection share output through a round-robin scheduler.

**Backpressure.** Each authenticated socket has an outbound memory budget and a `canSend` check. A reply that would exceed the size limit, or a send queue that overflows, closes the socket with a reason (`size` or `queue`). Clients can pause a terminal's output (opcode 16) and acknowledge bytes (opcode 13). Long-poll requests are classified and released when their client disconnects.

**Keepalive and limits.** Pre-authentication timeout 10 s; ping/pong every 15 s; 128 concurrent WebSocket connections; 256 TCP connections; 8 MiB JSON limit on terminal stream JSON payloads. Long-running requests are released when their socket closes.

**Agent-provider protocol (not a client surface).** Newline-delimited JSON over the stdio of agent provider processes (`src/main/jsonl-rpc/`). Limits: 8 MiB per line, 32 MiB queued writes, 128 pending requests, 30 s request timeout. It is listed only so it is not counted as a client protocol.

## New method groups in pv96

Each group below exists only at pv96. Counts are registrations; "phone" is the number of those that are on the phone allowlist. No method was removed, and no existing method changed its permission class or streaming flag.

### Bot teams: +33 registrations, +29 phone

Bot teams, projects, project files and workspace links, routines and scheduled runs, webhooks, team memory, Engram memory listing, connected apps, room messages and task assignment. The ten existing bot-team phone methods moved from the main allowlist into `runtime-rpc-mobile-bot-team-methods.ts`, so none was lost. Not on the phone: `botTeams.versions`, `botTeams.fileVersions`, `botTeams.versionFile`, `botTeams.restoreFileVersion`. Permission: all `workspace`. Behaviour of the teams themselves is in `bot-teams.md`.

Source: pv96 (unmerged). Phone reach for bot teams grows from 10 to 39 methods, and team files gain a version history.

### Marketplace: +21 registrations, +8 phone

`marketplace.apps.*` (8: list, connect, disconnect, setOpenMetadataToken, addOpenMetadataProfile, removeOpenMetadataProfile, setEngramServer, setAccessPolicy), `marketplace.vault.*` (5: status, set, setProfile, clear, clearProfile), `marketplace.service.*` (3), `marketplace.program.*` (4: status, install, reinstall, uninstall) and `marketplace.catalog.audit` (1). Permission: accounts-admin for 18, host-admin for the three install, reinstall and uninstall calls. Phone: the five apps calls (list, connect, setOpenMetadataToken, setEngramServer, setAccessPolicy) and the three vault calls (status, set, clear).

Source: pv96 (unmerged). Adds a marketplace of integration apps, host programs and hosted-service connections, with a credential vault and a catalog audit.

### Prototypes: +7 registrations, +7 phone

`prototypes.archive`, `prototypes.unarchive`, `prototypes.delete`, `prototypes.restoreVersion`, `prototypes.listDesignSystemVersions`, `prototypes.readDesignSystemVersion`, `prototypes.restoreDesignSystemVersion`. All `workspace` and all on the phone. The earlier five prototype methods are unchanged.

Source: pv96 (unmerged). Prototype requests gain archive, delete and design-system version history.

### Delegation and orchestration: +4 phone, +6 not phone

Delegation: `delegation.bringBack`, `delegation.bringBackStatus`, `delegation.bringBackAbort`, `delegation.bringBackResolve` (4, all `workspace`, all on the phone). Orchestration: `orchestration.askUser`, `orchestration.botTeamDelegate`, `orchestration.botTeamPost`, `orchestration.botTeamRemember`, `orchestration.botTeamTaskCreate`, `orchestration.botTeamTasks` (6, `workspace`, not on the phone). `orchestration.*` now has 51 registrations.

Delegation covers bringing a delegated child session back: read its status, bring it back, abort the bring-back, or resolve it with an agent (`getChildBringBackService`).

Source: pv96 (unmerged). Child sessions gain a bring-back flow, and a chat can post a question card to its own chat through the orchestration API.

### Chat results: +1 registration, not phone

`chat.results`, `workspace`: the environment URLs and changed files that a chat's Results card lists, read by the chat's host from its own journal. Additive; an older host answers `method_not_found`.

Source: pv96 (unmerged). Lists result links and changed files per chat. This is the method behind the Results card; no new summary method was added in pv96.

### Agent session checklist: +2 registrations, +2 phone

`agentSessionChecklist.read`, `agentSessionChecklist.setItem`. `workspace`, on the phone.

Source: pv96 (unmerged). Lets a client read and tick a chat's checklist.

### Jev decisions: +4 registrations, not phone

`jevDecisions.getStatus`, `jevDecisions.setEnabled`, `jevDecisions.saveApiKey`, `jevDecisions.clearApiKey`. All `accounts-admin`. The decision runs on the host that holds the chat, so the key is stored there, its value is never returned, and switching it on needs the same permission as the key.

Source: pv96 (unmerged). Adds a host-held key and an on/off switch for the Jev decision step.

### Appearance and GitHub accounts: +4 registrations, not phone

`appearance.getTheme` (`workspace`) and `appearance.setTheme` (`settings-write`). `github.removeAccount` and `github.switchActiveAccount` (`accounts-admin`).

Source: pv96 (unmerged). Theme is readable and writable over RPC; GitHub accounts can be removed and switched.

### Skill packages: +1 registration, not phone

`skills.installPackageVersion` (`skills-admin`). The skills group is now 16 methods plus 6 plugin methods.

Source: pv96 (unmerged). A skill package can be installed at a chosen version.

### Existing methods whose phone reach changed

`files.pathsExist`, `files.writeBase64` and `repo.initGit` were already registered and are now on the phone allowlist. Nothing was removed from the allowlist.

Source: pv96 (unmerged). These three existing methods became reachable from phones.

## Corrections to the custom/integration text

- Streaming names are 17, not 15. `terminal.subscribe` and `terminal.multiplex` are `defineStreamingMethod` registrations at both commits.
- Constant-named registrations are 5 at custom/integration. The two extra names the earlier text listed as "not matched by the literal pattern" are the duplicate `markdown.readTab` and `markdown.saveTab` registrations.
- Terminal output is set up through `terminal.subscribe` and `terminal.multiplex`, both of which are streaming registrations. The earlier text described `terminal.subscribe` as non-streaming.
- The `src/main/ipc/` file count is 675 non-test files at custom/integration (not 472), and 680 at pv96.
- The main-side push "216 call sites" could not be reproduced: 141 lines match `webContents.send(` at both commits.

## Event streams

Named streaming methods (17 names): `accounts.subscribe`, `agentSession.subscribe`, `agentSession.subscribeStatus`, `agentSession.subscribeTurnCompletions`, `browser.clientHost.attach`, `browser.screencast`, `files.watch`, `jira.getIssueStream`, `jira.issueCommentsStream`, `nativeChat.subscribe`, `network.browserTunnel`, `notifications.subscribe`, `runtime.clientEvents.subscribe`, `session.tabs.subscribe`, `session.tabs.subscribeAll`, `terminal.multiplex`, `terminal.subscribe`.

Client event kinds (9) on `runtime.clientEvents.subscribe`: `reposChanged`, `worktreesChanged`, `nativeChatLaunchDraftResolved`, `terminalSideEffects`, `sshStateChanged`, `worktreeTerminalSleepState` (phases started, committed, cancelled, woken), `automationsChanged`, `linearLinkedIssueUpdated`, `activateWorktree`. The stream opens with `ready` (optionally with a repository and remote-state snapshot) and closes with `end`.

Terminal output is set up through `terminal.subscribe` (a streaming registration; the binary stream setup was not read in detail) and `terminal.multiplex` (also a streaming registration).
