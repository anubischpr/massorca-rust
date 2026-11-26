//! The close confirm on a service store: a close while a session runs or waits asks first, Cancel keeps the window,
//! and Close window goes without a call to the service. With nothing at stake the window closes at once.

use gpui::{TestAppContext, VisualContext, VisualTestContext};
use massorca_protocol::methods::agents::{Activity, AgentSession};

use super::close_confirm::sessions_line;
use super::service_tests::{Rig, click, draw, rig};
use crate::model::close_confirm::CloseRisk;
use crate::service::StoreUpdate;
use crate::service::fixtures::agent;

/// A live session on workspace `w1` whose agent reports that it is working.
fn working(id: &str) -> AgentSession {
    let mut session = agent(id, "w1", 10);
    session.activity = Some(Activity {
        at_ms: 5,
        ..crate::service::fixtures::base_activity()
    });
    session
}

/// A connected shell on project `p1` with these sessions reported by the service.
fn shell_with(
    cx: &mut TestAppContext,
    sessions: Vec<AgentSession>,
) -> (Rig, &mut VisualTestContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.updates
        .send(StoreUpdate::AgentsResync(sessions))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    (rig, cx)
}

/// Asks to close the way the title-bar close button does; a Linux CI frame hides that button.
fn press_close(rig: &Rig, cx: &mut VisualTestContext) {
    rig.shell
        .update_in(cx, |shell, window, cx| shell.request_close(window, cx));
    cx.run_until_parked();
}

#[gpui::test]
fn a_close_with_a_running_session_asks_and_cancel_keeps_the_window(cx: &mut TestAppContext) {
    let (rig, cx) = shell_with(cx, vec![working("s1")]);
    let window = cx.window_handle();
    press_close(&rig, cx);
    draw(cx);
    assert!(
        cx.debug_bounds("dialog-panel").is_some(),
        "the confirm is on screen"
    );
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.close_confirm,
            Some(CloseRisk {
                running_sessions: 1
            })
        );
    });

    click(cx, "dialog-cancel");
    rig.shell.read_with(cx, |shell, _| {
        assert!(
            shell.close_confirm.is_none(),
            "Cancel takes the confirm away"
        );
    });
    assert!(
        cx.windows().contains(&window),
        "Cancel keeps the window open"
    );
}

#[gpui::test]
fn close_window_removes_the_window_and_sends_the_service_no_call(cx: &mut TestAppContext) {
    let (mut rig, cx) = shell_with(cx, vec![working("s1"), working("s2")]);
    let window = cx.window_handle();
    press_close(&rig, cx);
    draw(cx);
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.close_confirm,
            Some(CloseRisk {
                running_sessions: 2
            })
        );
    });

    click(cx, "dialog-action");
    assert!(
        !cx.windows().contains(&window),
        "Close window removes the window"
    );
    assert!(
        rig.script.next().is_none(),
        "closing sends the service no call"
    );
}

#[gpui::test]
fn a_close_with_nothing_running_closes_at_once_without_a_confirm(cx: &mut TestAppContext) {
    let (rig, cx) = shell_with(cx, vec![agent("idle", "w1", 10)]);
    let window = cx.window_handle();
    press_close(&rig, cx);
    rig.shell.read_with(cx, |shell, _| {
        assert!(
            shell.close_confirm.is_none(),
            "a session that is not running asks nothing"
        );
    });
    assert!(!cx.windows().contains(&window), "the window closed at once");
}

#[gpui::test]
fn the_os_close_request_waits_on_the_confirm_while_a_session_runs(cx: &mut TestAppContext) {
    let (rig, cx) = shell_with(cx, vec![working("s1")]);
    assert!(
        !cx.simulate_close(),
        "the close the OS asks for is held back"
    );
    draw(cx);
    assert!(
        cx.debug_bounds("dialog-panel").is_some(),
        "the confirm is on screen"
    );
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.close_confirm,
            Some(CloseRisk {
                running_sessions: 1
            })
        );
    });
}

#[gpui::test]
fn the_os_close_request_goes_ahead_with_nothing_running(cx: &mut TestAppContext) {
    let (rig, cx) = shell_with(cx, Vec::new());
    assert!(
        cx.simulate_close(),
        "with nothing running the close goes ahead"
    );
    rig.shell.read_with(cx, |shell, _| {
        assert!(shell.close_confirm.is_none(), "no confirm opens");
    });
}

#[test]
fn the_confirm_counts_its_sessions_in_plain_words() {
    assert_eq!(sessions_line(1), "1 session is running or waiting for you.");
    assert_eq!(
        sessions_line(3),
        "3 sessions are running or waiting for you."
    );
}
