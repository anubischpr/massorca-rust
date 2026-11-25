# 0100: Extension rollback and run log

Status: accepted, 2026-11-25.

## Context

An update (decision 0062, Updates) replaced a package's files in place and deleted the old version, so a bad update
could not be undone. Nothing recorded what installs, updates and removals a package had gone through.

pv96 keeps one rollback version per plugin: a predecessor kept beside the current one, and pruned when the next install
lands (`src/main/plugins/plugin-install.ts`, `plugin-install-publication.ts`). pv96 has no persisted record of install,
update or removal runs. Its only per-plugin log is an in-memory ring of 200 worker output lines, cleared on removal
(`src/main/plugins/plugin-uninstall-log-retention.test.ts`). The run log below therefore follows the installer output
rules of decision 0092 and about 20 runs, as the orchestrator set.

## Decision

- **Keep one previous version on update.** A lock entry records the version an update replaced as `previous` (its
  version, source, hash, files hash, path, consent and agent kinds). Its folder `pkgs/<id>/<version>-<hash8>/` is kept.
  The version before it is deleted when the next update lands. A built-in refresh keeps its replaced version the same
  way. The startup sweep counts the kept folder as referenced, so it is never swept. A lock entry without `previous`
  has nothing to roll back. The field is additive, so lockfile version 1 is unchanged.
- **Roll back.** `extensions.rollback { id, consent?, agent_token? }`, host administration, capability
  `extensions-rollback/1`. It swaps `previous` in and keeps the current version as the new `previous`, so a second
  rollback goes forward again.
  - Consent is that of an update: the review of the restored bytes must be accepted again. Without it the refusal is
    `extension.consent_required` and its data carries the restored `version` and `consent`. A built-in needs none.
  - Nothing kept is refused with `extension.no_previous_version`. A kept folder that does not hash to the lockfile is
    refused as `extension.tampered`.
  - The enabled state, install time, CLI registrations and unknown fields stay. Rollback never runs a vendor installer
    or uninstaller, and never touches the vendor CLI, accounts or `data/<id>`.
  - An agent run may not roll back, as for an update.
- **Run log.** Each install, update, rollback and remove that changes a package adds one run to
  `logs/<id>-runs.json`: time, action, version from and to, outcome (`ok` or `failed`) and the output tail. The newest
  20 runs are kept. The output tail is the bytes that run's vendor command wrote, redacted and capped by the
  install-log rules. A change refused before it touches anything is not a run. An install whose vendor command ran and
  then failed is a `failed` run. The log stays after removal, so the removal is in it. Writing it is best effort: the
  lockfile is the record of what is installed.
- **Show the log.** `extensions.log { id }`, read permission, capability `extensions-log/1`. The output lines are empty
  for a caller without host administration. An id that is not an extension id is refused as `extension.not_found`.
- **CLI.** `massorca extension rollback <id>` takes consent as `extension update` does: `--yes` accepts the review the
  refusal printed, and `--consent <FINGERPRINT>` accepts an earlier run's review. `massorca extension log <id>` prints
  the runs oldest first.

## Consequences

- Each update keeps one more copy of the package's files on disk until the next update replaces it.
- A rollback restores files only. If the older version needed a vendor CLI that an update changed, that CLI is not
  restored.
- The run log is best effort and can miss a run if its file cannot be written. The lockfile stays correct.
- The wire change is additive: two capabilities and two methods. An older host answers `method.not_found`.
- The built-in rollback path is not yet covered by a test, because the catalogue offers one version per build.
- The desktop More menu entries follow in a later change.
