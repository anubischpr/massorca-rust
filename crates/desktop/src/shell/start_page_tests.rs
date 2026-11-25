//! Element tests of the workspace start page: a workspace opens its page, and the project menu of the
//! page chooses the project's first workspace or none. A scripted handle stands in for the service.

use super::start_page::split_last_segment;
use super::*;
use crate::model::canvas::Canvas;
use crate::model::marketplace::UnavailableMarketplace;
use crate::model::regions::Region;
use crate::model::start_page::StartMode;
use crate::model::teams::{TeamsSource, UnavailableTeams};
use crate::service::fixtures::{agent, agent_kind, project, workspace};
use crate::service::{
    Command, ConnectionState, Refusal, Script, StoreUpdate, TerminalCommand, TerminalStreamItem,
};
use gpui::{AppContext, TestAppContext, VisualTestContext, px, size};
use massorca_profile::Profile;
use massorca_protocol::methods::agents::{AgentKind, AgentSession, AgentStartParams, PackageState};
use massorca_protocol::methods::projects::Project;
use massorca_protocol::methods::terminals::TerminalAttachParams;
use massorca_protocol::methods::workspaces::Workspace;
use massorca_ui::components::composer::{Composer, ComposerEvent};
use massorca_ui::tokens::{LAYOUT_CONVERSATION_READING, LAYOUT_SETTINGS_CONTROL_MAX, SPACE_4};
use massorca_ui::{Appearance, Theme};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::sync::oneshot;

struct Rig {
    shell: Entity<Shell>,
    _updates: UnboundedSender<StoreUpdate>,
    _script: Script,
}

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::set(Appearance::Dark, cx);
        bind_keys(cx);
    });
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// A connected shell on a scripted service with these projects and workspaces.
fn rig(
    cx: &mut TestAppContext,
    projects: Vec<Project>,
    workspaces: Vec<Workspace>,
) -> (Rig, &mut VisualTestContext) {
    setup(cx);
    let (updates, rx) = unbounded_channel();
    let (handle, script) = ServiceHandle::scripted();
    let (shell, cx) = cx.add_window_view(|window, cx| {
        let store = cx.new(|cx| ServiceStore::following(rx, cx));
        Shell::with_store(
            store,
            Some(handle),
            None,
            UnavailableTeams.teams(),
            Box::new(UnavailableMarketplace),
            window,
            cx,
        )
    });
    updates
        .send(StoreUpdate::Connection(ConnectionState::Connected {
            server: "massorca".into(),
            version: "0.1.0".into(),
        }))
        .expect("the store listens");
    updates
        .send(StoreUpdate::Resync {
            projects,
            workspaces,
        })
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    (
        Rig {
            shell,
            _updates: updates,
            _script: script,
        },
        cx,
    )
}

fn two_projects() -> (Vec<Project>, Vec<Workspace>) {
    (
        vec![project("p1", "alpha"), project("p2", "beta")],
        vec![
            workspace("w1", "p1", "fix-login", 1),
            workspace("w2", "p2", "feed", 2),
        ],
    )
}

fn canvas(rig: &Rig, cx: &mut VisualTestContext) -> Canvas {
    rig.shell.read_with(cx, |shell, _| shell.model().canvas())
}

/// Opens workspace `w1` on its start page and puts the focus in the page's composer.
fn focused_start_composer(rig: &Rig, cx: &mut VisualTestContext) -> Entity<Composer> {
    rig.shell
        .update(cx, |shell, _| shell.model.open_workspace("w1"));
    draw(cx);
    let composer = rig
        .shell
        .update(cx, |shell, cx| shell.start_composer("w1", cx));
    composer.update_in(cx, |composer, window, cx| composer.focus(window, cx));
    draw(cx);
    composer
}

fn draft_of(composer: &Entity<Composer>, cx: &VisualTestContext) -> String {
    composer.read_with(cx, |composer, cx| composer.draft(cx))
}

#[gpui::test]
fn the_start_page_composer_takes_typing_and_keeps_the_draft_across_redraws(
    cx: &mut TestAppContext,
) {
    let (projects, workspaces) = two_projects();
    let (rig, cx) = rig(cx, projects, workspaces);
    let composer = focused_start_composer(&rig, cx);
    cx.simulate_input("hello");
    draw(cx);
    assert_eq!(draft_of(&composer, cx), "hello");
}

#[gpui::test]
fn enter_keeps_the_draft_while_agent_sessions_are_not_available(cx: &mut TestAppContext) {
    let (projects, workspaces) = two_projects();
    let (rig, cx) = rig(cx, projects, workspaces);
    let composer = focused_start_composer(&rig, cx);
    cx.simulate_input("hi");
    cx.simulate_keystrokes("enter");
    assert_eq!(draft_of(&composer, cx), "hi");
}

#[gpui::test]
fn the_start_page_microphone_is_disabled_and_says_why(cx: &mut TestAppContext) {
    let (projects, workspaces) = two_projects();
    let (rig, cx) = rig(cx, projects, workspaces);
    let composer = focused_start_composer(&rig, cx);
    assert_eq!(
        composer.read_with(cx, |composer, _| composer
            .voice_unavailable_reason()
            .cloned()),
        Some("Voice input comes later.".into())
    );
}

