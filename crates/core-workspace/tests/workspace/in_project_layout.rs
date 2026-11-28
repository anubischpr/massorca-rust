//! The `in_project` layout (decision 0183): a git project's worktrees go in `<project>/.worktrees/<name>`, kept out of
//! git by a line in the common git dir's `info/exclude`.

use std::path::{Path, PathBuf};

use massorca_core_git::Git;
use massorca_core_workspace::{WorkspaceLayout, Workspaces};
use massorca_protocol::methods::projects::ProjectIcon;

use crate::fixture::{Fixture, canonical, create_params, project_add, run_git, text};

fn in_project(fx: &Fixture) -> Workspaces {
    fx.workspaces.with_layout(WorkspaceLayout::InProject)
}

fn exclude_file(repo: &Path) -> PathBuf {
    repo.join(".git").join("info").join("exclude")
}

fn exclude_lines(repo: &Path) -> usize {
    std::fs::read_to_string(exclude_file(repo))
        .unwrap_or_default()
        .lines()
        .filter(|line| line.trim() == "/.worktrees/")
        .count()
}

#[test]
fn in_project_is_the_default_layout_of_a_new_handle() {
    let fx = Fixture::new();
    let fresh = Workspaces::new(
        fx.store.clone(),
        Git::discover().expect("git is on PATH"),
        fx.path("defaults"),
    );

    let project = fresh
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(
        project.workspaces_root,
        text(&Path::new(&project.path).join(".worktrees"))
    );
}

#[test]
fn adding_a_project_records_the_root_but_creates_nothing() {
    let fx = Fixture::new();

    let project = in_project(&fx)
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(
        project.workspaces_root,
        text(&Path::new(&project.path).join(".worktrees"))
    );
    assert!(!fx.repo().join(".worktrees").exists());
    assert_eq!(exclude_lines(&fx.repo()), 0);
}

#[test]
fn the_first_worktree_creates_the_folder_and_excludes_it_once() {
    let fx = Fixture::new();
    let domain = in_project(&fx);
    let project = domain
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    let first = domain
        .create_workspace(create_params(&project.id, "feature"))
        .expect("first worktree");
    domain
        .create_workspace(create_params(&project.id, "second"))
        .expect("second worktree");

    assert_eq!(
        canonical(Path::new(&first.path)),
        canonical(&fx.repo().join(".worktrees").join("feature"))
    );
    assert_eq!(exclude_lines(&fx.repo()), 1);
    assert_eq!(run_git(&fx.repo(), &["status", "--porcelain"]), "");
    let gitignore = fx.repo().join(".gitignore");
    assert!(
        !gitignore.exists(),
        "the repository's .gitignore is never written"
    );
}

#[test]
fn removing_the_last_worktree_keeps_the_folder_and_the_exclude_line() {
    let fx = Fixture::new();
    let domain = in_project(&fx);
    let project = domain
        .add_project(project_add(&fx.repo()))
        .expect("add project");
    let workspace = domain
        .create_workspace(create_params(&project.id, "feature"))
        .expect("create worktree");

    fx.remove(&workspace.id);

    assert!(fx.repo().join(".worktrees").is_dir());
    assert!(!fx.repo().join(".worktrees").join("feature").exists());
    assert_eq!(exclude_lines(&fx.repo()), 1);
}

#[test]
fn a_project_added_from_a_linked_worktree_takes_the_main_repositorys_folder() {
    let fx = Fixture::new();
    let linked = fx.path("linked");
    run_git(
        &fx.repo(),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "linked",
            text(&linked).as_str(),
        ],
    );
    let domain = in_project(&fx);

    let project = domain
        .add_project(project_add(&linked))
        .expect("add project");
    domain
        .create_workspace(create_params(&project.id, "feature"))
        .expect("create worktree");

    assert_eq!(
        canonical(Path::new(&project.workspaces_root)),
        canonical(&fx.repo().join(".worktrees"))
    );
    assert_eq!(exclude_lines(&fx.repo()), 1);
    assert_eq!(run_git(&linked, &["status", "--porcelain"]), "");
}

#[test]
fn a_folder_project_keeps_a_root_under_the_default_root_until_git_is_initialized() {
    let fx = Fixture::new();
    let domain = in_project(&fx);
    let dir = fx.path("notes");
    std::fs::create_dir_all(&dir).expect("create folder");
    let mut params = project_add(&dir);
    params.kind = Some(massorca_protocol::methods::projects::ProjectKind::Folder);

    let folder = domain.add_project(params).expect("add folder project");
    assert_eq!(
        folder.workspaces_root,
        text(&fx.path("defaults").join("notes"))
    );
    assert!(!dir.join(".worktrees").exists());

    let git = domain.init_project_git(&folder.id).expect("initialize git");
    assert_eq!(
        git.workspaces_root,
        text(&Path::new(&git.path).join(".worktrees"))
    );
}

#[test]
fn a_project_added_before_the_change_keeps_its_root() {
    let fx = Fixture::new();
    let project = fx.repo_project();

    let workspace = in_project(&fx)
        .create_workspace(create_params(&project.id, "feature"))
        .expect("create worktree");

    assert_eq!(
        canonical(Path::new(&workspace.path)),
        canonical(&fx.path("defaults/repo/feature"))
    );
    assert!(!fx.repo().join(".worktrees").exists());
    assert_eq!(exclude_lines(&fx.repo()), 0);
}

#[test]
fn icon_detection_never_reads_inside_the_worktrees_folder() {
    let fx = Fixture::new();
    let icon = fx
        .repo()
        .join(".worktrees")
        .join("feature")
        .join("icon.png");
    std::fs::create_dir_all(icon.parent().expect("parent")).expect("create dirs");
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend_from_slice(b"filler");
    std::fs::write(&icon, png).expect("write icon");
    std::fs::write(
        fx.repo().join("index.html"),
        r#"<link rel="icon" href=".worktrees/feature/icon.png">"#,
    )
    .expect("write index");
    std::fs::write(
        fx.repo().join("package.json"),
        r#"{"icon": ".worktrees/feature/icon.png"}"#,
    )
    .expect("write package.json");

    let project = in_project(&fx)
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(project.icon, None::<ProjectIcon>);
}

#[cfg(windows)]
#[test]
fn a_worktree_path_over_the_windows_limit_is_refused_naming_the_path() {
    let fx = Fixture::new();
    let mut repo = fx.path("long");
    // Short of the limit itself, so git can write the repository; the worktree path goes over it.
    while text(&repo).len() < 195 {
        repo.push("segment-of-a-long-path");
    }
    std::fs::create_dir_all(&repo).expect("create the long folder");
    // No template: its hook samples would pass the limit before the test does.
    run_git(&repo, &["init", "-q", "--template="]);
    let domain = in_project(&fx);
    let project = domain.add_project(project_add(&repo)).expect("add project");

    let error = domain
        .create_workspace(create_params(&project.id, &"w".repeat(60)))
        .expect_err("too long");

    assert_eq!(
        error.code(),
        massorca_protocol::codes::WORKSPACE_PATH_TOO_LONG
    );
    assert!(
        error.message().contains(".worktrees"),
        "{}",
        error.message()
    );
    assert!(
        error.message().contains("workspaces.default_root"),
        "{}",
        error.message()
    );
    assert!(!repo.join(".worktrees").exists());
}
