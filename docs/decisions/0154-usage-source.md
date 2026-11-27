# 0154: Agent account usage from local logs, behind one switch

Status: accepted, 2026-11-26. Covers ACC-09 (Claude usage and limits), ACC-10 (Codex usage and limits) and ACC-15
(usage refresh cadence and caching) in `docs/inventory/coverage.md`, on top of the accounts of 0059 and the settings of
0070.

## Context

The desktop rail rings drew sample data: the service had no usage source and no cadence. Each agent account already has
its own vendor config folder (`CLAUDE_CONFIG_DIR` or `CODEX_HOME`), and the vendor CLIs write their session logs there.
Codex records the rate limits the vendor reported with each token count. Claude Code records each reply's token counts
but no limit. Reading usage from the vendor over the network needs the account's credential, and the service has no API
that hands a credential out yet.

## Decision

- **One switch.** The core setting `usage.enabled` (boolean, default `false`) in the existing settings store turns usage
  on for the host. It is not per account. While it is off nothing is read or kept, and every answer is
  `collection: "off"`, never an empty list. Turning it off drops the cache and announces each dropped account.
- **Local data only.** A reading comes from the vendor CLI's logs under the account's folder, found through
  `paths::valid_account_home` for the ids the store lists. Only files that hold no secrets are read, and only their
  numbers are kept: never `.credentials.json`, `.claude.json`, `auth.json` or `config.toml`. Walks do not follow links
  and stop at fixed bounds (8 days back, 400 newest files, lines up to 1 MiB).
  - Claude: `projects/*/*.jsonl`. Each reply's tokens (input, output, cache creation and cache read) are counted once per
    message and request id. The session window opens at the hour of the first reply after the last one closed and lasts
    five hours; the weekly window is the last seven days. The percent is unknown, except that a reply saying the limit
    was reached marks the window `limited` at 100% until its reset.
  - Codex: `sessions/YYYY/MM/DD/rollout-*.jsonl`. The newest `token_count` record with `rate_limits` wins. A window of
    up to a day is the session limit, a longer one the weekly limit. Once a recorded reset has passed, its percent is
    unknown, not zero and not the old figure.
- **Source seam.** `usage::UsageSource` is one way to read a vendor; `usage::sources(kind)` lists a kind's sources in the
  order they are tried, and the first with a reading wins. The local logs are the last source of each kind, so a vendor
  endpoint added before them falls back to them. Readings carry the source name (`local_logs`).
- **Cadence and cache.** The service keeps one reading per account in memory. A refresher reads each account when it is
  due: every 60 seconds after a good read, and after failed reads waits that double up to 15 minutes. One fetch per
  account runs at a time; a fetch within 5 seconds of the last answers the cache. Reads never fetch: an account not read
  yet answers `pending` and wakes the refresher. An error state carries no figures, so stale numbers never show as
  current.
- **Wire.** Capability `usage/1`: `usage.list` (read, from the cache) and `usage.refresh` (read, one coalesced fetch then
  the list), both taking an optional `kind` and `account`, and the topic `usage.changed` with id `<kind>/<account>`,
  published when what a reader shows changes. All fields beyond the new types are optional, so the change is additive.
- **Each host for itself.** The host that runs the agent reads its accounts and answers for itself only. A client reads a
  paired host through the existing host link (`massorca --host NAME usage`); the service opens no connection to another
  host. Aggregating hosts is the desktop hub's job over its live hosts.
- **CLI.** `massorca usage [--kind] [--account] [--refresh]`, with the global `--host` and `--json`.

## Consequences

- Claude windows show tokens, not a percent, until a vendor source exists.
- A reading is as fresh as the vendor's last log line; `observed_at_ms` says when that was.
- The cache lives in memory: a restart starts with `pending` readings.

## Remaining

- A stacked follow-up adds vendor endpoint sources behind the same switch, through the agents session's
  `Agents::with_account_credential` (closure-only access to the zeroized secret). Its rules: the credential goes only to
  that vendor's official endpoint over TLS; results are cached with backoff and respect the vendor's rate limits; nothing
  is sent anywhere else and each host queries only its own accounts; no query runs while the switch is off; tests use a
  fake vendor endpoint.
- A byte budget per refresh: the walks bound files and line length, not the total bytes one refresh reads across an
  account's logs.