#[gpui::test]
fn the_key_hints_start_at_the_composer_left_edge_not_centred(cx: &mut TestAppContext) {
    let (_rig, cx) = start_page_rig(cx, Vec::new());
    draw(cx);
    let column = cx
        .debug_bounds("start-column")
        .expect("the page column is drawn");
    let enter = cx
        .debug_bounds("start-key-hint-Enter")
        .expect("the Enter hint is drawn");
    let column_left = f32::from(column.origin.x);
    let enter_left = f32::from(enter.origin.x);
    assert!(
        enter_left - column_left <= EDGE_TOLERANCE,
        "the Enter hint starts at {enter_left} px, away from the column's left edge at {column_left} px"
    );
}

#[gpui::test]
fn the_start_page_places_its_host_project_and_branch_in_one_card(cx: &mut TestAppContext) {
    let (_rig, cx) = start_page_rig(cx, Vec::new());
    draw(cx);
    for selector in [
        "start-placement",
        "start-host",
        "start-project",
        "start-branch",
    ] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector} is drawn");
    }
}

#[gpui::test]
fn the_placement_card_sits_on_the_composer_inset_twelve_px_from_each_side(cx: &mut TestAppContext) {
    let (_rig, cx) = start_page_rig(cx, Vec::new());
    draw(cx);
    let card = cx
        .debug_bounds("start-placement")
        .expect("the placement card is drawn");
    let column = cx
        .debug_bounds("composer-column")
        .expect("the composer column is drawn");
    let inset_left = f32::from(card.origin.x) - f32::from(column.origin.x);
    let inset_right =
        f32::from(column.origin.x + column.size.width) - f32::from(card.origin.x + card.size.width);
    let gap_above = f32::from(column.origin.y) - f32::from(card.origin.y + card.size.height);
    assert!(
        (inset_left - 12.0).abs() <= EDGE_TOLERANCE,
        "the card starts {inset_left} px inside the composer's left edge, not 12 px"
    );
    assert!(
        (inset_right - 12.0).abs() <= EDGE_TOLERANCE,
        "the card ends {inset_right} px inside the composer's right edge, not 12 px"
    );
    assert!(
        gap_above.abs() <= EDGE_TOLERANCE,
        "the card sits {gap_above} px above the composer, not on its top edge"
    );
}

#[gpui::test]
fn the_placement_pills_are_tab_stops_and_enter_opens_the_project_menu(cx: &mut TestAppContext) {
    let (projects, workspaces) = two_projects();
    let (rig, cx) = rig(cx, projects, workspaces);
    rig.shell
        .update(cx, |shell, _| shell.model.open_workspace("w1"));
    draw(cx);
    // A click focuses the off host pill and does nothing else; Tab then reaches the project pill.
    click(cx, "start-host");
    cx.update(|window, cx| window.focus_next(cx));
    // Enter is a keyboard click only when its press and release both reach the focused pill.
    let keystroke = gpui::Keystroke::parse("enter").expect("enter is a keystroke");
    cx.simulate_event(gpui::KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    cx.simulate_event(gpui::KeyUpEvent { keystroke });
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("menu-panel").is_some(),
        "Enter on the focused project pill opens the project menu"
    );
}

#[gpui::test]
fn an_off_pill_names_its_reason_when_keyboard_focus_reaches_it(cx: &mut TestAppContext) {
    let (projects, workspaces) = two_projects();
    let (rig, cx) = rig(cx, projects, workspaces);
    rig.shell
        .update(cx, |shell, _| shell.model.open_workspace("w1"));
    draw(cx);
    click(cx, "start-host");
    // Focus events reach the tip only in an active window; a key press makes the next focus a keyboard focus.
    cx.update(|window, _| window.activate_window());
    let keystroke = gpui::Keystroke::parse("f1").expect("f1 is a keystroke");
    cx.simulate_event(gpui::KeyDownEvent {
        keystroke: keystroke.clone(),
        is_held: false,
        prefer_character_input: false,
    });
    cx.simulate_event(gpui::KeyUpEvent { keystroke });
    cx.update(|window, cx| window.focus_next(cx));
    cx.update(|window, cx| window.focus_prev(cx));
    cx.run_until_parked();
    draw(cx);
    cx.run_until_parked();
    assert!(
        cx.debug_bounds("tooltip-panel").is_some(),
        "the host pill's reason shows when keyboard focus reaches it"
    );
}

#[gpui::test]
fn the_chosen_agent_stays_chosen_when_the_installed_kinds_are_listed_again(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = start_page_rig(
        cx,
        vec![agent_kind("claude", "Claude", PackageState::Enabled)],
    );
    rig._updates
        .send(StoreUpdate::AgentKindsResync(vec![
            agent_kind("claude", "Claude", PackageState::Enabled),
            agent_kind("codex", "Codex", PackageState::Enabled),
        ]))
        .expect("the store listens");
    cx.run_until_parked();
    let composer = composer_of(&rig, cx);
    let (offered, chosen) = composer.read_with(cx, |composer, cx| {
        let state = composer.picker().read(cx).state();
        (state.agents().len(), state.choice().model.to_string())
    });
    assert_eq!(offered, 2);
    assert_eq!(
        chosen, "claude",
        "the chosen agent is kept across the refresh"
    );
}

