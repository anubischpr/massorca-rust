# Server, pairing and mobile connectivity (SRV)

Functional inventory of the reference app's headless server mode, its network listener, device pairing, authentication and encryption, the mobile connection, multi-client behaviour, version negotiation, reconnect and resync, and the web bundles served to phones. Behaviour only; no design is prescribed.

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits). Lines marked `Source: pv96` say what changed versus `custom/integration` (`f23a48ca8ca`). Where pv96 did not touch the code, the integration text stands.

The reference app can run without a window as a server (`serve`). Same-machine clients (the desktop UI and the local CLI) reach it over a local owner channel. A second, network listener serves paired phones, paired CLIs and remote desktops, but it stays on loopback until the user creates a LAN or Tailscale pairing offer. A pairing offer is a QR code or URL that carries the endpoint, a per-device token, the server's public key and a device id. Every network connection is protected by the application's own end-to-end encryption (not TLS), authenticated with the device token inside that encrypted channel, and then speaks the same JSON request/response protocol as the local channel, filtered by a per-caller allowlist or permission set. A cloud Relay path lets a phone reach a host that has no LAN route. Version compatibility is a protocol number plus a capability list stored per socket.

## Source read

Reference repository root (read-only): `D:\projects\massorca\.claude\worktrees\massorca-server-language-820c84`. Paths below are relative to that root.

- `src/main/server/` (all 4 non-test files)
- `src/main/runtime/runtime-rpc/`: pairing, mobile pairing, network exposure, lifecycle, shutdown, request admission (header), allowlist (entries and counts), state, socket metadata (names)
- `src/main/runtime/rpc/`: `core.ts`, `dispatcher.ts` (header), `rpc-caller-scope.ts`, `rpc-method-permission.ts`, `ws-transport.ts`, `node-websocket-lifecycle.ts`, `e2ee-channel.ts`, `mobile-socket-wiring.ts`, `mobile-e2ee-v2-desktop-session.ts`, `unpaired-device-auth-throttle.ts`, `static-web-client-handler.ts` (header), `methods/index.ts`, `methods/status.ts`, `methods/pairing.ts`, `methods/mobile-web-bundle.ts` (header), `methods/` file list and registration counts, `unix-socket-transport.ts` and `ws-fallback-port-store.ts` (names only)
- `src/main/runtime/`: `device-registry.ts`, `pairing-endpoint.ts`, `pairing-network-interfaces.ts`, `mobile-pairing-qr.ts`, `mobile-pairing-files.ts`, `tls-certificate.ts`, `bundled-mobile-web-bundle.ts`, `runtime-subscription-registry.ts` (header), `runtime-client-event-bus.ts` (names), `remote-server-updater.ts` (header), `runtime-server-environment-commands.ts` (header), `relay/` (file list; `relay-control-protocol.ts`, `relay-session-broker.ts` head, `relay-control-requests.ts` message types)
- `src/main/startup/serve-options.ts`, `src/main/startup/serve-mode-argv.ts` (names), `src/main/serve-update-handoff.ts` (header), `src/main/protected-secret-persistence.ts` (header and slot names), `src/main/integration-credential-file.ts` (header)
- `src/shared/`: `protocol-version.ts`, `serve-option-validation.ts`, `serve-update-handoff.ts`, `pairing.ts`, `pairing-url.ts` (under `shared/network/`), `mobile-pairing-protocol-limits.ts`, `mobile-e2ee-v2-contract.ts`, `mobile-e2ee-v2-framing.ts` (header), `terminal-stream-protocol.ts` (opcode list), `runtime-client-events.ts`, `runtime-rpc-error-code.ts` (header), `runtime-rpc-envelope.ts` (header), `remote-runtime-shared-control-connection.ts` (first 40 lines), `remote-runtime-shared-control-reconnect.ts` (header), `runtime-subscription-replay.ts`, `mobile-web-bundle/manifest-contract.ts` (constants and first half), `mobile-web-bundle/bundle-rpc-contract.ts` (constants)
- `src/cli/specs/serve.ts`, `src/cli/runtime/serve-orcad-launch.ts` (header)
- `src/shared/rpc-contract/` and `src/shared/network/` (listing only, plus `pairing-url.ts`)
- `src/main/jsonl-rpc/` (headers and limits only; it is the agent-provider process protocol, not a client protocol)
- `README.md` lines 220-260 (mobile section); `docs/readme/massorca-mobile.md` (complete server pairing and troubleshooting guide, English)
- pv96 additions: `src/main/runtime/runtime-rpc/runtime-rpc-mobile-method-allowlist.ts`, `runtime-rpc-mobile-agent-session-methods.ts`, `runtime-rpc-mobile-bot-team-methods.ts` and `runtime-rpc-mobile-pairing.ts` (the phone allowlist, diffed against `custom/integration` and counted by name); `src/main/runtime/rpc/rpc-caller-scope.ts` (how a phone call is checked); `src/shared/protocol-version.ts` (diff: constants and protocol numbers); `src/shared/orchestration-runtime-capabilities.ts` and `src/shared/bot-team-room-attachments.ts` (capability values); `src/main/runtime/rpc/methods/marketplace-vault-methods.ts` and `files-mutation-methods.ts` (permission classes of two of the writes in Finding 7); `mobile/src` (names of the methods the phone calls, from `git grep` over non-test files).
- `odd/tasks/mobile-create-session.md`, `mobile-chat-summary-artifacts.md`, `remote-session-recorded-file-read.md` and `session-history-scope.md` (read in full at `d3c6311a531`).

