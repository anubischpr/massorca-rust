//! Element tests of the Source control panel on a scripted service: the watch that follows the selected
//! workspace, the stage call and its refusal, the discard flow and its waiver, the commit box's
//! refusal, and every state of the panel drawing. Each call is answered by hand; no window opens.

use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::codes;
use massorca_protocol::methods::git::ChangeKind;
use massorca_protocol::methods::git::commit::GitCommitParams;
use massorca_protocol::methods::git::index::GitDiscarded;
use massorca_protocol::methods::git::log::{Commit, GitLog, Signature};
use massorca_protocol::methods::git::status::{BranchState, GitStatus, StatusEntry, StatusTotals};

use super::service_tests::{Rig, draw, rig};
use crate::model::regions::Region;
use crate::model::source_control_flow::{DiscardStage, DiscardTarget};
use crate::service::fixtures::{project, workspace};
use crate::service::{Command, GitUpdate, Refusal, StoreUpdate};

/// Selects workspace `w1` and opens the Source control tab for it, then draws the window.
fn open_panel(rig: &Rig, cx: &mut VisualTestContext) {
    rig.shell
        .update(cx, |shell, _| shell.model.open_workspace("w1"));
    rig.shell
        .update(cx, |shell, cx| shell.open_source_control(cx));
    cx.run_until_parked();
    draw(cx);
}

fn entry(path: &str) -> StatusEntry {
    StatusEntry {
        path: path.to_owned(),
        original_path: None,
        staged: None,
        unstaged: None,
        untracked: false,
        conflict: None,
        staged_lines: None,
        unstaged_lines: None,
        submodule: false,
        path_lossy: false,
    }
}

fn status(entries: Vec<StatusEntry>) -> GitStatus {
    GitStatus {
        branch: BranchState {
            head: Some("feat/scm".into()),
            oid: Some("a".repeat(40)),
            upstream: Some("origin/main".into()),
            ahead: Some(1),
            behind: Some(0),
            upstream_gone: false,
        },
        operation: None,
        entries,
        totals: StatusTotals::default(),
        truncated: false,
    }
}

fn history() -> GitLog {
    let signature = Signature {
        name: "Ada".into(),
        email: "ada@example.invalid".into(),
        time_ms: 1,
    };
    GitLog {
        commits: vec![Commit {
            id: "0123456789abcdef0123456789abcdef01234567".into(),
            parents: Vec::new(),
            author: signature.clone(),
            committer: signature,
            subject: "first".into(),
            message: "first\n".into(),
            message_truncated: false,
        }],
        next_cursor: None,
    }
}

/// The first call the panel sent, which must be the one a test expects.
fn next(rig: &mut Rig) -> Command {
    rig.script.next().expect("the panel sent a call")
}

#[gpui::test]
fn the_panel_watches_the_selected_workspace_while_its_tab_is_open(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_panel(&rig, cx);
    assert!(matches!(
        next(&mut rig),
        Command::GitWatch { workspace } if workspace == "w1"
    ));

    // Closing the Project sidebar stops the watch: the panel is no longer on screen.
    rig.shell.update(cx, |shell, _| {
        shell.model.toggle_region(Region::ProjectSidebar)
    });
    draw(cx);
    assert!(matches!(
        next(&mut rig),
        Command::GitUnwatch { workspace } if workspace == "w1"
    ));
}

#[gpui::test]
fn staging_sends_one_call_and_a_refusal_shows_under_the_panel(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_panel(&rig, cx);
    next(&mut rig);

    rig.shell.update(cx, |shell, cx| {
        shell.set_staged(Some("README.md".into()), false, cx);
    });
    cx.run_until_parked();
    let Command::GitStage { params, reply } = next(&mut rig) else {
        panic!("a stage call was expected");
    };
    assert_eq!(params.paths, ["README.md"]);
    assert!(!params.all);
    let _ = reply.send(Err(Refusal::Failed {
        code: codes::GIT_INDEX_LOCKED.into(),
        message: "The index is locked.".into(),
    }));
    cx.run_until_parked();
    draw(cx);

    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.source_control.error.as_deref(),
            Some("The index is locked.")
        );
        assert!(!shell.source_control.busy, "the answer frees the controls");
    });
}