#[gpui::test]
fn a_workspace_opens_its_start_page_and_the_page_is_drawn(cx: &mut TestAppContext) {
    let (projects, workspaces) = two_projects();
    let (rig, cx) = rig(cx, projects, workspaces);
    rig.shell
        .update(cx, |shell, _| shell.model.open_workspace("w1"));
    draw(cx);
    assert_eq!(canvas(&rig, cx), Canvas::Workspace("w1".into()));
    assert!(
        cx.debug_bounds("start-project-link").is_some(),
        "the heading names the project"
    );
}

#[gpui::test]
fn choosing_no_project_in_the_page_menu_returns_to_the_start_page_with_no_workspace(
    cx: &mut TestAppContext,
) {
    let (projects, workspaces) = two_projects();
    let (rig, cx) = rig(cx, projects, workspaces);
    rig.shell
        .update(cx, |shell, _| shell.model.open_workspace("w1"));
    draw(cx);
    rig.shell.update_in(cx, |shell, window, cx| {
        shell.open_start_project_menu(window, cx)
    });
    draw(cx);
    assert!(
        cx.debug_bounds("menu-panel").is_some(),
        "the project menu opens"
    );
    // The menu ends with "Add a project…", so "No project" is the row above the last one.
    cx.simulate_keystrokes("end up enter");
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(shell.model().selected_workspace(), None);
        assert!(shell.dialogs.menu.is_none(), "the menu closed");
    });
    assert_eq!(
        canvas(&rig, cx),
        Canvas::Home(crate::model::navigation::Mode::Workspaces)
    );
}

#[gpui::test]
fn choosing_a_project_in_the_page_menu_opens_its_first_workspace(cx: &mut TestAppContext) {
    let (projects, workspaces) = two_projects();
    let (rig, cx) = rig(cx, projects, workspaces);
    rig.shell
        .update(cx, |shell, _| shell.model.open_workspace("w1"));
    draw(cx);
    rig.shell.update_in(cx, |shell, window, cx| {
        shell.open_start_project_menu(window, cx)
    });
    draw(cx);
    // The rows are alpha, beta, No project; the current project is checked. Down moves to beta.
    cx.simulate_keystrokes("down enter");
    assert_eq!(
        rig.shell.read_with(cx, |shell, _| shell
            .model()
            .selected_workspace()
            .map(str::to_owned)),
        Some("w2".to_owned())
    );
    assert_eq!(canvas(&rig, cx), Canvas::Workspace("w2".into()));
}

#[gpui::test]
fn the_page_menu_opens_over_the_start_page_and_draws(cx: &mut TestAppContext) {
    let (projects, workspaces) = two_projects();
    let (rig, cx) = rig(cx, projects, workspaces);
    rig.shell
        .update(cx, |shell, _| shell.model.open_workspace("w1"));
    draw(cx);
    rig.shell.update_in(cx, |shell, window, cx| {
        shell.open_start_project_menu(window, cx)
    });
    draw(cx);
    assert!(
        cx.debug_bounds("menu-panel").is_some(),
        "the project menu is drawn"
    );
}

#[test]
fn a_fact_value_cut_keeps_its_last_path_segment_whole() {
    assert_eq!(split_last_segment("/repos/first"), ("/repos", "/first"));
    assert_eq!(
        split_last_segment(r"D:\projects\desktop-start-path"),
        (r"D:\projects", r"\desktop-start-path")
    );
    assert_eq!(split_last_segment("/repos/first/"), ("/repos/first", "/"));
    assert_eq!(split_last_segment("Ready"), ("", "Ready"));
}

/// The artboard's window (ws-start, 1440 x 900).
const WINDOW_WIDTH: f32 = 1440.0;
const WINDOW_HEIGHT: f32 = 900.0;
/// Slack for pixel rounding in the layout checks.
const EDGE_TOLERANCE: f32 = 1.0;

