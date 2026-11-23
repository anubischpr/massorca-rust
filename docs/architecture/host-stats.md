# Host stats

Figures the desktop status bar shows, read on the host that runs the work: the memory MassOrca uses, and later the
ports its processes listen on. The record is decision 0081; this page shows how the pieces fit.

## Shape

```
 desktop / CLI (`massorca host memory`)          polls; never pushed
        |  host/1: host.memory (Read)
 +----------------------------- massorca service --------------------------------+
 |  server: host.memory handler, on the blocking pool                            |
 |     TerminalManager::processes()  -> live terminals and their pids            |
 |        ProcessTree::members(): job (Windows) | session (Linux) | walk (macOS) |
 |     Agents::list()                -> the session whose current run is there   |
 |     massorca-host-stats           -> memory of each pid, native calls only    |
 +-------------------------------------------------------------------------------+
```

## `host.memory`

Read permission, no params. The answer:

| Field | Meaning |
|---|---|
| `metric` | `working_set` (Windows) or `resident` (Linux, macOS). |
| `total_bytes` | The service plus every terminal, as a plain sum: a shared page counts once per process. |
| `service` | `{ pid, bytes }` of the service process. |
| `terminals` | One entry per live terminal, by creation time: `terminal_id`, `workspace_id`, `session_id` when an agent session's current run uses the terminal, `processes` (how many were read) and `bytes`. |
| `unreadable` | Processes listed but not read: they exited between the listing and the read, or access was denied. They are in no sum. |
| `sampled_at_ms` | When the sample was taken. |

This is the reference desktop's "Memory" with two differences: the desktop's own process is not counted (the desktop
may add its own figure locally), and the figure is not pushed.

## Reading

`crates/host-stats` reads with system calls and never starts a program:

- Windows: `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)`, `GetExitCodeProcess` (an ended process whose handle is
  still held is not counted) and `K32GetProcessMemoryInfo` for the working set; a Toolhelp32 snapshot for names and
  parent ids.
- Linux: `VmRSS` from `/proc/<pid>/status`; names and parents from `/proc/<pid>/stat`.
- macOS: `proc_pidinfo` (`PROC_PIDTASKINFO` for the resident size, `PROC_PIDTBSDINFO` for parents and names) and
  `proc_listallpids`.

A terminal's processes come from its `ProcessTree`: exact on Windows (the job) and Linux (the session). macOS has no
session listing, so the tree is the shell and everything below it by parent id; a process that detached from the shell
is missed there.

## Polling

There is no event and no watch. A client polls while it shows the figure: every 2 seconds while a memory detail is
open, otherwise with its usual status refresh. One sample reads each terminal's process list and one system call per
process, so a poll costs little, but nothing samples while nobody asks.

A client that cannot reach the service shows `unverifiable`, never zero: the CLI exits 4 with `memory.unverifiable`.
