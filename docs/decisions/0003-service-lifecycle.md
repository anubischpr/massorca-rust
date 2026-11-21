# 0003: Service lifecycle

Status: accepted, 2026-11-20.

## Context

The core always runs as a service. The CLI must find it, start it when it is
not running, and talk to it. One service per profile. Later the same binary
runs headless on a server.

## Decision

- **One binary, `massorca`.** It is the CLI and, through `massorca service
  run`, the service. The headless server is the same command with a network
  listener enabled (`--listen`, Phase 3). The name `massorca` avoids the
  GNOME screen reader `orca` and the production MassOrca install.
- **Finding the service.** The client derives the local socket name from the
  profile (0004) and connects. No discovery file is needed to connect. The
  service still writes `service.json` (pid, version, protocol range, start
  time) for people and diagnostics.
- **Starting it.** When the connection fails because nobody listens, the CLI
  starts `massorca service run --profile <p>` detached (Windows:
  `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW`; Unix: new
  process group, stdio to null) and retries the connection for up to 10
  seconds. `--no-autostart` or `MASSORCA_NO_AUTOSTART=1` disables this.
- **Single instance.** The service takes an exclusive lock on
  `<profile>/service.lock` (`std::fs::File::try_lock`, released by the OS if
  the process dies) before binding. While a service answers on the endpoint, a
  held lock means another service serves the profile: the new one exits with
  code 3, a stable contract for supervisors and for the auto-start race (two
  CLIs starting it at once: the loser exits 3, both connect to the winner).
  While nothing answers, the predecessor may still be draining its connections,
  so the successor waits up to 10 seconds for the lock. After taking the lock
  it removes a stale Unix socket file. A bind the system refuses for a moment
  is retried for up to 5 seconds.
- **Shutdown.** `massorca service stop` sends `service.shutdown`. The service
  stops accepting connections, answers in-flight requests, ends streams,
  closes the store and exits 0. Ctrl+C and SIGTERM do the same. The shutdown
  request is answered once the service has stopped accepting connections, so a
  client that connects right after the answer cannot reach the old service.
- **Status.** `massorca service status` connects without auto-starting and
  prints the `welcome` data or "not running".
- **Endpoint ownership.** Only the current user may reach the service, and a
  client refuses an endpoint another user serves. Windows: the pipe is created
  with an owner-only descriptor (`D:P(A;;GA;;;<current user SID>)`, protected
  from inheritance); the first-instance flag stops a second server taking the
  name, and remote clients are rejected by default. Unix: the profile directory
  is `0700` (0004) and the socket file is `0600` once bound. Before the first
  `hello`, the client checks the far end: on Windows the server process must run
  as the current user (`GetNamedPipeServerProcessId`, then the process token),
  on Unix the peer's effective uid must equal its own. Any endpoint that fails
  the check, or whose owner cannot be read, is refused with
  `service.untrusted_endpoint` and never retried.
- **Known limits.** A process of the same user can still stand in for the
  service; the check separates users, not processes of one user. On Windows, a
  restart right after shutdown can be refused with access denied for a moment
  while the old pipe instance is released. The service retries its bind for up
  to five seconds, and a client with autostart waits out the same refusal (busy
  or access denied) until its connect timeout.

## Alternatives

- **Separate `massorcad` binary.** Two artifacts to version and ship, and the
  CLI must locate the other binary. One binary removes that whole class of
  mismatch.
- **OS services (systemd units, Windows services) for the desktop.** Fit the
  headless server, not a per-user desktop session; they can be added as
  installers later around the same `service run` command.
- **Discovery file as the only locator.** Can go stale; a deterministic name
  cannot.

## Consequences

- A CLI newer than the running service just negotiates fewer capabilities.
  `massorca service restart` is the explicit way to move to the new binary.
- Tests can run the service in-process (client tests) or spawn the real binary
  (CLI tests) with a temporary `MASSORCA_HOME`.