#[gpui::test]
fn a_long_path_truncates_in_its_fact_row_and_the_page_stays_in_the_window(cx: &mut TestAppContext) {
    let (projects, mut workspaces) = two_projects();
    // 153 characters with no space, so the text has nowhere to wrap.
    workspaces[0].path = format!("/repos/{}/first", "segment".repeat(20));
    let (rig, cx) = rig(cx, projects, workspaces);
    cx.simulate_resize(size(px(WINDOW_WIDTH), px(WINDOW_HEIGHT)));
    cx.run_until_parked();
    rig.shell
        .update(cx, |shell, _| shell.model.open_workspace("w1"));
    draw(cx);

    let column = cx
        .debug_bounds("start-column")
        .expect("the start page column is drawn");
    let value = cx
        .debug_bounds("start-path-value")
        .expect("the path value is drawn");
    let column_width = f32::from(column.size.width);
    let column_right = f32::from(column.origin.x + column.size.width);
    let value_left = f32::from(value.origin.x);
    let value_right = f32::from(value.origin.x + value.size.width);

    assert!(
        column_width <= LAYOUT_CONVERSATION_READING + EDGE_TOLERANCE,
        "the column is {column_width} px wide, past its {LAYOUT_CONVERSATION_READING} px maximum"
    );
    assert!(
        column_right <= WINDOW_WIDTH + EDGE_TOLERANCE,
        "the column ends at {column_right} px, past the {WINDOW_WIDTH} px window"
    );
    // The value takes the control column at the row's right edge (16 px row padding, 1 px
    // section border), so the label keeps the space to its left.
    let row_right = column_right - SPACE_4 - 1.0;
    let control_left = row_right - LAYOUT_SETTINGS_CONTROL_MAX;
    assert!(
        value_left >= control_left - EDGE_TOLERANCE && value_right <= row_right + EDGE_TOLERANCE,
        "the value spans {value_left} to {value_right} px, outside the control column \
         {control_left} to {row_right} px"
    );
    // The text itself: the head that gives way and the last segment that stays whole.
    let head = cx
        .debug_bounds("start-path-value-head")
        .expect("the path's head is drawn");
    let tail = cx
        .debug_bounds("start-path-value-tail")
        .expect("the path's last segment is drawn");
    let head_left = f32::from(head.origin.x);
    let head_right = f32::from(head.origin.x + head.size.width);
    let tail_left = f32::from(tail.origin.x);
    let tail_right = f32::from(tail.origin.x + tail.size.width);
    assert!(
        head_left >= control_left - EDGE_TOLERANCE,
        "the path's head starts at {head_left} px, left of the control column at {control_left} px"
    );
    assert!(
        head_right <= tail_left + EDGE_TOLERANCE,
        "the path's head ends at {head_right} px, over its last segment at {tail_left} px"
    );
    assert!(
        tail_right <= row_right + EDGE_TOLERANCE,
        "the path's last segment ends at {tail_right} px, past the row at {row_right} px"
    );
}

/// The start page of workspace w1 on a connected service that declares these agent kinds.
fn start_page_rig(cx: &mut TestAppContext, kinds: Vec<AgentKind>) -> (Rig, &mut VisualTestContext) {
    let (rig, cx) = rig(
        cx,
        vec![project("p1", "alpha")],
        vec![workspace("w1", "p1", "fix-login", 1)],
    );
    rig._script.set_connection(ConnectionState::Connected {
        server: "massorca".into(),
        version: "0.1.0".into(),
    });
    rig._updates
        .send(StoreUpdate::AgentKindsResync(kinds))
        .expect("the store listens");
    rig.shell.update(cx, |shell, _| {
        shell.model.select_workspace(Some("w1".to_owned()))
    });
    cx.run_until_parked();
    draw(cx);
    (rig, cx)
}

fn composer_of(rig: &Rig, cx: &mut VisualTestContext) -> Entity<Composer> {
    rig.shell
        .update(cx, |shell, cx| shell.start_composer("w1", cx))
}

/// Types `text` into the start page's composer and presses Enter.
fn type_and_send(rig: &Rig, cx: &mut VisualTestContext, text: &str) {
    let composer = composer_of(rig, cx);
    composer.update_in(cx, |composer, window, cx| composer.focus(window, cx));
    cx.run_until_parked();
    cx.simulate_input(text);
    cx.simulate_keystrokes("enter");
}

/// An `agent.start` the shell sent, with the reply that answers it.
type StartCall = (
    AgentStartParams,
    oneshot::Sender<Result<AgentSession, Refusal>>,
);

/// The next `agent.start` the shell sends.
fn start_call(script: &mut Script) -> Option<StartCall> {
    while let Some(command) = script.next() {
        if let Command::AgentStart { params, reply } = command {
            return Some((params, reply));
        }
    }
    None
}

#[gpui::test]
fn the_start_page_offers_only_the_installed_and_enabled_kinds_and_starts_on_the_first(
    cx: &mut TestAppContext,
) {
    let mut cursor = agent_kind("cursor", "Cursor", PackageState::Enabled);
    cursor.detected = false;
    let (rig, cx) = start_page_rig(
        cx,
        vec![
            agent_kind("claude", "Claude", PackageState::Enabled),
            agent_kind("codex", "Codex", PackageState::Disabled),
            cursor,
        ],
    );
    let composer = composer_of(&rig, cx);
    let (offered, chosen) = composer.read_with(cx, |composer, cx| {
        let state = composer.picker().read(cx).state();
        (
            state
                .agents()
                .iter()
                .map(|agent| agent.models[0].id.to_string())
                .collect::<Vec<_>>(),
            state.choice().model.to_string(),
        )
    });
    assert_eq!(offered, ["claude"]);
    assert_eq!(chosen, "claude");
}