Method note: the reference repository has no `.codegraph/` index. Building one would write into the read-only reference tree, which the brief forbids, so I used Grep, Glob and Read only. Registration and channel counts are grep counts over non-test files (method is stated in `protocol-surface.md`).

pv96 method note: the pv96 reference is the worktree `D:\projects\massorca\.claude\worktrees\unified-custom-profile-build-547ea6` (HEAD `d3c6311a531`, clean). It has a `.codegraph/` index, but one CodeGraph query about phone chat, create-session and RPC terms returned unrelated test fixtures and the index's freshness at this commit was not established, so no CodeGraph result is relied on. Every pv96 fact came from `git grep`, `git show`, `git diff` and `git log` with explicit refs (`custom/integration` and `d3c6311a531`). Nothing was initialised, synced or written in that worktree.

## Not read / low confidence

- `src/main/runtime/` beyond connectivity: `orchestration/` (208 files), agent-session stores, terminal daemon and orcad internals (`src/main/orcad/`), browser host leases, SSH transport. Only file names were scanned.
- `src/main/ipc/` (472 non-test files): counted through channel literals in the preload only; no handler read individually.
- `src/preload/index.ts` and the 147 files under `src/preload/api/`: counted, not read.
- `src/shared/rpc-contract/` (107 files): listed; parameter schemas not read.
- `src/shared/network/manual-address.ts` and `server-share-address.ts`: not read.
- The relay service itself (`cloud/` in the reference repository, the cell side of the control protocol): not read. Relay behaviour below comes from the host side only, so relay region selection and cell-side limits are low confidence.
- `src/main/runtime/push/` (12 files), `e2ee-keypair.ts` contents, `runtime-rpc-long-poll.ts`, `runtime-rpc-reply-size-limit.ts`, `static-web-client-handler.ts` body, `unix-socket-transport.ts` framing: names or headers only.
- Mobile app UI (`mobile/`) and desktop Mobile screen: out of scope per the brief.
- Test files were skipped except for names.
- Manifest contract details beyond the limits listed in SRV-19.
- pv96: the phone feature bodies in `mobile/src` (create session, chat summary, artifacts, background work, terminal panel) were read from their feature documents and from the names of the methods they call, not re-read in code. Their commits (for example `6ed87388ad6`, `963896d3d41`, `877ffc63ebc`) are all in `custom/integration`, so they are not pv96 changes.
- pv96: the transport, listener, pairing endpoint, device registry, E2EE, TLS and bind-policy files have no commit between `custom/integration` and `d3c6311a531`; SRV-02 to SRV-15 keep the integration text and were not re-read.
- pv96: the auth-path checks in Finding 3 were not re-read at this commit.

## Features

### SRV-01 Headless server mode (serve)

- **What the user does:** starts the core with `serve` (or the serve mode of the desktop binary) without a window, typically on a VPS or a Linux machine without a screen. Options (`--port`, `--pairing-address`, `--mobile-pairing`, `--no-pairing`, `--grant-desktop-control`, `--project-root`, `--recipe-json`, `--json`) also accept `--serve-` prefixed spellings. The process keeps running until stopped with Ctrl+C or a signal.
- **Observable behaviour:** prints readiness output (SRV-02), then serves until signalled. Shutdown first refuses new LAN widening, waits for a widening in progress, closes every network and local transport, and flushes pending device last-seen writes. The shared metadata file is intentionally left in place, because another live runtime may share the same data directory.
- **Rules and edge cases:**
  - `--mobile-pairing` and `--no-pairing` are mutually exclusive. `--grant-desktop-control` applies only to the default runtime offer. `--recipe-json` needs `--project-root` and pairing.
  - Pairing flag typos are rejected with a message; Electron/Chromium switches pass through unchecked.
  - Port must be an integer 0-65535; 0 means a random port.
  - The CLI spec describes `serve` as starting a runtime server without a desktop window; the CLI can also hand the request to a local orcad slot when app-side selection says so (flow not read).
  - Systemd guidance (docs): a service usually runs with `--no-pairing`; to pair, a temporary override adds `--pairing-address` and `--mobile-pairing`, the service restarts, and the override is removed afterwards. Each restart briefly disconnects clients, and paired devices keep access.
  - Idle-exit support: the runtime reports open connections, in-flight requests and the last request time, so a host can decide when no client is using the server.