#[gpui::test]
fn discard_asks_then_retries_with_only_the_waiver_its_refusal_names(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_panel(&rig, cx);
    next(&mut rig);

    let tracked = DiscardTarget {
        path: "README.md".into(),
        untracked: false,
    };
    rig.shell
        .update(cx, |shell, cx| shell.open_discard(vec![tracked], cx));
    draw(cx);
    rig.shell.update(cx, |shell, cx| shell.confirm_discard(cx));
    cx.run_until_parked();
    let Command::GitDiscard { params, reply } = next(&mut rig) else {
        panic!("a discard call was expected");
    };
    assert!(!params.delete_untracked && !params.include_staged);
    let _ = reply.send(Err(Refusal::GitRefused {
        code: codes::GIT_DISCARD_STAGED.into(),
        message: "The file has staged changes.".into(),
        paths: vec!["README.md".into()],
    }));
    cx.run_until_parked();
    draw(cx);
    rig.shell.read_with(cx, |shell, _| {
        let stage = shell
            .source_control
            .discard
            .as_ref()
            .map(|flow| flow.stage().clone());
        assert_eq!(
            stage,
            Some(DiscardStage::Staged {
                paths: vec!["README.md".into()]
            })
        );
    });

    rig.shell.update(cx, |shell, cx| shell.confirm_discard(cx));
    cx.run_until_parked();
    let Command::GitDiscard { params, reply } = next(&mut rig) else {
        panic!("the waived discard was expected");
    };
    assert!(params.include_staged && !params.delete_untracked);
    let _ = reply.send(Ok(GitDiscarded::default()));
    cx.run_until_parked();
    draw(cx);
    rig.shell.read_with(cx, |shell, _| {
        assert!(shell.source_control.discard.is_none(), "the dialog closes");
    });
}

#[gpui::test]
fn the_first_dialog_of_an_untracked_selection_says_delete_and_confirms_once(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_panel(&rig, cx);
    next(&mut rig);

    let untracked = DiscardTarget {
        path: "notes.md".into(),
        untracked: true,
    };
    rig.shell
        .update(cx, |shell, cx| shell.open_discard(vec![untracked], cx));
    draw(cx);
    rig.shell.read_with(cx, |shell, _| {
        let copy = shell
            .source_control
            .discard
            .as_ref()
            .map(|flow| flow.copy())
            .expect("the dialog is open");
        assert_eq!(copy.title, "Delete notes.md?");
        assert_eq!(copy.action, "Delete file");
    });

    // One confirmation sends the call that deletes the file: there is no second dialog.
    rig.shell.update(cx, |shell, cx| shell.confirm_discard(cx));
    cx.run_until_parked();
    let Command::GitDiscard { params, reply } = next(&mut rig) else {
        panic!("the discard call was expected");
    };
    assert!(params.delete_untracked && !params.include_staged);
    assert_eq!(params.paths, ["notes.md"]);
    let _ = reply.send(Ok(GitDiscarded::default()));
    cx.run_until_parked();
    draw(cx);
    rig.shell.read_with(cx, |shell, _| {
        assert!(shell.source_control.discard.is_none());
    });
}

#[gpui::test]
fn a_refused_commit_shows_its_reason_under_the_message_field(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_panel(&rig, cx);
    next(&mut rig);

    rig.shell.update(cx, |shell, cx| shell.submit_commit(cx));
    cx.run_until_parked();
    let Command::GitCommit { params, reply } = next(&mut rig) else {
        panic!("a commit call was expected");
    };
    let GitCommitParams { message, .. } = params;
    assert_eq!(
        message.as_deref(),
        Some(""),
        "the blank message goes to the service"
    );
    let _ = reply.send(Err(Refusal::Failed {
        code: codes::GIT_COMMIT_MESSAGE_EMPTY.into(),
        message: "The message is empty.".into(),
    }));
    cx.run_until_parked();
    draw(cx);
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.source_control.commit.message_error.as_deref(),
            Some("Write a commit message.")
        );
        assert!(!shell.source_control.commit.busy());
    });
}

#[gpui::test]
fn every_state_of_the_panel_draws(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_panel(&rig, cx);
    next(&mut rig);
    let send = |rig: &Rig, update: GitUpdate| {
        rig.updates
            .send(StoreUpdate::Git(update))
            .expect("the store listens");
    };

    // Loading: nothing answered yet. Then a folder that is not a repository.
    send(
        &rig,
        GitUpdate::Status {
            workspace: "w1".into(),
            result: Err(Refusal::Failed {
                code: codes::GIT_NOT_A_REPOSITORY.into(),
                message: "This folder is not a repository.".into(),
            }),
        },
    );
    cx.run_until_parked();
    draw(cx);

    // A status the service could not read.
    send(
        &rig,
        GitUpdate::Status {
            workspace: "w1".into(),
            result: Err(Refusal::Failed {
                code: codes::GIT_FAILED.into(),
                message: "git could not read the index".into(),
            }),
        },
    );
    cx.run_until_parked();
    draw(cx);

    // A repository with every kind of change, its history, and a watch that is degraded.
    let mut staged = entry("src/main.rs");
    staged.staged = Some(ChangeKind::Modified);
    let mut changed = entry("README.md");
    changed.unstaged = Some(ChangeKind::Modified);
    let mut untracked = entry("notes.md");
    untracked.untracked = true;
    let mut conflicted = entry("Cargo.lock");
    conflicted.conflict = Some(massorca_protocol::methods::git::status::ConflictKind::BothModified);
    send(
        &rig,
        GitUpdate::Status {
            workspace: "w1".into(),
            result: Ok(status(vec![staged, changed, untracked, conflicted])),
        },
    );
    send(
        &rig,
        GitUpdate::Log {
            workspace: "w1".into(),
            result: Ok(history()),
        },
    );
    send(
        &rig,
        GitUpdate::Watch {
            workspace: "w1".into(),
            degraded: Some("The file system cannot be watched.".into()),
        },
    );
    cx.run_until_parked();
    draw(cx);

    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(shell.source_control.watching.as_deref(), Some("w1"));
    });
}