#[gpui::test]
fn a_kind_that_is_off_is_not_offered_and_draws_no_notice(cx: &mut TestAppContext) {
    let (rig, cx) = start_page_rig(
        cx,
        vec![agent_kind("codex", "Codex", PackageState::Disabled)],
    );
    draw(cx);
    assert!(cx.debug_bounds("start-kind-notice").is_none());
    assert!(cx.debug_bounds("button:start-kind-marketplace").is_none());
    let offered = composer_of(&rig, cx).read_with(cx, |composer, cx| {
        composer.picker().read(cx).state().agents().len()
    });
    assert_eq!(offered, 0);
}

#[gpui::test]
fn send_with_no_agent_installed_stays_off_and_sends_nothing(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page_rig(
        cx,
        vec![agent_kind("codex", "Codex", PackageState::Disabled)],
    );
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    assert!(start_call(&mut rig._script).is_none(), "no agent.start");
    let composer = composer_of(&rig, cx);
    let (error, draft, primary) = composer.read_with(cx, |composer, cx| {
        (
            composer.error_reason().map(|reason| reason.to_string()),
            composer.draft(cx),
            composer.primary_action(cx),
        )
    });
    assert_eq!(error, None, "nothing was refused: Send is simply off");
    assert_eq!(draft, "fix the login");
    assert_eq!(
        format!("{primary:?}"),
        "Disabled",
        "the primary button is off"
    );
}

#[gpui::test]
fn sending_the_prompt_asks_the_service_to_start_the_chosen_kind_in_the_workspace(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = start_page_rig(
        cx,
        vec![agent_kind("claude", "Claude", PackageState::Enabled)],
    );
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (params, _reply) = start_call(&mut rig._script).expect("the start is sent to the service");
    assert_eq!(params.workspace.as_deref(), Some("w1"));
    assert_eq!(params.kind, "claude");
    assert_eq!(params.prompt.as_deref(), Some("fix the login"));
}

#[gpui::test]
fn a_refused_start_shows_the_services_sentence_and_keeps_the_prompt(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page_rig(
        cx,
        vec![agent_kind("claude", "Claude", PackageState::Enabled)],
    );
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (_, reply) = start_call(&mut rig._script).expect("the start is sent to the service");
    let refusal = Refusal::Failed {
        code: "agent.not_signed_in".to_owned(),
        message: "the agent is not signed in".to_owned(),
    };
    reply
        .send(Err(refusal.clone()))
        .expect("the shell is waiting for the answer");
    cx.run_until_parked();
    draw(cx);
    let composer = composer_of(&rig, cx);
    let (error, draft) = composer.read_with(cx, |composer, cx| {
        (
            composer.error_reason().map(|reason| reason.to_string()),
            composer.draft(cx),
        )
    });
    assert_eq!(error, Some(refusal.sentence()));
    assert_eq!(draft, "fix the login");
}

#[gpui::test]
fn nothing_shows_as_started_until_the_service_answers(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page_rig(
        cx,
        vec![agent_kind("claude", "Claude", PackageState::Enabled)],
    );
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (_, _reply) = start_call(&mut rig._script).expect("the start is sent to the service");
    cx.run_until_parked();
    draw(cx);
    let (canvas, pending) = rig.shell.read_with(cx, |shell, _| {
        (shell.model.canvas(), shell.start_pending.clone())
    });
    assert_eq!(canvas, Canvas::Workspace("w1".to_owned()));
    let status = pending.map(|pending| pending.status());
    assert_eq!(status.as_deref(), Some("Starting Claude on This PC"));
}

#[gpui::test]
fn a_started_session_opens_on_the_session_canvas_once_the_store_lists_it(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page_rig(
        cx,
        vec![agent_kind("claude", "Claude", PackageState::Enabled)],
    );
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (_, reply) = start_call(&mut rig._script).expect("the start is sent to the service");
    let session = agent("0196a6f0-0000-7000-8000-00000000abcd", "w1", 5);
    reply
        .send(Ok(session.clone()))
        .expect("the shell is waiting for the answer");
    rig._updates
        .send(StoreUpdate::AgentChanged(session.clone()))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    let (canvas, active) = rig.shell.read_with(cx, |shell, _| {
        (
            shell.model.canvas(),
            shell.model.snapshot().active_session.clone(),
        )
    });
    assert!(matches!(canvas, Canvas::Session(_)), "{canvas:?}");
    assert_eq!(active.as_deref(), Some(session.id.as_str()));
}

/// A `terminal.attach` the shell sent, with the reply that answers it.
type AttachCall = (
    TerminalAttachParams,
    oneshot::Sender<Result<UnboundedReceiver<TerminalStreamItem>, Refusal>>,
);

/// The next `terminal.attach` the shell sends.
fn attach_call(script: &mut Script) -> Option<AttachCall> {
    while let Some(command) = script.next() {
        if let Command::Terminal(TerminalCommand::Attach { params, reply }) = command {
            return Some((params, reply));
        }
    }
    None
}

