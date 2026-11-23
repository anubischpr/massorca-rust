//! Element tests of the workspace start page: a workspace opens its page, and the project menu of the
//! page chooses the project's first workspace or none. A scripted handle stands in for the service.

use super::start_page::split_last_segment;
use super::*;
use crate::model::canvas::Canvas;
use crate::model::marketplace::UnavailableMarketplace;
use crate::model::teams::{TeamsSource, UnavailableTeams};
use crate::service::fixtures::{project, workspace};
use crate::service::{ConnectionState, Script, StoreUpdate};
use gpui::{AppContext, TestAppContext, VisualTestContext, px, size};
use massorca_protocol::methods::projects::Project;
use massorca_protocol::methods::workspaces::Workspace;
use massorca_ui::components::composer::Composer;
use massorca_ui::tokens::{LAYOUT_CONVERSATION_READING, LAYOUT_SETTINGS_CONTROL_MAX, SPACE_4};
use massorca_ui::{Appearance, Theme};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

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
