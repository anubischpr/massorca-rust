//! The Work panel on a scripted service: its strip and panes for the selected workspace, the calls
//! they make, and the attach streams they follow. Each call is answered by hand, and no window
//! opens.

use std::time::Instant;

use gpui::{TestAppContext, VisualTestContext};
use massorca_client::AttachUpdate;
use massorca_protocol::methods::terminals::{
    AttachMode, Terminal, TerminalAttachParams, TerminalCreateParams, TerminalExit, TerminalState,
};
use massorca_ui::components::work_panel_tabs::WorkTabsAction;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::oneshot;

use super::service_tests::{Rig, click, connected, draw, rig};
use super::work_panel::PANE_WAIT_FRAMES;
use super::*;
use crate::model::work_panel::{NEW_TERMINAL_COLS, NEW_TERMINAL_ROWS};
use crate::service::{
    Command, ConnectionState, Refusal, Script, StoreUpdate, StreamEnd, TerminalCommand,
    TerminalStreamItem, TerminalWrite,
};

type TerminalReply = oneshot::Sender<Result<Terminal, Refusal>>;
type AttachReply = oneshot::Sender<Result<UnboundedReceiver<TerminalStreamItem>, Refusal>>;

/// A live terminal of workspace w1, as the service lists it.
fn live(id: &str) -> Terminal {
    terminal(id, TerminalState::Live, None)
}

fn exited(id: &str, code: i64) -> Terminal {
    terminal(
        id,
        TerminalState::Exited,
        Some(TerminalExit {
            code: Some(code),
            ..crate::service::fixtures::base_terminal_exit()
        }),
    )
}

fn terminal(id: &str, state: TerminalState, exit: Option<TerminalExit>) -> Terminal {
    Terminal {
        id: id.to_owned(),
        state,
        cols: 120,
        rows: 40,
        exit,
        ..crate::service::fixtures::base_terminal()
    }
}

/// The first call of the work panel that `pick` takes; the calls before it are passed over.
fn find_call<T>(script: &mut Script, pick: impl Fn(Command) -> Option<T>) -> Option<T> {
    while let Some(command) = script.next() {
        if let Some(found) = pick(command) {
            return Some(found);
        }
    }
    None
}

fn create_call(script: &mut Script) -> Option<(TerminalCreateParams, TerminalReply)> {
    find_call(script, |command| match command {
        Command::Terminal(TerminalCommand::Create { params, reply }) => Some((params, reply)),
        _ => None,
    })
}

fn attach_call(script: &mut Script) -> Option<(TerminalAttachParams, AttachReply)> {
    find_call(script, |command| match command {
        Command::Terminal(TerminalCommand::Attach { params, reply }) => Some((params, reply)),
        _ => None,
    })
}

fn close_call(script: &mut Script) -> Option<(String, TerminalReply)> {
    find_call(script, |command| match command {
        Command::Terminal(TerminalCommand::Close { terminal, reply }) => Some((terminal, reply)),
        _ => None,
    })
}

fn input_call(script: &mut Script) -> Option<(String, Vec<u8>)> {
    find_call(script, |command| match command {
        Command::Terminal(TerminalCommand::Write(TerminalWrite::Input { terminal, bytes })) => {
            Some((terminal, bytes))
        }
        _ => None,
    })
}

/// A shell on the start page of workspace w1, with the service connected.
fn start_page(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.shell.update(cx, |shell, _| {
        shell.model.select_workspace(Some("w1".to_owned()))
    });
    cx.run_until_parked();
    draw(cx);
    (rig, cx)
}

/// "Open terminal" on the start page, answered with the live terminal t1 the store then lists.
fn open_terminal(rig: &mut Rig, cx: &mut VisualTestContext) {
    let (params, reply) = start_terminal_create(rig, cx);
    assert_eq!(params.workspace, "w1");
    let terminal = live("t1");
    rig.updates
        .send(StoreUpdate::TerminalChanged(terminal.clone()))
        .expect("the store listens");
    reply
        .send(Ok(terminal))
        .expect("the shell waits for its answer");
    cx.run_until_parked();
}

/// One frame as the platform runs it: the callbacks scheduled for it, then the paint.
fn frame(cx: &mut VisualTestContext) {
    frame_callbacks(cx);
    draw(cx);
    cx.run_until_parked();
}

/// Runs the callbacks scheduled for the next frame, without painting it.
fn frame_callbacks(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.simulate_next_frame(cx);
    });
}

/// "Open terminal" on the start page, run through the frames its create waits for: the pane paints
/// its size first, then the create goes out at that grid.
fn start_terminal_create(
    rig: &mut Rig,
    cx: &mut VisualTestContext,
) -> (TerminalCreateParams, TerminalReply) {
    click(cx, "start-terminal");
    frame(cx);
    frame(cx);
    create_call(&mut rig.script).expect("a terminal.create call once the pane has painted")
}

