//! Reads the one-file-per-step folder `crates/core-store/migrations`: file names, ids and the generated step table.
//! `build.rs` writes that table, and the crate's tests check it (decision 0119).

use std::fmt::Write;
use std::fs;
use std::path::Path;

const FOREIGN_KEYS_OFF_LINE: &str = "-- foreign_keys_off";

/// A step file found in the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStep {
    /// `<block>-<yyyymmddhhmm>`: the sort key, and the id recorded in `store_migrations`.
    pub id: String,
    /// The file name, embedded with `include_str!`.
    pub file_name: String,
    /// Whether the first line is `-- foreign_keys_off`.
    pub foreign_keys_off: bool,
}

/// The id of a step file: `0110-202611261430_example.sql` gives `0110-202611261430`.
pub fn parse_file_name(file_name: &str) -> Result<String, String> {
    let rule = || {
        format!(
            "migration file {file_name} must be named <block>-<yyyymmddhhmm>_<slug>.sql, for example 0110-202611261430_example.sql"
        )
    };
    let stem = file_name.strip_suffix(".sql").ok_or_else(rule)?;
    let (id, slug) = stem.split_once('_').ok_or_else(rule)?;
    if is_id(id) && is_slug(slug) {
        Ok(id.to_owned())
    } else {
        Err(rule())
    }
}

/// `<block>-<yyyymmddhhmm>`: a four-digit block, then a UTC timestamp with a real month, day, hour and minute.
fn is_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    let shape = bytes.len() == 17
        && bytes[4] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || byte.is_ascii_digit());
    shape && valid_timestamp(&id[5..])
}

fn valid_timestamp(stamp: &str) -> bool {
    let field = |range: std::ops::Range<usize>| stamp[range].parse::<u32>().ok();
    matches!(
        (
            field(0..4),
            field(4..6),
            field(6..8),
            field(8..10),
            field(10..12)
        ),
        (
            Some(2000..),
            Some(1..=12),
            Some(1..=31),
            Some(0..=23),
            Some(0..=59)
        )
    )
}

/// Lowercase letters, digits and single underscores between them, starting and ending with a letter or digit.
fn is_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && !slug.starts_with('_')
        && !slug.ends_with('_')
        && !slug.contains("__")
}

/// Every step file in `dir`, validated and sorted by id. Only `README.md` may sit beside them.
pub fn scan(dir: &Path) -> Result<Vec<FileStep>, String> {
    let entries =
        fs::read_dir(dir).map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
    let mut steps = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read {}: {error}", dir.display()))?;
        let file_name = entry
            .file_name()
            .into_string()
            .map_err(|name| format!("migration file name {name:?} is not UTF-8"))?;
        if file_name == "README.md" {
            continue;
        }
        if !file_name.ends_with(".sql") {
            return Err(format!(
                "{file_name} does not belong in the migrations folder: only README.md and step files ending in .sql"
            ));
        }
        let id = parse_file_name(&file_name)?;
        let sql = fs::read_to_string(entry.path())
            .map_err(|error| format!("cannot read migration file {file_name}: {error}"))?;
        let foreign_keys_off = check_sql(&file_name, &sql)?;
        steps.push(FileStep {
            id,
            file_name,
            foreign_keys_off,
        });
    }
    steps.sort_by(|left, right| left.id.cmp(&right.id));
    if let Some(pair) = steps.windows(2).find(|pair| pair[0].id == pair[1].id) {
        return Err(format!(
            "migration files {} and {} share the id {}",
            pair[0].file_name, pair[1].file_name, pair[0].id
        ));
    }
    Ok(steps)
}

/// Whether the first line of `sql` is exactly the `-- foreign_keys_off` marker; a later line does not count.
pub fn first_line_is_foreign_keys_off(sql: &str) -> bool {
    sql.lines()
        .next()
        .is_some_and(|line| line.trim_end() == FOREIGN_KEYS_OFF_LINE)
}

