# 0057: Agent crates, wire capability and testing

Status: accepted, 2026-11-21.

## Context

Every core feature has a CLI entry point and is tested through it, and
tests must be deterministic on a loaded Windows machine and in WSL. Real
agent CLIs need accounts, network access and change their output between
versions, so they cannot drive the tests.

## Decision

- **Crate `core-agents`** (package `massorca-core-agents`): kind
  declarations and their loader, program resolution, the launch plan (args,
  environment, generated hook files), the status store and the delegation
  rules. It uses `core-terminal` and `core-workspace` and has no protocol
  IO. Session rows live in `core-store` (table `agent_sessions`, a new
  migration). The merge for bring-back is a `core-git` operation.
- **Capability `agents/1`** with methods `agent.kinds`, `agent.start`,
  `agent.list`, `agent.show`, `agent.stop`, `agent.forget`, `agent.prompt`,
  `agent.resume`, `agent.hook`, `agent.watch` (stream), `agent.delegate`,
  `agent.tree`, `agent.bring_back`, and `agent.credentials.import`,
  `agent.credentials.show`, `agent.credentials.remove`. Permission class:
  `read` for kinds, list, show, tree and watch; `workspace` for the rest.
  Error codes live under `agent.*`.
- **CLI group `massorca agent`**: `kinds`, `start`, `list`, `status`
  (`--watch`, `--until <activity|exited>` with `--timeout`), `stop`,
  `forget`, `attach`, `prompt`, `resume`, `hook`, `delegate`, `tree`,
  `bring-back`, `credentials import|show|remove`. `status --until` waits on
  the watch stream, so scripts and tests wait on an observed state instead
  of sleeping.
- **The fake agent** is a binary `massorca-fake-agent` built with the CLI
  package for its tests (`CARGO_BIN_EXE_massorca-fake-agent`). Installers
  ship only `massorca`. It behaves like an agent CLI: prints a banner,
  reads prompt lines, reports `idle`, `working`, `waiting` and `done`
  through `massorca agent hook` (mechanism `env-command`), asks a question
  when a prompt says so, writes and commits a file when told (for
  bring-back), honours `--session-id` and `--resume`, reads a token file
  from its credential home and prints only whether it was found. Tests
  declare it as kind `fake` in the test profile.
- **Real agents are opt-in.** One smoke test starts the real `claude` kind
  only when `MASSORCA_REAL_AGENT_SMOKE=claude` is set; it is `#[ignore]`d
  otherwise and never runs in CI. It uses whatever login the profile has
  and prints no credential.
- **Determinism.** Tests wait on reported states (`status --until`) and on
  markers in terminal output, with generous upper bounds; never on fixed
  sleeps.

## Alternatives

- **A script fake agent** (PowerShell and sh). Two implementations to keep
  equal, and quoting differences between them.
- **A separate test-only package for the fake agent.** Cargo gives a
  package's integration tests only that package's binaries, so the CLI
  tests could not find it without an extra build step.

## Consequences

- `cargo install` of the CLI package would also install the fake agent; the
  release packaging names the `massorca` binary only.
