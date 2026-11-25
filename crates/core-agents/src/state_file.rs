//! An account home's state file (decision 0099): the file in which a kind's CLI keeps its own state beside the login,
//! such as Claude Code's `.claude.json`.
//!
//! It is never copied raw. An import builds it from the source file: only the rule's `keep` keys present there, then
//! the `set` keys with their values. Before every account check and every launch on an account home, [`ensure`] puts
//! the `set` keys back when they are missing or differ, keeping every other key, so the check and the session see the
//! same home. Contents are never logged or returned: key names and reasons only.

use std::fs;
use std::io;
use std::path::Path;

use serde_json::{Map, Value};

use crate::credentials::{self, MISSING};
use crate::kind::StateDecl;

/// What an import puts in the home for the state file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Imported {
    /// The file to write. `found` when the source was used; `note` says why it was not, when it was there.
    Built {
        bytes: Vec<u8>,
        found: bool,
        note: Option<String>,
    },
    /// A required state file that cannot be used, and why (`missing`, `a link`, `not a JSON object`...).
    Refused(String),
}

/// What [`ensure`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Repair {
    Unchanged,
    Created,
    Merged,
}

/// The reason of a state file that parses but is not a JSON object.
pub(crate) const NOT_AN_OBJECT: &str = "not a JSON object";

/// The source's object, when `bytes` is one.
pub(crate) fn parse_object(bytes: &[u8]) -> Option<Map<String, Value>> {
    match serde_json::from_slice(bytes) {
        Ok(Value::Object(object)) => Some(object),
        _ => None,
    }
}

/// The state file built from `source`: its `keep` keys that are present, then every `set` key.
pub(crate) fn build(rule: &StateDecl, source: Option<&Map<String, Value>>) -> Vec<u8> {
    let mut state = Map::new();
    if let Some(source) = source {
        for key in &rule.keep {
            if let Some(value) = source.get(key) {
                state.insert(key.clone(), value.clone());
            }
        }
    }
    for (key, value) in &rule.set {
        state.insert(key.clone(), value.clone());
    }
    to_bytes(state)
}

/// Builds the state file of an import from the source at `path` (`None` when no location is known). A required file
/// that cannot be used is refused; an optional one is built from the `set` keys alone, with a note when it was there.
pub(crate) fn import_source(rule: &StateDecl, path: Option<&Path>, required: bool) -> Imported {
    let read = match path {
        Some(path) => credentials::read_capped(path, rule.max_source_bytes),
        None => Err(MISSING.to_owned()),
    };
    let used = read.and_then(|bytes| parse_object(&bytes).ok_or_else(|| NOT_AN_OBJECT.to_owned()));
    match used {
        Ok(source) => Imported::Built {
            bytes: build(rule, Some(&source)),
            found: true,
            note: None,
        },
        Err(reason) if required => Imported::Refused(reason),
        Err(reason) => {
            let note = (reason != MISSING).then(|| {
                let keys: Vec<&str> = rule.set.keys().map(String::as_str).collect();
                format!(
                    "{} was built with only {} because the source is {reason}",
                    rule.file,
                    keys.join(", ")
                )
            });
            Imported::Built {
                bytes: build(rule, None),
                found: false,
                note,
            }
        }
    }
}

/// Puts the rule's `set` keys into the state file of `home`, creating it when it is missing; every other key is kept,
/// and nothing is written when they already hold. A link, anything but a regular file, a file over the rule's cap or one
/// that is not a JSON object is left as it is, and the reason is returned. The caller holds the homes gate.
pub(crate) fn ensure(home: &Path, rule: &StateDecl) -> Result<Repair, String> {
    let path = home.join(&rule.file);
    let bytes = match credentials::read_capped(&path, rule.max_source_bytes) {
        Err(reason) if reason == MISSING => {
            replace(home, &rule.file, &build(rule, None))?;
            return Ok(Repair::Created);
        }
        other => other?,
    };
    let mut state = parse_object(&bytes).ok_or_else(|| NOT_AN_OBJECT.to_owned())?;
    if rule
        .set
        .iter()
        .all(|(key, value)| state.get(key) == Some(value))
    {
        return Ok(Repair::Unchanged);
    }
    for (key, value) in &rule.set {
        state.insert(key.clone(), value.clone());
    }
    replace(home, &rule.file, &to_bytes(state))?;
    Ok(Repair::Merged)
}

