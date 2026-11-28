//! `Git::file_content` for the spellings of a path and for the places a path can lead: git's own folders, links,
//! junctions, long Windows paths, FIFOs and the error text. Each test reads the same way on every side it names.

use std::path::Path;

use massorca_core_git::{FileContentError, Git, SideContent};
use massorca_protocol::codes;
use massorca_protocol::methods::git::diff::FileSide;

use crate::fixture::{Fixture, commit_file, run_git, text};

fn git() -> Git {
    Git::discover().expect("git is installed")
}

fn bytes(contents: &str) -> SideContent {
    SideContent::Bytes(contents.as_bytes().to_vec())
}

fn revision(rev: &str) -> FileSide {
    FileSide::Revision {
        rev: rev.to_owned(),
    }
}

/// The code of a refusal; any other outcome fails the test.
fn refusal(result: Result<SideContent, FileContentError>) -> &'static str {
    match result {
        Err(FileContentError::Refused { code, .. }) => code,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

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

/// Runs `call` on another thread and fails the test if it does not return, so a blocked open cannot hang the run.
#[cfg(unix)]
fn returns_promptly<T: Send + 'static>(call: impl FnOnce() -> T + Send + 'static) -> T {
    use std::sync::mpsc;
    use std::time::Duration;

    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(call());
    });
    receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("the read returned instead of blocking")
}

#[test]
fn every_spelling_of_a_path_reads_the_same_file_on_every_side() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    std::fs::create_dir_all(repo.join("docs")).expect("create dir");
    commit_file(&repo, "docs/guide.md", "guide\n", "add guide");

    for spelling in ["./README.md", "README.md/.", ".//README.md"] {
        for side in [FileSide::Worktree, FileSide::Index, revision("HEAD")] {
            let content = git()
                .file_content(&repo, &side, spelling, 1 << 20)
                .expect("content");
            assert_eq!(content, bytes("hello\n"), "{spelling} on {side:?}");
        }
    }
    for spelling in ["docs//guide.md", "./docs/./guide.md", "docs/./guide.md"] {
        for side in [FileSide::Worktree, FileSide::Index, revision("HEAD")] {
            let content = git()
                .file_content(&repo, &side, spelling, 1 << 20)
                .expect("content");
            assert_eq!(content, bytes("guide\n"), "{spelling} on {side:?}");
        }
    }
}

#[cfg(windows)]
#[test]
fn a_backslash_spelling_reads_the_same_file_on_every_side() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    std::fs::create_dir_all(repo.join("docs")).expect("create dir");
    commit_file(&repo, "docs/guide.md", "guide\n", "add guide");

    for spelling in [r"docs\guide.md", r"docs\\guide.md", r"docs/guide.md"] {
        for side in [FileSide::Worktree, FileSide::Index, revision("HEAD")] {
            let content = git()
                .file_content(&repo, &side, spelling, 1 << 20)
                .expect("content");
            assert_eq!(content, bytes("guide\n"), "{spelling} on {side:?}");
        }
    }
}

#[cfg(windows)]
#[test]
fn backslash_and_verbatim_spellings_that_climb_or_name_a_drive_are_refused() {
    let fixture = Fixture::new();
    let repo = fixture.repo();

    for spelling in [
        r"..\README.md",
        r"docs\..\..\README.md",
        r"\\?\C:\README.md",
        r"\\?\UNC\server\share\README.md",
        r"C:README.md",
        r"\README.md",
    ] {
        for side in [FileSide::Worktree, FileSide::Index, revision("HEAD")] {
            assert_eq!(
                refusal(git().file_content(&repo, &side, spelling, 1 << 20)),
                codes::GIT_PATH_OUTSIDE_WORKTREE,
                "{spelling} on {side:?}"
            );
        }
    }
}

#[test]
fn git_s_own_folder_is_refused_in_every_spelling() {
    let fixture = Fixture::new();
    let repo = fixture.repo();

    for spelling in [
        ".git/config",
        ".GIT/config",
        ".Git/HEAD",
        "./.git/config",
        ".git//config",
    ] {
        assert_eq!(
            refusal(git().file_content(&repo, &FileSide::Worktree, spelling, 1 << 20)),
            codes::REQUEST_INVALID_PARAMS,
            "{spelling}"
        );
    }
}

#[test]
fn a_git_folder_under_another_name_inside_the_workspace_is_refused() {
    let fixture = Fixture::new();
    let root = fixture.sibling("relocated");
    std::fs::create_dir_all(&root).expect("create dir");
    let gitdata = root.join("gitdata");
    run_git(&root, &["init", "-q", "--separate-git-dir", text(&gitdata)]);
    std::fs::write(root.join("notes.txt"), "plain\n").expect("write");

    assert_eq!(
        git()
            .file_content(&root, &FileSide::Worktree, "notes.txt", 1 << 20)
            .expect("content"),
        bytes("plain\n")
    );
    assert_eq!(
        refusal(git().file_content(&root, &FileSide::Worktree, "gitdata/config", 1 << 20)),
        codes::REQUEST_INVALID_PARAMS,
        "the repository's own folder, whatever its name"
    );
    assert_eq!(
        refusal(git().file_content(&root, &FileSide::Worktree, ".git", 1 << 20)),
        codes::REQUEST_INVALID_PARAMS,
        "the git file that points at it"
    );
}

