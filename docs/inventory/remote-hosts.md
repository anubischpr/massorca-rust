# Remote execution hosts: functional inventory (REM)

Source reference: the current MassOrca Electron app, read-only, at
`D:\projects\massorca\.claude\worktrees\massorca-server-language-820c84`.
Paths below are repo-relative to that reference checkout.

Reference: pv96, commit `d3c6311a531` (custom/integration plus 852 unmerged commits), read in the worktree `D:\projects\massorca\.claude\worktrees\unified-custom-profile-build-547ea6` with `git grep` and `git show` at the commit (the worktree is clean). Lines marked `Source: pv96` say what changed versus `custom/integration`.

## Domain summary

A workspace can run its terminals, agents, git and file access on a machine other than
the desktop: a remote SSH host, a WSL distribution on Windows, or an ephemeral VM created
per workspace. For SSH hosts the app connects with its own SSH stack (or the system
OpenSSH binary when a feature needs it), deploys a small relay daemon to the host, and
keeps every remote terminal, git call and file operation going through that daemon.
Connection loss is handled by a reconnect ladder, and the relay keeps remote terminals
alive for a configurable grace period so they can be re-attached. Host keys are verified
against the user's known_hosts files and an app-owned trust store. Port forwards are
per host and restored on reconnect. The current product is moving SSH hosts onto a
managed Orca server (`orcad`) on the host, with the relay as a fallback; both are
described here because the rebuild must decide which of them survives. WSL is treated as
a Windows-only host variant, not as an execution-host kind of its own. Ephemeral VMs are
created by user-written recipe scripts; the app ships no VM provider.

## Source directories read

- `src/main/ssh/` (about 700 entries; roughly 500 non-test files). Read: connection,
  config parsing and import, auth, host keys, transport selection, relay deploy and
  session, reconnect, port forwards, port scanner, PTY leases, orcad (managed server)
  headers, install model and GC. Most was read by file header and rationale comments only.
- `src/main/host/` (electron-only host plumbing; secret protection report).
- `src/main/wsl/` and root `src/main/wsl*.ts`.
- `src/main/network/` (names and headers only).
- `src/main/ports/` (scanner and advertised-URL headers).
- `src/relay/` (daemon entry, grace lifecycle, RPC method names, PTY, git, fs, port scan,
  Windows ConPTY). Agent-hook and ai-vault files were excluded per the brief.
- Root `src/main/` files matching `ephemeral*`, `provisioned*`, `remote*`, `host*`,
  `sidecar*`, `localhost*`, `execution*`.
- `src/main/runtime/` (host-scoped handlers, by file name and a few headers).
- Shared contracts: `src/shared/ssh-types.ts`, `execution-host.ts`,
  `runtime-host-status*.ts`, `runtime-host-contact.ts`, `ephemeral-vm-*.ts`,
  `relay-protocol` in `src/main/ssh/`, `windows-breakaway-launch.ts`, `wsl-paths.ts`,
  `orcad-managed-runtime.ts`.
- `src/main/ipc/ssh-target-crud-handlers.ts` (target validation and re-adoption rules).
- `README.md`, `skill-guides/orca-per-workspace-env*` (ephemeral VM recipes).
- pv96 additions: `src/relay/relay-runtime-services.ts` (handler registrations), `src/relay/relay-github-accounts.ts`, `src/relay/relay-marketplace-apps.ts`, `src/relay/relay-marketplace-vault.ts` (headers and registrations only), `src/relay/ai-vault-service-filesystem.ts` (diff only), `src/main/ssh/ssh-target-id-migration.ts` (diff), `src/main/orcad/orcad-entry.ts` (diff), `src/main/runtime/rpc/methods/marketplace-vault-methods.ts` (permission class), and the recorded-file read pair `src/main/native-chat/agent-session-recorded-file-read.ts` and `src/main/runtime/rpc/methods/structured-agent-session-recorded-file.ts` (no pv96 change).
- Commit messages read for the pv96 relay and orcad changes: `1fee6abfe58`, `66675267387`, `d3b61d1a5b9`, `c4471f6086f`, `750700871a7`, `a9efb9cbf9b`.
- `odd/tasks/remote-session-recorded-file-read.md` (read in full at `d3c6311a531`).

## Not read / low confidence

Not read in depth:
- `ssh-relay-session.ts` (3,600 lines) and `ssh-relay-deploy.ts` (2,600 lines): only
  rationale comments and exported function names.
