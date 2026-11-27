//! Project icons through the CLI (decision 0091): `--icon` on `project add`, `project update` with `--icon` or `--clear-icon`,
//! and the icon in list, show and JSON output.

mod harness;

use harness::{Sandbox, git_repo, stdout_text};
use predicates::str::contains;
use serde_json::json;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfiller";

fn repo_in(sandbox: &Sandbox) -> String {
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    repo.to_string_lossy().into_owned()
}

#[test]
fn an_emoji_icon_is_stored_and_shown_by_add_list_and_show() {
    let sandbox = Sandbox::new();
    let repo = repo_in(&sandbox);
    let ws = sandbox.home().join("ws").to_string_lossy().into_owned();

    let added = sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            repo.as_str(),
            "--workspaces-root",
            ws.as_str(),
            "--icon",
            "emoji:🚀",
        ],
    );
    assert_eq!(added["icon"], json!({ "type": "emoji", "emoji": "🚀" }));

    let listed = sandbox.run("default", &["project", "list"]).success();
    let text = stdout_text(&listed);
    assert!(
        text.lines()
            .next()
            .is_some_and(|header| header.contains("ICON")),
        "the table has an ICON column: {text}"
    );
    assert!(text.contains("emoji:🚀"), "the row shows the icon: {text}");

    sandbox
        .run("default", &["project", "show", "repo"])
        .success()
        .stdout(contains("icon: emoji:🚀"));
}

#[test]
fn project_update_sets_a_lucide_icon_and_clear_icon_removes_it() {
    let sandbox = Sandbox::new();
    let repo = repo_in(&sandbox);
    sandbox
        .run("default", &["project", "add", repo.as_str()])
        .success();

    sandbox
        .run(
            "default",
            &["project", "update", "repo", "--icon", "lucide:Rocket"],
        )
        .success()
        .stdout(contains("project repo icon is lucide:Rocket"));
    let shown = sandbox.json_ok("default", &["project", "show", "repo"]);
    assert_eq!(shown["icon"], json!({ "type": "lucide", "name": "Rocket" }));

    sandbox
        .run("default", &["project", "update", "repo", "--clear-icon"])
        .success()
        .stdout(contains("project repo has no icon"));
    let cleared = sandbox.json_ok("default", &["project", "show", "repo"]);
    assert!(
        cleared.get("icon").is_none(),
        "no icon after clear: {cleared}"
    );
}

#[test]
fn project_update_sets_a_coloured_lucide_icon_and_show_prints_it_back() {
    let sandbox = Sandbox::new();
    let repo = repo_in(&sandbox);
    sandbox
        .run("default", &["project", "add", repo.as_str()])
        .success();

    sandbox
        .run(
            "default",
            &[
                "project",
                "update",
                "repo",
                "--icon",
                "lucide:folder:usage-3",
            ],
        )
        .success()
        .stdout(contains("project repo icon is lucide:folder:usage-3"));
    let shown = sandbox.json_ok("default", &["project", "show", "repo"]);
    assert_eq!(
        shown["icon"],
        json!({ "type": "lucide", "name": "folder", "color": "usage-3" })
    );
    sandbox
        .run("default", &["project", "show", "repo"])
        .success()
        .stdout(contains("icon: lucide:folder:usage-3"));
}

#[test]
fn a_lucide_colour_that_is_not_a_token_is_refused_before_anything_is_sent() {
    let sandbox = Sandbox::new();
    let repo = repo_in(&sandbox);
    sandbox
        .run("default", &["project", "add", repo.as_str()])
        .success();

    sandbox
        .run(
            "default",
            &["project", "update", "repo", "--icon", "lucide:folder:red"],
        )
        .failure()
        .stderr(contains("usage-1 to usage-6 or neutral"));
    let shown = sandbox.json_ok("default", &["project", "show", "repo"]);
    assert!(shown.get("icon").is_none(), "no icon was set: {shown}");
}

#[test]
fn an_image_icon_is_read_from_a_png_file_and_stored_as_an_upload() {
    let sandbox = Sandbox::new();
    let repo = repo_in(&sandbox);
    let image = sandbox.home().join("logo.png");
    std::fs::write(&image, PNG).expect("write the image");
    let spec = format!("image:{}", image.to_string_lossy());

    let added = sandbox.json_ok(
        "default",
        &["project", "add", repo.as_str(), "--icon", spec.as_str()],
    );

    assert_eq!(added["icon"]["type"], "image");
    assert_eq!(added["icon"]["source"], "upload");
    assert_eq!(added["icon"]["label"], "logo.png");
    assert!(
        added["icon"]["src"]
            .as_str()
            .is_some_and(|src| src.starts_with("data:image/png;base64,")),
        "a PNG data URI: {}",
        added["icon"]
    );
}

#[test]
fn an_image_that_is_not_a_png_is_refused_and_nothing_is_registered() {
    let sandbox = Sandbox::new();
    let repo = repo_in(&sandbox);
    let notes = sandbox.home().join("notes.txt");
    std::fs::write(&notes, b"not an image").expect("write the file");
    let spec = format!("image:{}", notes.to_string_lossy());

    sandbox
        .run(
            "default",
            &["project", "add", repo.as_str(), "--icon", spec.as_str()],
        )
        .failure()
        .stderr(contains("PNG"));
    sandbox
        .run("default", &["project", "list"])
        .success()
        .stdout(contains("no projects"));
}

#[test]
fn an_icon_names_its_kind_emoji_lucide_or_image() {
    let sandbox = Sandbox::new();
    let repo = repo_in(&sandbox);

    sandbox
        .run(
            "default",
            &["project", "add", repo.as_str(), "--icon", "rocket"],
        )
        .failure()
        .stderr(contains("emoji:<emoji>, lucide:<name> or image:<PNG file>"));
}

#[test]
fn an_empty_emoji_is_refused_by_the_service() {
    let sandbox = Sandbox::new();
    let repo = repo_in(&sandbox);

    sandbox
        .run(
            "default",
            &["project", "add", repo.as_str(), "--icon", "emoji:   "],
        )
        .failure()
        .stderr(contains("invalid icon"));
    sandbox
        .run("default", &["project", "list"])
        .success()
        .stdout(contains("no projects"));
}

#[test]
fn a_project_added_without_an_icon_flag_takes_the_icon_its_files_hold() {
    let sandbox = Sandbox::new();
    let repo = repo_in(&sandbox);
    std::fs::create_dir_all(sandbox.home().join("repo").join("public")).expect("create public");
    std::fs::write(
        sandbox
            .home()
            .join("repo")
            .join("public")
            .join("favicon.png"),
        PNG,
    )
    .expect("write the favicon");

    let added = sandbox.json_ok("default", &["project", "add", repo.as_str()]);

    assert_eq!(added["icon"]["type"], "image");
    assert_eq!(added["icon"]["source"], "file");
    assert_eq!(added["icon"]["label"], "public/favicon.png");
}
