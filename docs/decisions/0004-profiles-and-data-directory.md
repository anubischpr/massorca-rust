# 0004: Profiles and the data directory

Status: accepted, 2026-11-20.

## Context

The owner will run a clean test profile beside the real one. Data, config,
logs and the service socket must be isolated per profile from the start;
retrofitting isolation later means migrating every path.

## Decision

- **Home.** `MASSORCA_HOME` if set, otherwise the platform data directory from
  the `directories` crate (`BaseDirs::data_dir()` plus the app folder):
  `%APPDATA%\MassOrca` on Windows,
  `$XDG_DATA_HOME/massorca` (or `~/.local/share/massorca`) on Linux,
  `~/Library/Application Support/MassOrca` on macOS.
- **Profile.** `--profile <name>` on any command, else `MASSORCA_PROFILE`,
  else `default`. Names match `[a-z0-9][a-z0-9_-]{0,31}`, so they are safe in
  file and pipe names on every system.
- **Layout.** Everything of one profile lives under
  `<home>/profiles/<name>/`:

  ```
  service.lock    single-instance lock (0003)
  service.json    informational: pid, version, protocol range
  service.sock    Unix socket (Unix only)
  store.sqlite3   durable state (0005)
  logs/           service logs (0006)
  extensions/     reserved for extensions: pkgs/, dev/, data/, lock.json
                  (docs/architecture/extensions.md); not created in Phase 1
  ```

- **Socket name.** Unix: `<profile dir>/service.sock`. Windows: the named pipe
  `massorca-<profile>-<hash>`, where `<hash>` is 16 hex digits of FNV-1a over
  the profile directory in one normal form, so two homes (for example two test
  runs) never share a pipe and one directory never gets two. The normal form is
  the absolute path (`std::path::absolute` on Windows also resolves `.`, `..`
  and `/`) with a `\\?\` prefix dropped from drive and UNC paths, `\` as the
  separator, and lowercase: NTFS ignores case, so `C:\X` and `c:\x` name one
  pipe. A directory with per-directory case sensitivity shares its pipe with
  its case-insensitive twin, an accepted trade-off. On Unix the path is hashed
  as given, case included. The Unix socket path must fit the 104-byte
  `sun_path` limit; the service refuses to start with a clear message when it
  does not.

## Alternatives

- **One data directory, profile as a column.** Leaks across profiles on every
  bug and cannot isolate sockets or logs.
- **Profile selected only by a data-path variable** (as in the old app). Works,
  but a name is easier to type and to show in the UI; `MASSORCA_HOME` still
  covers the "put it somewhere else" case.

## Consequences

- Tests isolate themselves by pointing `MASSORCA_HOME` at a temporary
  directory.
- Removing a profile is deleting one directory once its service is stopped.