- **Data persisted:** the user-data directory (device registry, E2EE keypair, fallback port, metadata file; see SRV-04, SRV-05, SRV-08, SRV-11).
- **Depends on:** SRV-02, SRV-03, SRV-04, SRV-05, SRV-08.

### SRV-02 Readiness and machine-readable output

- **What the user gets:** after start, a readiness block: bound endpoint, advertised endpoint, build identity and terminal-daemon health when reported, web client URL, mobile pairing QR (terminal) and pairing URL, or the reason pairing is unavailable with guidance.
- **Rules:**
  - JSON mode prints one object (`orca_server_ready`, schema version 1) with runtime id, bound and advertised endpoints, managed-WSL reconciliation state, the pairing object and optional health.
  - Recipe mode prints only the pairing code and project root, for VM recipes; it refuses to run without pairing.
  - Readiness is published once per process; a second publish is an error.
- **Depends on:** SRV-01, SRV-06, SRV-07.

### SRV-03 Local owner channel for same-machine clients

- **What the user does:** nothing directly. The desktop UI and the local CLI find the running core and send requests to it.
- **Observable behaviour:** the core listens on a per-process local socket (Unix domain socket on POSIX; named pipe on Windows). On POSIX, socket files left by killed processes are swept at start; Windows named pipes leave nothing on disk, so there is no sweep.
- **Rules:**
  - Requests from this channel carry the owner token (random per process, in the metadata file). A missing token returns `unauthorized`; a wrong token falls through to the SSH bridge credential check and then returns `unauthorized`.
  - The owner caller has every permission (SRV-09). An unstamped call is treated as this host's own desktop or owner-token CLI.
  - Replies are JSON, one per request, with the runtime id in `_meta`.
- **Data persisted:** none beyond the socket and metadata.
- **Depends on:** SRV-05.

### SRV-04 Network WebSocket listener and bind policy

- **What the user does:** a phone, a remote CLI or a remote desktop connects to the server over a WebSocket port.
- **Observable behaviour:**
  - Starts bound to loopback. Creating the first LAN or Tailscale pairing offer widens the listener to all interfaces. Widening restarts the listener; live loopback clients drop and reconnect on the same port, and already issued endpoints stay valid.
  - If the bind host was pinned (for example by an explicit host option), widening is refused with an error, and the offer is reported unavailable instead of advertising an endpoint nothing listens on.
  - Concurrent offers share one widening.
  - If the WebSocket listener cannot start (for example, the port is busy), the core keeps serving the local channel only; pairing then reports `websocket_unavailable`.
- **Port rules:**
  - An explicit `--port` is preferred on later launches, so a stale fallback cannot steal a pinned port.
  - Without an explicit port, the last port that was actually bound is remembered in a fallback-port file, so devices paired to it keep their endpoint across restarts.
  - Port 0 chooses a random port; the resolved port is read back for metadata and QR.
- **Limits:** 1 MiB maximum WebSocket message; 128 concurrent WebSocket connections (over capacity the socket closes with 1013 "Maximum connections reached"); 256 TCP connections in total; 10 s to authenticate before the socket is dropped; a 15 s ping/pong sweep for half-open connections.
- **Also on this listener:** the static browser client (SRV-20).
- **Depends on:** SRV-05, SRV-10, SRV-11, SRV-15.

### SRV-05 Runtime discovery file and ownership

- **What the user gets:** the local CLI and desktop find the running core without configuration.
- **Observable behaviour:**
  - A metadata file in the user-data directory lists the runtime id, process id, transports (kind and endpoint, one local and optionally one WebSocket), the owner token and start time. It is rewritten when the set of transports changes.
  - A watcher polls ownership. If the file is overwritten by another runtime, this one stops republishing. If the owning process is gone, a new runtime reclaims the file and logs that it did so.
  - After teardown the core never republishes endpoints it already stopped.
- **Rules:** the metadata file is not deleted on shutdown (SRV-01).
- **Depends on:** SRV-03.

### SRV-06 Pairing offer (QR and URL)

