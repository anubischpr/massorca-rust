# 0088 CLI on PATH

Status: accepted, 2026-11-24.

## Context

Two inventory rows are open. CLI-01: agent runs get the bundled binary path for their hooks, but no stable command is put
on PATH. PLAT-27: one binary serves as the CLI and the service, the desktop is a separate binary, and nothing puts the
command on PATH. The reference app installs its command per platform: a symlink in `/usr/local/bin` or `~/.local/bin` on
macOS and Linux, and a command wrapper in a per-user folder that PowerShell adds to the Windows user PATH.

## Decision

- D1 Scope. `massorca cli install` makes the command for this user only. It needs no administrator rights, edits no
  system PATH, and never edits a shell profile. The three `cli` commands are local: no service request, and `--host` is
  refused.
- D2 Unix. A symbolic link `~/.local/bin/massorca` points at the canonical path of the binary that ran the install, so a
  link is never made to itself. When the folder is missing from `PATH`, `status` reports `not_on_path` and the user adds it.
- D3 Windows. A command file `massorca.cmd` in `%LOCALAPPDATA%\Programs\MassOrca\bin` runs the binary with `%*`. The
  folder is appended to `HKCU\Environment\Path`, and `WM_SETTINGCHANGE` is broadcast so running programs see it. The
  value is written through the registry API, because the Windows rules forbid spawning PowerShell or `setx`. A target that
  cmd could misread (a quote, a percent sign, a line break) is refused before anything is written.
- D4 Ownership. Install overwrites only a link to a binary named `massorca` or `massorca.exe` (Unix), or a command file
  that carries the marker line install writes (Windows). Anything else is `cli.command_conflict`. Uninstall removes only
  what passes the same test. The Windows folder and its PATH entry go only when the folder is empty.
- D5 Reads before writes. Install reads the Windows user PATH before it changes anything, so an unreadable PATH stops it
  (`cli.user_path_unreadable`). `status` reports `bin_dir_on_path: null` rather than guess.
- D6 Target. The command runs the binary that ran `cli install`. Installers do not yet place a stable copy, so an update
  that moves that binary leaves the command on the old path until install runs again; `different_binary` shows it. A stable
  installed location is the follow-up that removes this step.
- D7 Tests. Every test uses temporary folders and an in-memory user PATH; none reads or writes the real registry or the
  real PATH. Two parts have no test of their own: the Windows registry adapter, a thin wrapper over the registry calls,
  and `run`, which the integration test covers only for help and the host refusal.

## Consequences

- The command is `massorca` on every platform; a development name is not split out yet.
- Only a link or command file massorca made is ever removed, so another program's `massorca` is never deleted.
- The PATH change is per user, so each user on a machine runs `cli install` once.
