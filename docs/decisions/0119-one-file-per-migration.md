# 0119: One file per migration step

Status: accepted, 2026-11-26.

## Context

- Every session appended a step to the single `MIGRATIONS` list in `crates/core-store/src/migrations.rs`, applied by
  position through SQLite's `user_version`.
- Two open pull requests that each add a step conflict textually. Whoever lands second re-merges and keeps the list
  append-only. It happened seven times in one day.

## Decision

- **Frozen list.** The existing steps stay in `MIGRATIONS`, keep their `user_version` numbering, and are never edited.
  `crates/core-store/tests/frozen_migrations.rs` pins the count and a fingerprint of each step's SQL and foreign-keys
  flag. The frozen list never grows.
- **One file per new step.** A step is `crates/core-store/migrations/<block>-<yyyymmddhhmm>_<slug>.sql`: `<block>` is
  the author session's decision block, `<yyyymmddhhmm>` the UTC time. A first line `-- foreign_keys_off` runs the step
  with foreign keys off, as the frozen rebuild steps do.
- **Build script.** `crates/core-store/build.rs` lists the folder, validates each name and rejects duplicate ids, sorts
  by id and writes an `include_str!` table to `OUT_DIR`. Adding a step edits no shared file. The build fails, naming
  the file, when:
  - a file other than `README.md` does not end exactly in lowercase `.sql` (for example `.SQL` or `.sql.txt`);
  - a file mentions `foreign_keys` but its first line is not exactly `-- foreign_keys_off`;
  - a file contains `DROP TABLE` (any case) without that first line;
  - a statement outside comments starts with `BEGIN`, `COMMIT`, `END`, `ROLLBACK`, `SAVEPOINT` or `RELEASE` (any
    case): the runner owns each step's transaction. A trigger body's `BEGIN ... END` is allowed.
- **Tracking table.** Applied file steps are recorded by id in
  `store_migrations(id TEXT PRIMARY KEY, applied_at INTEGER NOT NULL)`. The runner creates it with the first file step
  that runs. It is not a frozen step, because the frozen list must not grow. A database with no file steps keeps the
  schema it had.
- **Order.** The frozen steps run first, as before, then every file step whose id is not recorded, in id order. Each
  runs in its own transaction with its id insert, under the same OS lock and write lock as before. A lower id added
  after a higher one was applied still runs.
- **Backup.** A file database is backed up once, before the first change of any kind in a run. A database this run
  created empty is not backed up. A run that starts with a frozen step writes `<file>.bak-v<user_version>`; a run that
  starts with a file step writes `<file>.bak-<id>` after that step's id. An existing backup is never replaced: a
  taken name gets `.2`, `.3`, and so on. The five newest backups of either kind are kept.
- **Refusal.** A `user_version` above the frozen count is still refused. An applied id this build does not know is not
  refused: it is logged as a warning once per open, because another branch's step may be applied to a development
  database. So an older build that opens a database a newer build migrated with file steps is no longer refused, as it
  was when every step raised `user_version`; it opens the database with the newer steps applied. Only the frozen
  list's newer-version refusal remains.
- **Authors.** A step must not depend on another session's step that is not on `main` yet. Never edit or rename a file
  once it is on `main`. The rules are in `crates/core-store/migrations/README.md`.

## Consequences

- A new step adds one file and touches no shared file, so two open steps no longer conflict.
- Open pull requests that append to `MIGRATIONS` move their step into a file under their own block id. The frozen-list
  test names the folder when a shipped step is edited.
- The order of steps on a database is the order they were applied, which can differ from id order across branches.
  Each step must therefore stand on its own, or on steps already on `main`.
