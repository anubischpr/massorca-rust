# 0176: Account credential access

Status: accepted, 2026-11-27.

## Context

The usage feature shows how much of a vendor plan an account has used. The vendor serves that from an endpoint that
takes the account's own credential, so the service must read the credential file of an account (decision 0059) and
send it to the vendor. Until now no code read the contents of a credential file: import copies files, show reports
names, sizes and times, and launch only points the agent at the home. This decision adds the one place that reads the
contents, and the rules around it.

## Decision

- **API.** `Agents::with_account_credential(kind, account, url, |checked: &CheckedUrl, secret: &Secret| -> T) ->
  Result<T, CredentialAccessError>` in `massorca-core-agents` (`account_credential.rs`). It lends the account's access
  token to the closure beside the checked URL, and returns only the closure's own value. It is blocking (store and
  disk reads); an async caller runs it on a blocking thread.
- **Refusal order.** The switch is off (`UsageDisabled`); the URL is refused (`Endpoint(EndpointRefused)`, which
  includes a kind with no official origin); the kind is unknown or invalid; this crate knows no token for the kind
  (`TokenUnsupported`); the account is not one of the kind's; a link or bad file (`Refused(AgentError)`); no access
  token in the file (`TokenMissing`). The first two touch neither the store nor the disk.
- **One global switch.** It runs only while the setting `usage.enabled` (owner `core`, bool, default false) is on.
  There is no per-account opt-in. `AgentsConfig.usage_gate` is a `UsageGate`, a shared `Fn() -> bool` asked at every
  call and never cached; its default is off. Off, the call returns `CredentialAccessError::UsageDisabled` before it
  touches the store or the disk. The server builds the gate over the settings store: only a stored JSON `true` is on;
  unset, unreadable or any other value is off. The key is read by name; declaring it on the Settings page belongs to the
  usage feature's own change, so this decision does not register it.
- **The same checks as showing credentials.** In order: the kind is known and valid (`declared`), the kind has an
  official vendor origin (below), the account is a row of that kind (`account_of`, `agent.account_not_found`
  otherwise), then, under the homes lock, `usable_account_home` (a valid account id, `<profile>/agents` and
  `accounts/<kind>` plain directories, the home a plain directory) and `read_capped_into` (the file a regular file,
  opened without following a link, unchanged between the check and the open, at most 1 MiB). These are the functions
  show, import and launch already use; none is re-implemented. Between those checks and the read, the directories from
  `<profile>/agents` to the home are held (`hold_account_chain`): checked again and, on Windows, pinned through the
  same `hold_plain_dir` and `PinnedDir` that `root_dir` uses for writes, so none can be renamed or swapped for a
  junction while the file is read. Elsewhere the second check and the no-follow open stand alone. A refusal carries the code showing credentials would
  give, in `CredentialAccessError::Refused(AgentError)`; its message names the file and why, never anything read. The
  homes lock is released before the closure runs, so a slow vendor call never blocks launches or imports.
- **Which file, and only the token.** The credential file is the first required file the kind declares:
  `.credentials.json` for `claude`, `auth.json` for `codex`. It must be UTF-8 text. Only its access token is lent:
  `claudeAiOauth.accessToken` for `claude`, `tokens.access_token` for `codex` (`access_token.rs`); the refresh token
  and the rest of the file never leave core-agents. The file is read exactly (`read_exact_capped`: a zeroed buffer of
  the size the opened handle reports, `read_exact`, then a one-byte read that must find the end), so no std buffer or
  stack probe holds a copy, and a file that changes size while read is refused. It is parsed in place: the token is a
  slice of the zeroed buffer, copied once into a `Zeroizing<String>`; a string with escapes, which would need an
  unescaped copy, is refused. A missing, repeated, empty or non-string token is `TokenMissing`, which names the kind
  and account, never a value; parse errors are dropped, since their messages may quote what was read. The two shapes
  come from the owner's instructions: no fixture or fake agent in the repository has either, and no real file was
  read to confirm them.
- **The secret.** `Secret` wraps a `Zeroizing<String>` holding the token: its bytes are zeroed when it drops, and so
  is the whole file buffer, which is dropped before the closure runs. `Debug` prints `[redacted]`; there is no `Display`, `Serialize` or `Clone` (compile-fail doc tests hold
  this). The value is never logged, never put in an argument list or a child's environment, and never put in a protocol
  type: this API has no wire method, and nothing returns the value over the wire.
- **Official endpoint only.** `vendor_endpoint::official_origin(kind)` names the one https origin a kind's credential
  may go to: `https://api.anthropic.com` for `claude`, `https://chatgpt.com` for `codex`, none for any other kind. The
  table is code, not kind data, so a kind declared in a profile or a package never names where a credential goes. A
  kind with no origin is refused before anything is read. `with_account_credential` itself calls `vendor_endpoint::check_url(kind, url)` before anything else but the switch, and
  a refused URL never reaches the store or the home. The check returns a `CheckedUrl`, which only the check can make,
  and the closure receives it beside the secret. The scheme must be https and the origin
  (host and port, the default port allowed written or not) exactly the official one. Its parse is strict on purpose, so
  it never reads a host other than the one an HTTP client would connect to: non-ASCII, a user part (`@`), a backslash,
  a percent sign, an IPv6 literal, a trailing dot, an empty or zero port are refused, not interpreted. A refusal never
  repeats the URL.
- **Standing overrides never apply.** The origin comes from the code table only: the credential path never consults
  standing kind settings (AG-11), a `*_BASE_URL` variable such as `ANTHROPIC_BASE_URL`, or proxy variables. Those
  reach agent runs only; a test runs `check_url` with `ANTHROPIC_BASE_URL` set to a loopback origin and sees it refused.
- **Redirects and proxies.** No HTTP client crate (reqwest, ureq) is a workspace dependency, so no client helper is
  added here. `CheckedUrl`'s documentation requires the caller to build its client with redirects disabled, https
  only and proxy variables ignored (`reqwest`'s `.no_proxy()` or the like), and to treat a 3xx answer as a failure: a
  followed redirect or a proxy would carry the token where the check never looked. When a client crate is added, a
  helper that builds such a client belongs next to `CheckedUrl`.
- **Fake endpoints in tests.** `vendor_endpoint::test_hooks::override_origin(kind, origin)` gives a kind a fake origin
  while its guard lives. It is compiled only under `cfg(test)` and the existing `test-hooks` feature, which
  `tests/test_hooks_gate.rs` already forbids outside dev-dependencies, and it accepts only a loopback origin
  (`127.0.0.1` or `localhost`, no path). Reason: `cfg(test)` alone would not reach the usage feature's tests in another
  crate, and an environment variable would be one more input a release build reads. Even a leaked override cannot send
  a credential off the machine.

## Consequences

- `AgentsConfig` gains `usage_gate`. Breaks literals of `AgentsConfig`: the server's and the core-agents test service's
  are updated.
- Tests use temporary profiles, fake credential files and loopback origins only; no test reads a real vendor home or
  calls a vendor.
- The usage feature owns the request itself: it sends to the `CheckedUrl` over TLS with redirects and proxies off,
  and maps `CredentialAccessError` onto its own wire codes. A new vendor is one line in each of the origin and token
  tables, with their tests. Codex's usage endpoint may also want the account id from `auth.json`; it is not lent, and
  adding it would be a further, explicit change to this decision.
- Not covered: on Unix a directory swapped between the second check and the open is not pinned; the no-follow open
  only guards the file itself. A file symlink test on Windows skips when symbolic links cannot be made, as the import
  test does.
