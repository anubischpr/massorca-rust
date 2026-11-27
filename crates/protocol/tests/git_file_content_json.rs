//! The `git.file_content` wire shape: absent fields are omitted, the side is flattened into the params, and
//! previewable images are named by their extension.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use massorca_protocol::methods::git::diff::{
    FileSide, GitFileContent, GitFileContentParams, preview_mime,
};
use serde_json::json;

fn absent(path: &str) -> GitFileContent {
    GitFileContent {
        path: path.to_owned(),
        size: None,
        too_large: false,
        content_base64: None,
        mime: None,
    }
}

#[test]
fn a_side_with_no_file_omits_its_size_and_bytes() {
    assert_eq!(
        serde_json::to_value(absent("new.png")).expect("serializes"),
        json!({"path": "new.png"})
    );
}

#[test]
fn a_file_over_the_cap_names_its_size_and_sends_no_bytes() {
    let big = GitFileContent {
        size: Some(5_000_000),
        too_large: true,
        mime: Some("image/png".to_owned()),
        ..absent("big.png")
    };
    assert_eq!(
        serde_json::to_value(&big).expect("serializes"),
        json!({"path": "big.png", "size": 5_000_000, "too_large": true, "mime": "image/png"})
    );
    assert_eq!(big.decoded(), None);
}

#[test]
fn the_bytes_are_base64_and_decode_back_exactly() {
    let bytes = [0_u8, 1, 2, 255, b'\n'];
    let file = GitFileContent {
        size: Some(5),
        content_base64: Some(STANDARD.encode(bytes)),
        ..absent("blob.bin")
    };
    let value = serde_json::to_value(&file).expect("serializes");
    assert_eq!(value["content_base64"], "AAEC/wo=");
    let decoded: GitFileContent = serde_json::from_value(value).expect("deserializes");
    assert_eq!(decoded.decoded().as_deref(), Some(bytes.as_slice()));
}

#[test]
fn the_side_is_flattened_into_the_params_beside_the_path() {
    let params = |side| GitFileContentParams {
        workspace: "feature".to_owned(),
        side,
        path: "logo.png".to_owned(),
        max_bytes: None,
    };
    assert_eq!(
        serde_json::to_value(params(FileSide::Worktree)).expect("serializes"),
        json!({"workspace": "feature", "side": "worktree", "path": "logo.png"})
    );
    assert_eq!(
        serde_json::to_value(params(FileSide::Index)).expect("serializes"),
        json!({"workspace": "feature", "side": "index", "path": "logo.png"})
    );
    let revision = params(FileSide::Revision {
        rev: "HEAD~1".to_owned(),
    });
    let value = serde_json::to_value(&revision).expect("serializes");
    assert_eq!(
        value,
        json!({"workspace": "feature", "side": "revision", "rev": "HEAD~1", "path": "logo.png"})
    );
    let decoded: GitFileContentParams = serde_json::from_value(value).expect("deserializes");
    assert_eq!(decoded, revision);
}

#[test]
fn only_previewable_images_get_a_mime_type_by_extension() {
    assert_eq!(preview_mime("logo.PNG"), Some("image/png"));
    assert_eq!(preview_mime("photos/cat.jpeg"), Some("image/jpeg"));
    assert_eq!(preview_mime("a.jpg"), Some("image/jpeg"));
    assert_eq!(preview_mime("anim.gif"), Some("image/gif"));
    assert_eq!(preview_mime("hero.webp"), Some("image/webp"));
    assert_eq!(preview_mime("scan.bmp"), Some("image/bmp"));
    assert_eq!(preview_mime("favicon.ico"), Some("image/x-icon"));
    assert_eq!(preview_mime("icon.svg"), Some("image/svg+xml"));
    assert_eq!(preview_mime("manual.pdf"), None);
    assert_eq!(preview_mime("Makefile"), None);
}