/// Rejects SQL the runner cannot apply safely, and returns whether the step runs with foreign keys off.
/// A step runs inside the runner's transaction, where `PRAGMA foreign_keys` is a no-op and transaction control breaks
/// the runner's own transaction; a table drop or a misspelled marker would silently break rows that reference it.
pub fn check_sql(file_name: &str, sql: &str) -> Result<bool, String> {
    let foreign_keys_off = first_line_is_foreign_keys_off(sql);
    if !foreign_keys_off && sql.to_ascii_lowercase().contains("foreign_keys") {
        return Err(format!(
            "migration file {file_name} mentions foreign_keys but its first line is not exactly {FOREIGN_KEYS_OFF_LINE}"
        ));
    }
    let mut in_trigger = false;
    for statement in sanitize(sql).split(';') {
        let words: Vec<String> = statement
            .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .filter(|word| !word.is_empty())
            .map(str::to_ascii_uppercase)
            .collect();
        let first = words.first().map_or("", String::as_str);
        // A trigger body holds statements ended by `;` up to its closing `END`.
        if in_trigger {
            in_trigger = !(words.len() == 1 && first == "END");
            continue;
        }
        if matches!(
            first,
            "BEGIN" | "COMMIT" | "END" | "ROLLBACK" | "SAVEPOINT" | "RELEASE"
        ) {
            return Err(format!(
                "migration file {file_name} contains the transaction control statement {first}; the runner owns the transaction"
            ));
        }
        if first == "DROP" && words.get(1).is_some_and(|word| word == "TABLE") && !foreign_keys_off
        {
            return Err(format!(
                "migration file {file_name} drops a table without the first line {FOREIGN_KEYS_OFF_LINE}"
            ));
        }
        let create_trigger =
            first == "CREATE" && words.iter().skip(1).take(2).any(|word| word == "TRIGGER");
        in_trigger = create_trigger && words.iter().any(|word| word == "BEGIN");
    }
    Ok(foreign_keys_off)
}

/// `sql` with comments and quoted text replaced by spaces, so only statement keywords remain.
fn sanitize(sql: &str) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '-' if chars.peek() == Some(&'-') => {
                for skipped in chars.by_ref() {
                    if skipped == '\n' {
                        break;
                    }
                }
                out.push('\n');
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut previous = ' ';
                for skipped in chars.by_ref() {
                    if previous == '*' && skipped == '/' {
                        break;
                    }
                    previous = skipped;
                }
                out.push(' ');
            }
            '\'' | '"' | '`' => {
                // A doubled quote inside the text closes and reopens it, which this loop handles as two runs.
                for skipped in chars.by_ref() {
                    if skipped == character {
                        break;
                    }
                }
                out.push(' ');
            }
            '[' => {
                for skipped in chars.by_ref() {
                    if skipped == ']' {
                        break;
                    }
                }
                out.push(' ');
            }
            _ => out.push(character),
        }
    }
    out
}

