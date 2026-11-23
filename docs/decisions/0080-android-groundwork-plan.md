# 0080: Android groundwork plan (milestone M5)

Status: accepted, 2026-11-22, with the owner's decisions recorded under "Decisions taken". A plan for review before any Android code is written.

## Why this note is a decision record

The repository has no feature-document folder (there is no `odd/tasks/`). The working rules put every choice someone
will later ask "why" about in `docs/decisions/`, and this plan is mostly such choices: how the phone reaches the Rust
client, where its key lives, what the first screens are and how they are checked without a phone. It opens the 0080
range for Android, in the way 0040 opened the network decisions and 0050 the agent ones. Each task below ends in its own
pull request; later Android decisions get their own numbers in this range.

## Context

- M5 in `docs/WORKING-RULES.md`: "Android app driving the VPS server", validated phone to VPS.
- `docs/PLAN.md` fixes the stack: a React Native app in `mobile/` on top of `crates/client-ffi`, UniFFI bindings of the
  `client` crate. Decisions 0002 and 0040 say the phone uses the same Rust client as the CLI, so whatever the CLI speaks,
  the phone speaks. No web bundle is served (SRV-19 dropped).
- The inventory has no Android-only feature rows: M5 reaches the M1 to M4 features through the same service
  (`docs/inventory/coverage.md`, Method step 4). The Android screens are specified in `docs/design/screens/android.md`
  with screen IDs (`M-PA-02`, `M-HM-01` and so on) from `docs/design/screen-inventory.md`.
- The client crate already has what a phone needs: `Client::pair` (an offer, a new device key, the host stored),
  `Client::connect_host` (TLS with the pinned server key, the device certificate, the handshake), typed calls
  (`workspace_list`, `git_status`, `git_watch`, `agent_watch`, `terminal_attach`, `terminal_input`, `terminal_resize`),
  `Subscription`, and `ResilientSubscription`, which reconnects and reports `Link::Live` or `Link::Unverifiable`.
  The resilient terminal attach is in pull request #173 (open): it resumes from `TerminalAttachment::next_offset` and
  redraws from a snapshot when the gap is no longer held (decision 0035).
- TLS uses `rustls` with `ring`, chosen in 0040 because it builds for Android without CMake.
- Decision 0041 left one item for this phase: whether the device key moves into the Android Keystore.
- The owner's phone is not connected. Nothing in this plan assumes a device until the last task.

## Decision

### 1. The binding: `crates/client-ffi` with UniFFI for React Native

**Tool.** `uniffi` proc-macros in a new crate `massorca-client-ffi`, and `uniffi-bindgen-react-native` to generate the
TypeScript API and the C++ TurboModule that React Native loads. The same crate also produces Kotlin bindings through
plain `uniffi-bindgen`; they are used only by the JVM tests (section 3), not by the app.

**The client crate is split first.** Today `massorca-client` pulls in local-service code a phone cannot use:
`interprocess` (the local socket), `massorca-process` (autostart), the Windows handle code, and a `Profile`-based
`Hosts` store. Task A1 puts the local path behind a default feature `local`, so `client-ffi` depends on the client with
`default-features = false` and gets the network path only. `Hosts` gains a storage seam (section 4) so the phone can
supply where the host record and the key live.

**What is exposed first.** Only what the first screens need. Everything is an object or a plain record; no generic types
cross the boundary (0002 already keeps protocol types UniFFI-friendly).

| Exposed | Wraps | Used by |
|---|---|---|
| `parse_offer(text) -> Offer` | `massorca_protocol::pairing::Offer` parsing | M-PA-02 (scan or paste), checks the code before any network use |
| `MassOrca::new(storage, device_name)` | holds the tokio runtime, the host store and the `PeerInfo` of the app | app start |
| `MassOrca.hosts() -> Vec<HostRecord>` | `Hosts::list` | M-HM-01 |
| `MassOrca.pair(offer, host_name) -> HostRecord` (async) | `Client::pair` | M-PA-04 |
| `MassOrca.forget(host_name)` | `Hosts::remove`, after a self-revoke when reachable | M-HM-02 long press |
| `MassOrca.connect(host_name) -> Host` (async) | `Client::connect_host` | opening a host |
| `Host.whoami() -> WhoAmI` (async) | `device.whoami` (a typed `device_whoami` is added to the client crate) | host card, Edit host |
| `Host.protocol() -> ProtocolInfo` | `Client::welcome` | M-SH-02 protocol gate |
| `Host.workspaces(project?) -> Vec<Workspace>` (async) | `workspace_list` | M-WS-01 / M-PJ-01 |
| `Host.git_status(workspace) -> GitStatus` (async) | `git_status` | M-DF-02 Changes, read only |
| `Host.watch_git(workspace) -> GitWatch` | `ResilientSubscription` over `git_watch` | live Changes list |
| `Host.watch_agents(filter?) -> AgentWatch` | `ResilientSubscription` over `agent_watch` | status glyphs, M-SE-07 pill |
| `Host.attach_terminal(terminal, cols, rows) -> TerminalView` | the resilient attach of #173 | M-TM-01 |
| `TerminalView.input(bytes)`, `.resize(cols, rows)`, `.close()` | `terminal_input`, `terminal_resize`, drop | M-TM-01 to M-TM-03 |

