# Host stats

Figures the desktop status bar shows, read on the host that runs the work: the memory MassOrca uses, and the ports its
processes listen on. The record is decision 0081; this page shows how the pieces fit.

## Shape

```
 desktop / CLI (`massorca host memory`)          polls; never pushed
        |  host/1: host.memory, host.ports (Read)
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

## `host.ports`

Read permission, no params. The answer lists one entry per listening TCP socket that the processes of the live
terminals hold (agent runs included), and one status per live terminal:

| Field | Meaning |
|---|---|
| `ports` | One entry per listening socket: `port`, `address` (the bind address: a wildcard or one interface's), `connect_host` (`localhost` for a wildcard bind, otherwise the address), `pid`, `process_name`, `terminal_id`, `workspace_id`, and `session_id` when an agent session's current run uses the terminal. |
| `terminals` | One status per live terminal, by creation time: `terminal_id`, `workspace_id`, `unverifiable`, and `partial`. |
| `unverifiable` | The host's socket table cannot be read: nothing is known about any port, so `ports` is empty and every terminal is `unverifiable`. |
| `sampled_at_ms` | When the sample was taken. |

A process listening on IPv4 and on IPv6 has two entries, with the same `port` and `pid` and two addresses. Nothing is
merged: a client that shows one row per process groups them by `pid` and `port`. The service's own listener is never
listed, since only a terminal's processes are attributed to it (decision 0081, D2).

A terminal is `unverifiable` when its members cannot be listed or none of them can be read: its ports are unknown, so
it lists none. It is `partial` when some of its members cannot be read or have left it, or when the process table was
incomplete: its list may miss ports. The flags are additive; an answer without them reads as `false`.

Each platform reads the listening sockets natively, and only for the processes asked about:

- Windows: `GetExtendedTcpTable` for IPv4 and IPv6 with `TCP_TABLE_OWNER_PID_LISTENER`, which names each listener's owner
  pid; addresses and ports are read in network byte order. After the read, each owner that is a terminal member is
  checked again on its own handle (`IsProcessInJob`), so a pid reused meanwhile is not attributed.
- Linux: the LISTEN rows of `/proc/net/tcp` and `/proc/net/tcp6` give each socket's inode. Each member's descriptors
  (`/proc/<pid>/fd`, `socket:[inode]`) name the inodes it holds. A socket several members hold is owned by the one that
  started first, since a forked child starts after its parent; pids wrap, so the number gives no order. A member is read
  only while it is the one listed: its start time and session must match before and after the read.
- macOS: not read yet. libc 0.2.191 declares neither `socket_fdinfo` nor the TCP socket structures, and their layouts
  cannot be checked here, so the socket table reads as unreadable and the answer is unverifiable until a macOS build
  verifies them.

Limits, which the answer does not hide:

- A link-local IPv6 listener (`fe80::/10`) loses its zone, the scope id: its address is listed without it, so the bare
  address may not connect on its own.
- On Windows the owner pid is the process that created the socket. A listener handed to a child after its creator
  exited (for example through an inherited handle) may be missed, or attributed to another member of the same terminal,
  because the table does not say which process holds it now.
- Linux reads only the sockets of the service's own network namespace. A terminal in another namespace (a container, or
  `unshare -n`) may listen on sockets this table does not show.
- A member that ends between the membership listing and the check is not confirmed, so its terminal is `partial`: its
  list may miss the ports of that member, which has ended.

## Reading

`crates/host-stats` reads with system calls and never starts a program:

- Windows: the job's pid list (`QueryInformationJobObject`, repeated with the size the job reports when it holds more
  than 1024 processes); then per pid `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)`, `GetExitCodeProcess` (an ended
  process whose handle is still held is not counted), `IsProcessInJob` on that handle (a pid reused since the listing
  is not read) and `K32GetProcessMemoryInfo` for the working set. A Toolhelp32 snapshot gives names and parent ids.
- Linux: `VmRSS` from `/proc/<pid>/status`; names, parents, session ids and start times from `/proc/<pid>/stat`. A `/proc`
  mounted with `hidepid` lists only some processes (read from `/proc/self/mountinfo`): the table is then incomplete, and
  the terminals read from it are `partial`.
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
table that still does not fit after its bounded retries, or a Linux listing that `hidepid` keeps short, is incomplete: the
terminals read from it are `partial`. On Windows a terminal's processes come from its job, so the snapshot's completeness
does not decide them.

## Polling

There is no event and no watch. A client polls while it shows the figure: every 2 seconds while a memory detail is
open, otherwise with its usual status refresh. One sample reads the process table once (Unix) or each terminal's
job (Windows), and a few system calls per process, so a poll costs little, but nothing samples while nobody asks. A sample is shared for about a second: a call within that
time is answered with the sample already taken, and its `sampled_at_ms` is when that sample was taken. Ports are polled
every 30 seconds while they are visible, as the reference does.

A client that cannot reach the service shows `unverifiable`, never zero: the CLI exits 4 with `memory.unverifiable`
(`ports.unverifiable` for ports), and never reports "no ports" for an answer it could not get.
