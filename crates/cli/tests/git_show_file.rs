//! `massorca git show-file` against a real service and real repositories.

mod harness;

use std::path::PathBuf;

use harness::{Sandbox, git, git_repo, stdout_text};
use predicates::str::contains;

/// A git project at `<home>/repo` with a workspace `feature`; returns the workspace directory.
fn workspace(sandbox: &Sandbox) -> PathBuf {
    let repo = sandbox.home().join("repo");
    git_repo(&repo);
    let repo_text = repo.to_string_lossy().into_owned();
    let root_text = sandbox.home().join("ws").to_string_lossy().into_owned();
    sandbox.json_ok(
        "default",
        &[
            "project",
            "add",
            &repo_text,
            "--workspaces-root",
            &root_text,
        ],
    );
    let created = sandbox.json_ok("default", &["workspace", "create", "repo", "feature"]);
    PathBuf::from(created["path"].as_str().expect("workspace path"))
}

#[test]
fn the_summary_names_the_size_and_the_image_type_and_json_carries_the_bytes() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    // `PNG` then a NUL and a SOH: five bytes.
    std::fs::write(dir.join("logo.png"), b"PNG\x00\x01").expect("write");

    let text = sandbox
        .run(
            "default",
            &["git", "show-file", "logo.png", "-w", "feature"],
        )
        .success();
    let text = stdout_text(&text);
    assert!(text.contains("logo.png: 5 bytes, image/png"), "{text}");

    let json = sandbox.json_ok(
        "default",
        &["git", "show-file", "logo.png", "-w", "feature"],
    );
    assert_eq!(json["size"], 5);
    assert_eq!(json["mime"], "image/png");
    assert_eq!(json["content_base64"], "UE5HAAE=");
}

#[test]
fn the_index_and_a_revision_are_chosen_by_their_flags() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    std::fs::write(dir.join("README.md"), "staged\n").expect("write");
    git(&dir, &["add", "README.md"]);
    std::fs::write(dir.join("README.md"), "unstaged edit\n").expect("write");
    std::fs::write(dir.join("untracked.txt"), "new\n").expect("write");

    let index = sandbox
        .run(
            "default",
            &["git", "show-file", "README.md", "-w", "feature", "--index"],
        )
        .success();
    assert!(
        stdout_text(&index).contains("README.md: 7 bytes"),
        "the staged content is seven bytes"
    );

    let working = sandbox
        .run(
            "default",
            &["git", "show-file", "README.md", "-w", "feature"],
        )
        .success();
    assert!(
        stdout_text(&working).contains("README.md: 14 bytes"),
        "the working file is the unstaged edit"
    );

    let head = sandbox
        .run(
            "default",
            &[
                "git",
                "show-file",
                "README.md",
                "-w",
                "feature",
                "--rev",
                "HEAD",
            ],
        )
        .success();
    assert!(
        stdout_text(&head).contains("README.md: 6 bytes"),
        "the committed hello line is six bytes"
    );

    let absent = sandbox
        .run(
            "default",
            &[
                "git",
                "show-file",
                "untracked.txt",
                "-w",
                "feature",
                "--index",
            ],
        )
        .success();
    assert!(
        stdout_text(&absent).contains("untracked.txt: no file in the index"),
        "an untracked file is not in the index"
    );
}

#[test]
fn out_writes_the_exact_bytes_and_refuses_a_side_with_no_file() {
    let sandbox = Sandbox::new();
    let dir = workspace(&sandbox);
    let original: Vec<u8> = vec![0x00, 0xff, b'\n', 0x89, b'P', 0x0d, 0x0a];
    std::fs::write(dir.join("blob.bin"), &original).expect("write");
    let out = sandbox.home().join("exported.bin");
    let out_text = out.to_string_lossy().into_owned();

    let written = sandbox
        .run(
            "default",
            &[
                "git",
                "show-file",
                "blob.bin",
                "-w",
                "feature",
                "--out",
                &out_text,
            ],
        )
        .success();
    assert!(
        stdout_text(&written).contains("written to"),
        "a confirmation line"
    );
    assert_eq!(std::fs::read(&out).expect("exported file"), original);

    // Untracked, so the index holds no bytes to write.
    let refused_out = sandbox.home().join("never.bin");
    let refused_text = refused_out.to_string_lossy().into_owned();
    sandbox
        .run(
            "default",
            &[
                "git",
                "show-file",
                "blob.bin",
                "-w",
                "feature",
                "--index",
                "--out",
                &refused_text,
            ],
        )
        .failure()
        .stderr(contains("git.no_bytes"));
    assert!(
        !refused_out.exists(),
        "nothing is written when there are no bytes"
    );
}

#[test]
fn a_path_that_leaves_the_workspace_is_refused() {
    let sandbox = Sandbox::new();
    let _dir = workspace(&sandbox);

    sandbox
        .run(
            "default",
            &["git", "show-file", "../repo/README.md", "-w", "feature"],
        )
        .failure()
        .stderr(contains("git.path_outside_worktree"));
}