#[gpui::test]
fn the_session_terminal_is_attached_when_its_session_is_shown(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page_rig(
        cx,
        vec![agent_kind("claude", "Claude", PackageState::Enabled)],
    );
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (_, reply) = start_call(&mut rig._script).expect("the start is sent to the service");
    let terminal = "0196a6f0-0000-7000-8000-0000000000aa";
    let mut session = agent("0196a6f0-0000-7000-8000-00000000abcd", "w1", 5);
    session.terminal_id = Some(terminal.to_owned());
    reply
        .send(Ok(session.clone()))
        .expect("the shell is waiting for the answer");
    rig._updates
        .send(StoreUpdate::AgentChanged(session.clone()))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    cx.run_until_parked();
    let (attach, _attach_reply) =
        attach_call(&mut rig._script).expect("the session's terminal is attached");
    assert_eq!(attach.terminal, terminal);
}

#[gpui::test]
fn new_session_from_the_workspace_menu_opens_the_start_page_of_that_workspace(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = start_page_rig(
        cx,
        vec![agent_kind("claude", "Claude", PackageState::Enabled)],
    );
    let session = agent("0196a6f0-0000-7000-8000-00000000abcd", "w1", 5);
    rig._updates
        .send(StoreUpdate::AgentChanged(session.clone()))
        .expect("the store listens");
    cx.run_until_parked();
    rig.shell
        .update(cx, |shell, _| shell.model.open_session(&session.id));
    draw(cx);
    let open = rig.shell.read_with(cx, |shell, _| shell.model.canvas());
    assert!(matches!(open, Canvas::Session(_)), "{open:?}");
    rig.shell
        .update(cx, |shell, cx| shell.open_new_session("w1", cx));
    draw(cx);
    let start = rig.shell.read_with(cx, |shell, _| shell.model.canvas());
    assert_eq!(start, Canvas::Workspace("w1".to_owned()));
}

/// Clicks the element `selector` on the drawn page.
fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let center = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("no element with selector {selector}"))
        .center();
    cx.simulate_click(center, gpui::Modifiers::default());
    cx.run_until_parked();
    draw(cx);
}

#[gpui::test]
fn chat_is_chosen_until_terminal_is_clicked_and_terminal_says_it_is_limited(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = start_page_rig(
        cx,
        vec![agent_kind("claude", "Claude", PackageState::Enabled)],
    );
    let mode = |rig: &Rig, cx: &mut VisualTestContext| {
        rig.shell.read_with(cx, |shell, _| shell.start_mode("w1"))
    };
    assert_eq!(mode(&rig, cx), StartMode::Chat);
    assert!(cx.debug_bounds("start-terminal-note").is_none());

    click(cx, "segment-1");
    assert_eq!(mode(&rig, cx), StartMode::Terminal);
    assert!(
        cx.debug_bounds("start-terminal-note").is_some(),
        "Terminal shows its note under the composer"
    );

    click(cx, "segment-0");
    assert_eq!(mode(&rig, cx), StartMode::Chat);
    assert!(cx.debug_bounds("start-terminal-note").is_none());
}

#[gpui::test]
fn a_terminal_start_opens_the_agents_terminal_in_the_work_panel_and_keeps_the_page(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = start_page_rig(
        cx,
        vec![agent_kind("claude", "Claude", PackageState::Enabled)],
    );
    click(cx, "segment-1");
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (params, reply) = start_call(&mut rig._script).expect("the start is sent to the service");
    assert_eq!(params.prompt.as_deref(), Some("fix the login"));
    let terminal = "0196a6f0-0000-7000-8000-0000000000aa";
    let mut session = agent("0196a6f0-0000-7000-8000-00000000abcd", "w1", 5);
    session.terminal_id = Some(terminal.to_owned());
    reply
        .send(Ok(session.clone()))
        .expect("the shell is waiting for the answer");
    rig._updates
        .send(StoreUpdate::AgentChanged(session))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    let (canvas, work_panel, selected) = rig.shell.read_with(cx, |shell, _| {
        (
            shell.model.canvas(),
            shell.model.region_visible(Region::WorkPanel),
            shell
                .model
                .terminal_selection()
                .selected("w1")
                .map(str::to_owned),
        )
    });
    assert_eq!(canvas, Canvas::Workspace("w1".to_owned()), "the page stays");
    assert!(work_panel, "the Work panel shows the agent's terminal");
    assert_eq!(selected.as_deref(), Some(terminal));
    let (attach, _attach_reply) =
        attach_call(&mut rig._script).expect("the agent's terminal is attached");
    assert_eq!(attach.terminal, terminal);
}

fn claude_with_modes() -> AgentKind {
    use massorca_protocol::methods::agents::PermissionMode;
    let mode = |id: &str, label: &str, bypass: bool| PermissionMode {
        id: id.into(),
        label: label.into(),
        description: None,
        bypass,
        live_switch: false,
        may_fall_back: false,
    };
    let mut kind = agent_kind("claude", "Claude", PackageState::Enabled);
    kind.permission_modes = vec![
        mode("auto", "Auto", false),
        mode("plan", "Plan", false),
        mode("bypassPermissions", "Bypass permissions", true),
    ];
    kind.default_permission_mode = Some("auto".into());
    kind
}

