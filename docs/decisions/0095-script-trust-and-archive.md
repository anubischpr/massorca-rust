# 0095: Script trust and archive scripts

Status: accepted, 2026-11-24. Extends 0086 (inventory CUS-01).

## Context

0086 runs `scripts.setup` of a new worktree's `orca.yaml` on every create by a `workspace` caller. Anyone who can
commit to the repository therefore chooses a program the host runs. 0086 also left `scripts.archive` unread.

The reference (pv96, read-only at commit d3c6311a531) does both:

- **Trust.** `src/renderer/src/lib/ensure-hooks-confirmed.ts` asks before a `setup`, `archive`, `issueCommand` or
  `vmRecipe` script runs. It hashes the trimmed content with SHA-256 (`orca-hook-trust.ts`), compares it with the hash
  stored per repository and kind (`trustedOrcaHooks[repoId][kind].contentHash`), and opens a dialog that shows the
  script, its hash, and whether an earlier content was approved. A dismissed or declined dialog is `skip`; a repository
  can also be trusted for every script (`all`). The check runs in the renderer, before the IPC call.
- **Archive.** `src/main/ipc/worktrees/removal/execute-worktree-removal.ts` reads the archive script from the
  repository root's `orca.yaml` (`getArchiveHooksForRemoval`, `getEffectiveHooks(repo)`), and runs it
  (`runHook('archive', worktreePath, ...)` in `src/main/hooks.ts`) with the worktree as working directory and the
  setup variables, before any terminal is stopped or file deleted. A missing worktree skips it. It gets 120 seconds
  (`ARCHIVE_HOOK_TIMEOUT_MS`). A non-zero exit, a timeout or an unobserved outcome refuses the removal with
  `worktree_archive_hook_failed` and "Nothing was stopped, deleted or deregistered" unless the caller passes
  `allowFailedArchiveHook` (`src/shared/worktree/archive-hook-removal-gate.ts`). An untrusted script sets
  `skipArchive` and the removal goes ahead (`remove-worktree.ts`). An `orca.yaml` that cannot be read counts as no
  archive script.

## Decision

- **Trust store.** Table `script_trust (project_id, kind, content_hash, trusted_at)`, one row per project and kind,
  removed with its project. The hash is lowercase hex SHA-256 of the trimmed script, the reference's normalisation.
  Trusting new content replaces the old hash, so changed content needs trust again.
- **Where it is checked.** On the service, before the runner is written, for `setup` (after `no_setup`, so a skipped
  setup still says `skipped`) and `archive`. The reference checks in the renderer; MassOrca's clients include the CLI
  and paired devices, so only the host can make the rule hold for all of them.
- **Untrusted.** The script does not run and nothing fails: a create's `setup` is
  `{"outcome": "untrusted", "reason", "hash"}`, a removal's `archive` is the same shape. The reason says whether the
  script is new or changed and names `massorca project trust-scripts <project>` (with `--workspace` for setup, whose
  copy is the worktree's).
- **Trusting.** `project.scripts` (read) shows each script of the project's `orca.yaml`, or of a workspace's copy,
  with its hash, `trusted` and `changed`. `project.trust_script {project, kind, hash, workspace?}` trusts it only when
  `hash` is the hash of the content read now, else `script.changed`; no script of that kind is `script.not_found`. It
  needs host-admin: trusting lets the host run the script for every later create or removal, so a paired device with
  reduced scopes cannot grant it. The local socket is the owner, so any process of the owner's account, an agent
  session included, can; this guards against repository authors, not against the owner's own processes. With
  `workspace`, only `setup` is shown or trusted (`archive` with a workspace is `request.invalid_params`), as removal
  reads `archive` from the project's copy. Both are capability `script-trust/1`. The CLI is
  `massorca project trust-scripts <PROJECT> [--workspace W] [--hash H]...`; it trusts only a hash it has just shown.
- **Archive.** `workspace.remove` of a worktree whose directory exists reads `scripts.archive` of the project's
  `orca.yaml` (the reference's source), checks trust, and runs it after `begin_remove` and `workspace.removing`, and
  before any run or terminal is stopped. It uses 0086's runner (`archive-runner.cmd` or `.sh` in the worktree's git
  directory), its variables, and a service terminal titled `Archive` with the worktree as working directory, so it is
  watched like setup. The removal waits up to 120 seconds. Exit 0 is `{"outcome": "succeeded", "terminal"}`. A
  non-zero exit, a timeout (the terminal is killed), an unverifiable outcome or a runner that cannot start refuses
  with `workspace.archive_failed`; the row returns to `ready` (`Workspaces::cancel_remove`) and `workspace.updated` is
  published. `allow_failed_archive` (`--allow-failed-archive`) removes it anyway and reports
  `{"outcome": "failed", "reason", "terminal"}`.
- **Folders.** A folder workspace has no archive script, as it has no setup script.
- **Wire.** Every new field is optional; old peers decode the new outcomes as `unknown`. An older client cannot pass
  `allow_failed_archive`, so a failing archive script blocks its removals until the script is fixed or the owner
  removes it from a newer client.

## Differences from the reference

- No repository-wide "trust all" and no `issueCommand` or `vmRecipe` kinds: MassOrca runs only setup and archive.
- No interactive prompt during create or remove: the reply carries the reason and the hash, and trusting is a separate
  host-admin call. A create whose setup was skipped as untrusted does not run it later; the next create does.
- An unreadable or oversized `orca.yaml` still reports a failed setup (0086); for archive it counts as no script, as
  in the reference.
- Trust is per project, not per execution host; the service is the host.
- Trust covers the script's text, not what it runs: a trusted `pnpm install` or `./setup.sh` still runs the branch's
  own package scripts or file.
- An untrusted or changed archive script is skipped and the removal goes ahead, as in the reference: the reply's
  `archive` says `untrusted`, but nothing is archived. An unreadable `orca.yaml` counts as no archive script.

## Consequences

- Existing projects stop running setup until the owner trusts their scripts once.
- `workspace.remove` can now take up to two minutes longer when an archive script runs.
