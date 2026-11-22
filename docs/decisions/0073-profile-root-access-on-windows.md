# 0073: Profile root access on Windows

Status: accepted, 2026-11-22. Revises the Windows part of 0004's
**Permissions** rule.

## Context

0004 let the Windows profile directory keep the access it inherits from its
parent, and relied on the owner-only pipe descriptor (0003). The service also
reads files from the profile, for example `agents/kinds/*.json`. When
`MASSORCA_HOME` sits in a folder other users can write to, another user can
create the profile directory or plant files in it before the service first
runs, and the service would trust them.

## Decision

- **At creation.** The profile root `<home>/profiles/<name>` is created with
  `CreateDirectoryW` and a descriptor that names the current user as owner and
  carries a protected DACL with one entry: full access for that user,
  inherited by every file and folder below (`O:<user>D:P(A;OICI;FA;;;<user>)`).
  The directory is never reachable by others, not even briefly. Folders made
  later inherit from it. On Unix the root is created `0700` in one step.
- **At every service start**, before the lock file or the store is opened,
  the existing root is checked (`massorca_profile::secure_root`):
  - **Refuse** when the root's owner is not the current user, or when a file
    or folder up to four levels below it is owned by anyone other than the
    current user, SYSTEM or Administrators, or its owner cannot be read. The
    service fails with `ServiceError::ForeignEntries`; the CLI reports
    `profile.foreign_entries` with up to ten paths in `data.paths`. Links are
    read themselves, never followed.
  - **Repair** otherwise, when the root's DACL is not the protected owner-only
    one: the DACL is replaced, Windows passes the change on to the inherited
    entries of everything below, and the service logs a warning naming the
    path. On Unix a root that is not `0700` is set to `0700`.

Repair keeps an ordinary existing profile working after an upgrade. Refusal
stops content another user may have planted before the repair from being
trusted; the user removes it, or takes ownership, and starts again.

## Alternatives

- **Create, then restrict.** Leaves a window in which another user can open
  the directory or plant files in it.
- **Repair everything, refuse nothing.** Hides planted files behind a new
  access list instead of exposing them.
- **Refuse any non-private root.** Breaks every profile created before this
  decision.

## Consequences

- `service run` checks the root before it creates the logs folder, so a refused
  profile receives nothing, not even a log file.
- Entries an elevated process created belong to Administrators and are
  accepted. A root owned by Administrators is refused, since a normal user
  could not repair it.
- An entry more than four levels deep is not checked; the folders the service
  reads are shallower.
- The refusal is tested with an injected owner, because setting another
  user's owner on a file needs privileges a test does not have.