#[gpui::test]
fn the_mode_chip_starts_on_the_clis_default_and_sends_no_mode(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page_rig(cx, vec![claude_with_modes()]);
    let composer = composer_of(&rig, cx);
    let modes = composer.read_with(cx, |composer, _| {
        composer
            .access_modes()
            .iter()
            .map(|mode| (mode.name.to_string(), mode.bypass))
            .collect::<Vec<_>>()
    });
    assert_eq!(
        modes,
        [
            ("Auto".to_owned(), false),
            ("Auto".to_owned(), false),
            ("Plan".to_owned(), false),
            ("Bypass permissions".to_owned(), true),
        ]
    );
    let details = composer.read_with(cx, |composer, _| {
        (0..4)
            .map(|index| composer.access_detail(index).map(|text| text.to_string()))
            .collect::<Vec<_>>()
    });
    assert_eq!(
        details,
        [Some("The CLI's default".to_owned()), None, None, None],
        "the default entry says it is the CLI's default; the declared Auto does not"
    );
    assert_eq!(
        composer.read_with(cx, |composer, _| composer.access_index()),
        0,
        "the CLI's default is selected"
    );
    draw(cx);
    assert!(cx.debug_bounds("composer-access").is_some());
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (params, _reply) = start_call(&mut rig._script).expect("the start is sent to the service");
    assert_eq!(params.permission_mode, None);
}

#[gpui::test]
fn a_picked_mode_is_sent_and_remembered_per_kind_until_the_default_is_picked(
    cx: &mut TestAppContext,
) {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let profile = Profile::resolve_with(
        Some(temp.path().to_path_buf()),
        Some("main".to_owned()),
        |_| None,
    )
    .expect("a profile");
    let (mut rig, cx) = start_page_rig(cx, vec![claude_with_modes()]);
    cx.update(|_, cx| ui_state::install(cx, profile.clone(), ui_state::UiState::default()));
    let composer = composer_of(&rig, cx);
    composer.update(cx, |composer, cx| {
        composer.set_access(2, cx);
        cx.emit(ComposerEvent::AccessChanged(2));
    });
    draw(cx);
    cx.run_until_parked();
    assert_eq!(
        ui_state::load(&profile)
            .start_permission_modes
            .get("claude"),
        Some(&"plan".to_owned()),
        "the pick is remembered for its kind"
    );
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (params, _reply) = start_call(&mut rig._script).expect("the start is sent to the service");
    assert_eq!(params.permission_mode.as_deref(), Some("plan"));

    composer.update(cx, |composer, cx| {
        composer.set_access(0, cx);
        cx.emit(ComposerEvent::AccessChanged(0));
    });
    cx.run_until_parked();
    assert!(
        !ui_state::load(&profile)
            .start_permission_modes
            .contains_key("claude"),
        "picking the CLI's default clears the remembered pick"
    );
}

#[gpui::test]
fn a_kind_without_modes_shows_no_mode_chip(cx: &mut TestAppContext) {
    let (rig, cx) = start_page_rig(
        cx,
        vec![agent_kind("claude", "Claude", PackageState::Enabled)],
    );
    let composer = composer_of(&rig, cx);
    assert!(composer.read_with(cx, |composer, _| composer.access_modes().is_empty()));
    assert!(cx.debug_bounds("composer-access").is_none());
}

#[gpui::test]
fn a_remembered_pick_comes_back_on_the_start_chip_and_is_sent(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let profile = Profile::resolve_with(
        Some(temp.path().to_path_buf()),
        Some("main".to_owned()),
        |_| None,
    )
    .expect("a profile");
    let mut saved = ui_state::UiState::default();
    saved
        .start_permission_modes
        .insert("claude".to_owned(), "plan".to_owned());
    // The kind first lists no modes, so the chip is built only once the remembered pick is there.
    let (mut rig, cx) = start_page_rig(
        cx,
        vec![agent_kind("claude", "Claude", PackageState::Enabled)],
    );
    cx.update(|_, cx| ui_state::install(cx, profile.clone(), saved));
    rig._updates
        .send(StoreUpdate::AgentKindsResync(vec![claude_with_modes()]))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (params, _reply) = start_call(&mut rig._script).expect("the start is sent to the service");
    assert_eq!(params.permission_mode.as_deref(), Some("plan"));
}

#[test]
fn a_start_sends_only_the_mode_the_chip_shows_from_the_same_list() {
    use super::start_page::{access_modes, sent_mode};
    use crate::model::permission_modes::start_choices;
    let choices = start_choices(&claude_with_modes());
    let shown = access_modes(&choices);
    assert_eq!(
        sent_mode(&choices, &shown, 0),
        None,
        "the CLI's default sends no mode"
    );
    assert_eq!(sent_mode(&choices, &shown, 1).as_deref(), Some("auto"));
    assert_eq!(sent_mode(&choices, &shown, 2).as_deref(), Some("plan"));
    assert_eq!(
        sent_mode(&choices, &shown, 9),
        None,
        "past the end sends no mode"
    );
    // A chip built from another list never lends its index to this one.
    let other = access_modes(&choices[..1]);
    assert_eq!(sent_mode(&choices, &other, 2), None);
}

