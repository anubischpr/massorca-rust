# 0132: Server self-update over the paired link

Status: accepted, 2026-11-26.

## Context

0045 upgrades a server by replacing its binary and restarting the systemd user unit, and `host add-ssh --binary`
does both over SSH. A server reached only through its paired connection (0041, 0046) had no upgrade path except by
hand. This decision adds one over that connection, for one host at a time, at the person's request.

## Decision

**Command.** `massorca host update NAME` addresses one paired host. Before anything is sent it prints the host name,
the version the host runs and the version to install, and asks `Update host NAME? [y/N]` at a terminal. Anywhere else
(`--json`, a pipe, a script) it needs `--yes`; without it, it prints the review and fails with
`update.confirmation_required`, having sent nothing. There is no bulk or automatic update. `--rollback` puts back the
binary the last update replaced, after the same confirmation.

**Source.** The client pushes a binary it already has; the host never downloads anything. The default is the running
`massorca` itself. Any other file (`--binary PATH`) needs `--sha256 HEX`, the checksum its release publishes; the CLI
refuses a file whose sha256 differs before contacting the host.

**Provenance.** The repository has no release signing, no SHA256SUMS and no minisign or similar key: CI builds
artifacts but publishes no checksums or signatures. So the strongest rule available is used: the client computes the
sha256 of the bytes it sends, the host recomputes it over the bytes it received and refuses a mismatch before anything
is replaced, and a file other than the client's own executable needs the checksum the person copied from its release
page. The gap that remains is plain: nothing proves who built the binary. A checksum copied from the same place as a
tampered file proves nothing, and the client's own executable is trusted because the person already runs it. A
release signature (for example minisign, with the public key built into the client) would close the gap; when one
exists, the CLI and the host verify it instead of a typed checksum.

**Permission and capability.** The methods need `host-admin`. They sit under the capability `self-update/1`; the CLI
refuses a host that does not offer it with `service.capability_missing`, before sending it anything. Every host that offers the capability answers the methods; one with no supervisor refuses
`update.unsupported` before the upload starts.

**Methods.**

| Method | Params | Does |
|---|---|---|
| `host.update_begin` | `size`, `sha256` | Refuses a size of 0 or above 256 MiB (`update.too_large`); opens the staging file. A new upload replaces an unfinished one. |
| `host.update_chunk` | `upload_id`, `offset`, `data` (base64, at most 512 KiB decoded) | Appends at `offset`, which must be the bytes received so far (`update.bad_chunk`); refuses a chunk past `size`. |
| `host.update_finish` | `upload_id`, `expected_version`, `allow_downgrade`, `force` | Checks, swaps, answers, then restarts. |
| `host.update_rollback` | `expected_version`, `dry_run` | Checks the kept binary; a dry run only names its version. |

Chunks ride in ordinary requests: no new frame kind or stream opcode, so older peers are unaffected. The new fields
are all optional.

**Checks before the swap.** In order, each refusal deleting the staged file and leaving the installed binary as it
was: the received length and sha256 must match the announced ones (`update.checksum_mismatch`); the file must run
`--version` within 10 seconds and print `massorca <version> (protocol <min>..<max>)`, with a range that overlaps the
host's own (`update.incompatible`); the version must be the one the person confirmed (`update.version_mismatch`); and
it must be newer than the running service. An older version needs `allow_downgrade` (`update.downgrade`), the same
version needs `force` (`update.same_version`). The CLI applies the version rule too and shows a downgrade in the
review, but the host enforces it whatever the client sent. `massorca --version` now prints the protocol range for this
check; a binary from before this decision prints none and is refused, so it can only be installed with `add-ssh`.

A binary built for another system cannot run on the client, so its version is unknown there: the person gives it with
`--expect-version`, the review shows it, and the host refuses the binary if it reports anything else.

**Swap.** The upload is staged next to the installed binary (`massorca.update`, `massorca.update.exe` on Windows), on
the same filesystem, owner-only and executable. The installed binary is hard-linked (or copied) to `massorca.previous`,
then the staged file is renamed over the installed path, so that path never goes missing.

**Restart.** The only supervisor is the systemd user unit of 0045: a service whose `/proc/self/cgroup` places it in
`massorca.service` runs `systemctl --user --no-block restart massorca.service` 300 ms after it answers. Windows and
macOS have no supervisor that restarts the service (the desktop and CLI autostart it on demand), so their hosts refuse
with `update.unsupported` before the upload. The CLI then reconnects for up to 90 seconds and reports the new version;
a host that does not come back fails with `update.not_confirmed`.

**Rollback.** Manual, not automatic. A broken new binary cannot be trusted to roll itself back, and an automatic
watchdog (a timer run by the old binary) is not in this decision. When the new service runs, `host update NAME
--rollback` names the kept version (a dry run), asks for confirmation, then restores it and restarts. When it does not
come up, the recovery is on the host, over SSH or a console:

```sh
mv ~/.massorca/bin/massorca.previous ~/.massorca/bin/massorca
systemctl --user restart massorca
```

Only one previous binary is kept; a rollback consumes it.

## Alternatives

- **The host downloads a release.** Needs network access to a release server and a trust root this project does not
  have yet.
- **Binary frames for the upload.** Frames flow server to client only today; client-sent frames would need a new
  negotiated opcode for a rare operation that 512 KiB requests handle.
- **Automatic rollback by a watchdog.** Left for later: it needs a timer that survives the restart (a transient
  `systemd-run` unit) and a health rule, and would act without the person.

## Consequences

- A host with SSH keeps `add-ssh` as the first install and the fallback; one without SSH can now be updated once it
  runs this version under systemd.
- Tests drive the real service with a fake binary in a temporary folder, through two test-only variables
  (`MASSORCA_TEST_SELF_UPDATE_TARGET`, `MASSORCA_TEST_SELF_UPDATE_RESTART`); nothing touches a real install.
- The restart through systemd and the recovery steps above are not exercised by automated tests.
