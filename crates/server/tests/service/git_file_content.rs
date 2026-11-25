//! `git.file_content` over the wire: each side's bytes, the caps, and the refusals.

use std::path::Path;

use massorca_protocol::codes;
use massorca_protocol::methods::git::diff::{self, FileSide, GitFileContent, GitFileContentParams};

use super::common::remote_code;
use super::git_fixture::{GitService, git};

/// Makes `link` a link to the directory `target`: a symlink on Unix, and a junction on Windows, which needs no
/// privilege.
fn link_directory(target: &Path, link: &Path) {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).expect("create symlink");
    }
    #[cfg(windows)]
    {
        let output = massorca_process::command("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .expect("run mklink");
        assert!(
            output.status.success(),
            "mklink /J failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn params(side: FileSide, path: &str) -> GitFileContentParams {
    GitFileContentParams {
        workspace: "feature".to_owned(),
        side,
        path: path.to_owned(),
        max_bytes: None,
    }
}

#[tokio::test]
async fn each_side_returns_its_bytes_and_an_image_names_its_type() {
    let service = GitService::start().await;
    let dir = service.dir();
    std::fs::write(dir.join("logo.png"), [0x89, b'P', b'N', b'G', 0, 1]).expect("write");
    git(&dir, &["add", "logo.png"]);
    std::fs::write(dir.join("logo.png"), [0x89, b'P', b'N', b'G', 0, 2, 3]).expect("write");

    let worktree: GitFileContent = service
        .client
        .call(diff::FILE_CONTENT, &params(FileSide::Worktree, "logo.png"))
        .await
        .expect("worktree");
    assert_eq!(worktree.size, Some(7));
    assert_eq!(worktree.mime.as_deref(), Some("image/png"));
    assert_eq!(
        worktree.decoded(),
        Some(vec![0x89, b'P', b'N', b'G', 0, 2, 3])
    );

    let index: GitFileContent = service
        .client
        .call(diff::FILE_CONTENT, &params(FileSide::Index, "logo.png"))
        .await
        .expect("index");
    assert_eq!(index.decoded(), Some(vec![0x89, b'P', b'N', b'G', 0, 1]));

    // Not committed yet: the revision side has no file, and no bytes either.
    let head: GitFileContent = service
        .client
        .call(
            diff::FILE_CONTENT,
            &params(
                FileSide::Revision {
                    rev: "HEAD".to_owned(),
                },
                "logo.png",
            ),
        )
        .await
        .expect("head");
    assert_eq!(head.size, None);
    assert_eq!(head.content_base64, None);
    assert_eq!(head.mime.as_deref(), Some("image/png"));
    service.stop().await;
}

#[tokio::test]
async fn a_file_over_the_cap_names_its_size_and_sends_no_bytes() {
    let service = GitService::start().await;
    let dir = service.dir();
    std::fs::write(dir.join("big.bin"), vec![b'x'; 5 << 20]).expect("write");

    let default: GitFileContent = service
        .client
        .call(diff::FILE_CONTENT, &params(FileSide::Worktree, "big.bin"))
        .await
        .expect("default cap");
    assert!(default.too_large);
    assert_eq!(default.size, Some(5 << 20));
    assert_eq!(default.content_base64, None);

    // The cap is clamped to 4 MiB, so asking for more does not read the file.
    let asked = GitFileContentParams {
        max_bytes: Some(u32::MAX),
        ..params(FileSide::Worktree, "big.bin")
    };
    let clamped: GitFileContent = service
        .client
        .call(diff::FILE_CONTENT, &asked)
        .await
        .expect("clamped cap");
    assert!(clamped.too_large);
    assert_eq!(clamped.content_base64, None);
    service.stop().await;
}

#[tokio::test]
async fn a_directory_is_refused_and_a_dash_led_revision_is_not_a_revision() {
    let service = GitService::start().await;
    std::fs::create_dir_all(service.dir().join("docs")).expect("create dir");

    let error = service
        .client
        .call::<_, GitFileContent>(diff::FILE_CONTENT, &params(FileSide::Worktree, "docs"))
        .await
        .expect_err("a directory is not a file");
    assert_eq!(remote_code(&error), codes::GIT_PATH_IS_DIRECTORY);

    let error = service
        .client
        .call::<_, GitFileContent>(
            diff::FILE_CONTENT,
            &params(
                FileSide::Revision {
                    rev: "--output=/tmp/x".to_owned(),
                },
                "README.md",
            ),
        )
        .await
        .expect_err("an option is not a revision");
    assert_eq!(remote_code(&error), codes::GIT_REVISION_NOT_FOUND);
    service.stop().await;
}

#[tokio::test]
async fn a_link_out_of_the_workspace_is_refused_by_the_core_check_over_the_wire() {
    let service = GitService::start().await;
    let outside = service.home.path().join("outside");
    std::fs::create_dir_all(&outside).expect("create dir");
    std::fs::write(outside.join("secret.txt"), "secret\n").expect("write");
    let link = service.dir().join("link");
    link_directory(&outside, &link);

    let error = service
        .client
        .call::<_, GitFileContent>(
            diff::FILE_CONTENT,
            &params(FileSide::Worktree, "link/secret.txt"),
        )
        .await
        .expect_err("a link out of the workspace is not a workspace file");
    assert_eq!(remote_code(&error), codes::GIT_PATH_OUTSIDE_WORKTREE);
    #[cfg(windows)]
    std::fs::remove_dir(&link).expect("remove the junction, not its target");
    service.stop().await;
}

#[tokio::test]
async fn paths_that_could_leave_the_workspace_are_refused_on_every_side() {
    let service = GitService::start().await;
    for path in ["../repo/README.md", "/etc/passwd", "a/../../b", "C:/x", ""] {
        for side in [
            FileSide::Worktree,
            FileSide::Index,
            FileSide::Revision {
                rev: "HEAD".to_owned(),
            },
        ] {
            let error = service
                .client
                .call::<_, GitFileContent>(diff::FILE_CONTENT, &params(side.clone(), path))
                .await
                .expect_err("refused");
            assert_eq!(
                remote_code(&error),
                codes::GIT_PATH_OUTSIDE_WORKTREE,
                "{path} on {side:?}"
            );
        }
    }
    service.stop().await;
}