fn to_bytes(state: Map<String, Value>) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(&Value::Object(state)).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

/// Writes `name` in `home` atomically: a new owner-only sibling file, renamed over it. A rename replaces a link at the
/// path itself and never writes through it.
fn replace(home: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    let temporary = home.join(format!("{name}.massorca-new"));
    if let Ok(meta) = fs::symlink_metadata(&temporary) {
        // A leftover of an interrupted write; only a file or a link is removed, never followed.
        if meta.is_dir() {
            return Err(format!("{} is in the way", temporary.display()));
        }
        fs::remove_file(&temporary).map_err(|error| reason("clear", &error))?;
    }
    if let Err(error) = credentials::write_private_file(&temporary, bytes) {
        let _ = fs::remove_file(&temporary);
        return Err(reason("write", &error));
    }
    fs::rename(&temporary, home.join(name)).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        reason("replace", &error)
    })
}

fn reason(doing: &str, error: &io::Error) -> String {
    format!("cannot {doing} the state file: {error}")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;

    fn rule(max_source_bytes: u64) -> StateDecl {
        StateDecl {
            file: "state.json".to_owned(),
            keep: vec!["account".to_owned(), "theme".to_owned()],
            set: BTreeMap::from([("onboarded".to_owned(), json!(true))]),
            max_source_bytes,
        }
    }

    const CAP: u64 = 16 * 1024 * 1024;

    fn object_of(bytes: &[u8]) -> Value {
        serde_json::from_slice(bytes).expect("the built file is JSON")
    }

    fn built(imported: Imported) -> (Value, bool, Option<String>) {
        match imported {
            Imported::Built { bytes, found, note } => (object_of(&bytes), found, note),
            Imported::Refused(reason) => panic!("refused: {reason}"),
        }
    }

    #[test]
    fn a_source_with_extra_keys_keeps_only_the_kept_and_set_keys() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("state.json");
        let text = json!({
            "account": { "email": "fake@example.com" },
            "theme": "dark",
            "projects": { "/fake/project": { "history": ["fake"] } },
            "onboarded": false,
            "userID": "fake-id"
        });
        fs::write(&source, text.to_string()).unwrap();

        let (state, found, note) = built(import_source(&rule(CAP), Some(&source), false));
        assert_eq!(
            state,
            json!({ "account": { "email": "fake@example.com" }, "theme": "dark", "onboarded": true })
        );
        assert!(found);
        assert_eq!(note, None);
    }

    #[test]
    fn a_source_over_one_mib_within_its_cap_is_used() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("state.json");
        let history = "x".repeat(2 * 1024 * 1024);
        fs::write(
            &source,
            json!({ "theme": "light", "projects": { "fake": history } }).to_string(),
        )
        .unwrap();

        let (state, found, _) = built(import_source(&rule(CAP), Some(&source), false));
        assert_eq!(state, json!({ "theme": "light", "onboarded": true }));
        assert!(found);
    }

    #[test]
    fn a_missing_source_gives_the_set_keys_alone_or_refuses_a_required_one() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("state.json");
        assert_eq!(
            built(import_source(&rule(CAP), Some(&source), false)),
            (json!({ "onboarded": true }), false, None)
        );
        assert_eq!(
            built(import_source(&rule(CAP), None, false)),
            (json!({ "onboarded": true }), false, None)
        );
        assert_eq!(
            import_source(&rule(CAP), Some(&source), true),
            Imported::Refused(MISSING.to_owned())
        );
    }

    #[test]
    fn an_unusable_optional_source_gives_the_set_keys_and_says_why_and_a_required_one_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("state.json");
        for (content, reason) in [
            (b"[1, 2]".to_vec(), NOT_AN_OBJECT.to_owned()),
            (b"not json".to_vec(), NOT_AN_OBJECT.to_owned()),
            (vec![b' '; 2048], "larger than 1024 bytes".to_owned()),
        ] {
            fs::write(&source, &content).unwrap();
            let (state, found, note) = built(import_source(&rule(1024), Some(&source), false));
            assert_eq!(state, json!({ "onboarded": true }));
            assert!(!found);
            let note = note.expect("the import says why the source was not used");
            assert!(
                note.contains(&reason) && note.contains("onboarded"),
                "{note}"
            );
            assert_eq!(
                import_source(&rule(1024), Some(&source), true),
                Imported::Refused(reason)
            );
        }
    }

    #[test]
    fn ensure_creates_a_missing_file_merges_a_differing_one_and_keeps_other_keys() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("state.json");
        assert_eq!(ensure(home.path(), &rule(CAP)), Ok(Repair::Created));
        assert_eq!(
            object_of(&fs::read(&path).unwrap()),
            json!({ "onboarded": true })
        );

        fs::write(
            &path,
            json!({ "onboarded": false, "projects": { "fake": 1 }, "theme": "dark" }).to_string(),
        )
        .unwrap();
        assert_eq!(ensure(home.path(), &rule(CAP)), Ok(Repair::Merged));
        assert_eq!(
            object_of(&fs::read(&path).unwrap()),
            json!({ "onboarded": true, "projects": { "fake": 1 }, "theme": "dark" })
        );

        let before = fs::read(&path).unwrap();
        assert_eq!(ensure(home.path(), &rule(CAP)), Ok(Repair::Unchanged));
        assert_eq!(
            fs::read(&path).unwrap(),
            before,
            "a file that holds is not rewritten"
        );
        let names: Vec<_> = fs::read_dir(home.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["state.json"], "no temporary file is left behind");
    }

    #[test]
    fn ensure_leaves_a_file_that_is_not_an_object_as_it_is() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("state.json");
        fs::write(&path, b"[true]").unwrap();
        assert_eq!(
            ensure(home.path(), &rule(CAP)),
            Err(NOT_AN_OBJECT.to_owned())
        );
        assert_eq!(fs::read(&path).unwrap(), b"[true]");
    }

    #[test]
    #[cfg_attr(
        windows,
        expect(
            clippy::print_stderr,
            reason = "a skipped test must name why it skipped"
        )
    )]
    fn ensure_refuses_a_linked_state_file_and_never_writes_through_it() {
        let home = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let target = elsewhere.path().join("real.json");
        fs::write(&target, b"{}").unwrap();
        let link = home.path().join("state.json");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        #[cfg(windows)]
        if let Err(error) = std::os::windows::fs::symlink_file(&target, &link) {
            eprintln!("skipped: symbolic links cannot be made here ({error})");
            return;
        }
        assert_eq!(ensure(home.path(), &rule(CAP)), Err("a link".to_owned()));
        assert_eq!(fs::read(&target).unwrap(), b"{}");
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the link is left as it is"
        );
    }

    /// A junction needs no privilege on Windows, so this always runs there: the state file path is a reparse point.
    #[cfg(windows)]
    #[test]
    fn ensure_refuses_a_junction_at_the_state_file() {
        let home = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let link = home.path().join("state.json");
        let made = massorca_process::command("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&link)
            .arg(elsewhere.path())
            .output()
            .unwrap();
        assert!(made.status.success(), "mklink /J failed");
        assert_eq!(ensure(home.path(), &rule(CAP)), Err("a link".to_owned()));
        assert_eq!(fs::read_dir(elsewhere.path()).unwrap().count(), 0);
        assert!(
            fs::symlink_metadata(&link).is_ok(),
            "the junction is left as it is"
        );
    }
}