/// The Rust source of `FILE_MIGRATIONS`, included by `migration_files.rs`. Each SQL file is embedded at compile time.
pub fn render(steps: &[FileStep]) -> String {
    let mut source = String::from(
        "// Generated by crates/core-store/build.rs from crates/core-store/migrations; do not edit.\n",
    );
    source.push_str("pub(crate) const FILE_MIGRATIONS: &[FileMigration] = &[\n");
    for step in steps {
        writeln!(
            source,
            "    FileMigration {{\n        id: \"{}\",\n        sql: include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/migrations/{}\")),\n        foreign_keys_off: {},\n    }},",
            step.id, step.file_name, step.foreign_keys_off
        )
        .expect("writing to a String cannot fail");
    }
    source.push_str("];\n");
    source
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        FileStep, check_sql, first_line_is_foreign_keys_off, parse_file_name, render, scan,
    };

    const EXAMPLE: &str = "0110-202611261430_example.sql";

    #[test]
    fn a_well_formed_name_gives_its_id() {
        assert_eq!(parse_file_name(EXAMPLE), Ok("0110-202611261430".to_owned()));
        assert_eq!(
            parse_file_name("0007-202501010000_add_team_room_index.sql"),
            Ok("0007-202501010000".to_owned())
        );
    }

    #[test]
    fn a_malformed_name_is_rejected_with_the_rule() {
        let bad = [
            "0110-202611261430.sql",
            "110-202611261430_example.sql",
            "0110-20261126143_example.sql",
            "0110-202613011430_example.sql",
            "0110-202611321430_example.sql",
            "0110-202611261460_example.sql",
            "0110-202611261430_Example.sql",
            "0110-202611261430_.sql",
            "0110-202611261430_example_.sql",
            "0110-202611261430_a__b.sql",
            "0110-202611261430_example.txt",
            "example.sql",
        ];
        for name in bad {
            let error = parse_file_name(name).unwrap_err();
            assert!(error.contains(name), "{error}");
            assert!(
                error.contains("<block>-<yyyymmddhhmm>_<slug>.sql"),
                "{error}"
            );
        }
    }

    #[test]
    fn only_the_first_line_can_mark_foreign_keys_off() {
        assert!(first_line_is_foreign_keys_off(
            "-- foreign_keys_off\nALTER TABLE t ADD c;"
        ));
        assert!(first_line_is_foreign_keys_off(
            "-- foreign_keys_off  \r\nALTER TABLE t ADD c;"
        ));
        assert!(!first_line_is_foreign_keys_off(
            "  -- foreign_keys_off\nALTER TABLE t ADD c;"
        ));
        assert!(!first_line_is_foreign_keys_off(
            "ALTER TABLE t ADD c;\n-- foreign_keys_off"
        ));
        assert!(!first_line_is_foreign_keys_off(""));
    }

    #[test]
    fn scan_sorts_by_id_reads_the_marker_and_ignores_other_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("README.md"), "not a step").unwrap();
        fs::write(dir.path().join("0111-202611261430_later.sql"), "SELECT 2;").unwrap();
        fs::write(dir.path().join(EXAMPLE), "-- foreign_keys_off\nSELECT 1;").unwrap();

        let steps = scan(dir.path()).unwrap();

        assert_eq!(
            steps,
            [
                FileStep {
                    id: "0110-202611261430".to_owned(),
                    file_name: EXAMPLE.to_owned(),
                    foreign_keys_off: true,
                },
                FileStep {
                    id: "0111-202611261430".to_owned(),
                    file_name: "0111-202611261430_later.sql".to_owned(),
                    foreign_keys_off: false,
                },
            ]
        );
    }

    #[test]
    fn scan_rejects_two_files_with_one_id_and_names_both() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(EXAMPLE), "SELECT 1;").unwrap();
        fs::write(dir.path().join("0110-202611261430_other.sql"), "SELECT 2;").unwrap();

        let error = scan(dir.path()).unwrap_err();

        assert!(error.contains(EXAMPLE), "{error}");
        assert!(error.contains("0110-202611261430_other.sql"), "{error}");
        assert!(error.contains("share the id 0110-202611261430"), "{error}");
    }

    #[test]
    fn scan_names_a_file_with_a_bad_name() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("oops.sql"), "SELECT 1;").unwrap();

        let error = scan(dir.path()).unwrap_err();

        assert!(error.contains("oops.sql"), "{error}");
    }

    #[test]
    fn a_foreign_keys_mention_without_the_exact_marker_is_rejected() {
        let bad = [
            "-- foreign_keys off\nALTER TABLE t ADD c;",
            "--foreign_keys_off\nALTER TABLE t ADD c;",
            "  -- foreign_keys_off\nALTER TABLE t ADD c;",
            "-- rebuild t\n-- foreign_keys_off\nALTER TABLE t ADD c;",
            "PRAGMA FOREIGN_KEYS = OFF;\nALTER TABLE t ADD c;",
        ];
        for sql in bad {
            let error = check_sql(EXAMPLE, sql).unwrap_err();
            assert!(error.contains(EXAMPLE), "{error}");
            assert!(error.contains("-- foreign_keys_off"), "{error}");
        }
        assert_eq!(
            check_sql(EXAMPLE, "-- foreign_keys_off\nALTER TABLE t ADD c;"),
            Ok(true)
        );
        assert_eq!(check_sql(EXAMPLE, "ALTER TABLE t ADD c;"), Ok(false));
    }

    #[test]
    fn a_table_drop_without_the_marker_is_rejected() {
        for sql in [
            "DROP TABLE t;",
            "drop  table IF EXISTS t;",
            "SELECT 1;\n  Drop\nTable t;",
        ] {
            let error = check_sql(EXAMPLE, sql).unwrap_err();
            assert!(error.contains(EXAMPLE), "{error}");
            assert!(error.contains("drops a table"), "{error}");
        }
        assert_eq!(
            check_sql(EXAMPLE, "-- foreign_keys_off\nDROP TABLE t;"),
            Ok(true)
        );
        assert_eq!(
            check_sql(
                EXAMPLE,
                "-- DROP TABLE t;\nDROP INDEX i; SELECT 'drop table';"
            ),
            Ok(false)
        );
    }

    #[test]
    fn transaction_control_is_rejected() {
        let bad = [
            "BEGIN;\nSELECT 1;",
            "begin immediate; SELECT 1;",
            "SELECT 1; COMMIT;",
            "SELECT 1; end transaction;",
            "SELECT 1; End;",
            "SELECT 1;\nROLLBACK;",
            "SAVEPOINT s; SELECT 1;",
            "SELECT 1; /* done */ RELEASE s;",
        ];
        for sql in bad {
            let error = check_sql(EXAMPLE, sql).unwrap_err();
            assert!(error.contains(EXAMPLE), "{error}");
            assert!(error.contains("transaction control"), "{error}");
        }
    }

    #[test]
    fn transaction_words_outside_statement_starts_are_allowed() {
        let sql = "-- BEGIN; COMMIT;\n\
            /* ROLLBACK; */\n\
            CREATE TABLE t (id INTEGER PRIMARY KEY ON CONFLICT ROLLBACK, \"end\" TEXT, note TEXT DEFAULT 'commit;');\n\
            CREATE TRIGGER t_check BEFORE INSERT ON t\n\
            WHEN NEW.note = 'x'\n\
            BEGIN\n\
              SELECT RAISE(ROLLBACK, 'no x');\n\
              UPDATE t SET note = CASE WHEN note IS NULL THEN 'a' ELSE note END;\n\
            END;\n\
            INSERT OR ROLLBACK INTO t (note) VALUES ('begin');";

        assert_eq!(check_sql(EXAMPLE, sql), Ok(false));
    }

    #[test]
    fn scan_rejects_a_file_that_is_not_a_step_or_the_readme() {
        for name in [
            "0110-202611261430_example.SQL",
            "0110-202611261430_example.sql.txt",
            "notes.md",
        ] {
            let dir = tempfile::tempdir().unwrap();
            fs::write(dir.path().join("README.md"), "not a step").unwrap();
            fs::write(dir.path().join(name), "SELECT 1;").unwrap();

            let error = scan(dir.path()).unwrap_err();

            assert!(error.contains(name), "{error}");
            assert!(error.contains("only README.md"), "{error}");
        }
    }

    #[test]
    fn scan_names_a_file_whose_sql_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(EXAMPLE), "DROP TABLE t;").unwrap();

        let error = scan(dir.path()).unwrap_err();

        assert!(error.contains(EXAMPLE), "{error}");
    }

    #[test]
    fn render_embeds_each_file_with_its_id_and_flag() {
        let steps = [FileStep {
            id: "0110-202611261430".to_owned(),
            file_name: EXAMPLE.to_owned(),
            foreign_keys_off: true,
        }];

        let source = render(&steps);

        assert!(source.contains("pub(crate) const FILE_MIGRATIONS: &[FileMigration] = &["));
        assert!(source.contains("id: \"0110-202611261430\","));
        assert!(source.contains(
            "include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/migrations/0110-202611261430_example.sql\"))"
        ));
        assert!(source.contains("foreign_keys_off: true,"));
        assert!(render(&[]).contains("= &[\n];"));
    }
}