/// Answers the attach of the selected view with a stream the test feeds.
fn attach_with_stream(
    script: &mut Script,
) -> (TerminalAttachParams, UnboundedSender<TerminalStreamItem>) {
    let (params, reply) = attach_call(script).expect("a terminal.attach call");
    let (items, received) = unbounded_channel();
    reply
        .send(Ok(received))
        .expect("the view waits for its attach");
    (params, items)
}

/// Feeds the attach a snapshot that shows "hello" on the first row.
fn show_hello(items: &UnboundedSender<TerminalStreamItem>) {
    items
        .send(TerminalStreamItem::Update(AttachUpdate::Attached {
            terminal: live("t1"),
            mode: AttachMode::Snapshot,
            offset: 0,
        }))
        .expect("the view listens");
    items
        .send(TerminalStreamItem::Update(AttachUpdate::Snapshot {
            offset: 0,
            cols: 120,
            rows: 40,
            bytes: b"hello".to_vec(),
        }))
        .expect("the view listens");
}

/// Opens t1 and shows its pane with "hello" on it; returns the stream that feeds the pane.
fn open_and_show(rig: &mut Rig, cx: &mut VisualTestContext) -> UnboundedSender<TerminalStreamItem> {
    open_terminal(rig, cx);
    let (_, items) = attach_with_stream(&mut rig.script);
    show_hello(&items);
    cx.run_until_parked();
    draw(cx);
    items
}

/// A live terminal the service created at `cols` x `rows`, as the store then lists it.
fn live_at(id: &str, cols: u16, rows: u16) -> Terminal {
    Terminal {
        cols,
        rows,
        ..live(id)
    }
}

fn resize_call(script: &mut Script) -> Option<String> {
    find_call(script, |command| match command {
        Command::Terminal(TerminalCommand::Write(TerminalWrite::Resize { terminal, .. })) => {
            Some(terminal)
        }
        _ => None,
    })
}

#[gpui::test]
fn a_new_work_panel_terminal_opens_at_the_grid_its_pane_applies_without_a_resize(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = start_page(cx);
    // t1 is attached but no output is fed, so its emulator keeps the grid its pane applied on the first draw.
    open_terminal(&mut rig, cx);
    let (_, _items) = attach_with_stream(&mut rig.script);
    cx.run_until_parked();
    draw(cx);
    // t1's pane has painted, so the Work panel knows its size; the grid its surface applied is the one it fits.
    let applied = rig.shell.read_with(cx, |shell, cx| {
        let feed = shell
            .terminal_views
            .get("t1")
            .expect("t1 has a view")
            .surface
            .read(cx)
            .feed();
        (feed.columns() as u16, feed.lines() as u16)
    });
    while rig.script.next().is_some() {}

    cx.update(|window, cx| {
        rig.shell.update(cx, |shell, cx| {
            shell.work_action(WorkTabsAction::Add, window, cx)
        })
    });
    cx.run_until_parked();
    let (params, reply) = create_call(&mut rig.script).expect("a terminal.create call");
    assert_eq!(
        (params.cols, params.rows),
        (Some(applied.0), Some(applied.1)),
        "the second terminal opens at the grid t1's pane applied"
    );

    let terminal = live_at("t2", applied.0, applied.1);
    rig.updates
        .send(StoreUpdate::TerminalChanged(terminal.clone()))
        .expect("the store listens");
    reply
        .send(Ok(terminal))
        .expect("the shell waits for its answer");
    cx.run_until_parked();
    draw(cx);
    cx.run_until_parked();

    assert!(
        resize_call(&mut rig.script).is_none(),
        "the first draw fits the grid the service already holds"
    );
    rig.shell.read_with(cx, |shell, cx| {
        let feed = shell
            .terminal_views
            .get("t2")
            .expect("t2 has a view")
            .surface
            .read(cx)
            .feed();
        assert_eq!(
            (feed.columns() as u16, feed.lines() as u16),
            applied,
            "the surface applies the grid terminal.create sent"
        );
    });
}

#[gpui::test]
fn a_fresh_run_opens_its_first_terminal_at_the_grid_its_pane_applies_without_a_resize(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = start_page(cx);
    // No pane has painted yet in this run, so the create waits for the first paint and sends the grid it measured.
    let (params, reply) = start_terminal_create(&mut rig, cx);
    let (cols, rows) = (params.cols.unwrap(), params.rows.unwrap());
    let terminal = live_at("t1", cols, rows);
    rig.updates
        .send(StoreUpdate::TerminalChanged(terminal.clone()))
        .expect("the store listens");
    reply
        .send(Ok(terminal))
        .expect("the shell waits for its answer");
    cx.run_until_parked();
    draw(cx);
    cx.run_until_parked();

    assert!(
        resize_call(&mut rig.script).is_none(),
        "the first draw fits the grid the service already holds"
    );
    rig.shell.read_with(cx, |shell, cx| {
        let feed = shell
            .terminal_views
            .get("t1")
            .expect("t1 has a view")
            .surface
            .read(cx)
            .feed();
        assert_eq!(
            (feed.columns() as u16, feed.lines() as u16),
            (cols, rows),
            "the surface applies the grid terminal.create sent"
        );
    });
}

