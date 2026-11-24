//! The `git.diff_file` result: a submodule pointer is an optional field, so older clients still parse it.

use massorca_protocol::methods::git::diff::{GitDiffFile, SubmodulePointer};
use serde_json::json;

fn plain(path: &str) -> GitDiffFile {
    GitDiffFile {
        path: path.to_owned(),
        patch: None,
        binary: false,
        truncated: false,
        old_size: None,
        new_size: None,
        submodule: None,
    }
}

#[test]
fn a_result_without_the_submodule_field_still_parses() {
    let file: GitDiffFile =
        serde_json::from_value(json!({"path": "README.md", "patch": "@@\n"})).expect("parses");
    assert_eq!(file.submodule, None);
    assert_eq!(file.patch.as_deref(), Some("@@\n"));
}

#[test]
fn a_submodule_pointer_is_omitted_when_absent_and_named_when_present() {
    assert_eq!(
        serde_json::to_value(plain("README.md")).expect("serializes"),
        json!({"path": "README.md"})
    );
    let pointer = GitDiffFile {
        submodule: Some(SubmodulePointer {
            old_commit: Some("a1".to_owned()),
            new_commit: None,
            dirty: Some(true),
            summary: "Subproject commit a1".to_owned(),
        }),
        ..plain("vendor/lib")
    };
    let value = serde_json::to_value(&pointer).expect("serializes");
    assert_eq!(
        value,
        json!({
            "path": "vendor/lib",
            "submodule": {
                "old_commit": "a1",
                "dirty": true,
                "summary": "Subproject commit a1"
            }
        })
    );
    let decoded: GitDiffFile = serde_json::from_value(value).expect("deserializes");
    assert_eq!(decoded, pointer);
}
