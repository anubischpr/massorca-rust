//! Project icons (decision 0091): their wire shape, their checks and their stored form.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use massorca_protocol::capabilities;
use massorca_protocol::methods::projects::{
    self, Project, ProjectAddParams, ProjectIcon, ProjectIconSource, ProjectKind,
    ProjectUpdateParams,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::fmt::Debug;

/// Checks that `value` serializes to `expected` and that `expected` decodes back to `value`.
fn assert_wire<T>(value: &T, expected: &Value)
where
    T: Serialize + DeserializeOwned + Debug + PartialEq,
{
    assert_eq!(&serde_json::to_value(value).expect("serializes"), expected);
    let decoded: T = serde_json::from_value(expected.clone()).expect("deserializes");
    assert_eq!(&decoded, value);
}

fn png_bytes() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend_from_slice(b"filler");
    bytes
}

fn webp_bytes() -> Vec<u8> {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&[0, 0, 0, 0]);
    bytes.extend_from_slice(b"WEBPVP8 ");
    bytes
}

fn png_data_url(bytes: &[u8]) -> String {
    format!("data:image/png;base64,{}", STANDARD.encode(bytes))
}

fn webp_data_url() -> String {
    format!("data:image/webp;base64,{}", STANDARD.encode(webp_bytes()))
}

fn upload(src: String) -> ProjectIcon {
    ProjectIcon::Image {
        src,
        source: ProjectIconSource::Upload,
        label: None,
    }
}

#[test]
fn each_icon_kind_has_the_documented_wire_shape() {
    assert_wire(
        &ProjectIcon::Emoji {
            emoji: "🚀".to_owned(),
        },
        &json!({ "type": "emoji", "emoji": "🚀" }),
    );
    assert_wire(
        &ProjectIcon::Lucide {
            name: "Rocket".to_owned(),
        },
        &json!({ "type": "lucide", "name": "Rocket" }),
    );
    assert_wire(
        &ProjectIcon::Image {
            src: png_data_url(&png_bytes()),
            source: ProjectIconSource::Upload,
            label: Some("logo.png".to_owned()),
        },
        &json!({
            "type": "image",
            "src": png_data_url(&png_bytes()),
            "source": "upload",
            "label": "logo.png"
        }),
    );
}

#[test]
fn project_add_params_and_projects_carry_an_icon_only_when_it_is_set() {
    let bare = ProjectAddParams {
        path: "/repos/app".to_owned(),
        name: None,
        kind: None,
        workspaces_root: None,
        icon: None,
    };
    assert_wire(&bare, &json!({ "path": "/repos/app" }));
    assert_wire(
        &ProjectAddParams {
            icon: Some(ProjectIcon::Emoji {
                emoji: "🚀".to_owned(),
            }),
            ..bare
        },
        &json!({
            "path": "/repos/app",
            "icon": { "type": "emoji", "emoji": "🚀" }
        }),
    );

    let without = json!({
        "id": "p-1",
        "kind": "git",
        "path": "/repos/app",
        "name": "app",
        "workspaces_root": "/repos/app-workspaces",
        "added_at_ms": 1
    });
    let project: Project = serde_json::from_value(without.clone()).expect("a project without icon");
    assert_eq!(project.icon, None);
    assert_eq!(serde_json::to_value(&project).expect("serializes"), without);
    assert_eq!(project.kind, ProjectKind::Git);
}

#[test]
fn an_icon_is_normalized_to_its_trimmed_text() {
    assert_eq!(
        ProjectIcon::Emoji {
            emoji: "  🚀 ".to_owned()
        }
        .normalized(),
        Ok(ProjectIcon::Emoji {
            emoji: "🚀".to_owned()
        })
    );
    assert!(
        ProjectIcon::Emoji {
            emoji: "   ".to_owned()
        }
        .normalized()
        .is_err()
    );
    // An emoji is at most 16 UTF-16 units.
    assert!(
        ProjectIcon::Emoji {
            emoji: "a".repeat(16)
        }
        .normalized()
        .is_ok()
    );
    assert!(
        ProjectIcon::Emoji {
            emoji: "a".repeat(17)
        }
        .normalized()
        .is_err()
    );
    assert_eq!(
        ProjectIcon::Lucide {
            name: " Rocket ".to_owned()
        }
        .normalized(),
        Ok(ProjectIcon::Lucide {
            name: "Rocket".to_owned()
        })
    );
    let long = "a".repeat(41);
    for name in ["", "1Rocket", "rocket-ship", long.as_str()] {
        let icon = ProjectIcon::Lucide {
            name: name.to_owned(),
        };
        assert!(icon.normalized().is_err(), "{name:?} is not a Lucide name");
    }
}

