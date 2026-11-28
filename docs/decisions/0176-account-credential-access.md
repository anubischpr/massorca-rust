# 0176: Account credential access

Status: accepted, 2026-11-27.

## Context

The usage feature shows how much of a vendor plan an account has used. The vendor serves that from an endpoint that
takes the account's own credential, so the service must read the credential file of an account (decision 0059) and
send it to the vendor. Until now no code read the contents of a credential file: import copies files, show reports
names, sizes and times, and launch only points the agent at the home. This decision adds the one place that reads the
contents, and the rules around it.

## Decision

- **API.** `Agents::with_account_credential(kind, account, |secret: &Secret| -> T) -> Result<T, CredentialAccessError>`
  in `massorca-core-agents` (`account_credential.rs`). It lends the contents to the closure and returns only the
  closure's own value. It is blocking (store and disk reads); an async caller runs it on a blocking thread.
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
  show, import and launch already use; none is re-implemented. A refusal carries the code showing credentials would
  give, in `CredentialAccessError::Refused(AgentError)`; its message names the file and why, never anything read. The
  homes lock is released before the closure runs, so a slow vendor call never blocks launches or imports.
- **Which file.** The credential file is the first required file the kind declares: `.credentials.json` for `claude`,
  `auth.json` for `codex`. It must be UTF-8 text.
- **The secret.** `Secret` wraps a `Zeroizing<String>`: its bytes are zeroed when it drops, and the read buffer is
  reserved once at the file's size, so no reallocation leaves a copy behind. A file that is not text is zeroed before
  the refusal. `Debug` prints `[redacted]`; there is no `Display`, `Serialize` or `Clone` (compile-fail doc tests hold
  this). The value is never logged, never put in an argument list or a child's environment, and never put in a protocol
  type: this API has no wire method, and nothing returns the value over the wire.
- **Official endpoint only.** `vendor_endpoint::official_origin(kind)` names the one https origin a kind's credential
  may go to: `https://api.anthropic.com` for `claude`, `https://chatgpt.com` for `codex`, none for any other kind. The
  table is code, not kind data, so a kind declared in a profile or a package never names where a credential goes. A
  kind with no origin is refused with `NoVendorEndpoint` before anything is read. `vendor_endpoint::check_url(kind,
  url)` is the caller's gate before every request that carries the secret: the scheme must be https and the origin
  (host and port, the default port allowed written or not) exactly the official one. Its parse is strict on purpose, so
  it never reads a host other than the one an HTTP client would connect to: non-ASCII, a user part (`@`), a backslash,
  a percent sign, an IPv6 literal, a trailing dot, an empty or zero port are refused, not interpreted. A refusal never
  repeats the URL.
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
- The usage feature owns the request itself: it calls `check_url` before sending, sends over TLS, and maps
  `CredentialAccessError` onto its own wire codes. A new vendor origin is one line in the table and its test.
- Not covered: the Windows pins `import` holds while it writes are not taken for this read; the link checks and the
  no-follow open stand alone, as they do for show and launch. A file symlink test on Windows skips when symbolic links
  cannot be made, as the import test does.
