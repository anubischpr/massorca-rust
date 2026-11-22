//! The bring-back confirm on a service store: a finished child opens it, its confirm sends the call, a
//! refusal keeps the confirm open under the plain sentence, and the answer closes it. A scripted handle
//! answers the call by hand, so no service runs.

use gpui::TestAppContext;
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    Activity, ActivitySource, ActivityState, BringBackResult, Outcome,
};

use super::service_tests::{click, draw, rig};
use crate::service::fixtures::{agent, workspace};
use crate::service::{Command, Refusal, StoreUpdate};

#[gpui::test]
fn a_finished_child_is_brought_back_once_confirmed_and_a_refusal_keeps_the_confirm(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    let mut child = agent("c", "w2", 20);
    child.parent_id = Some("p".into());
    child.title = Some("Feed endpoints".into());
    child.activity = Some(Activity {
        state: ActivityState::Done,
        outcome: Some(Outcome::Success),
        source: ActivitySource::Hook,
        at_ms: 5,
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

    // Confirming again sends again, and the answer closes the confirm.
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
    rig.shell
        .read_with(cx, |shell, _| assert!(shell.bring_back.is_none()));
}