#[gpui::test]
fn a_pane_that_never_paints_opens_its_terminal_at_the_fixed_size(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page(cx);
    // The Work panel stays hidden, so its pane never paints and no size arrives: the wait runs out.
    cx.update(|window, cx| {
        rig.shell.update(cx, |shell, cx| {
            shell.work_action(WorkTabsAction::Add, window, cx)
        })
    });
    cx.run_until_parked();
    for _ in 0..PANE_WAIT_FRAMES {
        frame_callbacks(cx);
    }
    cx.run_until_parked();
    assert!(
        create_call(&mut rig.script).is_none(),
        "the create waits while its frames run out"
    );
    frame_callbacks(cx);
    cx.run_until_parked();
    let (params, _) = create_call(&mut rig.script).expect("a terminal.create call");
    assert_eq!(
        (params.cols, params.rows),
        (Some(NEW_TERMINAL_COLS), Some(NEW_TERMINAL_ROWS)),
        "without a measured pane the terminal opens at the fixed size"
    );
}

#[gpui::test]
fn open_terminal_opens_the_work_panel_and_shows_the_terminal_it_creates(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page(cx);
    let _items = open_and_show(&mut rig, cx);

    rig.shell.read_with(cx, |shell, cx| {
        assert!(shell.model().region_visible(Region::WorkPanel));
        let view = shell.terminal_views.get("t1").expect("t1 has a view");
        assert_eq!(view.surface.read(cx).feed().row_text(0), "hello");
        assert_eq!(shell.model().snapshot().status.terminals, 1);
    });
}

#[gpui::test]
fn the_pane_declares_that_it_answers_the_queries_it_forwards(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page(cx);
    open_terminal(&mut rig, cx);
    let (params, _items) = attach_with_stream(&mut rig.script);
    assert_eq!(params.terminal, "t1");
    assert_eq!(
        params.answers_queries,
        Some(true),
        "the pane forwards its replies"
    );
}

#[gpui::test]
fn typing_in_the_pane_sends_the_keys_to_the_terminal(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page(cx);
    let _items = open_and_show(&mut rig, cx);

    cx.simulate_keystrokes("a");
    cx.run_until_parked();
    assert_eq!(
        input_call(&mut rig.script),
        Some(("t1".to_owned(), b"a".to_vec()))
    );
}

#[gpui::test]
fn keep_running_hides_the_pane_and_leaves_the_tab_and_the_process(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page(cx);
    let items = open_and_show(&mut rig, cx);

    click(cx, "close-0");
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(shell.stop_prompt.as_deref(), Some("t1"))
    });
    click(cx, "dialog-cancel");

    rig.shell.read_with(cx, |shell, _| {
        assert!(
            !shell.terminal_views.contains_key("t1"),
            "the pane is hidden"
        );
        assert_eq!(
            shell.model().terminal_selection().selected("w1"),
            Some("t1")
        );
        assert!(shell.stop_prompt.is_none());
    });
    assert!(items.is_closed(), "dropping the view detaches the attach");
    assert!(close_call(&mut rig.script).is_none(), "nothing was closed");
    draw(cx);
    assert!(cx.debug_bounds("work-tab-0").is_some(), "the tab stays");
}

#[gpui::test]
fn a_kept_terminal_still_counts_in_the_status_bar(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page(cx);
    let _items = open_and_show(&mut rig, cx);

    click(cx, "close-0");
    click(cx, "dialog-cancel");
    // Any store change recomputes the status bar; the store still holds the kept terminal.
    rig.updates
        .send(StoreUpdate::Connection(connected()))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);

    rig.shell.read_with(cx, |shell, cx| {
        assert!(
            !shell.terminal_views.contains_key("t1"),
            "the pane is hidden"
        );
        assert!(
            shell
                .store
                .read(cx)
                .state()
                .terminals
                .iter()
                .any(|terminal| terminal.id == "t1"),
            "the store still holds the kept terminal"
        );
        let status = &shell.model().snapshot().status;
        assert_eq!(status.terminals, 1, "the kept terminal may still run");
        assert_eq!(status.terminals_text(), "1 terminal");
    });
    assert!(cx.debug_bounds("status-terminals").is_some());
}