Not exposed in M5: git writes (stage, commit, push), agent start and prompt, teams, secrets, config, device admin and
pairing offers. They follow the same pattern later, one task each.

**Async calls.** Exported as `async fn` with UniFFI's tokio runtime; TypeScript sees a `Promise`. One multi-thread tokio
runtime lives inside `MassOrca`, started once. Every call takes an optional `AbortSignal` on the TypeScript side, mapped
to dropping the Rust future.

**Streams: pull, not callbacks.** Each watch is an object with `async next() -> Option<Event>` and `close()`. A small
TypeScript wrapper turns it into an `AsyncIterator`, so screens write `for await (const e of host.watchAgents())`.

- Why pull: it matches the Rust `Subscription::next`, gives back-pressure for free (Rust reads the socket only as fast as
  JS asks), and a closed screen simply stops asking and calls `close()`.
- `Event` is an enum: `Item(...)`, `Link(Live)` or `Link(Unverifiable { attempt, retry_in_ms, reason })`. The app shows
  the X-CN-01 banner from `Link` events. Loss of contact is never shown as "stopped" (the `live` / `unverifiable` /
  `exited` vocabulary).
- Terminal output is batched: `TerminalView.next()` returns everything that arrived since the last call, up to 64 KiB,
  as one `Output { offset, bytes }` or a `Snapshot`, so a busy build does not mean thousands of bridge crossings.
- Callbacks (UniFFI foreign traits) are used in one place only: the storage seam, which Rust calls into.

**Errors.** One flat `FfiError` enum crosses the boundary, each case with a stable `code` and a plain `message` the UI
may show as is. It is built from `ClientError` in one function, so the mapping is tested in Rust.

| `FfiError` case | From `ClientError` | Example message |
|---|---|---|
| `Unreachable` | `HostUnreachable`, `Unavailable`, `Disconnected` | "VPS can't be reached." |
| `IdentityMismatch` | `ServerIdentityMismatch` | "This host's identity changed. Pair again." |
| `NotPaired` | `Remote` with `auth.device_revoked` or an auth refusal | "VPS no longer trusts this phone." |
| `ProtocolMismatch { supported }` | `Handshake` with a version refusal | feeds the M-SH-02 copy |
| `InvalidOffer` | offer parse failure, or the offer was already used | "Not a valid pairing code." |
| `HostExists`, `HostNotFound`, `InvalidHostName` | the same cases | the client's own sentence |
| `Remote { code, message }` | any other `Remote` | the service's own code and message |
| `Storage` | `HostUnreadable`, `Io`, a storage callback failure | "The pairing key could not be read." |
| `Internal` | `Protocol` | "Unexpected data from VPS." |

Each case also says whether a retry can help (`is_transient`), so the UI decides between "Retry" and "Pair again"
without parsing text.

### 2. What stays in Rust

Everything that decides correctness stays in Rust: offer parsing, key generation, TLS and the server pin, the handshake
and version check, framing, reconnect and backoff, the `Live` / `Unverifiable` judgement, terminal offsets, resume and
snapshot reassembly, and error classification. TypeScript owns screens, navigation, the camera and QR reading, the
terminal renderer, and notifications. Kotlin owns one module: the Keystore wrapper (section 4).

### 3. Where secrets live on Android

- **Pairing secret.** Lives only in memory for the duration of `pair`; never written down, never logged.
- **Device key (ECDSA P-256, made by `rcgen`).** Generated in Rust as today. Before it is stored, a Kotlin module
  encrypts it with an AES-256-GCM key that the Android Keystore generates and never releases (hardware-backed where
  the phone has it). Only the ciphertext and the certificate go to app-private storage (`Context.getFilesDir()`), with
  backup excluded (`android:allowBackup="false"` plus data-extraction rules), so a cloud backup or a new phone never
  carries the key. Rust gets the plain key only in memory, while it builds the TLS config.
- **The storage seam.** A foreign trait `HostStorage { read_host, write_host, remove_host, list_hosts, seal, unseal }`
  that Kotlin implements and Rust calls. On the desktop and in tests the existing file store implements the same
  trait, so the CLI does not change.
- **Later, not in M5:** a key that never leaves the Keystore, with TLS signatures made through a callback into Android
  (rustls custom signer and an `rcgen` remote key). Stronger, but more moving parts; it is open question 2.