#[gpui::test]
fn a_chip_index_nobody_picked_sends_no_mode(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page_rig(cx, vec![claude_with_modes()]);
    let composer = composer_of(&rig, cx);
    // The index moves without the person choosing the entry: nothing is sent.
    composer.update(cx, |composer, cx| composer.set_access(2, cx));
    draw(cx);
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (params, _reply) = start_call(&mut rig._script).expect("the start is sent to the service");
    assert_eq!(params.permission_mode, None);
}

#[gpui::test]
fn the_shown_mode_and_the_sent_mode_agree_for_every_entry(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page_rig(cx, vec![claude_with_modes()]);
    let composer = composer_of(&rig, cx);
    let expected = [None, Some("auto"), Some("plan"), Some("bypassPermissions")];
    for (index, want) in expected.into_iter().enumerate() {
        composer.update(cx, |composer, cx| {
            composer.set_access(index, cx);
            cx.emit(ComposerEvent::AccessChanged(index));
        });
        draw(cx);
        type_and_send(&rig, cx, "fix the login");
        cx.run_until_parked();
        let (params, _reply) =
            start_call(&mut rig._script).expect("the start is sent to the service");
        assert_eq!(params.permission_mode.as_deref(), want, "entry {index}");
        rig.shell.update(cx, |shell, _| shell.start_pending = None);
    }
}

/// A profile whose ui state remembers `mode` as the start pick of Claude.
fn profile_remembering(temp: &tempfile::TempDir, mode: &str) -> (Profile, ui_state::UiState) {
    let profile = Profile::resolve_with(
        Some(temp.path().to_path_buf()),
        Some("main".to_owned()),
        |_| None,
    )
    .expect("a profile");
    let mut saved = ui_state::UiState::default();
    saved
        .start_permission_modes
        .insert("claude".to_owned(), mode.to_owned());
    (profile, saved)
}

/// Drops the start page's composer so the next draw builds a fresh one.
fn fresh_composer(rig: &Rig, cx: &mut VisualTestContext) -> Entity<Composer> {
    rig.shell.update(cx, |shell, _| {
        shell.start_composers.remove("w1");
    });
    draw(cx);
    composer_of(rig, cx)
}

#[gpui::test]
fn a_remembered_mode_on_a_fresh_composer_is_shown_and_sent(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let (profile, saved) = profile_remembering(&temp, "plan");
    let (mut rig, cx) = start_page_rig(cx, vec![claude_with_modes()]);
    cx.update(|_, cx| ui_state::install(cx, profile, saved));
    let composer = fresh_composer(&rig, cx);
    assert_eq!(
        composer.read_with(cx, |composer, _| composer.access_index()),
        2,
        "the chip shows the remembered Plan"
    );
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (params, _reply) = start_call(&mut rig._script).expect("the start is sent to the service");
    assert_eq!(params.permission_mode.as_deref(), Some("plan"));
}

#[gpui::test]
fn a_remembered_bypass_mode_is_not_restored_and_sends_no_mode(cx: &mut TestAppContext) {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let (profile, saved) = profile_remembering(&temp, "bypassPermissions");
    let (mut rig, cx) = start_page_rig(cx, vec![claude_with_modes()]);
    cx.update(|_, cx| ui_state::install(cx, profile, saved));
    let composer = fresh_composer(&rig, cx);
    assert_eq!(
        composer.read_with(cx, |composer, _| composer.access_index()),
        0,
        "a bypass is never restored from memory"
    );
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (params, _reply) = start_call(&mut rig._script).expect("the start is sent to the service");
    assert_eq!(params.permission_mode, None);
}

#[gpui::test]
fn a_mode_list_change_without_a_remembered_pick_sends_no_mode(cx: &mut TestAppContext) {
    let (mut rig, cx) = start_page_rig(cx, vec![claude_with_modes()]);
    let composer = composer_of(&rig, cx);
    composer.update(cx, |composer, cx| {
        composer.set_access(1, cx);
        cx.emit(ComposerEvent::AccessChanged(1));
    });
    draw(cx);
    // The kind drops Plan; the chip resets and the earlier pick of Auto no longer counts.
    let mut changed = claude_with_modes();
    changed.permission_modes.remove(1);
    rig._updates
        .send(StoreUpdate::AgentKindsResync(vec![changed]))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert_eq!(
        composer.read_with(cx, |composer, _| composer.access_index()),
        0
    );
    // The chip lands on Auto again without the person picking it: nothing is sent.
    composer.update(cx, |composer, cx| composer.set_access(1, cx));
    draw(cx);
    type_and_send(&rig, cx, "fix the login");
    cx.run_until_parked();
    let (params, _reply) = start_call(&mut rig._script).expect("the start is sent to the service");
    assert_eq!(params.permission_mode, None);
}
