# 0099: Claude account state file

Status: accepted, 2026-11-24.

## Context

A launch on an account home sets the kind's home variable (decision 0059). For Claude that is `CLAUDE_CONFIG_DIR`, and
with it set Claude Code reads both `.credentials.json` and its own global state `.claude.json` from that folder.

- When `.claude.json` is missing, or lacks `hasCompletedOnboarding`, a session opens Claude Code's first-run setup,
  which ends in a browser sign-in.
- The account check, `auth status`, judges only the credentials. It reports the account as signed in, so the account
  list says "Signed in" while the session asks for a sign-in.
- The person's own `.claude.json` is often over 1 MiB, mostly project history. The import of decision 0097 skips it as
  an unusable optional file, so imported accounts had no state file at all.
- The previous product (pv96) solved this by merging each profile's `.claude.json`: it set
  `hasCompletedOnboarding: true` and shared only selected keys, never the whole file.

## Decision

- A kind's `credentials` may declare a `state` rule:
  `{ file, keep: [keys], set: { key: value }, max_source_bytes }`.
  - `file` must be a declared credential file. `keep` and `set` name plain top-level keys, each once and in only one
    of them. `max_source_bytes` defaults to 1 MiB and must be between 1 byte and 64 MiB. The kind's problems list each
    breach.
- The built-in `claude` declares `.claude.json`, keeping `oauthAccount`, `theme` and `mcpServers`, setting
  `hasCompletedOnboarding` to `true`, with a 16 MiB source cap. The file stays optional. No other kind declares a rule.
- Import builds the state file and never copies it raw, from a folder or from detected sources.
  - The source is read with the import's checks (no link, a regular file, the same file throughout) and with
    `max_source_bytes` as its cap.
  - Only the `keep` keys present in it are written, then the `set` keys. Project history and every other key stay
    behind.
  - A missing source gives a file with the `set` keys only.
  - A source that is not a JSON object, or over its cap, refuses the import when the file is required. When it is
    optional, the file gets the `set` keys only, and the import says why in its `reason`.
  - `agent.accounts.import_candidates` judges the state file against its own cap, not 1 MiB.
- Before every account check and every launch on an account home, the service repairs the state file under the homes
  gate.
  - A missing file is created with the `set` keys. A file whose `set` keys are absent or differ gets them merged in;
    every other key is kept. A file that already holds them is not written.
  - The write goes to a new owner-only sibling, renamed over the file. A link or anything but a regular file is never
    followed or written through.
  - A file that cannot be repaired (not a JSON object, over its cap, a link) is left as it is. The check reports
    `needs_sign_in`, and a launch is refused with `agent.credentials_invalid` naming the file, so the check and the
    session never disagree.

## Consequences

- Imported Claude accounts open a session without the first-run setup, and accounts imported before this decision are
  repaired at their next check or launch, without a new import.
- Only the three kept keys travel from the person's own state into an account home. Project history, per-project
  settings and other state do not.
- The repair may rewrite a state file while a session of the same account writes it too. It only writes when a `set`
  key is missing or differs, so this happens at most once per change.
- Folder trust prompts, which Claude Code keeps per project in the same file, are out of scope. A session may still ask
  to trust a workspace folder.
- A kind declaration gains an optional field. A peer that predates it refuses a declaration that uses it, since
  declarations deny unknown fields.
