# 0031: Ending a terminal's process tree

Status: accepted, 2026-11-21.

## Context

Killing a terminal must not leave anything it started running, agents
included (TERM-13). Shells start background jobs, Git Bash (MSYS2) emulates
`fork` with new Windows processes, and on Unix job control puts jobs in other
process groups. If the service itself dies, its terminals must die too
(0033).

## Decision

**Windows: one job object per terminal.**

- Created with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` and neither
  `JOB_OBJECT_LIMIT_BREAKAWAY_OK` nor `JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK`.
  MSYS2 asks for breakaway only when the job allows it, so Git Bash children
  stay inside.
- The shell is assigned (`win32job`, by the handle `portable-pty` returns)
  right after `CreateProcess`. Processes the shell starts before that moment
  would escape; a shell does not start children during its first
  milliseconds, so the window is accepted, as in `core-git`.
- The service holds the only job handle. A graceful kill first closes the
  pseudoconsole, which sends `CTRL_CLOSE_EVENT` to every attached process,
  waits up to 5 s for the shell to exit, then drops the job handle, which
  terminates every process still in the job. `force` drops it at once. If
  the service dies, the OS closes the handle and the tree dies with it.
- Windows 8 and later allow nested jobs, so this works when the service
  itself runs inside a job (a CI runner, a terminal).

**Unix: the session.**

- `portable-pty` makes the shell a session leader, so the terminal's
  processes are the shell's session (sid = shell pid), whatever process
  groups job control creates.
- A graceful kill sends `SIGHUP` then `SIGCONT` to every process of the
  session (so stopped jobs see it), waits up to 5 s for the shell to be
  reaped, then sends `SIGKILL` to whatever of the session remains. `force`
  sends `SIGKILL` at once.
- Finding the session: on Linux, scan `/proc/<pid>/stat` for processes whose
  session is the shell's pid. On macOS (not a Phase 2 exit target), signal
  the shell's process group and the pty's foreground process group
  (`tcgetpgrp`, `portable-pty`'s `process_group_leader`).
- The scan runs before the shell is reaped, so its pid (the session id)
  cannot have been reused yet. Signals go through `rustix`'s safe
  `kill_process`/`kill_process_group`.
- If the service dies, closing the master hangs up the session leader; the
  kernel then hangs up the foreground group. Background jobs that ignore
  `SIGHUP` can survive a service crash; a clean stop kills them as above.

**Out of reach by design.** A process that leaves the session (`setsid`,
daemons) on Unix, or is created outside the job on Windows, is not the
terminal's any more and is not killed.

## Alternatives

- **`portable-pty`'s kill.** Signals only the shell.
- **`killpg` on the shell's group only.** Misses job-control groups.
- **cgroup v2 `cgroup.kill`.** Catches even `setsid` escapees, but needs a
  delegated writable cgroup, which a user service does not always have.
- **A helper process per terminal** that puts itself in the job before
  starting the shell (no assignment race). One more process per terminal
  that must itself die with the service; kept as the fix if the race shows
  up in practice.
- **`taskkill /T /F`.** Walks parent ids, which Windows reuses, and misses
  re-parented processes; the job does not.

## Consequences

- A killed terminal's tree is gone within about 5 s, at once with `force`.
- The `process_tree` module is the only place with platform kill code, and
  it uses no `unsafe`.
