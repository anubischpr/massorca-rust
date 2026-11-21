# Cloud services: functional inventory (CLD)

Reference: MassOrca worktree `massorca-server-language-820c84`, read-only. Paths are relative to that repository root unless marked otherwise. Desktop version in that snapshot: 1.4.224 (`package.json`).

pv96 reference: `d3c6311a531` (custom/integration plus 852 unmerged commits). No commit in the delta touches `cloud/` (checked with `git log` and `git diff --stat` against `custom/integration`), so the relay and push text below stands. The desktop side changed at pv96: feedback and crash reports, and the Engram memory refresh used by Bot Teams. Lines marked `Source: pv96` say so.

## Domain summary

The `cloud/` directory is an independent pnpm workspace. It runs two hosted services that the desktop and Android clients depend on, plus the tooling to operate them. The **relay** (a director and a set of cells) pairs a phone with a desktop host through outbound WebSockets and splices their frames without reading them. The **push gateway** holds the Apple signing key and sends APNs and FCM notifications to paired phones. Around these sit a relay operations console, a fence broker that changes relay infrastructure, Terraform roots, runbooks, and 27 `cloud-*` GitHub workflows that stay inert in this repository. The directory contains no user account store, no sync service and no website. Only the relay and the push gateway sit on the phone path, and neither is needed to run agents, terminals or git on a desktop.

## What the cloud part answers

| Question | Answer in this snapshot | Evidence |
| --- | --- | --- |
| Accounts | Desktop sign-in code exists (browser PKCE against `login.onorca.dev`). The login and API servers are in the private `stablyai/orca-cloud` repository, not here. | `cloud/README.md` "What is not here"; `src/main/orca-profiles/profile-cloud-auth-config.ts` |
| Relay | Yes. Production director at `relay.onorca.dev`; phones and hosts meet through it. | `cloud/README.md`; `profile-cloud-auth-config.ts` |
| Push | Yes. Production-only Cloud Run gateway at `push.onorca.dev`. | `cloud/docs/push-gateway.md` |
| Sync | No sync or state service. The relay forwards opaque frames only. | `cloud/packages/relay-contract/src/protocol-limits.ts` |
| Skill sharing | The desktop calls a skill-sharing API on the cloud API origin (`login.onorca.dev` in production) and accepts `orca://` skill-share links. Its server is outside `cloud/`. | `src/main/skills/skill-cloud-auth.ts`; `src/shared/skill-share-link.ts` (outside `cloud/`) |
| Telemetry backend | Not here. Product telemetry is opt-in and goes to PostHog. | `src/main/telemetry/client.ts` (see PLAT-17) |
| Feedback and crash intake | Not MassOrca cloud. Source: pv96. The desktop files feedback and crash reports as GitHub issues in the project's repository, through the user's active `gh` account, without logs or images; the old `onorca.dev` intake is gone. | `src/main/ipc/feedback.ts`, `src/main/ipc/crash-reporting-submission.ts` (outside `cloud/`) |
| Engram memory cloud | Not here. Source: pv96. Bot Teams Memory refreshes a project's memories through the external `engram` CLI with `--cloud`; the cloud login is whatever that CLI holds. | `src/main/bot-teams/bot-team-engram-sync.ts` (outside `cloud/`) |
| GitHub sign-in | Not here. Source: pv96. Sign-in runs through `gh auth login --web` (device flow); the token stays in gh's own store. | `src/main/github/gh-auth-sign-in.ts` (outside `cloud/`) |
| Website | Not here. The Homebrew cask names `onorca.dev` as homepage. | `Casks/orca.rb` (outside `cloud/`) |

## Source directories read