- Source: pv96 (unmerged). The relay copy in the pairing failure guidance now says "MassOrca Relay" instead of "Orca Relay" (commit `a9041ec27c5`); the offer, its reasons and its limits are unchanged.
- **What the user does:** in the desktop Mobile screen, or with `serve --mobile-pairing` (QR and URL printed in the terminal), or with the runtime pairing command (URL for a CLI or web client). The phone taps "Pair Desktop", then scans the QR or pastes the code, then confirms "Pair with this desktop?".
- **Offer contents:** protocol version of the offer, endpoint, device token, server public key, paired device id, scope (`mobile` or `runtime`), and, for relay offers, the relay section (SRV-13). The URL is the base64url JSON of that offer, length-capped.
- **Pending credential rules:**
  - Creating an offer creates a device row that has never been scanned.
  - A repeated request reuses the pending row if the scope and grants match; a request for a broader network reach widens the row, never narrows it.
  - "Regenerate" (rotate) drops un-scanned rows and mints a new token, so a leaked code can be retired.
  - A pending token becomes an authenticated device on its first successful use.
  - The offer is returned only after the registry write is durable.
- **Unavailable reasons (reported to the user with guidance):** `websocket_unavailable`, `device_registry_unavailable`, `e2ee_key_unavailable`, `invalid_advertised_endpoint`, `network_exposure_failed`, `relay_mint_failed` (with a stage code), `disabled_by_operator` (serve `--no-pairing`).
- **Web client URL (runtime scope only):** the same endpoint with an https/http scheme and the pairing URL placed in the URL fragment, so credentials stay out of proxy logs and referrers.
- **Docs guidance:** the pairing URL and QR are credentials; anyone who uses one first gets access; a code stays valid until a phone uses it.
- **Depends on:** SRV-04, SRV-07, SRV-08, SRV-11, SRV-13.

### SRV-07 Advertised address and override

- **What the user does:** chooses the address a phone or remote client will dial, either by letting the core pick one or by typing `--pairing-address` or a custom address in the desktop UI.
- **Rules:**
  - With no override, the offer advertises `127.0.0.1`. Pairing stays local-only unless the user explicitly advertises a reachable address.
  - Accepted override forms: host name, `host:port`, IPv4 or IPv6 literal (IPv6 is bracketed), `ws(s)://` or `http(s)://` URL (HTTP is normalised to WebSocket). Wildcard hosts (`0.0.0.0`, `::`, `*`) are rejected, as are port 0, control characters, paths, queries, fragments and credentials in host-only form, and values over 16 KiB.
  - Automatic choice on a LAN host (for the default offer): enumerates interfaces; drops internal interfaces, proxy fake-IP ranges (198.18.0.0/15), and link-local IPv6 (fe80-febf); ranks Tailscale (100.64.0.0/10) addresses first, then IPv4 over IPv6; penalises virtual bridge adapters (Hyper-V `vEthernet`) unless a default-route query proves they carry traffic (Windows only).
  - When the route query is unavailable, the bridge adapter gets no proof and is ranked lower.
- **Depends on:** SRV-04.

### SRV-08 Device registry and per-device tokens

- **What the user gets:** each paired phone or CLI has its own credential, which can be revoked on its own.
- **Observable behaviour:**
  - A device row holds: device id, name, scope (`mobile` or `runtime`), token, paired-at and last-seen times, pairing reach (`network` or `this-computer`), connection mode, relay binding, push registration and grants.
  - Tokens are 24 random bytes, hex-encoded (48 characters).
  - Revoking a mobile device queues durable cleanup of its relay binding and push registration, removes the row, forgets its navigation state, and closes that device's sockets. If the relay cleanup cannot be queued, the revoke fails and the device stays.
  - Revoking runtime access removes the row and closes its sockets.
  - Last-seen: the first sighting is written at once; later refreshes are coalesced into one write about every 250 ms.
- **Rules and edge cases:**
  - The registry is a JSON file with hardened permissions. A write persists before memory changes; a registry that exists but cannot be read blocks all writes, so an unreadable file can never overwrite (and revoke) every pairing.
  - Rows written by older versions default to scope `mobile`, connection mode `automatic` and reach `network`.
  - Stale temporary write files older than a day are removed at start.
  - Token lookup is an exact comparison over the list (SRV-10 notes the consequence).
- **Data persisted:** device registry file; relay revoke outbox and push unregister outbox are separate durable queues.
- **Depends on:** SRV-05, SRV-06, SRV-13, SRV-22.

### SRV-09 Device scopes and grants

