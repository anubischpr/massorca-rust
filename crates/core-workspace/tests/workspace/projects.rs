use std::fs;
use std::path::Path;
use std::path::PathBuf;

use massorca_protocol::codes;
use massorca_protocol::methods::projects::ProjectKind;

use crate::fixture::{Fixture, canonical, expect_code, project_add, text};

#[test]
fn a_path_inside_a_repository_registers_the_toplevel_as_a_git_project() {
    let fx = Fixture::new();
    let nested = fx.repo().join("src").join("deep");
    fs::create_dir_all(&nested).expect("create nested dir");

    let project = fx
        .workspaces
        .add_project(project_add(&nested))
        .expect("add project");

    assert_eq!(project.kind, ProjectKind::Git);
    assert_eq!(project.path, text(&canonical(&fx.repo())));
    assert_eq!(project.name, "repo");
    assert_eq!(
        project.workspaces_root,
        text(&fx.path("defaults").join("repo"))
    );
}

#[test]
fn a_directory_outside_every_repository_becomes_a_folder_project() {
    let fx = Fixture::new();
    let notes = fx.path("notes");
    fs::create_dir_all(&notes).expect("create dir");

    let project = fx
        .workspaces
        .add_project(project_add(&notes))
        .expect("add project");

    assert_eq!(project.kind, ProjectKind::Folder);
    assert_eq!(project.name, "notes");
    assert_eq!(project.path, text(&canonical(&notes)));
}

#[test]
fn folder_kind_registers_a_directory_inside_a_repository_as_itself() {
    let fx = Fixture::new();
    let docs = fx.repo().join("docs");
    fs::create_dir_all(&docs).expect("create dir");
    let mut params = project_add(&docs);
    params.kind = Some(ProjectKind::Folder);

    let project = fx.workspaces.add_project(params).expect("add project");

    assert_eq!(project.kind, ProjectKind::Folder);
    assert_eq!(project.path, text(&canonical(&docs)));
    assert_eq!(project.name, "docs");
}

#[test]
fn git_kind_on_a_plain_directory_is_refused() {
    let fx = Fixture::new();
    let plain = fx.path("plain");
    fs::create_dir_all(&plain).expect("create dir");
    let mut params = project_add(&plain);
    params.kind = Some(ProjectKind::Git);

    expect_code(fx.workspaces.add_project(params), codes::PROJECT_NOT_GIT);
    assert!(fx.workspaces.list_projects().expect("list").is_empty());
}

#[test]
fn an_unknown_kind_from_a_newer_peer_is_refused_as_invalid_params() {
    let fx = Fixture::new();
    let mut params = project_add(&fx.repo());
    params.kind = Some(ProjectKind::Unknown);

    expect_code(
        fx.workspaces.add_project(params),
        codes::REQUEST_INVALID_PARAMS,
    );
}

#[test]
fn a_missing_path_is_not_found() {
    let fx = Fixture::new();

    expect_code(
        fx.workspaces.add_project(project_add(&fx.path("missing"))),
        codes::PATH_NOT_FOUND,
    );
}

#[test]
fn a_file_is_not_a_directory() {
    let fx = Fixture::new();
    let file = fx.path("file.txt");
    fs::write(&file, "text").expect("write file");

    expect_code(
        fx.workspaces.add_project(project_add(&file)),
        codes::PATH_NOT_DIRECTORY,
    );
}

#[test]
fn an_explicit_name_replaces_the_directory_name() {
    let fx = Fixture::new();
    let mut params = project_add(&fx.repo());
    params.name = Some("Alpha".into());

    let project = fx.workspaces.add_project(params).expect("add project");

    assert_eq!(project.name, "Alpha");
    assert_eq!(
        fx.workspaces.show_project("Alpha").expect("show").id,
        project.id
    );
}

#[test]
fn the_same_path_cannot_be_registered_twice_under_another_name() {
    let fx = Fixture::new();
    fx.repo_project();
    let mut params = project_add(&fx.repo());
    params.name = Some("other".into());

    expect_code(fx.workspaces.add_project(params), codes::PROJECT_EXISTS);
}

#[test]
fn the_same_name_cannot_be_used_for_two_paths() {
    let fx = Fixture::new();
    let first = fx.path("first");
    let second = fx.path("second");
    fs::create_dir_all(&first).expect("create first");
    fs::create_dir_all(&second).expect("create second");
    let mut a = project_add(&first);
    a.name = Some("shared".into());
    fx.workspaces.add_project(a).expect("first project");
    let mut b = project_add(&second);
    b.name = Some("shared".into());

    expect_code(fx.workspaces.add_project(b), codes::PROJECT_EXISTS);
}

