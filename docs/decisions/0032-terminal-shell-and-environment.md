# 0032: Terminal shell, working directory and environment

Status: accepted, 2026-11-21.

## Context

A terminal must open the user's shell (TERM-10), in the workspace (TERM-01),
with a colour-capable environment (TERM-16). Windows has a specific trap: a
shell choice passed as a command typed into another shell becomes a child of
that shell, not the terminal's shell. Shell integration (readiness markers,
working-directory tracking, TERM-11) and startup commands come with agents.

## Decision

- **The shell is the program the terminal is.** `terminal.create` takes an
  optional `shell { program, args }`; the PTY spawns exactly that program
  with those arguments. There is no "command" field in Phase 2: text to type
  into a shell is sent with `terminal.input`. A startup command with a
  readiness barrier is added with agents, as a separate optional field.
- **Default shell.**
  - Windows: `pwsh.exe` when it is on `PATH`, else `powershell.exe` (the old
    app's `auto`). `cmd.exe` and WSL are explicit choices.
  - Unix: `$SHELL` when it is an absolute path to an executable file, else the
    first existing of `/bin/zsh`, `/bin/bash`, `/bin/sh`.
  - No profile arguments are added; the shell starts as an interactive shell
    because it runs on a terminal. A configurable default shell arrives with
    the settings registry (`terminal.default_shell`).
- **Working directory.** The workspace root, or `cwd` if given: a relative
  `cwd` is joined to the root, an absolute one is used as is. It must exist
  and be a directory, else `path.not_found` / `path.not_directory` before
  anything is spawned.
- **Environment.** The service's own environment, then:
  - removed: `NO_COLOR`, `FORCE_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE`;
  - set: `TERM=xterm-256color`, `COLORTERM=truecolor`,
    `TERM_PROGRAM=MassOrca`, `TERM_PROGRAM_VERSION=<service version>`,
    `MASSORCA_TERMINAL_ID=<id>`, `MASSORCA_WORKSPACE_ID=<id>`, and
    `MASSORCA_HOME` / `MASSORCA_PROFILE` of this service, so `massorca`
    typed inside the terminal talks to the same profile;
  - then the caller's `env`, which may override any of the above.
  Variable names compare case-insensitively on Windows. Values are never
  logged.
- **Windows PATH** is the service's inherited `PATH` in Phase 2. The registry
  merge of machine and user `PATH` (TERM-17) comes later; it matters when the
  service was started before a PATH change.

## Alternatives

- **Spawn `cmd.exe /c <shell>`** or a login wrapper: makes the chosen shell a
  child, breaks process identity, exit codes and tree kill.
- **Always `powershell.exe` on Windows.** Ignores PowerShell 7 where the user
  installed it; the old app's `auto` preferred it.
- **Login shell flags (`-l`) on Unix.** The old app added them only under
  conditions; an interactive shell on a TTY already reads its rc files.

## Consequences

- `--shell` in the CLI maps directly to `shell`. A user who wants a command
  run types it, or sends it with `massorca terminal send`.
- Shell integration and directory tracking can be added later without
  changing how the shell is chosen.

## Amendment, 2026-11-28: registry PATH (TERM-17)

This amendment replaces the Windows PATH bullet above. Nothing else in this decision changes.

- **Source.** On Windows the terminal's PATH is read at every start from the machine `Path`
  (`HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment`) and the user `Path`
  (`HKCU\Environment`), so a tool installed after the service started is found.
- **Order.** Machine entries, then user entries, as Explorer builds a new process's
  environment. Empty entries are dropped.
- **Expansion.** A `REG_EXPAND_SZ` value expands its `%NAME%` references from the service's
  environment; a reference that does not resolve stays as written. A `REG_SZ` value is used
  as it is.
- **Service entries.** Entries only the service's own PATH lists come after the registry
  entries, so nothing the service added is dropped.
- **Dedupe.** A service entry that a registry entry already lists is not repeated. Entries
  compare without regard to ASCII case or a trailing `\` or `/`. Registry entries are not
  deduplicated among themselves.
- **Unreadable values.** A value that cannot be read counts as not set. When neither value is
  set, the terminal keeps the service's PATH and reports `inherited`.
- **Other platforms.** The registry is read only on Windows (`cfg(windows)`). Elsewhere the
  terminal keeps the service's PATH unchanged and reports `inherited`.
- **Shell.** The default shell and an explicit `--shell` name resolve against the merged PATH,
  so a `pwsh` installed after the service started is preferred.
- **Visibility.** `terminal.show` returns `path_source` (decision 0035, amendment of the same
  date). `massorca terminal show` prints `path source: registry` or `path source: inherited`.
