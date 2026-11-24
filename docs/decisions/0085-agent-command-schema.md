# 0085: Agent command schema

Status: accepted, 2026-11-23.

## Context

Agents need to discover what the CLI and the service can do (inventory CLI-08). The command descriptors already name
each CLI path, the method it issues, a one-line summary and the JSON Schema of its parameters, but nothing exposed
them. The permission and capability a method needs are held by the method registry, not by the descriptor.

## Decision

- Capability `commands/1` offers one read-only method, `commands.describe`, with permission `read`. Its result is the
  command table: per entry `id` (the dotted CLI path), `title`, `method`, `capability`, `permission` (`read`,
  `workspace`, `host-admin` or `device-admin`) and `args_schema`. `capability` and `permission` are read from the
  method registry, so they are null only when the service registers no such method.
- `massorca agent-context` joins that table with the commands this CLI runs itself. With `--json` it prints
  `schema_version` (1), `command_count` and `commands`, sorted by `command`. Each command carries `command` (as typed,
  `git status`), `summary`, `local`, `method`, `capability`, `permission` and `args_schema`.
- Local commands are listed in `LOCAL_COMMANDS` (`crates/cli/src/agent_context.rs`). They make no request, so their
  summary is the clap help line and their method, capability, permission and parameters are null. `service run` is
  local because it hosts the service.
- Without `--json` the command prints a count and one line per command. Parameters appear only in JSON.
- The client offers `commands/1` in `hello`. A service whose welcome does not offer it predates the table: the command
  fails with `service.capability_missing` and asks for a restart instead of calling the method.

## Consequences

- The descriptor table holds no per-parameter help, aliases, usage strings, examples or notes, so schema 1 omits them
  rather than inventing them. Agents read `massorca <command> --help` for those.
- A new CLI command needs a descriptor in the core table or an entry in `LOCAL_COMMANDS`; the command coverage test
  fails until one of them exists.
- Schema 1 is additive. Removing or renaming a field, or changing what one means, bumps `schema_version`.