#[test]
fn an_explicit_workspaces_root_is_stored_as_given() {
    let fx = Fixture::new();
    let root = fx.path("elsewhere");
    let mut params = project_add(&fx.repo());
    params.workspaces_root = Some(text(&root));

    let project = fx.workspaces.add_project(params).expect("add project");

    assert_eq!(project.workspaces_root, text(&root));
}

#[test]
fn a_relative_workspaces_root_is_refused() {
    let fx = Fixture::new();
    let mut params = project_add(&fx.repo());
    params.workspaces_root = Some("relative/root".into());

    expect_code(
        fx.workspaces.add_project(params),
        codes::REQUEST_INVALID_PARAMS,
    );
}

#[test]
fn a_relative_path_is_refused_as_invalid_before_it_is_checked_for_existence() {
    let fx = Fixture::new();

    expect_code(
        fx.workspaces
            .add_project(project_add(Path::new("no/such/dir"))),
        codes::REQUEST_INVALID_PARAMS,
    );
}

#[test]
fn a_blank_or_separated_name_is_refused() {
    let fx = Fixture::new();
    for name in ["", "   ", "a/b", "a\\b", ".", ".."] {
        let mut params = project_add(&fx.repo());
        params.name = Some(name.into());

        expect_code(
            fx.workspaces.add_project(params),
            codes::REQUEST_INVALID_PARAMS,
        );
    }
    assert!(fx.workspaces.list_projects().expect("list").is_empty());
}

#[test]
fn projects_are_listed_by_name() {
    let fx = Fixture::new();
    fx.folder_project("zeta");
    fx.folder_project("alpha");

    let names: Vec<String> = fx
        .workspaces
        .list_projects()
        .expect("list")
        .into_iter()
        .map(|project| project.name)
        .collect();

    assert_eq!(names, ["alpha", "zeta"]);
}

#[test]
fn removing_a_project_never_deletes_its_files() {
    let fx = Fixture::new();
    let project = fx.folder_project("notes");

    let removed = fx
        .workspaces
        .remove_project(&project.id)
        .expect("remove project");

    assert_eq!(removed.id, project.id);
    assert!(fx.path("notes").join("notes.txt").exists());
    assert!(fx.workspaces.list_projects().expect("list").is_empty());
}

#[test]
fn a_project_with_workspaces_is_refused_until_they_are_gone() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    fx.create(&project.name, "wip");

    expect_code(
        fx.workspaces.remove_project(&project.id),
        codes::PROJECT_HAS_WORKSPACES,
    );

    fx.remove("wip");
    fx.workspaces
        .remove_project(&project.id)
        .expect("remove project once its workspaces are gone");
    assert!(fx.repo().join("README.md").exists());
}

#[test]
fn removing_an_unknown_project_is_not_found() {
    let fx = Fixture::new();

    expect_code(
        fx.workspaces.remove_project("no-such-project"),
        codes::PROJECT_NOT_FOUND,
    );
}

#[cfg(any(windows, target_os = "macos"))]
#[test]
fn a_path_that_differs_only_in_case_is_the_same_project() {
    let fx = Fixture::new();
    fx.repo_project();
    let mut upper = project_add(&fx.repo());
    upper.path = text(&fx.repo()).to_uppercase();

    expect_code(fx.workspaces.add_project(upper), codes::PROJECT_EXISTS);
}

#[cfg(not(any(windows, target_os = "macos")))]
#[test]
fn paths_that_differ_only_in_case_are_distinct_projects_on_a_case_sensitive_filesystem() {
    let fx = Fixture::new();
    let lower = fx.path("case");
    let upper = fx.path("CASE");
    fs::create_dir_all(&lower).expect("create lower");
    fs::create_dir_all(&upper).expect("create upper");

    let a = fx
        .workspaces
        .add_project(project_add(&lower))
        .expect("lower");
    let b = fx
        .workspaces
        .add_project(project_add(&upper))
        .expect("upper");

    assert_ne!(a.id, b.id);
}

#[test]
fn a_handle_with_another_default_root_uses_it_for_the_workspaces_root_and_shares_the_store() {
    let fx = Fixture::new();
    let handle = fx.workspaces.with_default_root(fx.path("changed-defaults"));

    let project = handle
        .add_project(project_add(&fx.repo()))
        .expect("add project");

    assert_eq!(
        project.workspaces_root,
        text(&fx.path("changed-defaults").join("repo"))
    );
    let seen = fx
        .workspaces
        .show_project(&project.id)
        .expect("the other handle sees the project");
    assert_eq!(seen.id, project.id);
}

#[test]
fn a_default_root_that_is_not_absolute_is_refused_when_a_project_would_take_it() {
    let fx = Fixture::new();
    let handle = fx
        .workspaces
        .with_default_root(PathBuf::from("relative/root"));

    expect_code(
        handle.add_project(project_add(&fx.repo())),
        codes::REQUEST_INVALID_PARAMS,
    );
    assert!(fx.workspaces.list_projects().expect("list").is_empty());
}
