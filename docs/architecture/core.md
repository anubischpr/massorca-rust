# Core architecture

This is the overview of the core as built in Phase 1. Each choice has a
record in `docs/decisions/`; this page shows how they fit together.

## Shape

```
 massorca (CLI)          GPUI desktop (Phase 5)     Android (Phase 6)
      |                         |                         |
   client crate  <------------- + ------------------> client-ffi (UniFFI)
      |  local socket (named pipe / Unix socket)          |  WebSocket + TLS (Phase 3)
      v                                                   v
 +--------------------------- massorca service run ---------------------------+
 |  server: listeners -> connections -> method registry -> handlers            |
 |  core-workspace (projects, workspaces) -> core-git (user's git binary)      |
 |  core-store (SQLite)                                                        |
 +----------------------------------------------------------------------------+
            one service per profile: <home>/profiles/<name>/
```

There is no in-process API next to the protocol: the CLI, the desktop and the
phone all speak the same messages, so a feature tested from the CLI is tested
for every surface.

## A connection, step by step

1. The client resolves the profile (`--profile`, `MASSORCA_PROFILE`,
   `default`) and its socket name (0004).
2. It connects. If nobody listens, the CLI starts `massorca service run`
   detached and retries (0003).
3. It sends `hello` with its protocol range and capabilities. The service
   answers `welcome` with the chosen version and its own capabilities (0002).
4. Requests carry a client-chosen id; responses carry the same id with a
   `result` or a typed `error`. A request can open a stream; the service then
   pushes `event` frames until `end` or `cancel`.

## Method registry

Handlers are registered by method name together with the capability they
belong to and the permission class they need. The dispatcher knows nothing
about specific features: it looks the method up, checks the caller holds the
permission, decodes the params and calls the handler. Capabilities offered in
`welcome` are computed from the registry. Extensions (see
`docs/architecture/extensions.md` once written) register methods and
capabilities the same way, so the core leaves room for them without a list to
edit.

Phase 1 capabilities:

| Capability | Methods |
|---|---|
| `service/1` | `service.info`, `service.shutdown` |
| `projects/1` | `project.add`, `project.list`, `project.show`, `project.remove` |
| `workspaces/1` | `workspace.create`, `workspace.list`, `workspace.show`, `workspace.remove` |

## Profiles on disk

```
<home>/profiles/<name>/
  service.lock   service.json   service.sock (Unix)   store.sqlite3   logs/
```

`<home>` is `MASSORCA_HOME` or the platform data directory. Tests point
`MASSORCA_HOME` at a temporary directory.

## Exit codes of `massorca`

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | the service answered with an error (the code is printed; `--json` prints it in full) |
| 2 | invalid command line |
| 3 | `service run`: another service owns this profile |
| 4 | the service could not be reached or started |

## Decisions

- 0002 protocol, transport and framing
- 0003 service lifecycle
- 0004 profiles and the data directory
- 0005 persistence
- 0006 async runtime, errors, logging and crate layout
- 0007 cross-platform rules
- 0008 projects and workspaces in Phase 1