#[gpui::test]
fn the_commit_draft_does_not_follow_the_selection_to_another_workspace(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.updates
        .send(StoreUpdate::Resync {
            projects: vec![project("p1", "alpha")],
            workspaces: vec![
                workspace("w1", "p1", "fix-login", 1),
                workspace("w2", "p1", "other", 2),
            ],
        })
        .expect("the store listens");
    cx.run_until_parked();
    open_panel(&rig, cx);

    // A message typed for w1 is in the box.
    rig.shell.update_in(cx, |shell, window, cx| {
        let field = shell.source_control.commit_field.clone();
        field.update(cx, |field, cx| field.set_text("half-written", window, cx));
    });
    cx.run_until_parked();
    draw(cx);
    rig.shell.read_with(cx, |shell, cx| {
        assert_eq!(
            shell.source_control.commit_field.read(cx).text(),
            "half-written"
        );
    });

    // Selecting w2 empties the box and the commit form; coming back to w1 does not restore the draft.
    rig.shell
        .update(cx, |shell, _| shell.model.open_workspace("w2"));
    cx.run_until_parked();
    draw(cx);
    rig.shell.read_with(cx, |shell, cx| {
        assert_eq!(shell.source_control.commit_field.read(cx).text(), "");
        assert_eq!(shell.source_control.commit.message, "");
    });
    rig.shell
        .update(cx, |shell, _| shell.model.open_workspace("w1"));
    cx.run_until_parked();
    draw(cx);
    rig.shell.read_with(cx, |shell, cx| {
        assert_eq!(shell.source_control.commit_field.read(cx).text(), "");
        assert_eq!(shell.source_control.commit.message, "");
    });
}

#[gpui::test]
fn a_late_discard_answer_does_not_reshape_the_dialog_of_another_workspace(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.updates
        .send(StoreUpdate::Resync {
            projects: vec![project("p1", "alpha")],
            workspaces: vec![
                workspace("w1", "p1", "fix-login", 1),
                workspace("w2", "p1", "other", 2),
            ],
        })
        .expect("the store listens");
    cx.run_until_parked();
    open_panel(&rig, cx);
    next(&mut rig);

    // A discard of a.txt in w1 is confirmed, and its call is out.
    let a = DiscardTarget {
        path: "a.txt".into(),
        untracked: false,
    };
    rig.shell
        .update(cx, |shell, cx| shell.open_discard(vec![a], cx));
    draw(cx);
    rig.shell.update(cx, |shell, cx| shell.confirm_discard(cx));
    cx.run_until_parked();
    let Command::GitDiscard { reply: a_reply, .. } = next(&mut rig) else {
        panic!("a discard call for a.txt was expected");
    };

    // The selection moves to w2 while that call is pending, and b.txt is opened for discard there.
    rig.shell
        .update(cx, |shell, _| shell.model.open_workspace("w2"));
    draw(cx);
    while rig.script.next().is_some() {}
    let b = DiscardTarget {
        path: "b.txt".into(),
        untracked: false,
    };
    rig.shell
        .update(cx, |shell, cx| shell.open_discard(vec![b], cx));
    draw(cx);

    // a.txt's answer arrives late: the service refused it because of its staged changes.
    let _ = a_reply.send(Err(Refusal::GitRefused {
        code: codes::GIT_DISCARD_STAGED.into(),
        message: "a.txt has staged changes.".into(),
        paths: vec!["a.txt".into()],
    }));
    cx.run_until_parked();
    draw(cx);

    // b.txt's dialog is still its first dialog, and confirming it sends b.txt alone.
    rig.shell.read_with(cx, |shell, _| {
        let flow = shell
            .source_control
            .discard
            .as_ref()
            .expect("b.txt's dialog is open");
        assert_eq!(flow.stage(), &DiscardStage::Confirm);
        assert_eq!(flow.copy().title, "Discard changes in b.txt?");
    });
    rig.shell.update(cx, |shell, cx| shell.confirm_discard(cx));
    cx.run_until_parked();
    let Command::GitDiscard { params, .. } = next(&mut rig) else {
        panic!("a discard call for b.txt was expected");
    };
    assert_eq!(params.paths, ["b.txt"]);
    assert!(!params.include_staged, "b.txt's staged changes stay");
}
