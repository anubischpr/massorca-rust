# Host stats

Figures the desktop status bar shows, read on the host that runs the work: the memory MassOrca uses, and later the
ports its processes listen on. The record is decision 0081; this page shows how the pieces fit.

## Shape

```
 desktop / CLI (`massorca host memory`)          polls; never pushed
        |  host/1: host.memory (Read)
 +----------------------------- massorca service --------------------------------+
 |  server: host.memory handler, on the blocking pool                            |
 |     TerminalManager::memory()    -> each live terminal's process memory      |
 |        one process table per sample (Unix), then per terminal:                 |
 |        job read through the job (Windows) | session (Linux) | walk (macOS)     |
 |     Agents::list()               -> the session whose current run is there    |
 |     massorca-host-stats          -> the reads, native calls only              |
 +-------------------------------------------------------------------------------+
```

## `host.memory`

Read permission, no params. The answer:

| Field | Meaning |
|---|---|
| `metric` | `working_set` (Windows) or `resident` (Linux, macOS). |
| `total_bytes` | The service plus every terminal not `unverifiable`, as a plain sum: a shared page counts once per process. |
| `service` | `{ pid, bytes }` of the service process. |
| `terminals` | One entry per live terminal, by creation time: `terminal_id`, `workspace_id`, `session_id` when an agent session's current run uses the terminal, `processes` (how many were read), `bytes`, `unverifiable`, and `partial`. |
| `unreadable` | Processes listed but not read: they exited between the listing and the read, or access was denied. They are in no sum. |
| `sampled_at_ms` | When the sample was taken. |

A terminal is `unverifiable` when its processes cannot be listed (a job query failed, the shell was just reaped but
its exit is not recorded yet, or the platform has no listing) or when none of them could be read. Its `processes` and
`bytes` are then 0 and mean nothing: a client shows it as unknown, never as `0 KB`. A terminal with some processes read and
some not is `partial`: its `processes` and `bytes` cover only those read, the total counts that sum, and a client marks
the row as partial. The flags are additive; an answer
without it reads as `false`.

This is the reference desktop's "Memory" with two differences: the desktop's own process is not counted (the desktop
may add its own figure locally), and the figure is not pushed.

## Reading

`crates/host-stats` reads with system calls and never starts a program:

- Windows: the job's pid list (`QueryInformationJobObject`, repeated with the size the job reports when it holds more
  than 1024 processes); then per pid `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)`, `GetExitCodeProcess` (an ended
  process whose handle is still held is not counted), `IsProcessInJob` on that handle (a pid reused since the listing
  is not read) and `K32GetProcessMemoryInfo` for the working set. A Toolhelp32 snapshot gives names and parent ids.
- Linux: `VmRSS` from `/proc/<pid>/status`; names, parents, session ids and start times from `/proc/<pid>/stat`.
- macOS: `proc_pidinfo` (`PROC_PIDTASKINFO` for the resident size, `PROC_PIDTBSDINFO` for parents, names and start
  times) and `proc_listallpids`, asked again while the count grows.

A terminal's processes come from its `ProcessTree`, and each platform counts a different set (decision 0081, D6).
Windows counts the job, whose members cannot leave it, since the terminal's job does not allow breakaway. Linux counts
the session, which a process leaves when it starts a session of its own, as daemons do; a process reparented to init
keeps its session. macOS walks parent ids from the shell, so a process whose parent exited (and was reparented to
launchd) is missed. On Unix the process table is read once per sample and shared by every terminal; a terminal's lock is
held only to check that its shell is not reaped, never during a scan. The parent walk is Unix only, since Windows parent
ids can name a parent that exited. The service's own process is never a member.

Each process is read only while it is the one the table listed: its start time, and on Linux its session, must match
the listing before and after the read. A pid reused since the listing is counted unreadable and never summed. A macOS
table that still does not fit after its bounded retries is incomplete: the terminals read from it are `partial`.

## Polling

There is no event and no watch. A client polls while it shows the figure: every 2 seconds while a memory detail is
open, otherwise with its usual status refresh. One sample reads the process table once (Unix) or each terminal's
job (Windows), and a few system calls per process, so a poll costs little, but nothing samples while nobody asks. A sample is shared for about a second: a call within that
time is answered with the sample already taken, and its `sampled_at_ms` is when that sample was taken.

A client that cannot reach the service shows `unverifiable`, never zero: the CLI exits 4 with `memory.unverifiable`.