#[cfg(windows)]
#[test]
fn a_long_path_inside_the_workspace_is_read() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let mut deep = repo.clone();
    for _ in 0..9 {
        deep.push("a-long-folder-name-0123456789");
    }
    std::fs::create_dir_all(&deep).expect("create deep dirs");
    std::fs::write(deep.join("file.txt"), "deep\n").expect("write");
    assert!(
        deep.join("file.txt").as_os_str().len() > 260,
        "the path must be longer than MAX_PATH"
    );

    let relative = (0..9)
        .map(|_| "a-long-folder-name-0123456789")
        .chain(["file.txt"])
        .collect::<Vec<_>>()
        .join("/");
    assert_eq!(
        git()
            .file_content(&repo, &FileSide::Worktree, &relative, 1 << 20)
            .expect("content"),
        bytes("deep\n")
    );
}

#[test]
fn a_directory_link_that_leaves_the_worktree_is_refused() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let outside = fixture.sibling("outside");
    std::fs::create_dir_all(&outside).expect("create dir");
    std::fs::write(outside.join("secret.txt"), "secret\n").expect("write");
    let link = repo.join("link");
    link_directory(&outside, &link);

    assert_eq!(
        refusal(git().file_content(&repo, &FileSide::Worktree, "link/secret.txt", 1 << 20)),
        codes::GIT_PATH_OUTSIDE_WORKTREE
    );
    #[cfg(windows)]
    std::fs::remove_dir(&link).expect("remove the junction, not its target");
}

#[test]
fn a_linked_worktree_refuses_its_git_file_and_the_main_git_folder_in_every_spelling() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let linked = fixture.sibling("linked");
    run_git(
        &repo,
        &["worktree", "add", "-q", "-b", "feature", text(&linked)],
    );
    assert!(
        linked.join(".git").is_file(),
        "a linked worktree's .git is a file"
    );

    for spelling in [".git", "./.git", ".GIT"] {
        assert_eq!(
            refusal(git().file_content(&linked, &FileSide::Worktree, spelling, 1 << 20)),
            codes::REQUEST_INVALID_PARAMS,
            "{spelling}"
        );
    }
    let main_config = repo.join(".git").join("config");
    for spelling in ["../repo/.git/config", text(&main_config)] {
        assert_eq!(
            refusal(git().file_content(&linked, &FileSide::Worktree, spelling, 1 << 20)),
            codes::GIT_PATH_OUTSIDE_WORKTREE,
            "{spelling}"
        );
    }
    let through_link = linked.join("main-git");
    link_directory(&repo.join(".git"), &through_link);
    assert_eq!(
        refusal(git().file_content(&linked, &FileSide::Worktree, "main-git/config", 1 << 20)),
        codes::GIT_PATH_OUTSIDE_WORKTREE,
        "the main folder reached through a link"
    );
    #[cfg(windows)]
    std::fs::remove_dir(&through_link).expect("remove the junction, not its target");

    assert_eq!(
        git()
            .file_content(&linked, &FileSide::Worktree, "README.md", 1 << 20)
            .expect("content"),
        bytes("hello\n"),
        "ordinary files still read"
    );
}

#[cfg(unix)]
#[test]
fn a_fifo_in_the_workspace_is_refused_without_blocking() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let pipe = repo.join("pipe");
    let made = massorca_process::command("mkfifo")
        .arg(&pipe)
        .status()
        .expect("run mkfifo");
    assert!(made.success(), "mkfifo is needed for this test");

    let outcome =
        returns_promptly(move || git().file_content(&repo, &FileSide::Worktree, "pipe", 1 << 20));
    assert_eq!(refusal(outcome), codes::REQUEST_INVALID_PARAMS);
}

#[test]
fn a_directory_with_many_files_is_a_directory_on_the_index_side() {
    let fixture = Fixture::new();
    let repo = fixture.repo();
    let bulk = repo.join("bulk");
    std::fs::create_dir_all(&bulk).expect("create dir");
    for index in 0..300 {
        std::fs::write(bulk.join(format!("file-{index:03}.txt")), "x\n").expect("write");
    }
    run_git(&repo, &["add", "bulk"]);

    for side in [FileSide::Worktree, FileSide::Index] {
        assert_eq!(
            refusal(git().file_content(&repo, &side, "bulk", 1 << 20)),
            codes::GIT_PATH_IS_DIRECTORY,
            "{side:?}"
        );
    }
}

#[test]
fn an_unreadable_workspace_names_the_requested_path_and_never_the_host_path() {
    let fixture = Fixture::new();
    let missing = fixture.sibling("no-such-workspace");

    let error = git()
        .file_content(&missing, &FileSide::Worktree, "docs/guide.md", 1 << 20)
        .expect_err("the workspace is missing");
    let message = error.to_string();
    assert!(message.contains("`docs/guide.md`"), "{message}");
    assert!(
        !message.contains("no-such-workspace"),
        "the host path must not reach the client: {message}"
    );
}