- `ssh-connection.ts` (1,450 lines): only comments.
- `ssh-channel-multiplexer.ts`, the relay PTY/git/fs handler bodies, and the mux writer.
- The bulk of the orcad (managed server) modules (about 200 files): headers only.
- Electron IPC handlers under `src/main/ipc/ssh-*` other than target CRUD.
- Renderer SSH settings UI (excluded by the brief); the mobile client.
- All `*.test.ts` files (used only where a test name was clearer than the code).
- `src/main/network/` proxy and Tailscale DNS diagnostics: names only.
- `cloud/` (the mobile pairing relay). It is a different relay, documented by its own
  inventory domain, and is not the SSH relay described here.

Cited by the code but absent from the reference checkout (not verifiable here):
`docs/reference/ssh-execution-boundary.md`, `docs/reference/ssh-host-key-verification.md`,
`docs/reference/wsl-command-execution.md`, `docs/reference/remote-server-install-model.md`,
`docs/ssh-relay-versioned-install-dirs.md`. Where a rule below cites the execution
boundary, it is taken from code comments that quote it.

Low confidence:
- Whether managed Orca server (`orcad`) runs on Windows SSH hosts. `shared/node-runtime-pin.ts`
  says managed launch is POSIX-only; `main/ssh/orcad-remote-host-support.ts` says every
  managed operation runs on Windows through a host script. The newer file is taken as
  current; this is unconfirmed.
- The exact upload fallback trigger (SFTP versus exec-based staging) in the relay deploy.
- How WSL distros are keyed in account and runtime selection beyond the helpers listed.
- The UI default grace period (24 h) is taken from the renderer draft, not verified in UI.
- pv96: the bodies of the marketplace vault and apps handlers (about 35 changed files each) and the OpenMetadata token path were read from headers and commit messages only.
- pv96: `src/main/ssh/` changed in two files only (target id migration); the rest of the SSH transport is taken as unchanged from `custom/integration` and was not re-read.
- pv96: the phone and renderer callers of the recorded-file read were not re-read; REM-27 comes from the feature document and the host read module (`agent-session-recorded-file-read.ts`).

## Cross-cutting rules

These rules recur across the features. They are the ones the owner's plan calls out.

- **Losing contact is not proof that a process died.** A dropped transport makes a host
  `disconnected` or `reconnecting`. Remote terminals and leases stay `detached`, not
  `terminated`. Only positive evidence counts as exit: for a relay version directory, its
  recorded PID must be dead and every socket in it must refuse connections. An unanswered
  probe is `unverifiable`, never `exited`. The shared runtime-host contact model uses four
  answers: live, unverifiable, refused and retired. Refused and retired are the only
  arms that carry positive evidence, and they are different things.
- **The host that runs the work owns its status.** The relay on the host keeps the PTYs and
  their grace timer; the desktop only re-attaches. (Agent status rows are a separate domain:
  STAT-13, STAT-16, STAT-17.)
- **Clients never fabricate host state.** Fields the renderer may not set (generation,
  provisioning intent, managed-server fences, runtime ladder cache) are main-owned.
- **Secrets are not persisted.** Passwords and passphrases are cached in memory for one
  connection only. The persisted flag `lastRequiredPassphrase` only tells startup which
  targets to defer.

---

## Features

### REM-01 Add an SSH host manually

- **User does:** enters a label, host, port, username, and optionally an identity file,
  identity agent, "identities only" flag, a proxy command or jump host, and a relay grace
  period (see REM-13). The host is saved as a manual target.
- **What happens:** the target gets a fresh id and a registration generation. Each new
  registration advances the generation so that automations fenced on an earlier
  registration cannot attach to the new one. Manual targets are never overwritten by
  config import (REM-02).
- **Rules and edge cases:**
  - Re-adding a host the user deleted lifts any config-import suppression for that
    alias, and re-adopts workspaces that still point at the old target id.
  - Changing connection fields tears down the transport and any tunnel built from the old
    fields.
  - Managed-runtime targets cannot be created or edited from SSH settings.
  - Cloning a repository onto an SSH host is refused. The user clones on the host and then
    registers the existing folder.
- **Data:** target record (label, host, port, username, identity fields, jump/proxy,
  grace, portForwards, remoteRuntime, generation, flags).
- **Depends on:** REM-03 (auth), REM-04 (host keys), REM-07 (connect), REM-22 (removal).

### REM-02 Import hosts from the OpenSSH config

- **User does:** opens the host picker, searches aliases from `~/.ssh/config`, and adds one
  or all. A passive sync also runs without the explicit flag, on open (per code comments;
  the exact trigger is not traced).
- **What happens:** config entries become targets. Effective values come from parsing the
  file (including `Include`) and resolving with `ssh -G`, so tokens and defaults match what
  the system would use.
