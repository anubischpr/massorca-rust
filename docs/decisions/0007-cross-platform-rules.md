# 0007: Cross-platform rules

Status: accepted, 2026-11-20.

## Decision

**Paths.**

- Build paths with `std::path`, never by joining strings with `/` or `\`.
- Canonicalize user-supplied paths with `dunce::canonicalize`, which avoids
  Windows `\\?\` verbatim prefixes that git and many tools reject.
- Paths cross the protocol and the store as UTF-8 strings. A non-UTF-8 path is
  rejected with `path.not_utf8` instead of being lossily converted.
- Path equality is case-insensitive on Windows and macOS, case-sensitive on
  Linux.

**Child processes.**

- All child processes go through one helper per crate family (git first) that
  sets `CREATE_NO_WINDOW` on Windows, never uses a shell, passes arguments as
  a list, and sets a timeout.
- Git runs with `GIT_TERMINAL_PROMPT=0`, `LC_ALL=C`, `-c core.quotepath=off`,
  and machine-readable output (`--porcelain`, `-z`) wherever the command
  offers it.
- **Git 2.25 is the baseline.** Every git subcommand and option used must exist
  in 2.25, or carry a fallback. Phase 1 uses `rev-parse`, `worktree add -b`,
  `worktree list --porcelain`, `worktree remove`, `status --porcelain`,
  `branch -d` and `show-ref`, all older than 2.25.

**Windows specifics.**

- Named pipes for IPC (0002, 0004).
- A directory in use by a process cannot be deleted; removal reports the
  failure with the path instead of retrying blindly.
- Line endings: the repository is LF (`.gitattributes`); code never assumes
  the line ending of files it reads from users' repositories.

**Linux** is checked in WSL (Ubuntu 24.04) during development and in CI on
`ubuntu-latest`. macOS is a target but not part of the Phase 1 exit.

## Consequences

- A test suite that passes on Windows and Linux is the gate for every pull
  request (CI runs both).
