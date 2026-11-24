# 0086: Workspace setup scripts

Status: accepted, 2026-11-23. Supersedes the deferral of setup scripts in 0008 and 0009 (inventory WS-05 step 5, WS-13).

## Context

0008 deferred setup scripts. The owner ruled: build them as the reference does (pv96, read-only at commit d3c6311a531);
on Windows the runner is a `.cmd` unless the script starts with `#!`; they run in a service terminal the person can
watch, never silently.

The reference reads `scripts.setup` from `orca.yaml` in the new worktree (the base branch's copy), writes it to a runner
file in the worktree's git directory (`git rev-parse --git-path orca/setup-runner.{cmd,sh}`), and opens a terminal that
runs it. It sets `ORCA_ROOT_PATH`, `ORCA_WORKTREE_PATH`, `ORCA_WORKSPACE_NAME`, `CONDUCTOR_ROOT_PATH`,
`GHOSTX_ROOT_PATH` and forces git's credential prompt guard. A failure to prepare setup never fails the create. The
create takes `setupDecision` (`run`, `skip`, `inherit`) and a per-project `setupRunPolicy`.

## Decision

- **Source.** `scripts.setup` of `orca.yaml` at the root of the new worktree, so the script is the one of the branch the
  worktree starts from. MassOrca has no per-project local script and no run policy: both would need a store change.
  The file is read as a YAML block mapping: a plain, single- or double-quoted one-line value, or a `|` / `>` block.
  Any other form (flow mappings, anchors) reads as no script. A file over 256 KiB or one that cannot be read is
  reported as a failed setup.
- **When.** Only `workspace.create` of a worktree runs it, after the row is stored and `workspace.created` is
  published. The method needs the `workspace` permission, which implies `operate` (0078), so no other caller starts
  it. Folder workspaces never run setup.
- **Opt-out.** `no_setup` on `workspace.create` (`--no-setup`), the reference's `--setup skip`. There is no `ask`
  policy: the default is the reference's default, run.
- **Runner.** Written to `<worktree git dir>/massorca/setup-runner.cmd` or `.sh`, found with core-git
  (`Git::repository_dirs`, Git 2.25 compatible). It is replaced by each create of the worktree and removed with the
  worktree's admin directory when the worktree is removed.
  - Windows: a `.cmd` (`@echo off`, `setlocal EnableExtensions DisableDelayedExpansion`, one `call <line>` per line
    followed by `if errorlevel 1 exit /b %errorlevel%`), started as `<ComSpec or %SystemRoot%\System32\cmd.exe> /d /c
    <runner>`. A script whose first line is `#!` naming a POSIX shell (`sh`, `bash`, `zsh`, `dash`, `ksh`, `ash`)
    gets the `.sh` runner below, started by Git for Windows' `bin\bash.exe` with `/c/...` paths. The user's terminal
    shell preference never decides it, the runner is never typed into a shell, and the `bash.exe` in System32 (the WSL
    launcher) is never used. A `#!` script with no Git Bash, or naming another interpreter, gets a `.cmd` that prints
    why in the terminal and exits 1, as the reference does.
  - A `.cmd` runner path containing one of `& < > ( ) @ ^ | % ! "` is refused (setup `failed`): cmd re-reads those in a
    `/c` line. A path with spaces is safe: cmd keeps two quotes around an existing file's name.
  - Linux and macOS: a `.sh` (`#!/usr/bin/env bash`, `set -e`, the `#!` line's `set` flags, then the script) started
    by `bash` from `PATH`, else `/bin/bash`.
- **Terminal.** A service terminal (0033, 0035) in the workspace, titled `Setup`, working directory the worktree,
  listed by `terminal list` and attachable by every client. Its request id is `workspace.setup:<workspace id>`, so a
  repeated create in the same service life returns the same terminal. It runs on the service's host, which is also
  the execution host on SSH; no client runs anything.
- **Environment.** `MASSORCA_ROOT_PATH` (the project path), `MASSORCA_WORKSPACE_PATH`, `MASSORCA_WORKSPACE_NAME`,
  `MASSORCA_WORKSPACE_BRANCH` (when there is one), the reference's `ORCA_ROOT_PATH`, `ORCA_WORKTREE_PATH`,
  `ORCA_WORKSPACE_NAME` (the folder name), `CONDUCTOR_ROOT_PATH`, `GHOSTX_ROOT_PATH`, and `GIT_TERMINAL_PROMPT=0`,
  `GCM_INTERACTIVE=never`. Paths are `/c/...` for the Git Bash runner on Windows.
- **Result.** The create reply carries an optional `setup`: `{"outcome": "started", "terminal": ID}`, `skipped`
  (`no_setup` with a script), or `{"outcome": "failed", "reason": ...}`. It is absent when there is no script or the
  workspace is a folder. A failed setup never fails the create. The field and `no_setup` are optional on the wire.
- **Bot Teams.** `task.start` makes its worktrees through the domain, not `workspace.create`, so it runs no setup. The
  reference does run setup for a team task's worktree (and waits for it only under a wait-for-setup policy); that,
  and the routine-started case whose reference default is skip, is left to a later decision.

## Consequences

- A repository's `orca.yaml` setup now runs on every worktree create by a `workspace` caller. This is row 14 of 0078.
- A create replayed after a service restart can run setup a second time; scripts are expected to be repeatable.
- The minimal YAML reader is a deliberate subset; a full parser is a dependency change for later.