- `cloud/README.md`; `cloud/docs/` (all 13 file names; `push-gateway.md` lines 1-120; `relay-reconnect-2026-09-findings.md` lines 1-40 and 452-465).
- `cloud/apps/relay/src/`: configuration variable names (`config.ts`), HTTP route list (`app.ts`), `relay-token-verifier.ts`, `admin-token-verifier.ts` route groups, WebSocket upgrade paths (`relay-server.ts`), file names.
- `cloud/apps/push/src/push-server.ts` (routes); `cloud/packages/push-contract/src/push-limits.ts` and `send-messages.ts`; `config.ts` variable names.
- `cloud/apps/relay-fence-broker/src/app.ts` (lines 1-40, routes), `config.ts` variable names.
- `cloud/apps/relay-ops/README.md`, `src/index.ts` routes, file names.
- `cloud/packages/relay-contract/src/`: close codes, admission budgets, protocol limits, control, credential, director, host close reason, region, rehome and continuity contracts (export names and key constants).
- `cloud/packages/postgres-schema/` and `cloud/dev/` (file names only).
- `cloud/infra/terraform/` (README, file names, `environments/*.tfvars` searched for domains and service names).
- Outside `cloud/`, only to confirm how the desktop calls these services: `src/main/orca-profiles/profile-cloud-auth-config.ts`, `profile-cloud-service.ts` (sign-out path), `src/main/runtime/relay/relay-auth-coordinator.ts`, `src/main/runtime/relay/relay-session-broker.ts` (search), `src/main/telemetry/client.ts`, `src/main/telemetry/consent.ts`, `src/main/ipc/feedback-request.ts`, `src/renderer/src/i18n/locales/en.json` (one key), `.github/workflows/` (file count), `Casks/orca.rb`.

- pv96 checks: `git log` and `git diff --stat custom/integration d3c6311a531` over `cloud/`, `.github/workflows/`, the push, telemetry and cloud-auth files named above and `src/shared/skill-share-link.ts` (none changed); `src/main/orca-profiles/` changed in one unrelated file.
- pv96 reads: `src/main/ipc/feedback.ts`, `src/shared/feedback-submit-contract.ts`, `src/main/ipc/crash-reporting-submission.ts` (diff and header), `src/main/github/issue-create.ts` (signature), `src/shared/massorca-distribution.ts` (repository constant), `src/main/bot-teams/bot-team-engram-sync.ts` (command and header), `src/main/github/gh-auth-sign-in.ts` (header), and the commit messages `e1408b9570b`, `bacdb8bae84`, `4c485fdfe5b`.

## Not read or low confidence

- `apps/relay/src` is about 300 files. Only configuration, routes, the token verifiers and file names were read. The assignment store, cell and rehome workers, drain and fence logic and the SQL were not read. CLD-05 to CLD-09 rest on route names, configuration names and test names, so their detail is low confidence.
- `apps/push/src` internals were not read: the APNs HTTP/2 transport, FCM token exchange, durable queue and device registry. CLD-10 and CLD-11 rest on the README, `push-gateway.md`, the limits and the route list.
- `apps/relay-fence-broker`: only the routes and the configuration names. The mutation lease and fence runner were not read.
- `apps/relay-ops`: only the README and routes. Collectors and the cost model were not read.
- `packages/postgres-schema` and `dev/scripts` (about 75 scripts): names only.
- Terraform: names and comments only. No plan semantics were checked. The workflow count (27 files) is a file count, not a read. The cloud README says 25.
- Desktop cloud sign-in was traced only as far as configuration and the sign-out path. Whether a shipped production build can complete sign-in is not established from this repository.
- Feedback, skill-sharing and login servers are not in this repository and were not read.
- Relay and push test suites were not read, except names where they clarify a rule.
- CodeGraph has no index for this reference worktree. Per the brief, nothing was initialised or written there, so all reading used Read, Grep and Glob.
- pv96: the Engram sync and gh sign-in modules were read at their headers and command lines only; the Engram server and OpenMetadata endpoints set in Marketplace were not traced.
- pv96: `app-platform.md` still says feedback goes to `onorca.dev` (PLAT-16 and a sentence at line 257); that text is out of date and belongs to the app-platform worker.

## Features

### CLD-01 Phone-to-host relay splice

