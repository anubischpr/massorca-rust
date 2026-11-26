# 0166: Model catalog for chats

Status: accepted, 2026-11-25.

## Context

Decision 0096 gave each kind the models and effort levels its picker offers, as a static seed. It also said that the
reference application (pv96) discovers each account's models from the vendor CLI, and that discovery was not ported, so
an account-scoped kind showed no models. AG-23 asks for a service-side catalog that merges the seed with what the
installed CLI reports for the account, so the picker and the chat composer can read one list.

## Decision

- **A new method and capability.** `agent.models { kind, account?, refresh? }` (capability `agent-models/1`, decision 0166)
  returns `{ models: [{ id, label, default?, efforts?, source }], fetched_at_ms?, error? }`. `source` is `declared` for a
  model the kind's `model.choices` names and `discovered` for one its vendor CLI lists and the declaration does not name.
  `efforts` follows `model.choices`: absent means every level of the kind, `[]` means the model takes no effort.
  `error` is `{ code, message }`, with `code` one of `cli_missing`, `not_signed_in`, `timed_out`, `failed` or
  `no_models`. A failed listing is data: the declared models are still returned. `agent.kinds` is unchanged.
- **Discovery is data in the kind declaration.** `model.discover` names the listing: `args` after the kind's program,
  an optional `stdin` written then closed, and a `format`. Two formats exist. `lines` reads `id<TAB>label` per line
  (`agy models`). `claude-control` reads a stream-json `control_response` whose `models` lists the CLI's picker rows
  (Claude's `list_models` control request, sent on stdin of a `-p` run). An unknown format is a declaration problem.
- **Kinds that discover.** `antigravity` (`agy models`) and `claude` (the `list_models` request, a short non-interactive
  run; pv96's comment says it starts no API turn, which was not checked against a real CLI here). `codex` is not discovered: its model list comes from the app-server JSON-RPC session,
  which is not a plain command. `gentle-shell` declares no model option (its model is set live over RPC, decision 0096),
  so a listing would have nowhere to go. No built-in kind runs `pi --list-models`, so the Pi table is not parsed.
- **Merge.** The declared choices come first, in declared order, with their label, default and efforts, even when the
  listing does not name them (decision 0096: a retired model stays until the declaration changes). Listed models the
  declaration does not name follow in listed order. A listed model is never the default. An id is kept only when it is a
  valid option value (decision 0094), so a value a start would refuse is not offered: Claude's bracketed ids, such as
  `opus[1m]`, are skipped.
- **Account scope.** The listing runs with the account's home variable (`credentials.home_env`) set to the account's
  home, read as a run reads it: the named account, else the active one, else the vendor's usual home when the kind has
  never had an account. The names a check removes (`account_strip_names`, which includes the run identity) are removed
  from its environment. The process is started with `massorca_process::command`, runs under a 15 second limit, and its
  stdout is read to at most 8 MiB. A stopped listing takes its process tree with it.
- **Cache.** Held in memory per (kind, account id), or per kind for the usual home. A fetched listing is served for
  10 minutes (`LISTING_FRESH_MS`). A failed listing is served for 30 seconds (`FAILURE_FRESH_MS`) and keeps the last
  answer beside it. `refresh: true` asks again. At most 256 listings are kept. A successful listing replaces the earlier
  one only after the vendor CLI answers.
- **Invalidation.** An account's answer is dropped when the account is signed in, imported (an import that updates an
  existing account too), removed, or discarded after a failed sign-in. When the kind's active account changes, the
  usual home's answer and those of the previous and the new active account are dropped, since the default request uses
  them. A kind whose package is removed loses every answer it has. A listing that was running when its account changed
  answers the calls that waited for it, but caches nothing.
- **Dedupe.** Concurrent requests for one (kind, account) share the listing already running. One call runs the vendor
  CLI and the others wait for its answer on a condition variable, so no thread starts per waiter. A refresh skips a
  fresh answer but still joins a running listing.
- **`not_signed_in`.** When a listing for an account fails with `failed` or `no_models`, the account's check decides
  the code: `not_signed_in` when it reports the account needs a sign-in. A timeout is never re-checked, so a slow CLI
  does not wait twice.
- **Refusals.** A kind that is not enabled is refused with `agent.kind_not_enabled`, as a start refuses it. An account
  of another kind, or an unknown one, is refused with `agent.account_not_found`. An account with no usable home is refused
  with `agent.credentials_invalid`, as a run refuses it.
- **Testing seam.** `MASSORCA_MODEL_LISTING_TIMEOUT_MS` replaces the 15 second limit, in whole milliseconds, for tests.

## Consequences

- Schema: none. Wire: `agent.models`, its params and result are additive, and the capability is gated on the client.
- The built-in `claude` and `antigravity` declarations gain `model.discover`. Each packaged copy under
  `packages/agents/massorca.<kind>/agents/` stays byte-identical to its built-in declaration.
- The cache lives in the service process. A restart empties it. The invalidation rules above drop what an account
  change makes wrong, and the TTL bounds anything they miss.
- Not in this version: the state-directory store that AG-23 names (the cache is in memory), the grouping of agy's
  effort variants (pv96's `groupAntigravityModelVariants`, so `agy` ids that differ only by effort are listed one by
  one), a Codex listing, and the desktop picker's use of `agent.models` (the desktop continues to read `agent.kinds`).

## Amendment: versioned model labels (2026-11-26)

The desktop shows a model choice's `label` as `agent.kinds` gives it, and the built-in declarations name only the family
(`Opus`), so every chip read "Opus" with no version.

- **What the vendor reports.** Claude's `list_models` rows carry `resolvedModel`, the model the alias runs now (for
  example `opus` resolves to `claude-opus-5-5`). The catalog keeps it beside the listed row. `agy models` prints
  `id<TAB>label` and names no resolved id; its label already names the model as the vendor shows it, so it is kept
  as listed. Codex is not listed (see above); its declared choices are concrete versioned ids (`gpt-5.5`) whose labels
  already carry the version.
- **The label is derived, never looked up.** A resolved id of the shape `<vendor>-<family>-<n>[-<n>...]` (alphabetic
  words, numbers of one to three digits, an optional trailing eight-digit date, an optional bracketed variant) reads as
  the family title-cased and the numbers joined by dots: `claude-opus-5-5` is `Opus 5.5`, `claude-haiku-4-5-20251001`
  is `Haiku 4.5`. Any other shape, or no resolved id, keeps the declared label. No table maps names to versions.
- **Where it shows.** `agent.models` gives the declared row the derived label. `agent.kinds` gives
  `model.choices[].label` the derived label from the catalog's last answered listing for the kind's default request
  (its active account, else the usual home when the kind has never had an account). `agent.kinds` never starts a
  listing: the label follows the catalog when a listing answers or is refreshed, and stays declared until then.
  A discovered row keeps the vendor's own label.
- **No new field.** Nothing in the desktop, the CLI or the tests matches on a model's label, so `label` changes meaning
  in place ("the label the vendor's current model reads as") and no `version` field is added. Wire: no shape change.