### 4. The first screen set

Small on purpose: enough to pair, see the host, follow agents and use a terminal. Screen IDs from
`docs/design/screens/android.md`; feature IDs from `docs/inventory/first-version.md`.

| # | Screen | Screen IDs | Feature IDs | What M5 shows |
|---|---|---|---|---|
| 1 | Connect hint and pairing | M-PA-01, M-PA-02 (scan and paste), M-PA-04 | SRV-06, SRV-07, SRV-08, SRV-10 | scan or paste an offer, confirm, pair |
| 2 | Home with host cards | M-HM-01, M-HM-02, X-CN-01 | SRV-15, SRV-16 | hosts, state badge, attention summary from the agent watch |
| 3 | Protocol gate | M-SH-02 | SRV-16 | which side must update |
| 4 | Workspace list | M-WS-01 (as M-PJ-01) | WS-29, WS-02, WS-09 | workspaces by project, No Git badge, status glyph per row |
| 5 | Agent status in the list and session pill | X-AG-01, M-SE-07 | STAT-01 | live state words; last known state with its age when unverifiable |
| 6 | Terminal | M-TM-01, M-TM-02, M-TM-03 | TERM-05, TERM-07, TERM-14, REM-12 | attach, type, key row, resize, resume after a drop |
| 7 | Source control, Changes only | M-DF-02 (Changes segment) | GIT-05, REM-14 | read-only live status |
| 8 | Connection log | X-DG-01 | SRV-10 | the reasons from `Link::Unverifiable` and connect errors |

Not in M5: chat (M-CH-01), new session and prompts, diff review, Bot Teams, tasks, settings beyond the theme, the vault,
the Marketplace and the post-pair setup (M-PA-05). The navigation stack (M-SH-01) is built only in its compact form.

### 5. Validation without a phone

| Where | What runs | Catches |
|---|---|---|
| CI, Linux and Windows (`core` job) | `cargo test -p massorca-client-ffi`: the exported API driven from Rust against a real in-process server (the pattern of `crates/client/tests`): pair, connect, whoami, workspace list, git watch, agent watch, terminal attach, drop and resume, every `FfiError` mapping | behaviour of the binding |
| CI, Linux | the generated Kotlin bindings run on the JVM against the Linux `.so` and a real server, with Robolectric for the Keystore module (with a fake keystore, since Robolectric has none) | that the generated foreign API and the storage trait work across a real FFI boundary |
| CI, Linux | `cargo ndk -t arm64-v8a -t x86_64 build -p massorca-client-ffi` (targets `aarch64-linux-android`, `x86_64-linux-android`) | the crate and `ring` still cross-compile |
| CI, Linux | `mobile/`: TypeScript typecheck, lint and Jest with the binding replaced by a fake of the generated interface | screen logic and copy |
| CI, Linux, last | an Android emulator job (x86_64 image, KVM on `ubuntu-latest`): the debug app starts a pairing against a server the job runs on the runner, reached at `10.0.2.2`, and a Detox or Maestro flow walks screens 1, 2, 4 and 6 | the whole chain on a real Android runtime, without a phone |
| This Windows PC | the same cargo tests; `cargo ndk` builds with the Android NDK; the app on an Android Studio emulator against a local server | quick loops; never WSL |
| The owner's phone (only when connected over ADB) | install the debug build, pair with the VPS server (for example `vps.example.ts.net` or `100.64.0.10` on Tailscale), run the M5 checklist, write the milestone report | camera, Keystore hardware, real network changes and sleep |

The emulator job runs only when `crates/client*`, `crates/protocol` or `mobile/` change, because it is slow.

### 6. Tasks

Each task is one pull request of about 400 changed lines or less (lock files apart), with its tests, based on `main`.