- **What happens.** A phone and a desktop host, neither with an open inbound port, both open outbound WebSockets. The phone connects to `/v1/connect/<hostId>`. The host keeps a control socket at `/v1/host/control` and opens one data socket per connection at `/v1/host/data/<connId>`. The relay pairs the two and forwards frames. The README calls the splice an opaque end-to-end encrypted stream, so the relay does not read terminal or agent content.
- **Rules.** The first frame must arrive within 2 s. A frame may be up to 8 MiB, sized for a worktree catalogue response that already exceeds 1 MiB on large workspaces. A host takes at most 8 connections. An idle connection closes after 10 min. Control pings go out every 15 s, and 75 s of silence ends the control link. Close codes: 4401 bad outer credential, 4404 host offline, 4408 peer dropped, 4409 wrong cell, 4429 limit exceeded, 4503 draining. The splice follows a one-way state machine. An expired sign-in keeps an existing splice for 60 s.
- **Exchange.** The connecting side presents a 32-byte credential. The relay answers with a rejection (close code), an accepted invite, or an accepted resume with lease expiry and the accepted credential version (current or grace). The host sends a hello with its relay host id, assignment epoch, public key and app version, then proves the key (CLD-03).
- **Persists.** Assignment rows and credential hashes in the relay database (PostgreSQL in production, SQLite in tests).
- **Depends on.** CLD-02, CLD-03, CLD-07, CLD-16; core network pairing (plan Phase 3).
- **Status.** Later. See the open decision at the end.

### CLD-02 Pairing invites and resumable device credentials

- **What happens.** The host creates an invite. A phone uses it to pair. An invite lasts 10 min and allows 5 attempts. Attempts are spaced at least 2 s apart, and each attempt reserves the invite for 15 s. After pairing, the phone resumes with a credential that lasts 30 days. Credentials are versioned: a new version replaces the old one, and the previous version stays valid for a grace window. The host can install a new credential (an asynchronous request, polled by request id), confirm a resume within 30 s, or revoke a device.
- **Rules.** Two install modes exist: `relay-basis` (proved over an existing relay connection) and `authenticated-direct`. Resume confirmation names the base connection it confirms.
- **Exchange.** A phone sends its invite or resume credential. The relay answers with the accepted credential version and lease expiry, or with a close code (4401 for a bad credential, 4404 for an offline host).
- **Persists.** Hashes of resume tokens, current and grace versions, expiry times and relay device ids.
- **Depends on.** CLD-01; CLD-04 for the `authenticated-direct` mode.
- **Status.** Later.

### CLD-03 Host identity proof

- **What happens.** A desktop proves it holds its host key without an account. The service sends an ephemeral public key, a nonce and an encrypted challenge. The host returns a proof over a fixed transcript. Challenges expire after 10 s, and clocks may drift by 30 s.
- **Rules.** A push session lasts 24 h after a proof. The challenge and session routes are the only unauthenticated writes, and they are capped at 30 requests per minute per client IP. Logs carry a shortened host fingerprint, never the full one.
- **Persists.** Short-lived challenge and session state in the push and relay stores.
- **Depends on.** None.
- **Status.** Later. Needed only with CLD-01, CLD-10 or CLD-11.

### CLD-04 Cloud sign-in and relay tokens

- **What happens.** The desktop signs in through a browser flow (PKCE, public client id `orca-desktop`) against `login.onorca.dev`. It keeps the session per profile, refreshes it, reads the profile and organisation, and requests short-lived relay tokens (JWT, ES256, purpose `host-control`, valid 5 min). Sign-out revokes the server session and writes a local tombstone before any network call, so a refresh already in flight cannot save after sign-out.
- **Rules.** Packaged builds use built-in production endpoints. Loopback HTTP endpoints and a developer auth switch (`ORCA_CLOUD_DEV_AUTH`) are accepted only in unpackaged builds. When the cloud session is gone (absent, or cleared by a 401), relay control closes with reason `signed-out`, and the phone asks the user to sign in on the desktop.
- **Open point.** The settings copy describes cloud sign-in as a "dev-only preview, hidden in production". Whether a shipped build can sign in is not established from this repository.
- **Service side.** The login and API services, including the skill-sharing routes, live in the private `stablyai/orca-cloud` repository.
- **Persists (desktop).** A session file per profile, plus sign-out tombstones.
- **Depends on.** The private auth service; CLD-01 for relay tokens.
- **Status.** Drop. The plan has no account requirement. Revisit only if the relay is kept and the owner wants accounts.

