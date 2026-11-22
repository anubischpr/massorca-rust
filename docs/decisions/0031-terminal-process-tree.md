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
- A shell that exits by itself: the waiter drops the job handle right after
  the reap, so its leftovers end before the exit is recorded. The pseudoconsole
  close also ends processes still attached to it; the job covers the rest.
- Windows 8 and later allow nested jobs, so this works when the service
  itself runs inside a job (a CI runner, a terminal).

**Unix: the session.**

- `portable-pty` makes the shell a session leader, so the terminal's
  processes are the shell's session (sid = shell pid), whatever process
  groups job control creates.
- A graceful kill sends `SIGHUP` then `SIGCONT` to every process of the
  session (so stopped jobs see it), closes the pty, waits up to 5 s for the
  exit, then sends `SIGKILL` to whatever of the session remains. `force`
  sends `SIGKILL` at once.
- Finding the session: on Linux, read `/proc/<pid>/stat` for processes whose
  session is the shell's pid (zombies excluded). On macOS (not a Phase 2 exit
  target, no `/proc`), record the shell's process group and the pty's
  foreground process group (`tcgetpgrp`, `portable-pty`'s
  `process_group_leader`).
- **Scan order.** The waiter observes the shell's exit without reaping it
  (`waitid` with `WNOWAIT`), so the shell stays an unreaped zombie and its pid,
  the session id and group id, stays held. Every scan runs before the reap.
  The reap runs under the tree's lock and marks the tree reaped in the same
  step. After it nothing scans: a signal goes only to the members recorded
  before the reap, and each is checked again just before its signal (Linux:
  pid, start time and session; macOS: the recorded group). A pid can be reused
  after the reap, so a scan then could name an unrelated process.
- Signals go through `rustix`'s safe `kill_process`/`kill_process_group`.
- A kill of an exited terminal still runs the tree kill; it reaches only what
  was recorded before the reap, so it is idempotent and harmless.
- **A shell that exits by itself ends its leftovers too.** Before the reap the
  waiter sends the same `SIGHUP` and `SIGCONT`, waits up to 1 s for the session
  to empty, then `SIGKILL`s what remains and waits up to 1 s more. Only then is
  the shell reaped and its exit recorded, so the reader reaches end of output
  and its thread ends. On macOS there is no `/proc` to poll, so the grace is a
  fixed 1 s. The kernel already hangs up the foreground process group when its
  session leader exits, so a plain background job in that group dies with the
  shell anyway. This step ends what that hangup misses: a job in its own
  process group (job control) or one that ignores `SIGHUP`.
- **`nohup` and `disown` do not keep a process alive past its shell.** `nohup`
  only ignores `SIGHUP`, and the `SIGKILL` follows. The rule is one piece with
  "killing a terminal leaves nothing running". A process meant to outlive its
  terminal must leave the session (`setsid`), which is out of reach by design.
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
  A shell that exits by itself takes its leftovers with it within about 2 s.
- The `process_tree` module is the only place with platform kill code, and
  it uses no `unsafe`.
