# 0005: Persistence

Status: accepted, 2026-11-20.

## Context

The service needs durable state (projects, workspaces, later terminals'
layout, agent status history, devices) that survives restarts and crashes and
that a newer binary can upgrade while an older one refuses to damage.

## Decision

- **Engine.** SQLite through `rusqlite` with the `bundled` feature (no system
  library on any platform), WAL journal, `synchronous=NORMAL`,
  `foreign_keys=ON`, a busy timeout.
- **Access.** One connection owned by the store behind a mutex; async callers
  go through `tokio::task::spawn_blocking`. SQLite serialises writes anyway,
  and one connection keeps transactions simple.
- **Migrations.** Our own small runner: an ordered list of SQL steps; the
  applied count is SQLite's `user_version`. Each step runs in a transaction.
  A database whose `user_version` is newer than the binary knows is refused
  (`store.newer_schema`) and never touched. Before migrating an existing
  database the file is copied to `store.sqlite3.bak-v<old>`; the last five
  backups are kept.
- **What is durable.** Projects and workspaces, including a workspace's
  `removing` state, written before any file is deleted so an interrupted
  removal is visible and can be resumed. Connections, streams and anything
  derivable from git or the filesystem are not stored.
- **Ids.** UUID v7 strings, so ids sort by creation time.

## Alternatives

- **`rusqlite_migration`.** Does the same over `user_version`, but pins a
  `rusqlite` version range and adds a dependency for about thirty lines.
- **sled / redb.** Key-value stores; we want relational queries and the
  sqlite3 shell for diagnosis.
- **JSON files** (as in the old app). Fragile under concurrent writers and
  needs hand-written atomic-write discipline and versioning.
- **sqlx.** Async and compile-time checked queries, but heavier, needs a
  database at build time for its checking, and SQLite gains nothing from
  async IO.

## Consequences

- Schema changes are forward-only SQL steps with a test that migrates an
  empty database to the latest version.
- Downgrading the binary over newer data fails loudly instead of corrupting it.