### CLD-05 Director: host-to-cell assignment

- **What happens.** A director receives assignment requests from hosts and phones, places each host on a relay cell, and answers resolve requests so that a phone reaches the cell holding its host. It serves a region catalogue at `/v1/regions` and sends move notices to clients. A notice is trusted only when it is newer.
- **Rules.** Assignments are signed with a key of at least 32 bytes. Placement stops below each cell's socket ceiling. Cells have connection hard caps of 600, 1,000 or 3,000 (600 by default). Public assignment and sticky lanes use bounded queues with concurrency, wait and retry-after limits. Dormant assignments expire after a TTL.
- **Persists.** Durable per-host assignment rows, including the last cell, used to answer later resolve requests.
- **Depends on.** CLD-06, CLD-07, CLD-16.
- **Status.** Drop. Placement across several cells serves hosted scale. One owner's devices need at most one relay.

### CLD-06 Regions and region correction

- **What happens.** The relay runs in `us-central1` (default) and `asia-east2`. Clients can send latency measurement windows so the service can correct their region. Idle hosts can move to another region (idle regional rehome). The service answers with an outcome or a defer reason.
- **Rules.** Corrections and rehomes sit behind a placement flag and a cohort percentage. Defer reasons separate global causes from per-region causes. Hosts advertise the `idle-regional-rehome-v1` capability in a header.
- **Persists.** Placement version and correction state in the relay database.
- **Depends on.** CLD-05, CLD-08.
- **Status.** Drop. Latency optimisation for a hosted service.

### CLD-07 Admission control and overload shedding

- **What happens.** When a cell is saturated, new connections are refused with close code 4429 or held in a queue with a retry-after hint. Operators read the state through status routes.
- **Rules.** Admission budgets: 45 pre-auth connections in total, 4 per source, and 30 pre-auth attempts per source per minute. Reserved capacity: 100 host control sockets and 150 host data sockets. Process queue ceiling 64 MiB. Splice backpressure watermarks at 64 KiB and 256 KiB. A splice that stays wedged for 10 s is closed. Test names also cover a public-assignment circuit breaker and overload behaviour.
- **Exchange.** An overloaded relay answers new connections with close code 4429, or with a queued response that carries a retry-after hint.
- **Persists.** Nothing user-facing. Counters are aggregate.
- **Depends on.** CLD-01.
- **Status.** Later. Any relay needs limits, but these numbers were tuned for Cloud Run and GCE cells and must be measured again for a new design.

### CLD-08 Drain, evacuation and host migration

- **What happens.** Operators drain a cell so it stops taking hosts, evacuate hosts from a cell, and move hosts between cells or regions. Each change is a prepared attempt that is sent, receipted, planned, started, then completed or aborted. A failed cell can be superseded.
- **Rules.** Migrations run safety checks, a trust probe and an inventory census. Control messages carry drain instructions (the drain schema, and tests named for rebind).
- **Persists.** Attempt records, drain receipts, cell states and host assignments.
- **Depends on.** CLD-05, CLD-09, CLD-14.
- **Status.** Drop.

### CLD-09 Operator API and access tiers

- **What happens.** 36 `/v1/admin/*` routes let operators read status, drain, evacuate, apply admission selections, change cell configuration, run fence attempts, and preview or control regional rehomes.
- **Rules.** Each admin call carries a Google-signed identity token (RS256) for a service account. The token is verified against Google's JWKS with an admin audience and a verified email that must match a configured service account. Identities are `deploy` (every admin route), `capacity`, `asia-proof`, `monitor`, `fence` and `fence-broker`, and each may call only its own route list. The monitor list is mostly status routes but includes one mutating route, regional rehome control.
- **Exchange.** An operator tool sends an admin call with a Google-signed identity token. The relay checks the identity against that identity's route list and answers with the status or the operation result.
- **Depends on.** CLD-05, CLD-14.
- **Status.** Drop.

