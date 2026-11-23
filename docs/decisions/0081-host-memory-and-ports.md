# 0081 Host memory and ports

Status: accepted, 2026-11-22.

## Context

The desktop status bar has placeholders for two live figures: MassOrca's memory and the listening ports of the
processes MassOrca owns (docs/design/components/StatusBar.md). Both must be read on the host that runs the work, which
may be a paired remote host. No process-table reader exists in the workspace. `core-terminal`'s `ProcessTree` already
knows each terminal's processes exactly on Windows (a job object) and Linux (the session), and knows nothing on macOS.

The reference desktop counts as "Memory" its own processes (main, renderer, GPU) plus the process subtree of each local
terminal, as working set on Windows and resident set on macOS and Linux. Its total is the plain sum of per-process
samples, so a shared page is counted once per process. It refreshes by polling: once when ready and every 2 seconds
while the memory popover is open. It has no push event. Its ports figure counts every TCP listening socket on the
machine attributed to a workspace by working directory or command line, scanned every 30 seconds while visible.

## Decision

- D1 Memory meaning. `host.memory` reports the service process plus every process in each live terminal's tree. Agent
  runs run in terminals, so they are included, and a terminal that holds an agent session's current run is labelled
  with that session. The metric is the working set on Windows and the resident set on Linux and macOS; the answer
  names it (`working_set` or `resident`). The total is the plain sum, as in the reference. This matches the reference's
  meaning of "Memory" with two stated differences: the desktop's own process is not counted (the desktop is a separate
  program and may add its own figure locally), and nothing is pushed (D3).
- D2 Ports meaning. TCP listening sockets whose owning process is in a terminal's tree, per workspace and terminal,
  with the session when the terminal is an agent run. Not every listener on the machine: the reference's status-bar
  figure counts workspace ports only, and attribution by process tree is exact where the reference guesses by working
  directory. The service's own listener is not listed. Ports are a separate method, added after memory.
- D3 No push. Both figures are on-demand Read methods. The desktop polls: memory every 2 seconds while its detail is
  open and otherwise with its status refresh, ports every 30 seconds while visible, as the reference does. There is no
  event and no watch.
- D4 Readers. One shared reader crate, `massorca-host-stats`, makes native calls only and never starts a program.
  Windows: the terminal's job is listed (any size, the query is repeated with the size the job reports), and each
  process is opened with `PROCESS_QUERY_LIMITED_INFORMATION`, checked with `IsProcessInJob` on that handle so a reused
  pid is never read, and read with `K32GetProcessMemoryInfo`; a Toolhelp32 snapshot gives names and parent ids. Linux:
  `/proc/<pid>/status` (`VmRSS`) and `/proc/<pid>/stat` (parent and session). macOS: `proc_pidinfo`
  (`PROC_PIDTASKINFO`, `PROC_PIDTBSDINFO`) and `proc_listallpids` through `libc`. On Unix one process table is taken
  per sample, for every terminal: Linux groups it by the shell's session id, macOS walks parent ids from the shell
  down. The parent walk is Unix only: Windows parent ids can name a parent that exited. Ports use the same crate (a sockets module beside the memory one). Unsafe calls stay
  in named platform modules (decision 0007).
- D5 Loss of contact. The service that answers is always the executing host. A client that cannot reach it reports
  `unverifiable` (CLI output and a non-zero exit, desktop state), never zero memory and never "no ports". A process that
  cannot be read (it exited, or access is denied) is skipped and counted in `unreadable`; it never fails the sample.
  A terminal whose processes cannot be listed, or of which none could be read, is marked `unverifiable`, shown as
  unknown, and left out of the total: never reported as zero.
- D6 What a terminal's tree counts, and how a read is trusted. Each platform counts a different thing, and the answer
  does not hide the difference. Windows counts the job: its members cannot leave it, because the terminal's job does not
  allow breakaway, so the count is exact. Linux counts the session: a process that starts a session of its own (`setsid`,
  as daemons do) is not counted, while one reparented to init keeps its session and is counted. macOS counts the parent
  walk from the shell: a process whose parent exited and was reparented to launchd is not counted, although the terminal
  started it, and one that starts its own session but keeps its parent is counted. The service's own process is never a
  member. A process is read only while it is the one listed: its start time, and on Linux its session, must match the
  listing before and after the read, so a reused pid is unreadable. A macOS listing that grows while it is read is asked
  again with more room; one that still does not fit makes the table incomplete, and the terminals read from it are
  `partial`. Samples are shared for about a second, so a call within that time gets the sample already taken.
- D7 Ports, as settled. `host.ports` lists one entry per listening TCP socket, so a process on IPv4 and IPv6 has two
  (same `port` and `pid`, two addresses) and nothing is merged; a client groups them when it shows one row per process.
  `connect_host` is `localhost` for a wildcard bind. A terminal is `unverifiable` when none of its members can be read and
  `partial` when some cannot be read or have left it; when the host's socket table cannot be read, no port is listed and
  the answer is `unverifiable`. Polling is every 30 seconds while the ports are visible, and nothing is pushed. macOS is
  not read until its socket layouts are verified (libc 0.2.191 declares none of them), so its answer is unverifiable.

## Alternatives

- Spawning `ps`, `tasklist` or PowerShell per sample: rejected. A program per sample costs far more than the figure is
  worth, opens consoles on Windows, and depends on tools and their output format.
- A crate such as `sysinfo`: rejected. It refreshes the whole process table to answer for a few pids, and brings much
  more than the two readings needed.
- A push event or a watch for memory: rejected. The reference polls, the figure is only shown on demand, and a timer
  in the service would sample while nobody looks.
- Counting every listener on the machine as the reference's popover does: rejected for the service. The status bar
  shows workspace ports, and a process-tree attribution needs no guessing.

## Consequences

- Capability `host/1` with the Read methods `host.memory` and `host.ports`, the second added within the version: additive
  changes keep a capability's version.
- `core-terminal` depends on `massorca-host-stats` on every platform: its memory and membership reads go through it.
- `massorca host memory` targets the service like other service commands, so `--host` works for it, unlike the other
  `host` commands.
- Process-enumeration code gets an independent review before merge.
