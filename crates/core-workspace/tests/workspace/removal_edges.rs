//! Removal paths that depend on git's own state of a registered worktree (ADR 0008).

use std::path::{Path, PathBuf};

use massorca_core_workspace::RemovalWaivers;
use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::WorkspaceState;

use crate::fixture::{Fixture, expect_code};

/// The administrative directory git keeps for the worktree at `worktree`, read from its `.git` file.
fn admin_dir_of(worktree: &str) -> PathBuf {
    let dot_git = std::fs::read_to_string(Path::new(worktree).join(".git")).expect("read .git");
    PathBuf::from(
        dot_git
            .trim()
            .strip_prefix("gitdir: ")
            .expect("a gitdir line"),
    )
}

#[test]
fn a_registered_worktree_whose_status_cannot_be_read_refuses_removal_and_keeps_its_row() {
    let fx = Fixture::new();
    let project = fx.repo_project();
    let ws = fx.create(&project.name, "broken");
    std::fs::write(admin_dir_of(&ws.path).join("index"), "not an index")
        .expect("corrupt the index");

    expect_code(
        fx.workspaces.begin_remove("broken", RemovalWaivers::FORCE),
        codes::GIT_FAILED,
    );

    assert!(Path::new(&ws.path).join("README.md").is_file());
    let row = fx
        .store
        .get_workspace(&ws.id)
        .expect("read row")
        .expect("row kept");
    assert_eq!(row.state, massorca_core_store::WorkspaceState::Ready);
    assert_eq!(
        fx.workspaces.show_workspace(&ws.id).expect("show").state,
        WorkspaceState::Ready
    );
}
