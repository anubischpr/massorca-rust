//! The branch methods over the wire: listing, create, switch, rename and delete with every refusal.

use massorca_client::ClientError;
use massorca_protocol::codes;
use massorca_protocol::methods::git::GitWorkspaceRef;
use massorca_protocol::methods::git::branch::{
    GitBranchCreateParams, GitBranchDeleteParams, GitBranchRenameParams, GitBranchSwitchParams,
};
use serde_json::json;

use super::common::remote_code;
use super::git_fixture::{GitService, git};

fn create(name: &str, start: Option<&str>, switch: bool) -> GitBranchCreateParams {
    GitBranchCreateParams {
        workspace: "feature".to_owned(),
        name: name.to_owned(),
        start: start.map(str::to_owned),
        switch,
    }
}

fn switch(branch: &str) -> GitBranchSwitchParams {
    GitBranchSwitchParams {
        workspace: "feature".to_owned(),
        branch: branch.to_owned(),
    }
}

fn delete(branch: &str, force: bool) -> GitBranchDeleteParams {
    GitBranchDeleteParams {
        workspace: "feature".to_owned(),
        branch: branch.to_owned(),
        force,
    }
}

fn head(service: &GitService) -> String {
    git(&service.dir(), &["symbolic-ref", "--short", "HEAD"])
        .trim_end()
        .to_owned()
}

#[tokio::test]
async fn branches_put_the_workspace_branch_first_and_mark_other_worktrees() {
    let service = GitService::start().await;
    let listed = service
        .client
        .git_branches(&GitWorkspaceRef {
            workspace: "feature".to_owned(),
        })
        .await
        .expect("branches");
    let names: Vec<_> = listed.branches.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["feature", "main"]);
    assert!(listed.branches[0].current && !listed.branches[0].checked_out_elsewhere);
    assert!(!listed.branches[1].current && listed.branches[1].checked_out_elsewhere);

    let folder = service.folder_workspace().await;
    let error = service
        .client
        .git_branches(&GitWorkspaceRef {
            workspace: folder.id,
        })
        .await
        .expect_err("folder");
    assert_eq!(remote_code(&error), codes::GIT_NOT_A_REPOSITORY);
    service.stop().await;
}

#[tokio::test]
async fn create_switch_and_rename_with_their_refusals() {
    let service = GitService::start().await;
    let made = service
        .client
        .git_branch_create(&create("topic", None, true))
        .await
        .expect("create and switch");
    assert!(made.current && made.upstream.is_none());
    assert_eq!(head(&service), "topic");
    for (params, code) in [
        (create("bad..name", None, false), codes::BRANCH_NAME_INVALID),
        (create("main", None, false), codes::BRANCH_EXISTS),
        (
            create("x", Some("nowhere"), false),
            codes::GIT_REVISION_NOT_FOUND,
        ),
    ] {
        let error = service
            .client
            .git_branch_create(&params)
            .await
            .expect_err("refused");
        assert_eq!(remote_code(&error), code);
    }

    let switched = service
        .client
        .git_branch_switch(&switch("feature"))
        .await
        .expect("switch");
    assert_eq!(switched.branch, "feature");
    assert_eq!(head(&service), "feature");
    for (branch, code) in [
        ("missing", codes::BRANCH_NOT_FOUND),
        ("main", codes::BRANCH_CHECKED_OUT),
    ] {
        let error = service
            .client
            .git_branch_switch(&switch(branch))
            .await
            .expect_err("refused");
        assert_eq!(remote_code(&error), code);
    }

    let renamed = service
        .client
        .git_branch_rename(&GitBranchRenameParams {
            workspace: "feature".to_owned(),
            from: "topic".to_owned(),
            to: "topic-2".to_owned(),
        })
        .await
        .expect("rename");
    assert_eq!(renamed.name, "topic-2");
    let error = service
        .client
        .git_branch_rename(&GitBranchRenameParams {
            workspace: "feature".to_owned(),
            from: "topic-2".to_owned(),
            to: "main".to_owned(),
        })
        .await
        .expect_err("exists");
    assert_eq!(remote_code(&error), codes::BRANCH_EXISTS);
    service.stop().await;
}

#[tokio::test]
async fn a_switch_that_would_overwrite_local_changes_lists_them() {
    let service = GitService::start().await;
    let dir = service.dir();
    git(&dir, &["checkout", "-q", "-b", "other"]);
    std::fs::write(dir.join("README.md"), "other\n").expect("write");
    git(&dir, &["commit", "-q", "-am", "other readme"]);
    git(&dir, &["checkout", "-q", "feature"]);
    std::fs::write(dir.join("README.md"), "local\n").expect("write");

    let error = service
        .client
        .git_branch_switch(&switch("other"))
        .await
        .expect_err("would overwrite");
    assert_eq!(remote_code(&error), codes::GIT_CHECKOUT_WOULD_OVERWRITE);
    match error {
        ClientError::Remote(body) => {
            assert_eq!(body.data, Some(json!({ "paths": ["README.md"] })));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(head(&service), "feature");
    service.stop().await;
}

#[tokio::test]
async fn delete_refuses_checked_out_in_use_and_unmerged_branches() {
    let service = GitService::start().await;
    let dir = service.dir();
    let error = service
        .client
        .git_branch_delete(&delete("main", true))
        .await
        .expect_err("checked out");
    assert_eq!(remote_code(&error), codes::BRANCH_CHECKED_OUT);

    // The workspace moves off its branch; the branch stays protected while the workspace exists.
    service
        .client
        .git_branch_create(&create("other", None, true))
        .await
        .expect("create");
    let error = service
        .client
        .git_branch_delete(&delete("feature", true))
        .await
        .expect_err("in use");
    assert_eq!(remote_code(&error), codes::BRANCH_IN_USE);

    git(&dir, &["checkout", "-q", "-b", "topic"]);
    std::fs::write(dir.join("topic.txt"), "work\n").expect("write");
    git(&dir, &["add", "topic.txt"]);
    git(&dir, &["commit", "-q", "-m", "topic work"]);
    let commit = git(&dir, &["rev-parse", "HEAD"]).trim_end().to_owned();
    git(&dir, &["checkout", "-q", "other"]);
    let error = service
        .client
        .git_branch_delete(&delete("topic", false))
        .await
        .expect_err("unmerged");
    assert_eq!(remote_code(&error), codes::BRANCH_UNMERGED);
    let deleted = service
        .client
        .git_branch_delete(&delete("topic", true))
        .await
        .expect("forced");
    assert_eq!((deleted.branch.as_str(), deleted.commit), ("topic", commit));
    service.stop().await;
}
