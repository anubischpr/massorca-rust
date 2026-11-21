# 0004: Profiles and the data directory

Status: accepted, 2026-11-20.

## Context

The owner will run a clean test profile beside the real one. Data, config,
logs and the service socket must be isolated per profile from the start;
retrofitting isolation later means migrating every path.

## Decision

- **Home.** `MASSORCA_HOME` if set, otherwise the platform data directory from
  the `directories` crate: `%APPDATA%\MassOrca` on Windows,
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
  ```

- **Socket name.** Unix: `<profile dir>/service.sock`. Windows: the named pipe
  `massorca-<profile>-<hash>`, where `<hash>` is 16 hex digits of a hash of
  the absolute profile directory, so two homes (for example two test runs)
  never share a pipe. On Unix the path must fit the 104-byte `sun_path` limit;
  the service refuses to start with a clear message when it does not.

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