### CLD-10 Phone push token registration

- **What happens.** After pairing, the desktop registers each phone's native Android push token with the push gateway, lists its registrations and deletes them. The gateway retires a registration as soon as Google reports the token unregistered.
- **Rules.** A host may hold at most 64 registrations. Request bodies are capped at 16 KiB. The APNs path for iOS is out of scope, so it is not needed for the first version.
- **Persists.** A host-scoped device registry (PostgreSQL in production, SQLite in tests).
- **Depends on.** CLD-03, CLD-16.
- **Status.** Later.

### CLD-11 Push delivery and live dismissal

- **What happens.** After its own socket fan-out, the desktop calls `POST /v1/send` with one notification. The gateway queues each event as its own provider notification, for up to 20 registrations per send, and the phone shows it as an OS notification. A dismiss event carries a notification id and removes the banner. The notification socket is kept only for live dismissal and for tray reconciliation after a reconnect. It never creates a banner.
- **Rules.** Title at most 80 characters, body at most 180, and the serialised notification at most 3,000 bytes. A notification expires after 5 min. A host may send 300 events per 15 min. Authenticated requests are limited to 600 per host and 6,000 per IP per minute. FCM collapse can merge alerts while the phone is offline, so not every pending alert is guaranteed. Logs carry counters only: no tokens, titles or bodies. The desktop's notification categories stay authoritative.
- **Persists.** One durable queue row per notification event, claimed by workers. Queue format changes require stopping older revisions first (runbook).
- **Depends on.** CLD-10, CLD-03; the desktop's notification categories.
- **Status.** Later.

### CLD-12 Push gateway deployment and recovery

- **What happens.** Operators deploy `orca-cloud-push` as a public Cloud Run service in production only. It runs with a minimum of 1 and a maximum of 2 instances, a pool of 6 database connections per instance, and a dedicated HA PostgreSQL 17 database named `orca_push`.
- **Rules.** There is no staging gateway, by decision. Apple key material lives in Secret Manager, written out of band: Terraform owns the secret names and never the versions. The Apple secrets and the database carry `prevent_destroy`. Key rotation and recovery follow the runbook. Authentication is the host proof, not Cloud Run IAM.
- **Depends on.** CLD-11, CLD-15, CLD-16.
- **Status.** Drop.

### CLD-13 Relay operations console and incident monitor

- **What happens.** A loopback-only web console (`pnpm ops:relay`, port 2455) shows aggregate monitoring points, resource health, immutable image digests, alert-policy metadata and workflow runs for production and staging. It can be shared through Tailscale. An incident monitor samples health and can freeze a rollout when a threshold fails. Its dry-run history is recorded in `cloud/docs`.
- **Rules.** Read-only by default. Staging wake and sleep controls are opt-in, local-only, CSRF-protected, and only dispatch a workflow that requires a typed confirmation. Account, host, device, pairing and assignment identifiers never reach the browser. Missing inventory shows as Unknown, not Sleeping. Cost figures are labelled as planning estimates, because no billing export exists.
- **Exchange.** The local server reads cloud and GitHub data on the operator's behalf and sends only the aggregate payload to the browser. Controls are posted back through a loopback, CSRF-checked route.
- **Depends on.** CLD-05, CLD-15.
- **Status.** Drop.

### CLD-14 Mutation lease and fence broker

