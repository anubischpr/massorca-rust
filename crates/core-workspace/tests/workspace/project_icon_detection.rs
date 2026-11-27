//! A project added without an icon takes one from its files (decision 0091); an explicit icon always wins.

use std::fs;
use std::path::Path;

use massorca_protocol::methods::projects::{ProjectIcon, ProjectIconSource};

use crate::fixture::{Fixture, project_add};

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

fn svg_bytes() -> Vec<u8> {
    br#"<svg xmlns="http://www.w3.org/2000/svg"><circle r="4"/></svg>"#.to_vec()
}

fn write(path: &Path, contents: &[u8]) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("create dirs");
    fs::write(path, contents).expect("write file");
}

/// The source and label of a detected image icon; `None` when there is no icon.
fn detected(icon: Option<ProjectIcon>) -> Option<(ProjectIconSource, Option<String>)> {
    match icon? {
        ProjectIcon::Image { src, source, label } => {
            assert!(src.starts_with("data:image/"), "{src}");
            Some((source, label))
        }
        other => panic!("an image icon was expected: {other:?}"),
    }
}

fn detected_from(label: &str) -> Option<(ProjectIconSource, Option<String>)> {
    Some((ProjectIconSource::File, Some(label.to_owned())))
}

#[test]
fn a_conventional_image_file_becomes_the_icon() {
    let fx = Fixture::new();
    write(&fx.repo().join("public/favicon.png"), &png_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(detected(project.icon), detected_from("public/favicon.png"));
}

#[test]
fn the_earlier_conventional_location_wins() {
    let fx = Fixture::new();
    write(&fx.repo().join("logo.png"), &png_bytes());
    write(&fx.repo().join("favicon.webp"), &webp_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(detected(project.icon), detected_from("favicon.webp"));
}

#[test]
fn a_file_that_is_not_an_image_or_is_too_large_is_skipped() {
    let fx = Fixture::new();
    write(&fx.repo().join("favicon.png"), b"plain text, not an image");
    let mut oversized = webp_bytes();
    oversized.resize(256 * 1024 + 1, 0);
    write(&fx.repo().join("favicon.webp"), &oversized);
    write(&fx.repo().join("logo.png"), &png_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(detected(project.icon), detected_from("logo.png"));
}

#[test]
fn an_explicit_icon_is_kept_over_a_detected_one() {
    let fx = Fixture::new();
    write(&fx.repo().join("favicon.png"), &png_bytes());
    let mut params = project_add(&fx.repo());
    params.icon = Some(ProjectIcon::Emoji {
        emoji: "🚀".to_owned(),
    });

    let project = fx.workspaces.add_project(params).expect("add project");

    assert_eq!(
        project.icon,
        Some(ProjectIcon::Emoji {
            emoji: "🚀".to_owned()
        })
    );
}

#[test]
fn a_declared_icon_in_the_entry_page_is_used() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("index.html"),
        br#"<html><head><link rel="icon" href="/brand.png"></head></html>"#,
    );
    write(&fx.repo().join("public/brand.png"), &png_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(detected(project.icon), detected_from("public/brand.png"));
}

#[test]
fn a_declared_icon_resolves_against_its_own_page() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("public/index.html"),
        br#"<link href="icons/app.png" rel="shortcut icon">"#,
    );
    write(&fx.repo().join("public/icons/app.png"), &png_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(
        detected(project.icon),
        detected_from("public/icons/app.png")
    );
}

#[test]
fn a_declared_icon_cannot_leave_the_repository() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("index.html"),
        br#"<link rel="icon" href="../outside.png">"#,
    );
    write(&fx.path("outside.png"), &png_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(project.icon, None);
}

#[test]
fn a_declared_icon_with_a_query_is_not_followed() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("index.html"),
        br#"<link rel="icon" href="/brand.png?v=2">"#,
    );
    write(&fx.repo().join("public/brand.png"), &png_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(project.icon, None);
}

#[test]
fn a_folder_takes_its_icon_from_its_files_too() {
    let fx = Fixture::new();
    let notes = fx.path("notes");
    write(&notes.join("favicon.png"), &png_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&notes))
        .expect("add project");

    assert_eq!(detected(project.icon), detected_from("favicon.png"));
}

#[test]
fn a_package_json_icon_inside_the_repository_becomes_the_icon() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("package.json"),
        br#"{"name":"app","icon":"assets/app-mark.png"}"#,
    );
    write(&fx.repo().join("assets/app-mark.png"), &png_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(detected(project.icon), detected_from("assets/app-mark.png"));
}

#[test]
fn a_package_json_icon_may_be_a_png_data_uri() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("package.json"),
        br#"{"icon":"data:image/png;base64,iVBORw0KGgpmaWxsZXI="}"#,
    );

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(
        project.icon,
        Some(ProjectIcon::Image {
            src: "data:image/png;base64,iVBORw0KGgpmaWxsZXI=".to_owned(),
            source: ProjectIconSource::File,
            label: Some("package.json".to_owned()),
        })
    );
}

