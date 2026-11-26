//! The terminal of a session a paired host runs (STAT-16), in the rig: its attach goes to the host, its keys and resizes
//! reach that attach and never this computer's service, and a host that cannot be reached keeps the screen and says so.

use gpui::{TestAppContext, VisualTestContext};
use massorca_client::AttachUpdate;
use massorca_protocol::methods::agents::AgentSession;
use massorca_protocol::methods::terminals::{AttachMode, Terminal};

use super::service_tests::{Rig, click, draw, rig};
use crate::service::fixtures::{agent, workspace};
use crate::service::{
    Command, HostAgents, HostAttach, HostSessionsRead, HostTerminalItem, HostWrite, Refusal,
    Script, StoreUpdate,
};

/// Session r1 of host `vps`, running in its terminal t-remote.
fn remote_session() -> AgentSession {
    let mut session = agent("r1", "w1", 10);
    session.terminal_id = Some("t-remote".to_owned());
    session
}

/// A shell on the connected service, with host `vps` listing session r1, and r1 open in the Canvas.
fn open_remote(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.updates
        .send(StoreUpdate::HostSessions(HostSessionsRead {
            host: "vps".to_owned(),
            request: 1,
            answer: Some(HostAgents {
                sessions: vec![remote_session()],
                workspaces: vec![workspace("w1", "p1", "feat", 1)],
            }),
        }))
        .expect("the store listens");
    cx.run_until_parked();
    rig.shell
        .update(cx, |shell, _| shell.model.open_session("r1"));
    cx.run_until_parked();
    draw(cx);
    (rig, cx)
}

/// The host attach the view sent, if one waits in the script; other calls are passed over.
fn host_attach(script: &mut Script) -> Option<HostAttach> {
    while let Some(command) = script.next() {
        if let Command::HostTerminalAttach(attach) = command {
            return Some(attach);
        }
    }
    None
}

/// The local terminal calls the script holds: none may carry a remote session's keys, resizes or attach.
fn local_terminal_calls(script: &mut Script) -> usize {
    let mut calls = 0;
    while let Some(command) = script.next() {
        if matches!(command, Command::Terminal(_)) {
            calls += 1;
        }
    }
    calls
}

/// A live terminal of the host, as its attach describes it.
fn live(id: &str) -> Terminal {
    Terminal {
        id: id.to_owned(),
        cols: 120,
        rows: 40,
        ..crate::service::fixtures::base_terminal()
    }
}

/// Feeds the attach a snapshot that shows `text` on the first row, as the host's attach does on connect.
fn show(items: &tokio::sync::mpsc::UnboundedSender<HostTerminalItem>, text: &[u8]) {
    items
        .send(HostTerminalItem::Update(AttachUpdate::Attached {
            terminal: live("t-remote"),
            mode: AttachMode::Snapshot,
            offset: 0,
        }))
        .expect("the view listens");
    items
        .send(HostTerminalItem::Update(AttachUpdate::Snapshot {
            offset: 0,
            cols: 120,
            rows: 40,
            bytes: text.to_vec(),
        }))
        .expect("the view listens");
}

/// Whether the remote terminal's first row shows `text`.
fn first_row_shows(rig: &Rig, cx: &mut VisualTestContext, text: &str) -> bool {
    rig.shell.read_with(cx, |shell, cx| {
        shell
            .remote_terminal
            .as_ref()
            .is_some_and(|view| view.surface.read(cx).feed().row_text(0).contains(text))
    })
}

#[gpui::test]
fn a_remote_session_attaches_its_terminal_on_its_host_when_its_view_is_drawn(
    cx: &mut TestAppContext,
) {
    let (mut rig, _cx) = open_remote(cx);
    let attach = host_attach(&mut rig.script).expect("the view attaches the host's terminal");
    assert_eq!(attach.host, "vps");
    assert_eq!(attach.terminal, "t-remote");
    assert_eq!(
        local_terminal_calls(&mut rig.script),
        0,
        "this computer's service attaches nothing for the session"
    );
}

#[gpui::test]
fn typing_in_a_remote_session_goes_to_its_host_and_never_to_this_computers_service(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = open_remote(cx);
    let mut attach = host_attach(&mut rig.script).expect("the view attaches the host's terminal");
    let focus = rig.shell.read_with(cx, |shell, cx| {
        shell
            .remote_terminal
            .as_ref()
            .expect("the remote terminal is attached")
            .surface
            .read(cx)
            .focus_handle()
            .clone()
    });
    cx.update(|window, cx| window.focus(&focus, cx));

    cx.simulate_keystrokes("a");
    cx.run_until_parked();
    // The pane reports the grid it paints before any key, so a resize may come first: both go to the host.
    let writes: Vec<HostWrite> = std::iter::from_fn(|| attach.writes.try_recv().ok()).collect();
    assert!(
        writes.contains(&HostWrite::Input(b"a".to_vec())),
        "the key is typed into the host's terminal: {writes:?}"
    );
    assert!(
        writes
            .iter()
            .any(|write| matches!(write, HostWrite::Resize { .. })),
        "the pane's grid is sent to the host: {writes:?}"
    );
    assert_eq!(
        local_terminal_calls(&mut rig.script),
        0,
        "this computer's service was sent no terminal call"
    );
}

#[gpui::test]
fn a_lost_host_keeps_the_screen_and_says_so_until_its_attach_is_back(cx: &mut TestAppContext) {
    let (mut rig, cx) = open_remote(cx);
    let attach = host_attach(&mut rig.script).expect("the view attaches the host's terminal");
    show(&attach.items, b"hello");
    cx.run_until_parked();
    draw(cx);
    assert!(first_row_shows(&rig, cx, "hello"));

    attach
        .items
        .send(HostTerminalItem::Unreachable)
        .expect("the view listens");
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("remote-unreachable").is_some(),
        "the line says the host cannot be reached"
    );
    assert!(
        first_row_shows(&rig, cx, "hello"),
        "the last screen stays while the host is out of contact"
    );

    show(&attach.items, b"back");
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("remote-unreachable").is_none(),
        "the line goes once the attach is back"
    );
}

#[gpui::test]
fn a_host_that_refuses_the_terminal_shows_why_and_retry_attaches_again(cx: &mut TestAppContext) {
    let (mut rig, cx) = open_remote(cx);
    let attach = host_attach(&mut rig.script).expect("the view attaches the host's terminal");
    attach
        .items
        .send(HostTerminalItem::Refused(Refusal::unavailable(
            "The terminal is gone.",
        )))
        .expect("the view listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("session-attach-failed").is_some());

    click(cx, "button:session-retry-attach");
    draw(cx);
    let again = host_attach(&mut rig.script).expect("Retry attaches the host's terminal again");
    assert_eq!(again.terminal, "t-remote");
}

#[gpui::test]
fn the_grid_a_remote_terminal_shows_is_the_grid_a_new_session_starts_at(cx: &mut TestAppContext) {
    let (rig, cx) = open_remote(cx);
    rig.shell.update(cx, |shell, _| {
        shell.note_remote_grid("vps", "t-remote", 90, 25);
        assert_eq!(shell.session_grid(), Some((90, 25)));
        shell.note_remote_grid("gpu", "t-remote", 60, 20);
        assert_eq!(
            shell.session_grid(),
            Some((90, 25)),
            "another host's terminal is not the one shown"
        );
    });
}
