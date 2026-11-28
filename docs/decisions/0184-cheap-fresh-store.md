# 0184: Cheap fresh-store creation

Status: accepted, 2026-11-27.

## Context

The service's first start creates a brand-new store. On the Windows CI runner its single schema commit (decision 0119
already builds the frozen list and the migration files in one transaction) took 4.1-6.7 s against the client's 10 s start
wait, and main went red with `service.start_failed`. The time is spent in disk syncs of the rollback journal and the
database during that commit, not in the SQL.

## Decision

When the store file does not exist, `Store::open` builds it aside (`crates/core-store/src/fresh_store.rs`):

- Under the existing `<store>.migrating` lock, it re-checks that the file is still missing, removes any
  `<store>.creating-*` file (every builder holds the lock while its temporary file exists, so one seen under the lock is
  left by a crash), and builds the schema at `<store>.creating-<pid>-<n>` with `synchronous = OFF` and
  `journal_mode = MEMORY`, in one transaction.
- It closes that file, syncs it once, and gives it the real name with a hard link, which fails instead of replacing a
  file that appeared meanwhile; that file is then kept and the temporary one dropped. A file system without hard links
  falls back to a rename after re-checking that the name is free. On Unix the directory is synced after.
- The normal open follows: `synchronous = NORMAL`, the up-to-date check (which finds nothing to do) and the WAL switch,
  which still writes to the final file.
- A failed build, a failing migration file included, removes the temporary file and leaves no store; the next start
  builds again. Before, a failing migration file left a store holding the steps before it.

Existing stores, including an empty file that already exists, open and migrate exactly as before.

Durability: the relaxed settings only cover a file nobody can see. A crash before the sync and link leaves a temporary
file that the next creation removes, never a half-built store under the real name. After the sync, the link publishes a
complete file, and from then on the normal settings apply.

The observed open steps keep their names. For a new store their order is now `store migration lock`, `store schema`,
`store file`, `store wal`; the test that lists them was updated.

## Measurements

Local, Windows 11, debug build, `cargo test -p massorca-core-store fresh_store_timing -- --ignored --nocapture`, three
invocations of three opens each, total time of `Store::open_observed` on a missing file:

| | Opens (ms) | Median |
|---|---|---|
| Before | 459, 124, 123; 154, 107, 153; 85, 80, 75 | 123 ms |
| After | 52, 42, 52; 53, 54, 54; 186, 185, 113 | 54 ms |

The local disk is fast, so most of what is left is SQL work in a debug build; the machine was shared with other builds,
which explains the slow third run after.

Windows CI: **TO BE FILLED after the PR runs** (first-start `store schema` time on the runner, before 4.1-6.7 s).

## Alternatives

- Widening the client's 10 s start wait: hides the cost on every slow disk instead of removing it, and makes a real hang
  take longer to report. Whether the CLI test harness's 10 s also needs margin is decided from the CI numbers above.
- `synchronous = OFF` on the real file: a crash mid-build would leave a corrupt store under the real name.