#[test]
fn a_package_json_icon_outside_the_repository_is_refused() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("package.json"),
        br#"{"icon":"../outside.png"}"#,
    );
    write(&fx.path("outside.png"), &png_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(project.icon, None);
}

#[test]
fn an_absolute_package_json_icon_path_is_refused() {
    let fx = Fixture::new();
    let outside = fx.path("outside.png");
    write(&outside, &png_bytes());
    let manifest = serde_json::json!({ "icon": outside.display().to_string() });
    write(
        &fx.repo().join("package.json"),
        manifest.to_string().as_bytes(),
    );

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(project.icon, None);
}

#[test]
fn a_package_json_icon_image_over_the_size_limit_is_refused() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("package.json"),
        br#"{"icon":"assets/big.png"}"#,
    );
    let mut oversized = png_bytes();
    oversized.resize(256 * 1024 + 1, 0);
    write(&fx.repo().join("assets/big.png"), &oversized);

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(project.icon, None);
}

#[test]
fn a_package_json_over_the_size_limit_is_not_read() {
    let fx = Fixture::new();
    let padding = "x".repeat(128 * 1024);
    write(
        &fx.repo().join("package.json"),
        format!(r#"{{"icon":"assets/app-mark.png","padding":"{padding}"}}"#).as_bytes(),
    );
    write(&fx.repo().join("assets/app-mark.png"), &png_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(project.icon, None);
}

#[test]
fn a_conventional_file_wins_over_the_package_json_icon() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("package.json"),
        br#"{"icon":"assets/app-mark.png"}"#,
    );
    write(&fx.repo().join("assets/app-mark.png"), &png_bytes());
    write(&fx.repo().join("favicon.webp"), &webp_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(detected(project.icon), detected_from("favicon.webp"));
}

#[test]
fn a_declared_icon_in_a_jsx_route_root_is_used() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("app/root.tsx"),
        br#"export function Head() { return <link rel="icon" href="/brand/mark.png" />; }"#,
    );
    write(&fx.repo().join("public/brand/mark.png"), &png_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(
        detected(project.icon),
        detected_from("public/brand/mark.png")
    );
}

#[test]
fn a_links_entry_in_a_jsx_route_root_declares_the_icon() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("app/root.tsx"),
        br#"export const links = () => [{ rel: 'shortcut icon', href: '/brand/mark.png', type: 'image/png' }];"#,
    );
    write(&fx.repo().join("public/brand/mark.png"), &png_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(
        detected(project.icon),
        detected_from("public/brand/mark.png")
    );
}

#[test]
fn a_jsx_route_icon_with_a_computed_href_is_not_followed() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("app/root.tsx"),
        br#"<link rel="icon" href={markUrl} />"#,
    );
    write(
        &fx.repo().join("src/root.tsx"),
        br#"export const links = () => [{ rel: "icon", href: markUrl }];"#,
    );
    write(&fx.repo().join("public/brand/mark.png"), &png_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(project.icon, None);
}

#[test]
fn a_jsx_route_that_declares_only_an_svg_icon_gives_no_icon() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("app/root.tsx"),
        br#"<link rel="icon" href="/logo.svg" />"#,
    );
    write(&fx.repo().join("public/logo.svg"), &svg_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(project.icon, None);
}

#[test]
fn an_inline_svg_logo_in_a_jsx_route_is_not_rendered() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("src/root.tsx"),
        br#"export const Logo = () => <svg viewBox="0 0 8 8"><circle r="4" /></svg>;"#,
    );

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(project.icon, None);
}

#[test]
fn a_jsx_route_icon_cannot_leave_the_repository() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("app/root.tsx"),
        br#"<link rel="icon" href="../../outside.png" />"#,
    );
    write(&fx.path("outside.png"), &png_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(project.icon, None);
}

#[test]
fn an_entry_page_icon_wins_over_a_jsx_route_icon() {
    let fx = Fixture::new();
    write(
        &fx.repo().join("index.html"),
        br#"<link rel="icon" href="/brand.png">"#,
    );
    write(&fx.repo().join("public/brand.png"), &png_bytes());
    write(
        &fx.repo().join("app/root.tsx"),
        br#"<link rel="icon" href="/other.png" />"#,
    );
    write(&fx.repo().join("public/other.png"), &webp_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(detected(project.icon), detected_from("public/brand.png"));
}

#[test]
fn a_public_logo_in_webp_becomes_the_icon() {
    let fx = Fixture::new();
    write(&fx.repo().join("public/logo.webp"), &webp_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(detected(project.icon), detected_from("public/logo.webp"));
}

#[test]
fn an_svg_logo_is_not_an_icon() {
    let fx = Fixture::new();
    write(&fx.repo().join("public/logo.svg"), &svg_bytes());

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(project.icon, None);
}

#[cfg(unix)]
#[test]
fn a_symlink_to_an_icon_outside_the_repository_is_not_read() {
    let fx = Fixture::new();
    write(&fx.path("outside.png"), &png_bytes());
    std::os::unix::fs::symlink(fx.path("outside.png"), fx.repo().join("favicon.png"))
        .expect("create symlink");

    let project = fx
        .workspaces
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(project.icon, None);
}
