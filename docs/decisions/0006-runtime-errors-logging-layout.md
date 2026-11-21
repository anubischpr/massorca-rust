# 0006: Async runtime, errors, logging and crate layout

Status: accepted, 2026-11-20.

## Decision

**Toolchain.** Rust 1.92.0 pinned in `rust-toolchain.toml`; edition 2024;
`resolver = "3"` with `rust-version = "1.92"`, so Cargo picks dependency
versions that build on the pinned compiler.

**Async runtime.** tokio (multi-thread runtime in the service, current-thread
in the CLI). It is the ecosystem default and what `interprocess`,
`tokio-tungstenite` and future PTY crates integrate with.

**Errors.** Library crates define typed errors with `thiserror`. Errors that
cross the protocol become `{code, message, data}` (0002) at one boundary in
the service; handlers never build strings for clients to parse. `anyhow` is
allowed only in the `massorca` binary's `main`.

**Logging.** `tracing`. The service writes to `<profile>/logs/service.log`,
rotated daily, seven files kept (`tracing-appender`), filter from
`MASSORCA_LOG` (default `info`). The CLI logs warnings and errors to stderr
only, never to stdout, which belongs to command output. No secrets in logs:
tokens and credentials are never formatted with `Debug`.

**Crates.** Package names are prefixed `massorca-`; directories follow
`docs/PLAN.md`. Phase 1 builds:

| Crate | Role |
|---|---|
| `protocol` | message types, frames, versions, capabilities, error codes; no IO |
| `profile` | home and profile resolution, profile layout, socket names (new) |
| `core-store` | SQLite store and migrations |
| `core-git` | running the user's git binary |
| `core-workspace` | projects and workspaces: worktrees and folders |
| `server` | the service: listener, connection handling, method registry |
| `client` | connection, handshake, typed calls, auto-start |
| `cli` | the `massorca` binary |

`profile` is added because both the client (to find the socket) and the
server (to bind it) need the same answer; putting it in either would make the
other depend on it.

## Alternatives

- **async-std / smol.** Smaller, but the crates we need assume tokio.
- **`anyhow` everywhere.** Loses the typed codes the protocol needs.
- **`log` + `env_logger`.** No spans; spans matter once requests fan out to git
  and terminals.

## Consequences

- Moving to a newer Rust is a one-line change plus a CI run.
- Every new error that reaches a client needs a stable code.
