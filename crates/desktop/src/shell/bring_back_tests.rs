//! The bring-back confirm on a service store: a finished child opens it, its choices are sent with its call, a
//! refusal keeps the confirm open under the plain sentence, and the answer stays on screen until the person closes
//! it. A scripted handle answers the call by hand, so no service runs.

use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    Activity, ActivitySource, ActivityState, BringBackResult, Outcome,
};

use super::bring_back::{confirm_body, result_lines};
use super::service_tests::{Rig, click, draw, rig};
use crate::model::agent_sessions::BringBackPrompt;
use crate::service::fixtures::{agent, workspace};
use crate::service::{Command, Refusal, StoreUpdate};

/// A connected shell with a finished child `c` under its parent `p`, which works on the branch feat/fix-login.
fn finished_child(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    let mut child = agent("c", "w2", 20);
    child.parent_id = Some("p".into());
    child.title = Some("Feed endpoints".into());
    child.activity = Some(Activity {
        state: ActivityState::Done,
        outcome: Some(Outcome::Success),
        source: ActivitySource::Hook,
        at_ms: 5,
        seen_at_ms: None,
    });
    rig.updates
        .send(StoreUpdate::WorkspaceChanged(workspace(
            "w2",
            "p1",
            "catalog-api",
            2,
        )))
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::AgentsResync(vec![agent("p", "w1", 10), child]))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    (rig, cx)
}

#[gpui::test]
fn a_finished_child_is_brought_back_once_confirmed_and_a_refusal_keeps_the_confirm(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = finished_child(cx);
    rig.shell
        .update(cx, |shell, cx| shell.ask_bring_back("c", cx));
    draw(cx);
    assert!(
        cx.debug_bounds("dialog-panel").is_some(),
        "the confirm is on screen"
    );
    rig.shell.read_with(cx, |shell, _| {
        let prompt = &shell
            .bring_back
            .as_ref()
            .expect("the confirm is open")
            .prompt;
        assert_eq!(prompt.title, "Bring back Feed endpoints?");
        assert_eq!(
            prompt.body,
            "Merges into feat/fix-login without fast-forward. Nothing is pushed."
        );
    });

    click(cx, "dialog-action");
    let Some(Command::BringBack { params, reply }) = rig.script.next() else {
        panic!("an agent.bring_back call was expected");
    };
    assert_eq!(
        (params.child.as_str(), params.merge, params.prompt),
        ("c", true, true)
    );
    rig.shell.read_with(cx, |shell, _| {
        assert!(shell.bring_back.as_ref().expect("open").sending);
    });

    // A refusal keeps the confirm open, free again, under the plain sentence for its code.
    reply
        .send(Err(Refusal::Failed {
            code: codes::AGENT_MERGE_CONFLICT.to_owned(),
            message: "the service's own text".to_owned(),
        }))
        .expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        let confirm = shell.bring_back.as_ref().expect("still open");
        assert!(!confirm.sending);
        assert_eq!(
            confirm.error.as_deref(),
            Some("The merge stopped on conflicts, so it was aborted and nothing changed.")
        );
    });

    // Confirming again sends again, and the answer stays on screen with what the service did.
    click(cx, "dialog-action");
    let Some(Command::BringBack { reply, .. }) = rig.script.next() else {
        panic!("a second agent.bring_back call was expected");
    };
    reply
        .send(Ok(BringBackResult {
            child: agent("c", "w1", 20),
            merged: true,
            merge_commit: Some("abc1234".to_owned()),
            prompt_delivered: true,
            note: None,
        }))
        .expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        let confirm = shell
            .bring_back
            .as_ref()
            .expect("the answer stays on screen");
        assert_eq!(
            confirm.result.as_ref().map(|result| result.merged),
            Some(true)
        );
        assert_eq!(confirm.error, None);
    });
}

#[gpui::test]
fn the_merge_and_tell_choices_are_sent_and_the_service_note_stays_until_done(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = finished_child(cx);
    rig.shell
        .update(cx, |shell, cx| shell.ask_bring_back("c", cx));
    rig.shell.update(cx, |shell, cx| {
        shell.set_bring_back_merge(false, cx);
        shell.set_bring_back_tell_parent(false, cx);
    });
    click(cx, "dialog-action");
    let Some(Command::BringBack { params, reply }) = rig.script.next() else {
        panic!("an agent.bring_back call was expected");
    };
    assert_eq!((params.merge, params.prompt), (false, false));

    reply
        .send(Ok(BringBackResult {
            child: agent("c", "w1", 20),
            merged: false,
            merge_commit: None,
            prompt_delivered: false,
            note: Some("the parent's run is not live, so no prompt was sent".to_owned()),
        }))
        .expect("the shell waits");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, _| {
        let confirm = shell
            .bring_back
            .as_ref()
            .expect("the answer stays on screen");
        assert_eq!(
            confirm
                .result
                .as_ref()
                .and_then(|result| result.note.as_deref()),
            Some("the parent's run is not live, so no prompt was sent")
        );
    });

    click(cx, "dialog-cancel");
    rig.shell.read_with(cx, |shell, _| {
        assert!(shell.bring_back.is_none(), "Done closes it")
    });
}

#[test]
fn a_bring_back_that_does_not_merge_says_so_in_its_confirm() {
    let prompt = BringBackPrompt {
        child: "c".to_owned(),
        title: "Bring back Feed endpoints?".to_owned(),
        body: "Merges into feat/fix-login without fast-forward. Nothing is pushed.".to_owned(),
    };
    assert_eq!(confirm_body(&prompt, true), prompt.body);
    assert_eq!(
        confirm_body(&prompt, false),
        "Nothing is merged. The child is marked as brought back."
    );
}

#[test]
fn the_answer_says_what_the_merge_did_and_whether_the_parent_was_told() {
    let merged = BringBackResult {
        child: agent("c", "w1", 20),
        merged: true,
        merge_commit: Some("abc1234def5678".to_owned()),
        prompt_delivered: true,
        note: None,
    };
    assert_eq!(
        result_lines(&merged),
        [
            "Merged as commit abc1234.",
            "The parent session was told about it."
        ]
    );
    let kept = BringBackResult {
        merged: false,
        merge_commit: None,
        prompt_delivered: false,
        ..merged
    };
    assert_eq!(
        result_lines(&kept),
        ["Not merged.", "The parent session was not told."]
    );
}
