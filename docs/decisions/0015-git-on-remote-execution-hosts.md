# 0015: Git operations on a remote execution host

Status: accepted, 2026-11-21.

## Context

Phase 3 adds a network listener: the service runs on a server and a client on
a desktop drives it. The inventory also describes git inside WSL distributions
(GIT-29) and on SSH hosts through a relay (GIT-28). The git API built now must
work there without a second version.

## Decision

The service that owns a workspace is the execution host for everything that
touches it. A client never runs git and never watches files itself. Remote use
is the same protocol over a different transport (0002), not a different API.
`git/1` keeps that possible:

- **No host paths from clients.**
  - Methods name a workspace selector and workspace-relative paths (0010).
  - No method takes an absolute path on the host, except `project.add`
    (Phase 1), whose path is by definition on the host.
- **No host details in replies beyond the workspace path.** Results carry
  revisions, workspace-relative paths, counts and typed outcomes, not the
  host's environment or git configuration.
- **Credentials stay on the host.**
  - A remote operation authenticates with the host's git setup (0013).
  - A client cannot send a credential through `git/1`. A later phase may add a
    credential relay as its own capability, behind explicit consent.
- **Notifications come from the host.** `git.watch` events come from the
  host's file system (0012), so a desktop client gets the same events from a
  remote service.
- **Timeouts and cancellation are host-side.** Closing a connection cancels
  the client's in-flight requests on the host (Phase 1, item 6), and that kills
  the git process tree there.

### WSL and SSH relays

- A Windows service that reaches a repository inside a WSL distribution would
  need per-command routing (GIT-29). Phase 2 does not do that.
- A repository inside WSL is served by a service running inside that
  distribution, and the Windows client connects to it as to any other host.
- Per-command routing, if ever needed, plugs in below `core-git`'s runner.
  It would take an execution-host handle chosen per workspace and leave the
  protocol unchanged.
- The SSH relay of the current product is replaced by running the service on
  the SSH host (Phase 3).

## Consequences

- Phase 3 adds transport and authentication, and no git work.
- A client written against a local service works unchanged against a remote
  one. Contract tests stay valid because they never use host paths in
  requests.