#[test]
fn an_image_icon_must_be_an_image_of_its_source_and_size() {
    assert!(upload(png_data_url(&png_bytes())).normalized().is_ok());
    // An upload is a PNG; a file icon may also be a WebP.
    assert!(upload(webp_data_url()).normalized().is_err());
    let file = ProjectIcon::Image {
        src: webp_data_url(),
        source: ProjectIconSource::File,
        label: None,
    };
    assert!(file.normalized().is_ok());
    // The data must be the image format its prefix names.
    assert!(
        upload(format!(
            "data:image/png;base64,{}",
            STANDARD.encode(b"not a png")
        ))
        .normalized()
        .is_err()
    );
    assert!(
        upload("https://example.com/icon.png".to_owned())
            .normalized()
            .is_err()
    );
    // 256 KiB of image is the limit, before base64.
    let mut at_limit = png_bytes();
    at_limit.resize(256 * 1024, 0);
    assert!(upload(png_data_url(&at_limit)).normalized().is_ok());
    let mut over_limit = png_bytes();
    over_limit.resize(256 * 1024 + 1, 0);
    assert!(upload(png_data_url(&over_limit)).normalized().is_err());
}

#[test]
fn an_image_is_built_from_the_bytes_of_a_png_or_webp_file() {
    assert_eq!(
        ProjectIcon::image_from_bytes(
            ProjectIconSource::File,
            &webp_bytes(),
            Some("public/favicon.webp".to_owned())
        ),
        Ok(ProjectIcon::Image {
            src: webp_data_url(),
            source: ProjectIconSource::File,
            label: Some("public/favicon.webp".to_owned()),
        })
    );
    assert!(ProjectIcon::image_from_bytes(ProjectIconSource::Upload, &webp_bytes(), None).is_err());
    assert!(ProjectIcon::image_from_bytes(ProjectIconSource::File, b"plain text", None).is_err());
}

#[test]
fn a_stored_icon_round_trips_through_its_json() {
    let icon = ProjectIcon::Lucide {
        name: "Rocket".to_owned(),
    };
    let stored = icon.to_stored().expect("stores");
    assert_eq!(ProjectIcon::from_stored(&stored), Some(icon));
    assert_eq!(ProjectIcon::from_stored("not json"), None);
}

#[test]
fn the_label_is_trimmed_and_capped_at_eighty_characters() {
    let icon = ProjectIcon::Image {
        src: png_data_url(&png_bytes()),
        source: ProjectIconSource::Upload,
        label: Some(format!("  {}  ", "x".repeat(100))),
    };
    match icon.normalized() {
        Ok(ProjectIcon::Image { label, .. }) => assert_eq!(label, Some("x".repeat(80))),
        other => panic!("the label is kept: {other:?}"),
    }
}

#[test]
fn project_update_params_tell_an_absent_icon_from_a_null_one() {
    assert_wire(
        &ProjectUpdateParams {
            project: "app".to_owned(),
            icon: None,
        },
        &json!({ "project": "app" }),
    );
    assert_wire(
        &ProjectUpdateParams {
            project: "app".to_owned(),
            icon: Some(None),
        },
        &json!({ "project": "app", "icon": null }),
    );
    assert_wire(
        &ProjectUpdateParams {
            project: "app".to_owned(),
            icon: Some(Some(ProjectIcon::Emoji {
                emoji: "🚀".to_owned(),
            })),
        },
        &json!({ "project": "app", "icon": { "type": "emoji", "emoji": "🚀" } }),
    );
}

#[test]
fn the_update_method_and_its_capability_are_named_as_documented() {
    assert_eq!(projects::UPDATE, "project.update");
    assert_eq!(capabilities::PROJECT_UPDATE, "project-update/1");
}
