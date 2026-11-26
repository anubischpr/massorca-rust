# Schema migrations

Every schema change for the store is one file here, or one of the frozen steps in `src/migrations.rs`
(decision 0119).

## Adding a step

1. Name the file `<block>-<yyyymmddhhmm>_<slug>.sql`:
   - `<block>` is your session's decision block, four digits (for example `0110`).
   - `<yyyymmddhhmm>` is the UTC time you write it, for example `202611261430`.
   - `<slug>` is lowercase letters, digits and single underscores, for example `example`.
   - Example: `0110-202611261430_example.sql`.
2. Put plain SQL in it. Do not add Rust source.
3. Start with the line `-- foreign_keys_off` only when the step rebuilds a table that rows reference (SQLite's
   table-rebuild procedure). Such a step runs with foreign keys off, as the frozen rebuild steps do.
4. Do not touch `src/migrations.rs`. The build script (`build.rs`) finds the file; no shared file changes.

## Rules

- Never edit, rename or delete a file once it is on `main`. Add a new file to change the schema again.
- The frozen list in `src/migrations.rs` never grows. A test pins its contents.
- A step must not depend on another session's step that is not on `main` yet.
- Steps apply in id order on a new database. On an existing database, a step with a lower id than one already
  applied still runs: applied steps are recorded by id in `store_migrations`.
- A database that has an applied id this build does not know is opened, with a warning, not refused.