| ID | Task | Accepted when |
|---|---|---|
| A1 | Split `massorca-client`: a default `local` feature for the local socket and autostart; a `HostStorage` trait with the existing file store behind it; a typed `device_whoami` | `cargo test --workspace` passes unchanged; `cargo check -p massorca-client --no-default-features` builds without `interprocess` or `massorca-process` |
| A2 | `crates/client-ffi` skeleton: `MassOrca`, `parse_offer`, `hosts`, `pair`, `connect`, `whoami`, `protocol`, `FfiError` | Rust tests pair with an in-process server, reconnect, read whoami; one test per `FfiError` case |
| A3 | Cross-compile check in CI: `cargo ndk` for both Android targets | the job is green on a pull request that touches the client |
| A4 | Streams: `watch_agents`, `watch_git`, `workspaces`, `git_status` with the pull-based `next()` | Rust tests see `Item`, then `Link(Unverifiable)` when the server stops, then `Link(Live)` when it returns |
| A5 | Terminal: `attach_terminal`, `input`, `resize`, batched output (after #173 merges) | a Rust test drops the connection mid-output and reads every byte once, in order; a lost gap yields a `Snapshot` |
| A6 | Kotlin bindings test job on the JVM, with the Keystore module under Robolectric | the Kotlin test pairs and connects through the generated API; a sealed key round-trips; a wrong key fails with `Storage` |
| A7 | `mobile/` scaffold: React Native app, `uniffi-bindgen-react-native` wiring, design tokens from the generator of 0022, the navigation stack | the app builds a debug APK in CI; Jest runs |
| A8 | Screens 1 to 3: pairing, Home, protocol gate | Jest covers every state in their specs (invalid code, camera denied, failed, unreachable, auth failed, update needed) |
| A9 | Screens 4, 5 and 8: workspaces, agent status, connection log | Jest covers loading, empty, error and unverifiable states with their copy |
| A10 | Screens 6 and 7: terminal and Changes | Jest covers attach, exited, restored and unreachable states |
| A11 | Emulator job: the flow over screens 1, 2, 4 and 6 against a server on the runner | the job is green twice in a row |
| A12 | M5 validation on the phone over ADB against the VPS server; `docs/milestones/m5-android.md` | the report lists what was run, on which phone, and what failed or was not checked |

A1 to A6 need no React Native and no phone. A12 waits for the device.

## Alternatives

- **Callbacks for streams** (a foreign `Listener` trait that Rust calls on every item). Natural for push, but Rust then
  has no back-pressure, a slow screen grows a queue, and every listener needs explicit unregistering. Kept only for the
  storage seam.
- **A JSON bridge** (one `call(method, json) -> json` export). Smallest binding, but it moves typing, error mapping and
  reconnect logic into TypeScript, which 0040 explicitly wanted in Rust.
- **Native Kotlin and Jetpack Compose instead of React Native.** Simpler binding (UniFFI's Kotlin output is mature), but
  `docs/PLAN.md` and the Android screen specs are written for React Native and reuse the old app's screens. Changing
  that is an owner decision, not part of this plan.
- **Key stored in plain app-private storage** (what 0041 allows). Safe from other apps, but readable from a rooted phone
  or an unencrypted backup. The Keystore-wrapped key costs one small Kotlin module.
- **Testing only on the phone.** Fails the "no device" condition and would leave every regression to M5's end.

## Consequences

- `massorca-client` gains a feature flag and a storage trait; desktop and CLI keep their behaviour (A1 proves it).
- The repository gets a JavaScript toolchain in `mobile/` and an Android SDK in CI. CI time grows; the emulator job is
  path-gated to limit it.
- Every new phone feature later is: a typed call in `client`, one export in `client-ffi` with its Rust test, one screen.
- The phone permission question in `docs/inventory/open-questions.md` (item 10, the phone allowlist) stays with the
  server: the phone pairs with the scopes the offer grants, as any device.

## Decisions taken

The five open questions were decided on 2026-11-22, under the owner's rule for autonomous work.

1. **React Native with UniFFI**, as agreed in `docs/PLAN.md`. Native Kotlin is not used.
2. **Device key:** for M5, the key is encrypted with an Android Keystore key and kept in app-private storage. A key
   that never leaves the Keystore, which needs a custom TLS signer, is a recorded follow-up and not part of M5.
3. **Terminal:** `xterm.js` in a WebView for M5, because its behaviour is known and it reaches parity fastest. A native
   grid is built only if measured performance on the owner's phone is not acceptable. The terminal stays behind one
   component boundary, so the renderer can be swapped without touching the screens around it.
4. **Emulator flows:** Maestro.
5. **Scope:** chat and starting sessions are in scope for the product, because the whole inventory is wanted. They come
   right after the M5 screen set is validated. M5 itself is pairing, Home, the protocol gate, workspaces, agent status,
   the terminal, the read-only Changes list and the connection log.

## Open questions for the owner (decided above)

1. **React Native or native Kotlin.** The plan follows `docs/PLAN.md` (React Native). Confirm, or choose Kotlin and
   Compose, which would simplify the binding and drop the JavaScript toolchain.
2. **Device key strength.** Keystore-wrapped key in M5 (proposed), or the stronger key that never leaves the Keystore
   now, at the cost of a custom TLS signer.
3. **Terminal renderer (design G17, handoff S6).** Proposed for M5: `xterm.js` bundled in a WebView inside the terminal
   screen, fed by `TerminalView`. The alternative is a native text grid driven by the `alacritty_terminal` emulator the
   desktop already uses, exposed through the FFI; better fit, much more work.
4. **Emulator test tool.** Detox (React Native specific) or Maestro (simpler flows, any app).
5. **First screen set.** Confirm that chat (M-CH-01) and starting sessions can wait for the batch after M5, or name the
   screens to add.