- **Rules and edge cases:**
  - Config-sourced targets are kept in sync: a rotated port or changed identity is applied
    on the next import. Manual targets with the same alias are left alone.
  - Deleting a config-sourced host records a suppression so the passive sync does not
    re-add it. The explicit Import action clears the suppressions.
  - The picker marks hosts already in the app and suppressed hosts, and its "new" count
    excludes them.
  - Alias comparison is case-insensitive everywhere.
  - Include expansion is bounded (256 glob matches, 1 MiB per included file).
  - When the config cannot prove that no `Match` block claims an alias, the app assumes one
    might, and does not override OpenSSH's resolution.
- **Data:** targets with `source: ssh-config`; the list of suppressed aliases.
- **Depends on:** REM-01, REM-06 (for ProxyCommand/jump values).

### REM-03 Authenticate with keys, agent and password

- **User does:** connects; is prompted for a passphrase or password only when needed.
- **What happens:** the app tries, in order: an agent (the `SSH_AUTH_SOCK` socket, or the
  Windows OpenSSH agent pipe, or the target's `IdentityAgent`), then key files.
- **Rules and edge cases:**
  - Without an explicit identity, the app probes the default OpenSSH key names in
    `~/.ssh/` (ed25519, rsa, ecdsa, dsa, xmss, plus the FIDO2 `_sk` variants).
  - `IdentitiesOnly` limits agent offers to the listed key files.
  - `IdentityAgent none` disables the agent.
  - An encrypted key is tried through the agent first. Direct parsing and a passphrase
    prompt follow if that fails. Retries use a fresh proxy socket because the SSH library
    may have destroyed the old one.
  - Keyboard-interactive prompts that look like a password reuse the password flow and its
    cache. One-time-code prompts always ask the human.
  - A multi-step auth chain is bounded (four partial successes).
  - Security keys (FIDO2) and GSSAPI-enabled hosts are routed through the system OpenSSH
    binary (REM-06).
  - Cached credentials exist only for the life of one connection attempt and one connection.
- **Data:** the `lastRequiredPassphrase` flag. No secret is stored.
- **Depends on:** REM-06, REM-04.

### REM-04 Verify host keys

- **User does:** connects to a host for the first time, or later sees a rejection.
- **What happens:** the presented key is checked against the user's known_hosts files
  (unioned across files) and against the app's own host-key store (host, port, key type).
- **Rules and edge cases:**
  - A key that matches is accepted.
  - A key that mismatches known_hosts is rejected. The message gives the `ssh-keygen -R`
    remedy, because that is the fix that also unblocks plain ssh and git.
  - A revoked key is rejected even under lax settings.
  - A key that differs from the app's own store is rejected and the store file is named,
    because known_hosts cannot cure that case.
  - An unknown host is accepted and remembered, unless `StrictHostKeyChecking` is strict
    (then rejected), lax (accepted but not recorded), or the system SSH config could not be
    read (then rejected, because a site policy cannot be ruled out).
  - A known_hosts file that exists but cannot be opened lets the connection proceed, but
    nothing is recorded from that evidence.
  - Ephemeral VM targets (REM-21) accept an unknown key without recording it, because a
    new VM presents a new key every launch.
- **Data:** the app's trusted host-key store file; known_hosts is read, not written.
- **Depends on:** none.

### REM-05 Jump hosts and ProxyCommand

- **User does:** sets a jump host or proxy command on a target, or uses one from the
  OpenSSH config.
- **What happens:** the proxy is turned into a local spawned process whose stdio becomes the
  SSH socket. A jump host is treated as the equivalent `ProxyCommand`.
- **Rules and edge cases:**
  - Any proxy forces the system OpenSSH transport (REM-06).
  - On Windows, proxy values are checked for cmd.exe metacharacters and rejected rather than
    escaped. Hosts, ports and usernames never contain them legitimately.
- **Depends on:** REM-06, REM-02.

### REM-06 System OpenSSH transport

- **User does:** nothing directly; it applies when a feature needs OpenSSH behaviour.
- **What happens:** two transports exist. The default is the in-process SSH library. The
  other is the system `ssh` binary (`/usr/bin/ssh`, `/usr/local/bin/ssh`,
  `/opt/homebrew/bin/ssh` on POSIX; `System32\OpenSSH\ssh.exe` or PATH on Windows).
- **Rules and edge cases:**
  - The system transport is used for proxies, FIDO2 security keys, GSSAPI-enabled hosts,
    and when the environment variable `ORCA_SSH_FORCE_SYSTEM_TRANSPORT=1` is set.
  - A forced system transport with no binary present fails the connection. For a
    security-key host, the system transport is selected only when the binary exists.
  - The system transport has no interactive shell channel and no SFTP API. Folder downloads
    therefore are not offered on it. File transfers use `ssh` commands.
  - Connection reuse across setup commands is on by default and can be turned off per target.
  - On macOS, an app-level network policy can block the direct TCP socket while the system
    `ssh` is allowed. The system transport is the recovery path.
- **Depends on:** REM-03, REM-05.

### REM-07 Connection lifecycle, reconnect and status

- **User does:** connects, disconnects, and watches a status indicator per host.
- **What happens:** a host moves through `disconnected`, `connecting`, `auth-failed`,
  `deploying-relay`, `connected`, `reconnecting`, `reconnection-failed` and `error`, with a
  reconnect attempt counter.
- **Rules and edge cases:**
  - Failed handshakes retry on a backoff of 1, 2, 5, 5, 10, 10, 10, 30 and 30 seconds. After
    the ninth consecutive failed handshake the host becomes `reconnection-failed`, and only
    a user action restarts it.
  - A flap (connection drops after a successful handshake) keeps the backoff going but does
    not count toward giving up.
  - A connection that stays up for 60 seconds resets the ladder.
  - The retry delay is capped so that a full reconnect finishes before the shortest relay
    grace period (60 seconds) expires. Otherwise the relay would shut down and take its
    terminals with it.
  - Connect timeout is 30 seconds; the SSH transport sends keepalives every 15 seconds.
    The relay channel sends keepalives every 5 seconds and times out after 20 seconds.
  - After OS sleep or wake, the connection is torn down and rebuilt, because a dead TCP
    socket can look connected.
  - A newer attempt owns the state. A cancelled attempt does not publish an error.
  - Targets that needed a passphrase are deferred at startup. The rest connect eagerly.
  - An explicit disconnect keeps remote terminal ownership, so a later connect re-attaches
    them (REM-12).
- **Data:** no connection state persists except the passphrase flag and the terminal leases.
- **Depends on:** REM-03, REM-08, REM-12.

### REM-08 Deploy and update the remote relay

- Source: pv96 (unmerged). The relay deployed to an SSH host now also answers GitHub account, Marketplace app and Marketplace vault calls (`src/relay/relay-runtime-services.ts`; see REM-26); the deploy and update steps are unchanged.
- **User does:** connects. Deployment happens without a separate action. The user sees
  `deploying-relay` on first connect and after an update.
- **What happens:** on connect the app checks the host for a relay of the right version. If
  it is missing or different, it uploads the bundle for the host platform and launches it.
- **Rules and edge cases:**
  - Each relay version installs into its own immutable directory under `~/.orca-remote/`,
    named by version and content hash. A running daemon is never served new code from a
    directory being overwritten.
  - Installs take a remote lock with stale recovery. A completion marker and a version file
    are written after the upload.
  - Garbage collection removes superseded version directories only when no live socket or
    PID shows them in use (the liveness rule under "Cross-cutting rules").
  - The relay launches detached, so it survives the SSH exec channel closing. Its log is
    rotated in-process. Its socket name is hashed per target, so two targets sharing a
    remote account cannot attach to each other's relay. A long remote home directory would push
    the socket path past the Unix socket limit, so a shorter socket path is chosen then.
  - A running relay from an earlier launch in its grace period is re-attached rather than
    replaced, which keeps its terminals.
  - On a wire version mismatch the client exits with a dedicated code. That is treated as
    terminal ("reconnect manually"), not retried with backoff.
  - On Windows hosts the relay is started outside the SSH session's job object, so it survives
    the session (see REM-24).
  - A self-test mode exists for the relay runtime.
- **Data (remote):** `~/.orca-remote/` with version directories, the version file, the
  install-complete marker, the PID file, socket and log. **Data (local):** the cached
  runtime resolution keyed by glibc version, runtime hash and app major version.
- **Depends on:** REM-09 (runtime choice), REM-10 (native parts), REM-23 (platforms).

### REM-09 Runtime ladder and plain-SSH fallback (drop)

- **What exists:** the relay is a Node program. The app picks a runtime by rung:
  - A: the app's pinned Node plus prebuilt addons (glibc 2.28 or newer);
  - B: a compatibility pinned Node for older glibc (2.17 targets);
  - C: the host's own Node 18 or newer with prebuilt addons, no npm;
  - legacy: the host's Node with `npm install`, unsupported, opt-in per target;
  - D: no runtime at all. The target gets plain SSH terminals and SFTP browsing only. Those
    terminals end when the connection drops and are not restored.
- **Why drop:** the whole ladder exists because the relay needs a Node runtime on the host.
  A single static daemon in the rebuild removes the need. The rule "a host without a
  runtime still gets plain terminals that end on disconnect" can be kept as a degraded mode
  if the owner wants one.

### REM-10 Native terminal dependencies on the host (drop)

- **What exists:** the relay needs native addons for PTYs (node-pty) and file watching
  (@parcel/watcher). They are built or repaired on the host for its OS, architecture and
  Node version under the install lock. On Linux a missing C/C++ toolchain produces an
  install hint. Windows uses a ConPTY binding that cannot be proven healthy until first
  spawn.
- **Why drop:** a Rust daemon links its PTY and watch crates in, so nothing compiles on
  the host. Windows ConPTY still needs its own verification (REM-24).

### REM-11 Remote terminals

- **User does:** opens a terminal in a workspace on an SSH host; runs an agent in it.
- **What happens:** the desktop asks the relay to spawn a PTY on the host with a working
  directory, shell and size. Input, resize, signals, output and exit flow through the relay.
- **Rules and edge cases:**
  - Creation appears to be idempotent per client-generated id (inferred from the file names
    of the create-idempotency modules; not traced), so a retried create should not start a
    second shell.
  - The relay can report foreground process, child processes, current directory, default
    shell, profiles, and capabilities. Clear-buffer and input-mode reset are supported.
  - Scrollback is kept on the host so a re-attaching client gets the screen back (see
    TERM-07 for the client side).
  - Closing a terminal asks the host to shut the PTY down (the `pty.shutdown` operation), not
    only the local view.
  - Agents run inside these terminals (agent domain; see STAT-13 and STAT-16).
- **Data:** PTY lease records on the desktop (target, PTY id, tab and leaf, state
  attached, detached, terminated or expired).
- **Depends on:** REM-07, REM-08; TERM-01 to TERM-14 for terminal behaviour in general.

### REM-12 Terminals survive disconnects

- **User does:** loses the network or sleeps the laptop, then comes back and finds the
  terminals still running.
- **What happens:** the relay keeps PTYs alive when the SSH connection drops, for as long as
  its grace period lasts. On reconnect the desktop re-attaches each PTY it had leased to
  that host.
- **Rules and edge cases:**
  - Grace per host: `0` (the stored default) means until reset, with no expiry. Otherwise
    between 60 seconds and 7 days. The new-host form pre-fills 24 hours.
  - A relay with no PTYs exits after 15 minutes idle (overridable for tests). A relay started
    detached with no client attached to it exits after 60 seconds.
  - When the grace period ends, the relay shuts down and every PTY on it goes with it.
  - Re-attach runs at most 8 at a time, each attempt has 10 seconds, and retries are bounded.
  - A lease that has expired can still re-attach unless it was superseded by a newer lease
    for the same pane, or the host recycled the PTY id for a different shell.
  - A stop the host did not confirm is kept as a pending kill and replayed on the next
    handshake to the same target.
  - Plain-SSH terminals (REM-09 mode D) do not survive.
- **Depends on:** REM-07, REM-08, REM-11; STAT-17 for agent status during reconnect.

### REM-13 Remote file access (later)

- **User does:** browses, opens, edits, creates, renames, deletes, searches and watches files
  on the host from the file explorer.
- **What happens:** the relay serves file operations: list, bounded directory reads,
  stat, read (whole or range or streamed in 256 KB chunks), write, copy, rename, delete,
  search, markdown listing, watch with unwatch-and-wait, and workspace space scan.
- **Rules and edge cases:**
  - Time limits apply per operation (for example 25 seconds for file listing).
  - Terminal artifacts can be written and read through the same channel.
  - Without a relay, file browsing is the SFTP path of REM-09 mode D, which is not planned.
- **Depends on:** REM-08. File explorer UI is outside this inventory.

### REM-14 Remote git and worktrees on the host

- **User does:** works with git and creates or removes worktrees on an SSH host.
- **What happens:** the relay runs git on the host. About 45 operations cover status,
  diff, history, stage and unstage, commit, push, pull, fetch, upstream status, branch rename,
  local branch listing, worktree list, create and remove, submodule status, rebase onto
  base, fork sync and fast-forward. Fetch for GitHub and GitLab pull or merge request
  heads also goes through the host.
- **Rules and edge cases:**
  - Creating a remote worktree refreshes the local base ref on the host, creates the
    worktree there, marks it as created by Orca on the host, runs the setup command on the
    host, and then activates a terminal for it.
  - Large git output is streamed in chunks; a buffer-overflow path exists for oversized
    output (not traced in detail).
  - Upstream status has a 5-minute negative cache for repos with no effective upstream.
  - Force-deleting a preserved branch is a separate operation (`git.forceDeletePreservedBranch`).
  - Cloning onto a host is refused (REM-01).
- **Data:** the host's git repos and worktrees; Orca's remote-created marker.
- **Depends on:** REM-08, REM-11; git domain GIT-13 to GIT-20 for behaviour in general;
  GIT-28 for the existing SSH git note.

### REM-15 Remote CLI passthrough and orchestration (later)

- **User does:** runs `orca` on the SSH host (typically from an agent running there) to
  control the desktop, or to use orchestration and issue-tracker helpers.
- **What happens:** the host's CLI reaches the desktop through a relay channel. Most commands
  are forwarded; commands that need a TTY or a local tmux pane are not, because the buffered
  bridge cannot host them.
- **Rules and edge cases:**
  - Remote control is off by default. The host is a guest, not an owner, until the user
    enables `allowRemoteCliControl` for it. That setting lets the host's CLI control this
    app's other hosts, orchestration and files.
  - Orchestration commands (ask, check, post, send) and the Linear helpers have dedicated
    output adapters for the host.
- **Depends on:** REM-08; orchestration and issue-tracker domains.

### REM-16 Manual port forwards (later)

- **User does:** adds a forward from a local port on the loopback address to a remote host
  and port, per SSH host.
- **What happens:** the app listens on `127.0.0.1` and forwards connections over the SSH
  transport. A port of 0 asks the OS for a free port and the caller must use the bound port.
- **Rules and edge cases:**
  - Forwards are saved per host and restored on connect and reconnect. A forward that cannot
    be restored stays saved, to retry next time.
  - An unexpected exit is reported with its reason. Removing a forward is explicit.
  - The browser pane uses a separate SOCKS-style dynamic forward through the host for its
    network route (BRW-19). That is not a user feature.
- **Depends on:** REM-06, REM-07; BRW-19.

### REM-17 Remote port detection and advertised URLs (later)

- **User does:** starts a dev server on the host and sees it offered locally.
- **What happens:** the relay lists listening TCP ports on the host, attributed to their
  processes. The desktop polls, and matches dev-server URLs printed in SSH terminals to a
  local forward so the user opens `localhost:<port>`.
- **Rules and edge cases:**
  - Linux hosts are scanned through the kernel's TCP tables and process file descriptors.
    Windows hosts use the netstat listing. macOS hosts report no ports (the relay returns an
    empty list).
  - Port 22 is excluded. At most 50 ports are reported.
  - The poll runs every 12 seconds, backs off to 30 seconds while the port set is unchanged,
    and is parked while the window is hidden.
- **Depends on:** REM-16, REM-11, REM-23 (per-OS behaviour).

### REM-18 Managed Orca server on SSH hosts (later, XL)

- Source: pv96 (unmerged). `orcad` now starts the Bot Teams routine scheduler and keeps the host busy while a routine is on (`src/main/orcad/orcad-entry.ts`, commit `a9efb9cbf9b`; see AUTO-28); the conversion and ownership rules are unchanged.
- **User does:** connects to an SSH host and gets a long-running Orca server there, reached
  through a tunnel, instead of a per-connection relay. The user sees a one-time "move now"
  offer per app version when a host still runs a relay.
- **What happens (as the current code does):** every SSH host is set up to run a managed
  server (`orcad`) on a fixed remote port, reached through an SSH tunnel, or through a stdio
  bridge when sshd refuses forwarding. An empty host gets it deployed. A host with relay
  state is converted through a journaled migration that moves terminals and state. A host
  whose terminals are live, or cannot be proven exited, keeps the relay for that session and
  converts on a later connect. A host the server cannot run on keeps the relay ladder, with
  the reason recorded so the deploy is not retried on every connect.
- **Rules and edge cases:**
  - The server is updated to the app's version on connect, when its terminals allow.
  - Ownership is fenced on the host and on the desktop. An interrupted migration is recovered
    from its journal.
  - The migration machinery is specific to moving users off the relay; see the owner
    decision below.
- **Owner decision:** whether the rebuild keeps one host daemon (the Rust server) for both
  roles, which removes the relay/server split and most of this migration. Without a
  compatibility requirement, this is a good candidate to drop the conversion path.
- **Depends on:** REM-08, REM-11, REM-12; server domain SRV-01 to SRV-22.

### REM-19 Link an SSH host to a paired server (later)

- **User does:** links an SSH host to a managed server on it, so that the paired devices
  reach that server through the verified tunnel.
- **What happens:** linking is a prepare, verify, complete sequence (cancel and unlink also
  exist). The tunnel is verified before the link is written. A fingerprint of the target
  fields is used to detect later changes to the host.
- **Rules and edge cases:** a link is refused while the environment is being reconciled.
- **Depends on:** REM-18; server pairing SRV-06, SRV-18.

### REM-20 WSL distributions as an execution host (later, L, Windows only)

- **User does:** on Windows, runs workspaces, terminals and agents inside a WSL distro, with
  paths shown as `\\wsl.localhost\<distro>\...`.
- **What happens:** the app lists WSL distros through `wsl.exe` and drops the internal
  `docker-desktop*` distros. A cached availability probe decides whether WSL is usable, with
  a 45-second negative cache. Commands run through a single WSL runner that decides, per call,
  the login PATH, shell, output fencing and payload transport. The login environment is
  probed once per distro and cached.
- **Rules and edge cases:**
  - The app converts between Windows UNC paths and Linux paths for the same distro. Paths on
    `/mnt/<drive>` are recognised as the same location.
  - A running-distro check is cached and rate-limited; a broken `wsl.exe` does not make every
    WSL session vanish.
  - Accounts and agent selection are keyed per distro (AGENT and ACCOUNT domains).
  - The Orca CLI is registered into each distro; a pinned Node is installed into the guest on
    first use and never substitutes a user-installed runtime.
  - Deleting a WSL item uses a contained delete script, because the Recycle Bin is not
    available for WSL UNC paths.
  - WSL is not an execution-host kind. The execution-host model has only local, ssh and
    runtime. WSL is handled as a local path variant (low confidence on how far this goes).
- **Depends on:** GIT-29 (git in WSL), TERM-28 (WSL terminals), STAT-18 (WSL agent relay).

### REM-21 Ephemeral VMs from environment recipes (later, L, experimental)

- **User does:** defines an environment recipe in the project's `orca.yaml`, then starts a
  workspace that runs on a machine created for it.
- **What happens:** the recipe is a user-written set of lifecycle scripts: create, destroy,
  suspend and resume, with a doctor command to check the setup. Each create prints one JSON
  result. That result is either an SSH target to dial, or an Orca server pairing code plus the
  project root. A variant, `provisioned-root`, has the machine clone the finished workspace
  itself and requires direct SSH.
- **Rules and edge cases:**
  - Runtime states: provisioning, running, suspended, suspend_failed, resume_failed, failed,
    cleanup_pending, cleanup_failed, cleaned. Cleanup is retried, and retry metadata survives
    even when the feature state is unreadable.
  - Resuming a provisioned-root runtime is refused if its project root or SSH ownership changed
    while suspended.
  - The SSH target created for it is owned by the runtime and hidden from the SSH host list.
  - Its host keys are accepted without being recorded (REM-04).
  - Gated behind an experimental setting.
  - The app ships no provider. The bundled guide covers Vercel Sandbox, Fly, Modal, an existing
    SSH host, and local Docker over SSH. Provider choice, plans, regions and credentials belong
    to the user.
- **Depends on:** REM-04, REM-07, REM-08, REM-12, REM-14 (provisioned root); setup recipes
  domain.

### REM-22 Remove a host and clean its remote state (first version)

- **User does:** deletes an SSH host from settings.
- **What happens:** the target is removed and a tombstone keeps its last label, so workspaces
  still pinned to it show a readable name. Workspaces on the removed target are listed as
  orphaned. Re-adding the same host re-adopts them (REM-01).
- **Rules and edge cases:**
  - Removing a config-sourced host suppresses its alias (REM-02).
  - Whether removing a host also stops its remote terminals is not traced. Stops that could
    not be confirmed stay pending and are replayed on a later handshake (REM-12).
  - Remote worktree history cleanup is best effort, and older relays may not support it.
- **Depends on:** REM-01, REM-12, REM-14.

### REM-23 Linux and macOS remote hosts (first version)

- **User does:** connects to a Linux or macOS SSH host.
- **What happens:** the app detects the host OS by probing `uname`, then a PowerShell probe
  if `uname` is unrecognised. It then deploys the matching relay.
- **Rules and edge cases:**
  - Linux: full relay, including port detection (REM-17), and the pinned runtime for glibc 2.28
    or newer (2.17 targets get the compatibility runtime; see REM-09).
  - macOS: full relay, with no remote port detection. The local macOS desktop uses a different
    scanner (lsof).
  - If neither probe can identify the host, connection fails with both probe outputs in the
    error.
- **Depends on:** REM-08; REM-17 for ports.

### REM-24 Windows remote hosts (later, XL)

- **User does:** connects to an SSH host running Windows OpenSSH.
- **What happens:** the relay runs on Windows with its own terminal binding (ConPTY), shell
  detection, and a port scan via netstat.
- **Rules and edge cases:**
  - Exec commands go through the host's default shell, which is cmd.exe on a stock install.
    Commands are kept within cmd's length limit. PowerShell 7 is used only when a probe finds
    it. PowerShell 5.1 stdin writes have a bounded wait because a blocked read can hang.
  - Launching the relay uses a breakaway launcher, so it survives the session.
  - The Windows SSH test matrix is a separate CI lane.
  - Windows firewall scope rules (in `main/runtime/windows-firewall-remote-scope.ts`) govern the
    desktop's own listener, not the remote host.
- **Depends on:** REM-08, REM-11, REM-17.

### REM-25 Relay diagnostics and self-test (later)

- **User does:** nothing normally; a maintainer reads logs or runs the self-test when a host
  misbehaves.
- **What happens:** the relay writes a rotated log, a diagnostic log line per event, a
  self-test report, and deploy timing. The SSH host lifecycle also emits telemetry events.
- **Depends on:** REM-08.

---

### REM-26 Remote host answers GitHub account and Marketplace calls (later, L)

- Source: pv96 (unmerged). Before pv96 the SSH relay had no handlers for the desktop's GitHub account calls, so every SSH host looked out of date (`1fee6abfe58`); the Marketplace credential vault, app connections and OpenMetadata profile tokens on SSH hosts and paired servers are also new (`66675267387`, `d3b61d1a5b9`, `c4471f6086f`).
- **User does:** on an SSH host, lists the host's GitHub accounts, switches or removes its active account, connects Marketplace apps and sets the credentials those apps need.
- **What happens:** the desktop calls the relay with `github.listHostBindableAccounts`, `github.switchActiveAccount` and `github.removeAccount`; the relay answers with the desktop's own account helpers. Each answer carries the account's host, user, active flag, which token variable (if any) supplies it, source and scopes, never a token value. Marketplace calls (`marketplace.vault.status`, `marketplace.vault.set`, `marketplace.vault.clear`, their profile variants and `marketplace.apps.*`) run on the host that owns the credentials.
- **Rules and edge cases:**
  - Each SSH host keeps its own vault; its key is a 0600 file, because a plain Node process has no keychain (`src/relay/relay-marketplace-vault.ts`).
  - A relay built before pv96 has no handlers and answers method-not-found; the desktop then treats the host as out of date (`1fee6abfe58`).
  - The picked Marketplace host survives leaving the Marketplace and is renamed with its SSH target (`750700871a7`).
  - Phones can call the same Marketplace vault methods (SRV Finding 7).
- **Data:** the host's Marketplace vault file (0600) and its app connection state.
- **Depends on:** REM-07, REM-08, SCM (GitHub accounts), SKL (Marketplace).

### REM-27 Read a file an agent wrote on a paired runtime (later, M)

- Source: pv96 (unmerged), also in `custom/integration`. No pv96 commit changes this feature (`6297e6cc60c`, `299919f6540`, `e1b6b3f6e4c`, `3cca8f194aa` and `677b168fbdb` are all in `custom/integration`); this text was missing from the integration pass.
- **User does:** on a chat running on a paired remote runtime, opens a file the agent reported writing (for example an HTML artifact outside the worktree). Ctrl+Click on a Results link opens a read-only preview; Mod+Alt+click opens it in a tab (gestures per the feature document). The phone's Artifacts viewer reads such files for structured chats only.
- **What happens:** the client asks the paired runtime to stat or preview an absolute path in the chat's session. The host serves only a path that its own journal records as written by a file-writing tool (provider diff items and completed calls of write tools; reads, searches and user messages prove nothing). The client's path is only a membership check, and the host opens only the recorded spelling.
- **Rules and edge cases:**
  - Read-only. The host uses `lstat` and `O_NOFOLLOW` where the platform has it, reads only regular files, and opens with a non-blocking open so a writer-less FIFO cannot hang the read.
  - Reads are capped by the host's content budget (`remoteFileContentBudget`), and oversize files are refused rather than truncated.
  - UNC and device paths are rejected. PDF and other binaries show "cannot be previewed" on the desktop.
  - An older host answers method-not-found, and the client tells the user to update the host.
  - A file a shell command wrote is not in the journal, so it is refused; chat visuals use a separate channel (`agentSession.readVisual`).
  - A parent directory that is a symlink is followed (only the final component is checked). Windows has no `O_NOFOLLOW`, so there the link and regular-file checks are the defence.
- **Data:** none. The host reads its own journal snapshot on each call.
- **Depends on:** AG-15 (the chat's journal), SRV-18 (paired runtime).

---

## Feature summary for first-version scoping

First version (10 features): REM-01, REM-03, REM-04, REM-07, REM-08, REM-11, REM-12,
REM-14, REM-22, REM-23. These are the smallest set that lets an agent run on a remote Linux or
macOS host with terminals that survive a dropped connection, git on the host, and safe host
keys. Everything else is later or dropped.

Owner decisions this inventory surfaces:
1. Keep one host daemon for both relay and managed server (REM-18), or only the relay.
2. Whether WSL is first version on Windows (REM-20).
3. Whether a degraded plain-SSH mode is wanted at all (REM-09).