#[gpui::test]
fn stop_process_closes_the_terminal_and_its_tab_goes(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page(cx);
    let _items = open_and_show(&mut rig, cx);

    click(cx, "close-0");
    click(cx, "dialog-action");
    let (terminal, reply) = close_call(&mut rig.script).expect("a terminal.close call");
    assert_eq!(terminal, "t1");
    reply
        .send(Ok(live("t1")))
        .expect("the shell waits for its answer");
    rig.updates
        .send(StoreUpdate::TerminalGone("t1".to_owned()))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);

    rig.shell.read_with(cx, |shell, _| {
        assert!(!shell.terminal_views.contains_key("t1"));
        assert_eq!(shell.model().terminal_selection().selected("w1"), None);
    });
    assert!(
        cx.debug_bounds("work-tab-0").is_none(),
        "the strip is empty"
    );
}

#[gpui::test]
fn an_exited_terminal_shows_its_exit_row_and_restart_opens_another(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page(cx);
    let items = open_and_show(&mut rig, cx);
    items
        .send(TerminalStreamItem::Update(AttachUpdate::Exited {
            terminal: exited("t1", 0),
        }))
        .expect("the view listens");
    items
        .send(TerminalStreamItem::Ended(StreamEnd::Finished))
        .expect("the view listens");
    cx.run_until_parked();
    draw(cx);

    rig.shell.read_with(cx, |shell, cx| {
        let view = shell.terminal_views.get("t1").expect("t1 has a view");
        assert!(view.surface.read(cx).feed().exit().is_some());
    });
    click(cx, "terminal-restart");
    let (params, reply) = create_call(&mut rig.script).expect("a terminal.create call");
    assert_eq!(params.workspace, "w1");
    rig.updates
        .send(StoreUpdate::TerminalChanged(live("t2")))
        .expect("the store listens");
    reply
        .send(Ok(live("t2")))
        .expect("the shell waits for its answer");
    cx.run_until_parked();

    let (attached, _) = attach_with_stream(&mut rig.script);
    assert_eq!(attached.terminal, "t2");
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.model().terminal_selection().selected("w1"),
            Some("t2")
        );
    });
}

#[gpui::test]
fn an_unverifiable_terminal_says_it_may_still_run(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page(cx);
    let _items = open_and_show(&mut rig, cx);

    let mut unreachable = live("t1");
    unreachable.state = TerminalState::Unverifiable;
    rig.updates
        .send(StoreUpdate::TerminalChanged(unreachable))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("work-unreachable").is_some());
}

#[gpui::test]
fn a_lost_link_attaches_again_without_an_offset_once_the_service_is_back(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page(cx);
    let items = open_and_show(&mut rig, cx);

    let down = ConnectionState::Reconnecting {
        attempt: 1,
        retry_at: Instant::now(),
    };
    rig.script.set_connection(down.clone());
    rig.updates
        .send(StoreUpdate::Connection(down))
        .expect("the store listens");
    items
        .send(TerminalStreamItem::Ended(StreamEnd::Lost))
        .expect("the view listens");
    cx.run_until_parked();
    assert!(
        attach_call(&mut rig.script).is_none(),
        "nothing attaches while the link is down"
    );

    rig.script.set_connection(connected());
    rig.updates
        .send(StoreUpdate::Connection(connected()))
        .expect("the store listens");
    cx.run_until_parked();
    let (params, _) = attach_call(&mut rig.script).expect("the view attaches again");
    assert_eq!(params.since, None, "a snapshot, not a resume");
}

#[gpui::test]
fn a_gap_in_the_output_attaches_again_at_once(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page(cx);
    let items = open_and_show(&mut rig, cx);

    items
        .send(TerminalStreamItem::Update(AttachUpdate::Output {
            offset: 1_000,
            bytes: b"x".to_vec(),
        }))
        .expect("the view listens");
    cx.run_until_parked();
    let (params, _) = attach_call(&mut rig.script).expect("the view attaches again");
    assert_eq!(params.since, None);
}

#[gpui::test]
fn a_refused_attach_says_why_and_draws_no_pane(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page(cx);
    let (_, reply) = start_terminal_create(&mut rig, cx);
    rig.updates
        .send(StoreUpdate::TerminalChanged(live("t1")))
        .expect("the store listens");
    reply
        .send(Ok(live("t1")))
        .expect("the shell waits for its answer");
    cx.run_until_parked();

    let (_, attach) = attach_call(&mut rig.script).expect("a terminal.attach call");
    attach
        .send(Err(Refusal::Failed {
            code: "terminal.not_found".to_owned(),
            message: "There is no such terminal.".to_owned(),
        }))
        .expect("the view waits for its attach");
    cx.run_until_parked();
    draw(cx);

    assert!(cx.debug_bounds("work-attach-failed").is_some());
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.terminal_views["t1"].failure.as_deref(),
            Some("There is no such terminal.")
        );
    });
}