- **What happens.** A private, IAM-only service holds a durable mutation lease and performs the narrow compute change used when a registered relay target is superseded, or when a source cell is fenced. Requests carry an operation id, a request digest, the expected lease generation, and the typed confirmation `SUPERSEDE_TARGET`.
- **Rules.** A completed fence recovery is passed back with its attempt id, GCE operation, Terraform state serial and object generations. The calling workflow holds only read and invoke rights.
- **Persists.** The lease object in a Cloud Storage bucket, with generation checks. Terraform state references.
- **Exchange.** The calling workflow sends a lease-checked operation request. The broker answers from its lease and operation runner with the operation outcome.
- **Depends on.** CLD-08, CLD-15.
- **Status.** Drop.

### CLD-15 Infrastructure as code and gated deploy workflows

- **What happens.** Terraform provisions the relay: cells, director, a shared Cloud SQL instance, DNS, observability and Workload Identity providers. The foundation and apps roots live in the private repository. Deploys and operations run through 27 `cloud-*` workflow files.
- **Rules.** Every top-level job is gated on the repository variable `ORCA_CLOUD_OPERATIONS_ENABLED`, which is unset here, so scheduled and manual runs skip without steps. A Cloud SQL rollout lease serialises rollouts. Relay Workload Identity providers accept only `stablyai/orca`. `Cloud Verify` runs on every change under `cloud/`, including fork pull requests, and holds no credentials.
- **Depends on.** CLD-16.
- **Status.** Drop. The first version needs no hosted infrastructure.

### CLD-16 Relay and push storage and aggregate observability

- **What happens.** Relay and push keep state in PostgreSQL in production and in SQLite for local development and tests. The shared `postgres-schema` package applies schemas with lock targets and catalogue prechecks. Each service exposes health and readiness endpoints (`/health`, `/ready`). Logs carry aggregate counters.
- **Rules.** Readiness has its own JWKS and SQL grace periods. Relay tests that need PostgreSQL run only when a disposable PostgreSQL 16 or 17 URL is provided.
- **Depends on.** None on its own. Used by CLD-01, CLD-05 and CLD-10.
- **Status.** Later. Needed only if CLD-01 or CLD-10 is built. Dropped with them otherwise.

### CLD-17 Feedback and crash reports as GitHub issues (later, S)

- Source: pv96 (unmerged). The desktop no longer posts feedback to MassOrca's own intake; reports become issues in the project's repository (`e1408b9570b`, `src/main/ipc/feedback.ts`). PLAT-16 still describes the old route.
- **What happens.** The feedback form and a crash report call the same submit path, which opens an issue through whichever account `gh` holds active. The title is the first line of the report with a "Feedback:" prefix, cut at 80 characters. No logs or images are attached. The phone keeps only a copy action.
- **Rules.** The target repository is pinned in code, because issue-source resolution on a fork can pick the upstream repository. Diagnostic-bundle uploads refuse the upstream endpoint. The code adds no sign-in of its own.
- **Exchange.** The renderer hands the text to the main process, which creates the issue and returns its URL or an error.
- **Persists.** Nothing on MassOrca's side; the issue lives in the project's tracker.
- **Depends on.** PLAT-16 (report crash feedback); a GitHub login through `gh` (SCM). No MassOrca cloud service.
- **Status.** Later.

### CLD-18 Engram memory refresh for Bot Teams (later, S)

- Source: pv96 (unmerged). Bot Teams Memory refreshes a project's memories from the Engram cloud before reading them (`bacdb8bae84`, `4c485fdfe5b`); the integration text has no such step.
- **What happens.** Before the host lists a project's memory, it runs `engram sync --import --cloud --project <project>`. The import runs at most once every five minutes per project, and again when Memory opens; concurrent reads share one import. If the import fails, the local memories still show with a note that they could not be refreshed.
- **Depends on.** AUTO-31 (team files and memory); the external `engram` CLI and the cloud login it holds.
- **Status.** Later.

## Open decision

One real fork decides the cloud scope. Must the first version let an Android phone reach a desktop host that cannot accept inbound connections? If yes, CLD-01 to CLD-03 move into the first version (sizes L, M and S). The new protocol would then need its own pairing and identity design, because the current relay requires a cloud session. If no, the whole `cloud/` directory stays out of the first version.