- Source: pv96 (unmerged). The phone allowlist grew from about 320 to 373 method names: 53 added and none removed (29 `botTeams`, 8 `marketplace`, 7 `prototypes`, 4 `delegation`, 2 `agentSessionChecklist`, 2 `files`, 1 `repo.initGit`). Phones are still checked against the allowlist alone (`rpc-caller-scope.ts`).
- **What the user gets:** a phone, a paired CLI or a paired web client gets only the rights it needs. Administrative rights are granted at pairing time and cannot be added later.
- **Observable behaviour, by caller:**
  - Owner (local desktop or owner-token CLI): every permission.
  - Phone (`mobile` scope): only the methods on an explicit allowlist (about 373 method names at pv96, about 320 at `custom/integration`; see `protocol-surface.md`). The permission classes are not consulted for phones.
  - Paired runtime client (`runtime` scope, CLI or web): standard set of `workspace`, `settings-write`, `accounts-admin`, `skills-admin` and `host-admin`. `desktop-control` (drive this machine's desktop) and `pairing-admin` apply only if granted at pairing.
  - SSH bridge caller (the `orca` CLI of a remote SSH host, relayed through this host): only `workspace` methods, bound to its own target. A per-target opt-in widens it to the CLI's remote-control mode.
- **Rules:** the only grantable permission is `desktop-control`. Grants are stored per device row and are never widened after pairing. Revoking a runtime client removes its grant with the row.
- **Observation for review:** the allowlist for phones includes two methods whose declared permission is `pairing-admin` (`pairing.getEndpoints`, `pairing.provisionRelay`) and several `accounts-admin`, `host-admin` and `settings-write` methods. Code comments say pairing admin is refused unless granted for runtime clients, but phones are checked against the allowlist only. Whether that is intended is not stated in the code. At pv96 the list also adds `marketplace.vault.set` (declared `accounts-admin`, which stores a Marketplace credential on the host) and `files.writeBase64` (declared `workspace`); see Finding 7.
- **Depends on:** SRV-08, SRV-10.

### SRV-10 Connection handshake, authentication and limits

- **What the user does:** nothing visible when it works. A phone shows "Couldn't connect within 25s" (docs) when the handshake never completes.
- **Observable sequence (network path):**
  1. The client opens the WebSocket. The server starts a 10 s pre-authentication timer.
  2. The client sends a v2 encrypted-session hello (or, for the older path, a hello with its public key). The server answers with its own ready message containing its public key and nonces. Details in SRV-11.
  3. The client sends an encrypted auth message carrying its device token and its capability set.
  4. The server resolves the token against the registry. For relay connections the resolved device id must equal the relay device id, or the connection fails.
  5. On success the socket becomes ready, its capability set is stored, the device's last-seen time is updated, and RPC traffic starts.
- **Close codes and failures:**
  - `4001`: invalid handshake, unsupported hello, invalid public key, unauthorized device, invalid auth message, or an unencrypted binary frame before auth. An encrypted error reply with a code is sent first where possible.
  - `4003`: too many consecutive decryption failures after auth.
  - `1013`: over the connection cap.
  - A decryption failure before auth is treated as unauthorized (the client's bounded retry budget applies).
  - Pre-auth sockets get no heartbeat probes; the pre-auth timer alone bounds them.
- **Unpaired-device signal:** three unauthorized closes within 60 s raise a one-time signal for the desktop shell (the default threshold), so the user can be told that a previously paired phone is no longer recognised. Only the desktop shell can show it; the headless server does not.
- **Depends on:** SRV-08, SRV-11.

### SRV-11 End-to-end encryption on the network channel

- **What the user gets:** traffic between a phone and the server is unreadable to anyone on the same network, including tokens and terminal output.
- **Observable behaviour:**
  - Each server has a long-term public/secret keypair (persisted in its own file, migrated with the device registry). The public key travels inside the pairing offer, so the phone trusts it before the first connection.
  - Each connection uses fresh per-connection keys. Protocol v2 binds the handshake transcript to the session: both sides' public keys and nonces, the transport context, and per-direction keys.
  - Every frame carries a session id, a direction, a payload kind (text or binary) and a 64-bit counter. A frame that does not match the expected counter or direction is rejected, and a wrong key closes the socket.
  - Frame overhead is 82 bytes on top of the payload.
- **Rules and edge cases:**
  - The relay transport always requires v2. Direct connections also accept the older path: an ECDH shared key, with the ready message sent in plaintext.
  - Authenticated payloads are subject to size limits and an outbound memory budget. Exceeding the size limit or the send queue closes the socket.
  - Binary terminal frames are only accepted after authentication.
  - The code states that TLS was rejected because React Native cannot pin self-signed certificates (see SRV-12).
- **Data persisted:** the server keypair file.
- **Depends on:** SRV-08.

### SRV-12 TLS on the network listener (dormant in the reference)

- **What the code does:** can create and store a self-signed certificate and key in the user-data directory (the code adjusts the key file's permissions), and the WebSocket transport can serve HTTPS and WSS when given a certificate and key.
- **What actually happens in the reference:** the listener starts without a certificate. No non-test code calls the certificate generator, and the fingerprint field the mobile client would pin is never set. In the reference, the network listener is plain `ws://`. Confidentiality comes only from SRV-11. Comments in the code still describe `wss://`.
- **Advertised WSS:** an operator can still pass a `wss://` or `https://` pairing address (for example a reverse proxy with its own certificate); the core then advertises that URL.
- **Depends on:** SRV-04.

### SRV-13 Relay path for phones without LAN reach

- **What the user does:** chooses the Anywhere (relay) connection for a phone; the phone then reaches the host through a cloud relay, without Tailscale or a shared network.
- **Observable behaviour:**
  - To create an automatic (relay-capable) offer, the host requests a relay pairing invite and embeds the relay section in the offer. If the relay is unavailable or refuses, the offer is refused (not silently downgraded to LAN) with guidance to use LAN or retry.
  - The host keeps a control connection to the relay: it sends a hello and proves its identity by answering a challenge; the relay then announces incoming connections, drain instructions, pings and status. Connection kinds are `invite` (first contact) and `resume` (returning device, proven with a resume secret).
  - The host manages several relay origins with a pool and a region catalogue with preference and correction; it renews its authorization with jittered delays.
  - Device revocation and mode switches queue cleanup of old relay credentials durably, so an old invite cannot outlive its QR.
- **Rules:** relay connections require the v2 encryption path (SRV-11) and bind the device to its relay device id.
- **Limits:** the cloud service implementation was not read; cell-side limits and region rules are unverified.
- **Depends on:** SRV-06, SRV-08, SRV-11, SRV-14.

### SRV-14 Connection mode: automatic or LAN only

- **What the user does:** for each phone, chooses whether it may use the relay ("Anywhere", automatic: direct first, relay as fallback) or only the direct LAN or Tailscale path (local-only).
- **Observable behaviour:**
  - The choice is stored on the device row and is part of the credential: a QR made under one policy will not pair under another. Changing the policy rotates the pending code.
  - Local-only offers never mint relay material. Automatic offers never fall back to a LAN-only QR under the relay label.
  - Rows without a stored mode behave as automatic.
- **Depends on:** SRV-06, SRV-13.

### SRV-15 Multiple simultaneous clients and per-device revocation

- **What the user does:** uses the desktop, a phone, and one or more CLIs against the same core at the same time.
- **Observable behaviour:**
  - Each authenticated socket has its own connection id and capability set. Subscriptions belong to the connection and are removed when that socket closes, without affecting sibling sockets.
  - Device-scoped state (for example client navigation state) is cleared only when the device's last socket closes, because several sockets can share one device token.
  - Revoking a device closes that device's sockets immediately (no close handshake), so a revoked stream stops at once.
  - Idle sockets that lose their TCP connection without a close frame (typical when a phone is suspended in the background) are removed by the 15 s ping/pong sweep.
  - Terminal output can have several viewers; viewer and input ownership rules were not read in depth.
- **Depends on:** SRV-04, SRV-08, SRV-10.

### SRV-16 Protocol version and capability negotiation

- Source: pv96 (unmerged). The protocol number (3) and the minimum client version (2) are unchanged. pv96 adds the capability constants `worktree.manual-display-name.v1` (a host without it drops `manualDisplayName` from `worktree.set`), `agent-session.checklist.v1` (an older host shows the checklist read-only), `orchestration.ask-user.v1` and `bot-team.room-attachments.v1`. The top-level count below was not recounted.
- **What the user gets:** an older phone or CLI keeps working with a newer core, and new features switch on only where both sides support them. A phone shows an "update required" message when the core is too old for a feature.
- **Observable behaviour:**
  - The core protocol number is 3; the minimum compatible client protocol is 2 (the mobile constant is an alias of the same value).
  - The core advertises a capability list (77 top-level entries, some expanding to groups). Examples include terminal binary-stream and multiplex capabilities, workspace-ports, mobile tasks, remote push, runtime status compatibility and runtime environments.
  - A client declares its capabilities at authentication and may update them later with a dedicated method. Capabilities are stored on the authenticated socket, so an unauthenticated request cannot change them.
  - `status.get` returns the core's app version and whether remote update is supported.
- **Rules:**
  - Envelope fields added later are stripped; known discriminators and fields stay strictly validated.
  - An unknown method returns `method_not_found` to desktop and CLI clients. For phones, a method not on the allowlist returns `forbidden`, which the phone reads as "this desktop predates the method".
  - Update-required copy is per capability (several named messages exist for automation, file mutation, GitHub and GitLab features).
- **Depends on:** SRV-10, SRV-18.

### SRV-17 Reconnect and resync

- **What the user does:** loses Wi-Fi, switches networks, or backgrounds the phone; the app reconnects and the open screens catch up.
- **Observable behaviour (client side):**
  - Reconnect delays for the shared control connection: 250 ms, doubling to 30 s while active; up to 5 min while idle. A "stable" timer resets the backoff only after a connection has stayed up.
  - After reconnecting, the client replays its logical subscriptions. Each replayed subscription's first reply is tagged so consumers apply it even when the version or epoch numbers did not change (the host did not restart). Without the tag, mirrors stayed frozen.
  - Pending requests are tracked per connection; the client keeps a standing intent to stay connected and exposes diagnostics. (How each in-flight request is settled on disconnect was not verified.)
- **Observable behaviour (server side):**
  - Subscriptions are per socket and are not resumed by the server; the client re-issues them. A relay resume reconnects with a resume secret and receives a confirmation.
  - Restarting `serve` drops every client. They reconnect, and paired devices keep their access.
  - Terminal streams re-publish a snapshot on subscription.
- **Depends on:** SRV-15, SRV-16, SRV-18.

### SRV-18 Client connection to a server (client side)

- **What the user does:** the desktop or CLI connects to a remote server (for example a VPS) and drives it as if it were local. Desktop environments are added from a pairing code, verified, connected and disconnected from the desktop UI.
- **Observable behaviour:**
  - One shared control connection per server carries request/response and subscriptions over a single authenticated, encrypted socket. Requests that cannot be sent wait for readiness, and retired request ids are remembered so late replies are ignored.
  - The client sends its device token in the encrypted auth message (SRV-10) and reads the server's capabilities (SRV-16).
  - The desktop environment manager exposes add from pairing code (with and without verification), connect, disconnect, retry, remove and status snapshots.
- **Rules:** connection state is shown per environment; the environment manager and its SSH and orcad variants belong to other domains and were not read.
- **Depends on:** SRV-10, SRV-16, SRV-17.

### SRV-19 Mobile web bundle served over RPC

- **What the user gets:** in the reference, the phone downloads its web UI bundle from the paired desktop, so the phone app can match the desktop build.
- **Observable behaviour:** the desktop serves its install's bundle over the already authenticated RPC: one manifest call, then one call per 48 KiB chunk of each asset, or one call per gzip range of about 384 KiB when the manifest offers ranges.
- **Rules:**
  - Limits in the contract: 256 assets, 32 MiB total, 10 MiB per asset, 64 routes, 16 grants per route.
  - Asset paths are restricted to safe characters, and reserved Windows names are rejected.
  - Each asset read is verified against the bundle's manifest; any failure in that verify-and-read step reaches the phone as `mobile_web_bundle_asset_changed`.
  - The bundle is answered only from the desktop's own install. It is never forwarded to a remote host or the relay.
  - No bundle means the feature is absent, not an error.
- **Recommendation:** drop from the first version. The PLAN states that the new Android client is React Native with a Rust client core, so a bundle served from the desktop is an artifact of the current shell. The owner should confirm.
- **Depends on:** SRV-10, SRV-16.

### SRV-20 Browser web client served over HTTP

- **What the user does:** opens the server's web UI in a browser from a pairing offer (runtime scope), with the pairing data in the URL fragment.
- **Observable behaviour:** the WebSocket listener also answers plain HTTP for one page (`/web-index.html`) and a few static asset prefixes (`/assets/`, `/cmaps/`, `/standard_fonts/`, `/wasm/`). Any other path is refused.
- **Depends on:** SRV-04, SRV-06.

### SRV-21 Server self-update

- **What the user does:** updates a headless server from a connected client instead of downloading and restarting by hand.
- **Observable behaviour:**
  - A runtime reports its version and whether remote update is supported. Clients can check, download and install through the server update methods. Installation writes a handoff file that a supervisor process reads (`install-requested`, then `completed` or `failed`); the serving process then reports ready to the supervisor with its version and runtime id.
  - Phones show an update wall when the core is older than they need (SRV-16); the wall points the user to the server's update path.
- **Rules:** the handoff file is parsed defensively (malformed input parses to nothing).
- **Depends on:** SRV-01, SRV-16.

### SRV-22 Push registration for paired phones

- **What the user gets:** notifications while the phone app is closed.
- **Observable behaviour:** a paired mobile device row can hold a push registration, set and cleared by its own phone. A push token that the gateway reports dead clears the registration. Revoking the device queues a durable unregister. A malformed stored registration is dropped on load instead of failing it.
- **Depends on:** SRV-08, and the notifications domain (not read); the push gateway is external.

## Phone reach at pv96 (by area, with owners)

What a paired phone can call. Every name here is on the phone allowlist at pv96. The owning domain describes the behaviour; this table only says how far the phone reaches. Counts for new names are against `custom/integration`.

| Area | Methods (examples) | Owner |
|---|---|---|
| Agent chats: create, send, subscribe, approvals, visuals, recorded files | `agent.launch`, `agent.launchReplay`, `agentSession.*`, `nativeChat.*` | AG-08, AG-09, AG-15, AG-16, AG-24; REM-27 for recorded files |
| Chat checklist (new at pv96) | `agentSessionChecklist.read`, `agentSessionChecklist.setItem` | AG-15 |
| Delegated child work (new at pv96) | `delegation.bringBack`, `delegation.bringBackStatus`, `delegation.bringBackResolve`, `delegation.bringBackAbort` | AG-26, AUTO-32 |
| Bot Teams (29 new names at pv96) | `botTeams.*`: rooms, tasks, routines, webhooks, team files, memory, projects | AUTO-12 to AUTO-37 |
| Prototypes (7 new names at pv96) | `prototypes.*`: archive, delete, design system versions | PROTO-01 to PROTO-13 (`prototypes.md`) |
| Marketplace (8 new names at pv96) | `marketplace.apps.*`, `marketplace.vault.*` | `skills-and-plugins.md` (SKL); REM-26 for host credentials |
| Sessions, terminals, worktrees and repos | `session.tabs.*`, `terminal.*`, `worktree.*`, `repo.*` (`repo.initGit` is new) | `terminals.md`, `workspaces.md`, `git.md` |
| Files (`files.writeBase64` and `files.pathsExist` are new) | `files.*` | `settings-and-files.md` |
| GitHub and accounts | `github.*` (incl. `github.readAccountGitConfig`), `accounts.*` (incl. `accounts.consumeCodexResetCredit`) | `providers-and-trackers.md`; `accounts-and-usage.md` |
| Browser | `browser.*` (incl. `browser.screencast`) | `browser-and-devices.md` |
| Push and notifications | `notifications.registerPush`, `notifications.unregisterPush`, `notifications.subscribe` | SRV-22; CLD-10, CLD-11 |
| Pairing, status and preflight | `pairing.getEndpoints`, `pairing.provisionRelay`, `status.get`, `preflight.*`, `settings.get` | SRV-06, SRV-16, SRV-18 |

The phone feature bodies (create session, chat summary, artifacts, background work and the terminal panel) are in `custom/integration`, not new in pv96. They are described by their AG and AUTO owners.

## Findings to review

1. **TLS is dormant.** Certificate creation and the WSS-capable transport exist, but the listener never receives a certificate and no caller creates one. The reference's network channel is `ws://` protected only by SRV-11 (`src/main/runtime/runtime-rpc/runtime-rpc-lifecycle.ts`, `src/main/runtime/tls-certificate.ts`).
2. **Phones can call pairing-admin methods (still true at pv96).** The phone allowlist includes `pairing.getEndpoints` and `pairing.provisionRelay`, both declared `pairing-admin` (`src/main/runtime/runtime-rpc/runtime-rpc-mobile-method-allowlist.ts`, `src/main/runtime/rpc/methods/pairing.ts`).
3. **Bearer tokens are compared with plain equality.** The owner token check, the device token lookup and the SSH bridge credential lookup use `===` or `find`. No constant-time comparison appears in the RPC auth path. `timingSafeEqual` appears elsewhere (relay host-challenge envelope, SSH known hosts, skill package download, Linux package recovery), so the gap is specific to the RPC path.
4. **LAN is closed until an offer is made.** The listener binds loopback and widens only when a pairing offer is created, and the default advertised address is `127.0.0.1`. A phone can only reach a headless server after an explicit address and offer.
5. **Relay requires the stronger handshake.** Relay connections always need v2 encryption and bind the device to its relay device id; direct connections also accept the older path.
6. **Reconnect correctness depends on a replay tag.** Without tagging the first post-reconnect reply, mirrors can stay frozen when the host did not restart (`src/shared/runtime-subscription-replay.ts`).
7. **The phone allowlist adds writes that reach host secrets and files.** At pv96 a paired phone can call `marketplace.vault.set` (declared `accounts-admin`), `files.writeBase64` (declared `workspace`) and `repo.initGit`. Phones are checked against the allowlist only (`src/main/runtime/rpc/rpc-caller-scope.ts`), so these are reachable whatever permission class they carry (`src/main/runtime/runtime-rpc/runtime-rpc-mobile-method-allowlist.ts`, `src/main/runtime/rpc/methods/marketplace-vault-methods.ts`).

