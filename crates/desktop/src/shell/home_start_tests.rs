//! Element tests of the start page with no workspace: "+ Create" opens it, and Send creates a
//! workspace then starts the agent in it. A scripted handle stands in for the service.

use std::collections::BTreeMap;

use super::home_start::home_project_menu;
use super::service_tests::{Rig, click, draw, inspection, rig};
use super::workspace_dialogs::ProjectMenuChoice;
use super::*;
use crate::model::home_start::{HOME_START, NO_PROJECT_NEEDS_SERVICE};
use crate::model::start_page::StartMode;
use crate::model::workspace_flow::{NO_PROJECT_LABEL, ProjectChoice};
use crate::service::fixtures::{agent, agent_kind, project, workspace};
use crate::service::{ChatCommand, Command, KindChat, Refusal, Script, StoreUpdate};
use gpui::{Focusable, TestAppContext, VisualTestContext};
use massorca_protocol::methods::agents::{AgentSession, AgentStartParams, PackageState};
use massorca_protocol::methods::projects::{Project, ProjectKind};
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};
use massorca_ui::components::composer::Composer;
use massorca_ui::components::menu::MenuEntry;
use massorca_ui::components::text_field::TextField;
use tokio::sync::oneshot;

/// A connected shell with projects alpha and beta that declares Claude, on the page "+ Create" opens.
fn home_rig(cx: &mut TestAppContext) -> (Rig, &mut VisualTestContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha"), ("p2", "beta")]);
    rig.updates
        .send(StoreUpdate::AgentKindsResync(vec![agent_kind(
            "claude",
            "Claude",
            PackageState::Enabled,
        )]))
        .expect("the store listens");
    cx.run_until_parked();
    click(cx, "button:sidebar-create");
    (rig, cx)
}

/// The service offers chat sessions and Claude can run one, as a newer service reports on connect (`agent-chat/1`).
fn offer_chat(rig: &Rig) {
    rig.updates
        .send(StoreUpdate::AgentChatOffered(true))
        .expect("the store listens");
    let claude = KindChat {
        ..crate::service::fixtures::base_kind_chat()
    };
    rig.updates
        .send(StoreUpdate::AgentKindChatResync(BTreeMap::from([(
            "claude".to_owned(),
            claude,
        )])))
        .expect("the store listens");
}

fn home_composer(rig: &Rig, cx: &mut VisualTestContext) -> Entity<Composer> {
    rig.shell
        .update(cx, |shell, cx| shell.start_composer(HOME_START, cx))
}

fn type_and_send(rig: &Rig, cx: &mut VisualTestContext, text: &str) {
    let composer = home_composer(rig, cx);
    composer.update_in(cx, |composer, window, cx| composer.focus(window, cx));
    cx.run_until_parked();
    cx.simulate_input(text);
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
}

/// Types into one of the section's fields: focuses it, then sends the keys.
fn type_into(cx: &mut VisualTestContext, field: &Entity<TextField>, text: &str) {
    draw(cx);
    let focus = field.read_with(cx, |field, cx| field.focus_handle(cx));
    cx.update(|window, cx| window.focus(&focus, cx));
    cx.simulate_input(text);
    cx.run_until_parked();
}

type CreateCall = (
    WorkspaceCreateParams,
    bool,
    oneshot::Sender<Result<Workspace, Refusal>>,
);
type StartCall = (
    AgentStartParams,
    oneshot::Sender<Result<AgentSession, Refusal>>,
);

/// The calls the shell sent so far, in order: a create or a start.
enum Call {
    Create(CreateCall),
    Start(StartCall),
}

fn next_call(script: &mut Script) -> Option<Call> {
    while let Some(command) = script.next() {
        match command {
            Command::Create {
                params,
                unique_name,
                reply,
            } => return Some(Call::Create((params, unique_name, reply))),
            Command::AgentStart { params, reply } => return Some(Call::Start((params, reply))),
            Command::Chat(ChatCommand::Start { params, reply }) => {
                return Some(Call::Start((params, reply)));
            }
            _ => {}
        }
    }
    None
}

fn expect_create(script: &mut Script) -> CreateCall {
    match next_call(script) {
        Some(Call::Create(call)) => call,
        _ => panic!("a workspace.create call was expected"),
    }
}

fn expect_start(script: &mut Script) -> StartCall {
    match next_call(script) {
        Some(Call::Start(call)) => call,
        _ => panic!("an agent.start call was expected"),
    }
}

#[gpui::test]
fn a_new_workspace_shows_a_placeholder_row_until_its_session_replaces_it(cx: &mut TestAppContext) {
    let (mut rig, cx) = home_rig(cx);
    type_and_send(&rig, cx, "Fix the login bug");
    let (_, _, reply) = expect_create(&mut rig.script);
    let created = workspace("w9", "p1", "fix-the-login-bug", 9);
    reply
        .send(Ok(created.clone()))
        .expect("the shell waits for the create");
    rig.updates
        .send(StoreUpdate::WorkspaceChanged(created))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("sidebar-placeholder-w9").is_some(),
        "the workspace shows its row while the agent starts"
    );
    let (_, reply) = expect_start(&mut rig.script);
    let session = agent("s9", "w9", 9);
    reply
        .send(Ok(session.clone()))
        .expect("the shell waits for the start");
    rig.updates
        .send(StoreUpdate::AgentChanged(Box::new(session)))
        .expect("the store listens");
    cx.run_until_parked();
    draw(cx);
    assert!(cx.debug_bounds("sidebar-placeholder-w9").is_none());
    assert!(cx.debug_bounds("sidebar-session-s9").is_some());
}

#[gpui::test]
fn plus_create_opens_the_start_page(cx: &mut TestAppContext) {
    let (rig, cx) = home_rig(cx);
    draw(cx);
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.model().canvas(),
            crate::model::canvas::Canvas::Home(crate::model::navigation::Mode::Workspaces)
        );
    });
    assert!(cx.debug_bounds("home-start-column").is_some());
    assert!(cx.debug_bounds("home-start-project").is_some());
}

#[gpui::test]
fn the_home_page_starts_on_terminal_until_the_service_offers_chat(cx: &mut TestAppContext) {
    let (rig, cx) = home_rig(cx);
    let mode = |rig: &Rig, cx: &mut VisualTestContext| {
        rig.shell
            .read_with(cx, |shell, app| shell.start_mode(HOME_START, app))
    };
    assert_eq!(mode(&rig, cx), StartMode::Terminal);
    offer_chat(&rig);
    cx.run_until_parked();
    draw(cx);
    assert_eq!(mode(&rig, cx), StartMode::Chat);
}

#[gpui::test]
fn the_title_names_the_chosen_project_and_the_name_opens_its_menu(cx: &mut TestAppContext) {
    let (rig, cx) = home_rig(cx);
    draw(cx);
    assert!(cx.debug_bounds("home-start-project-link").is_some());
    click(cx, "home-start-project-link");
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.dialogs.menu.is_some())
    );
}

#[gpui::test]
fn the_home_key_hints_start_at_the_composer_left_edge_not_centred(cx: &mut TestAppContext) {
    let (_rig, cx) = home_rig(cx);
    draw(cx);
    let column = cx
        .debug_bounds("home-start-column")
        .expect("the page column is drawn");
    let enter = cx
        .debug_bounds("start-key-hint-Enter")
        .expect("the Enter hint is drawn");
    let offset = f32::from(enter.origin.x) - f32::from(column.origin.x);
    assert!(
        offset <= 1.0,
        "the Enter hint starts {offset} px inside the column, not at its left edge"
    );
}

#[gpui::test]
fn the_title_is_the_plain_question_while_no_project_exists(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[]);
    rig.updates
        .send(StoreUpdate::AgentKindsResync(vec![agent_kind(
            "claude",
            "Claude",
            PackageState::Enabled,
        )]))
        .expect("the store listens");
    cx.run_until_parked();
    click(cx, "button:sidebar-create");
    draw(cx);
    assert!(cx.debug_bounds("home-start-title").is_some());
    assert!(cx.debug_bounds("home-start-project-link").is_none());
}

#[gpui::test]
fn send_creates_a_workspace_named_from_the_prompt_then_starts_the_agent_in_it(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = home_rig(cx);
    offer_chat(&rig);
    type_and_send(&rig, cx, "Fix the login bug");
    let (create, _unique, reply) = expect_create(&mut rig.script);
    assert_eq!(create.project, "p1");
    assert_eq!(create.name, "fix-the-login-bug");
    assert!(
        rig.script.next().is_none(),
        "nothing starts before the create answers"
    );
    let created = workspace("w9", "p1", "fix-the-login-bug", 9);
    reply
        .send(Ok(created.clone()))
        .expect("the shell waits for the create");
    rig.updates
        .send(StoreUpdate::WorkspaceChanged(created))
        .expect("the store listens");
    cx.run_until_parked();
    let (start, reply) = expect_start(&mut rig.script);
    assert_eq!(start.workspace.as_deref(), Some("w9"));
    assert_eq!(start.kind, "claude");
    assert_eq!(start.prompt.as_deref(), Some("Fix the login bug"));
    let session = agent("0196a6f0-0000-7000-8000-00000000abcd", "w9", 9);
    reply
        .send(Ok(session.clone()))
        .expect("the shell waits for the start");
    rig.updates
        .send(StoreUpdate::AgentChanged(Box::new(session.clone())))
        .expect("the store listens");
    cx.run_until_parked();
    let active = rig
        .shell
        .read_with(cx, |shell, _| shell.model.snapshot().active_session.clone());
    assert_eq!(active.as_deref(), Some(session.id.as_str()));
}

/// A refusal of `workspace.create` because the name is taken.
fn taken_name() -> Refusal {
    Refusal::Failed {
        code: massorca_protocol::codes::WORKSPACE_NAME_TAKEN.to_owned(),
        message: "that name is taken".to_owned(),
    }
}

#[gpui::test]
fn with_the_unique_name_capability_a_derived_name_asks_for_a_free_one_and_is_not_retried(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = home_rig(cx);
    rig.updates
        .send(StoreUpdate::WorkspaceUniqueNameOffered(true))
        .expect("the store listens");
    cx.run_until_parked();
    offer_chat(&rig);
    type_and_send(&rig, cx, "Fix the login bug");
    let (create, unique, reply) = expect_create(&mut rig.script);
    assert_eq!(create.name, "fix-the-login-bug");
    assert!(unique, "a derived name asks the service for a free one");
    reply
        .send(Err(taken_name()))
        .expect("the shell waits for the create");
    cx.run_until_parked();
    assert!(
        next_call(&mut rig.script).is_none(),
        "the service makes names unique itself, so the client does not retry"
    );
}

#[gpui::test]
fn without_the_unique_name_capability_a_taken_derived_name_is_retried_with_a_suffix(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = home_rig(cx);
    offer_chat(&rig);
    type_and_send(&rig, cx, "Fix the login bug");
    let (first, unique, reply) = expect_create(&mut rig.script);
    assert_eq!((first.name.as_str(), unique), ("fix-the-login-bug", false));
    reply
        .send(Err(taken_name()))
        .expect("the shell waits for the create");
    cx.run_until_parked();
    let (second, unique, reply) = expect_create(&mut rig.script);
    assert_eq!(
        (second.name.as_str(), unique),
        ("fix-the-login-bug-2", false)
    );
    let created = workspace("w9", "p1", "fix-the-login-bug-2", 9);
    reply
        .send(Ok(created.clone()))
        .expect("the shell waits for the create");
    rig.updates
        .send(StoreUpdate::WorkspaceChanged(created))
        .expect("the store listens");
    cx.run_until_parked();
    let (start, _reply) = expect_start(&mut rig.script);
    assert_eq!(start.workspace.as_deref(), Some("w9"));
}

#[gpui::test]
fn without_the_unique_name_capability_every_name_taken_shows_the_error_and_starts_nothing(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = home_rig(cx);
    offer_chat(&rig);
    type_and_send(&rig, cx, "Fix the login bug");
    for attempt in 1..=crate::model::home_start::FREE_NAME_TRIES {
        let (create, unique, reply) = expect_create(&mut rig.script);
        let expected = if attempt == 1 {
            "fix-the-login-bug".to_owned()
        } else {
            format!("fix-the-login-bug-{attempt}")
        };
        assert_eq!((create.name, unique), (expected, false));
        reply
            .send(Err(taken_name()))
            .expect("the shell waits for the create");
        cx.run_until_parked();
    }
    assert!(
        next_call(&mut rig.script).is_none(),
        "every name is taken: no agent starts"
    );
    let shown = home_composer(&rig, cx).read_with(cx, |composer, _| {
        composer.error_reason().map(|text| text.to_string())
    });
    assert!(
        shown.is_some_and(|text| text.to_lowercase().contains("that name is taken")),
        "the first refusal is shown once every name is taken"
    );
}

#[gpui::test]
fn without_the_unique_name_capability_a_refusal_that_is_not_a_taken_name_is_shown_at_once(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = home_rig(cx);
    offer_chat(&rig);
    type_and_send(&rig, cx, "Fix the login bug");
    let (_, _, reply) = expect_create(&mut rig.script);
    reply
        .send(Err(Refusal::Failed {
            code: massorca_protocol::codes::WORKSPACE_NAME_INVALID.to_owned(),
            message: "that name is not valid".to_owned(),
        }))
        .expect("the shell waits for the create");
    cx.run_until_parked();
    assert!(
        next_call(&mut rig.script).is_none(),
        "only a taken name is retried: the other refusal stops the send"
    );
}

#[gpui::test]
fn the_placement_card_has_the_host_project_and_branch_pills_and_the_host_opens_nothing(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = home_rig(cx);
    draw(cx);
    for selector in [
        "start-placement",
        "home-start-host",
        "home-start-project",
        "home-start-branch",
    ] {
        assert!(cx.debug_bounds(selector).is_some(), "{selector} is drawn");
    }
    click(cx, "home-start-host");
    rig.shell.read_with(cx, |shell, _| {
        assert!(shell.dialogs.menu.is_none());
        assert!(!shell.home_details.branch_menu);
    });
}

/// Takes the `git.branches` call the popover sends, answering with the workspace it named.
fn expect_git_branches(
    script: &mut Script,
) -> (String, oneshot::Sender<Result<Vec<String>, Refusal>>) {
    loop {
        match script.next() {
            Some(Command::GitBranches { workspace, reply }) => return (workspace, reply),
            Some(_) => {}
            None => panic!("a git.branches call was expected"),
        }
    }
}

#[gpui::test]
fn the_branch_popover_starts_closed_and_opens_with_the_new_worktree_switch(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = home_rig(cx);
    assert!(
        cx.debug_bounds("home-start-branch-popover").is_none(),
        "the popover starts closed"
    );
    click(cx, "home-start-branch");
    draw(cx);
    assert!(cx.debug_bounds("home-start-branch-popover").is_some());
    assert!(
        cx.debug_bounds("home-start-new-worktree").is_some(),
        "the New worktree switch is in the popover"
    );
    rig.shell
        .read_with(cx, |shell, _| assert!(shell.home_details.branch_menu));
    click(cx, "home-start-branch");
    draw(cx);
    rig.shell
        .read_with(cx, |shell, _| assert!(!shell.home_details.branch_menu));
}

#[gpui::test]
fn the_branch_popover_lists_the_projects_branches_and_the_search_filters_them(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = home_rig(cx);
    // Branches belong to the repository, so a workspace of the project lists them.
    rig.updates
        .send(StoreUpdate::WorkspaceChanged(workspace(
            "w1",
            "p1",
            "alpha-work",
            1,
        )))
        .expect("the store listens");
    cx.run_until_parked();
    click(cx, "home-start-branch");
    draw(cx);
    let (workspace_id, reply) = expect_git_branches(&mut rig.script);
    assert_eq!(workspace_id, "w1");
    reply
        .send(Ok(vec![
            "main".to_owned(),
            "feature/map".to_owned(),
            "release".to_owned(),
        ]))
        .expect("the shell waits for the branches");
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("home-start-branch-row-2").is_some(),
        "every branch the service lists is in the list"
    );
    let search = rig
        .shell
        .read_with(cx, |shell, _| shell.home_details.search.clone());
    type_into(cx, &search, "rel");
    draw(cx);
    assert!(
        cx.debug_bounds("home-start-branch-row-0").is_some(),
        "the search keeps the matching branch"
    );
    assert!(
        cx.debug_bounds("home-start-branch-row-1").is_none(),
        "and drops the rest"
    );
}

#[gpui::test]
fn picking_a_branch_starts_the_new_worktree_from_it(cx: &mut TestAppContext) {
    let (mut rig, cx) = home_rig(cx);
    rig.updates
        .send(StoreUpdate::WorkspaceChanged(workspace(
            "w1",
            "p1",
            "alpha-work",
            1,
        )))
        .expect("the store listens");
    cx.run_until_parked();
    click(cx, "home-start-branch");
    draw(cx);
    let (_, reply) = expect_git_branches(&mut rig.script);
    reply
        .send(Ok(vec!["main".to_owned(), "release".to_owned()]))
        .expect("the shell waits for the branches");
    cx.run_until_parked();
    draw(cx);
    click(cx, "home-start-branch-row-1");
    draw(cx);
    assert!(
        cx.debug_bounds("home-start-branch-popover").is_none(),
        "picking a branch closes the popover"
    );
    offer_chat(&rig);
    type_and_send(&rig, cx, "Fix the login bug");
    let (create, _unique, _reply) = expect_create(&mut rig.script);
    assert_eq!(
        create.base.as_deref(),
        Some("release"),
        "the new worktree starts from the picked branch"
    );
    assert_eq!(create.branch, None);
    assert!(!create.existing_branch);
}

#[gpui::test]
fn an_untouched_popover_sends_the_plain_create_call(cx: &mut TestAppContext) {
    let (mut rig, cx) = home_rig(cx);
    offer_chat(&rig);
    type_and_send(&rig, cx, "Fix the login bug");
    let (create, _unique, _reply) = expect_create(&mut rig.script);
    assert_eq!(
        create.base, None,
        "no base picked: the service starts from the project's base"
    );
    assert_eq!(create.branch, None);
    assert!(!create.existing_branch);
}

#[gpui::test]
fn an_empty_prompt_still_creates_the_workspace_and_starts_the_agent(cx: &mut TestAppContext) {
    let (mut rig, cx) = home_rig(cx);
    let composer = home_composer(&rig, cx);
    rig.shell
        .update(cx, |shell, cx| shell.start_from_home("", composer, cx));
    cx.run_until_parked();
    let (create, _unique, reply) = expect_create(&mut rig.script);
    assert_eq!(create.name, "", "a blank name asks the service for one");
    reply
        .send(Ok(workspace("w9", "p1", "coral-reef", 9)))
        .expect("the shell waits for the create");
    cx.run_until_parked();
    let (start, _reply) = expect_start(&mut rig.script);
    assert_eq!(start.workspace.as_deref(), Some("w9"));
    assert_eq!(start.prompt, None);
}

#[gpui::test]
fn a_refused_create_shows_the_refusal_keeps_the_draft_and_starts_nothing(cx: &mut TestAppContext) {
    let (mut rig, cx) = home_rig(cx);
    type_and_send(&rig, cx, "fix the login");
    let (_, _, reply) = expect_create(&mut rig.script);
    let refusal = Refusal::NameProblem {
        message: "the name fix-the-login is taken".to_owned(),
    };
    reply
        .send(Err(refusal.clone()))
        .expect("the shell waits for the create");
    cx.run_until_parked();
    draw(cx);
    assert!(next_call(&mut rig.script).is_none(), "no agent.start");
    let composer = home_composer(&rig, cx);
    let (error, draft) = composer.read_with(cx, |composer, cx| {
        (
            composer.error_reason().map(|reason| reason.to_string()),
            composer.draft(cx),
        )
    });
    assert_eq!(error, Some(refusal.sentence()));
    assert_eq!(draft, "fix the login");
}

#[test]
fn no_project_is_listed_but_off_and_says_it_needs_a_newer_service() {
    let projects: Vec<Project> = vec![project("p1", "alpha")];
    let chosen = ProjectChoice::Project("p1".to_owned());
    let (entries, choices) = home_project_menu(&projects, Some(&chosen), false);
    let row = entries
        .iter()
        .position(
            |entry| matches!(entry, MenuEntry::Item(item) if item.label() == NO_PROJECT_LABEL),
        )
        .expect("the menu lists No project");
    assert!(matches!(&entries[row], MenuEntry::Item(item) if item.is_disabled()));
    assert_eq!(choices[row], None, "the row stands for nothing to choose");
    assert_eq!(NO_PROJECT_NEEDS_SERVICE, "Needs a newer service.");
}

#[test]
fn an_empty_start_page_menu_says_so_and_still_offers_add_a_project() {
    let (entries, choices) = home_project_menu(&[], None, false);
    // The caption, No project, a separator, then Add a project.
    assert_eq!(entries.len(), 4);
    assert_eq!(choices.first(), Some(&None), "the caption is not a choice");
    assert_eq!(choices.last(), Some(&Some(ProjectMenuChoice::AddProject)));
}

#[gpui::test]
fn clicking_no_project_chooses_nothing(cx: &mut TestAppContext) {
    let (rig, cx) = home_rig(cx);
    click(cx, "home-start-project");
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.dialogs.menu.is_some())
    );
    // The rows are alpha, beta, then No project.
    click(cx, "menu-item-2");
    rig.shell.read_with(cx, |shell, _| {
        assert!(
            shell.dialogs.menu.is_some(),
            "an off row does not close the menu"
        );
        assert_eq!(
            shell.home_project, None,
            "the first project stays the choice"
        );
    });
}

#[gpui::test]
fn a_project_added_from_the_page_menu_becomes_the_pages_project(cx: &mut TestAppContext) {
    let (mut rig, cx) = home_rig(cx);
    click(cx, "home-start-project");
    // The rows are alpha, beta, No project, a separator, then "Add a project…".
    click(cx, "menu-item-4");
    rig.shell.update(cx, |shell, cx| {
        let dialog = shell
            .dialogs
            .add_project
            .as_mut()
            .expect("the dialog opens");
        dialog.form.path = "/repos/gamma".to_owned();
        shell.submit_add_project(cx);
    });
    cx.run_until_parked();
    let reply = loop {
        match rig.script.next() {
            // The dialog asks first what the path is; a repository goes on to the add.
            Some(Command::InspectProject { reply, .. }) => {
                let _ = reply.send(Ok(inspection(ProjectKind::Git, "/repos/gamma", None)));
                cx.run_until_parked();
            }
            Some(Command::AddProject { reply, .. }) => break reply,
            Some(_) => {}
            None => panic!("a project.add call was expected"),
        }
    };
    reply
        .send(Ok(project("p3", "gamma")))
        .expect("the shell waits for the add");
    cx.run_until_parked();
    assert_eq!(
        rig.shell
            .read_with(cx, |shell, _| shell.home_project.clone()),
        Some(ProjectChoice::Project("p3".to_owned()))
    );
}

/// The refusal a data folder that is not secured gives every call (Windows, a new profile).
fn unsafe_location() -> Refusal {
    Refusal::Failed {
        code: massorca_protocol::codes::PROFILE_UNSAFE_LOCATION.to_owned(),
        message: "the data folder is in an unsafe location".to_owned(),
    }
}

#[gpui::test]
fn an_unsafe_data_folder_refusal_shows_a_banner_with_the_command_to_run(cx: &mut TestAppContext) {
    let (mut rig, cx) = home_rig(cx);
    draw(cx);
    assert!(cx.debug_bounds("start-profile-banner").is_none());
    type_and_send(&rig, cx, "fix the login");
    let (_, _, reply) = expect_create(&mut rig.script);
    reply
        .send(Err(unsafe_location()))
        .expect("the shell waits for the create");
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("start-profile-banner").is_some(),
        "the banner is drawn on the page"
    );
    assert!(super::start_page::PROFILE_UNSAFE_TEXT.contains("massorca profile secure --yes"));
}

#[gpui::test]
fn an_unsafe_data_folder_refusal_of_add_project_shows_the_banner_too(cx: &mut TestAppContext) {
    let (mut rig, cx) = home_rig(cx);
    click(cx, "home-start-project");
    click(cx, "menu-item-4");
    rig.shell.update(cx, |shell, cx| {
        let dialog = shell
            .dialogs
            .add_project
            .as_mut()
            .expect("the dialog opens");
        dialog.form.path = "/repos/gamma".to_owned();
        shell.submit_add_project(cx);
    });
    cx.run_until_parked();
    let reply = loop {
        match rig.script.next() {
            // The dialog asks first what the path is; a repository goes on to the add.
            Some(Command::InspectProject { reply, .. }) => {
                let _ = reply.send(Ok(inspection(ProjectKind::Git, "/repos/gamma", None)));
                cx.run_until_parked();
            }
            Some(Command::AddProject { reply, .. }) => break reply,
            Some(_) => {}
            None => panic!("a project.add call was expected"),
        }
    };
    reply
        .send(Err(unsafe_location()))
        .expect("the shell waits for the add");
    cx.run_until_parked();
    assert!(rig.shell.read_with(cx, |shell, _| shell.profile_unsafe));
}

#[test]
fn no_project_is_on_and_chosen_once_the_service_offers_it() {
    let projects: Vec<Project> = vec![project("p1", "alpha")];
    let (entries, choices) = home_project_menu(&projects, Some(&ProjectChoice::NoProject), true);
    let row = entries
        .iter()
        .position(
            |entry| matches!(entry, MenuEntry::Item(item) if item.label() == NO_PROJECT_LABEL),
        )
        .expect("the menu lists No project");
    assert!(
        matches!(&entries[row], MenuEntry::Item(item) if !item.is_disabled() && item.is_checked() == Some(true))
    );
    assert_eq!(
        choices[row],
        Some(ProjectMenuChoice::Project(ProjectChoice::NoProject))
    );
}

/// Chooses "No project" in the page's project menu, whose rows are alpha, beta, then No project.
fn choose_no_project(cx: &mut VisualTestContext) {
    click(cx, "home-start-project");
    click(cx, "menu-item-2");
}

#[gpui::test]
fn no_project_sends_no_model_or_effort_while_the_kinds_declare_no_models(cx: &mut TestAppContext) {
    let (mut rig, cx) = home_rig(cx);
    for update in [
        StoreUpdate::AgentNoProjectOffered(true),
        StoreUpdate::AgentOptionsOffered(true),
    ] {
        rig.updates.send(update).expect("the store listens");
    }
    cx.run_until_parked();
    choose_no_project(cx);
    type_and_send(&rig, cx, "Fix the login bug");
    let (start, _reply) = match next_call(&mut rig.script) {
        Some(Call::Start(call)) => call,
        _ => panic!("an agent.start call was expected"),
    };
    assert_eq!(start.workspace, None);
    assert_eq!((start.model, start.effort), (None, None));
}

#[gpui::test]
fn no_project_once_offered_starts_the_agent_with_no_workspace_and_opens_it(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = home_rig(cx);
    rig.updates
        .send(StoreUpdate::AgentNoProjectOffered(true))
        .expect("the store listens");
    offer_chat(&rig);
    cx.run_until_parked();
    choose_no_project(cx);
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(shell.home_project, Some(ProjectChoice::NoProject));
    });
    type_and_send(&rig, cx, "Fix the login bug");
    let (start, reply) = match next_call(&mut rig.script) {
        Some(Call::Start(call)) => call,
        Some(Call::Create(_)) => panic!("No project creates no workspace"),
        None => panic!("an agent.start call was expected"),
    };
    assert_eq!(start.workspace, None);
    assert_eq!(start.kind, "claude");
    assert_eq!(start.prompt.as_deref(), Some("Fix the login bug"));
    assert!(next_call(&mut rig.script).is_none(), "nothing else is sent");
    // The service lists the conversation's folder as a workspace of its hidden project; the sidebar groups it under No project.
    rig.updates
        .send(StoreUpdate::WorkspaceChanged(workspace(
            "c1",
            "hidden",
            "conversation",
            9,
        )))
        .expect("the store listens");
    let session = agent("0196a6f0-0000-7000-8000-00000000cafe", "c1", 9);
    reply
        .send(Ok(session.clone()))
        .expect("the shell waits for the start");
    rig.updates
        .send(StoreUpdate::AgentChanged(Box::new(session.clone())))
        .expect("the store listens");
    cx.run_until_parked();
    rig.shell.read_with(cx, |shell, cx| {
        assert_eq!(
            shell.model.snapshot().active_session.as_deref(),
            Some(session.id.as_str())
        );
        assert!(
            shell
                .store
                .read(cx)
                .state()
                .agents
                .iter()
                .any(|known| known.id == session.id)
        );
    });
}

#[gpui::test]
fn no_project_without_the_capability_stays_off_and_send_creates_in_the_first_project(
    cx: &mut TestAppContext,
) {
    let (mut rig, cx) = home_rig(cx);
    choose_no_project(cx);
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(
            shell.home_project, None,
            "No project is off until the service offers it"
        );
    });
    type_and_send(&rig, cx, "fix the login");
    let (create, _, _) = expect_create(&mut rig.script);
    assert_eq!(create.project, "p1");
}

#[gpui::test]
fn a_refused_no_project_start_shows_the_refusal_and_keeps_the_draft(cx: &mut TestAppContext) {
    let (mut rig, cx) = home_rig(cx);
    rig.updates
        .send(StoreUpdate::AgentNoProjectOffered(true))
        .expect("the store listens");
    cx.run_until_parked();
    choose_no_project(cx);
    type_and_send(&rig, cx, "fix the login");
    let (_, reply) = expect_start(&mut rig.script);
    let refusal = Refusal::Failed {
        code: "agent.start_refused".to_owned(),
        message: "the agent cannot start without a project".to_owned(),
    };
    reply
        .send(Err(refusal.clone()))
        .expect("the shell waits for the start");
    cx.run_until_parked();
    draw(cx);
    let composer = home_composer(&rig, cx);
    let (error, draft) = composer.read_with(cx, |composer, cx| {
        (
            composer.error_reason().map(|reason| reason.to_string()),
            composer.draft(cx),
        )
    });
    assert_eq!(error, Some(refusal.sentence()));
    assert_eq!(draft, "fix the login");
    assert!(rig.shell.read_with(cx, |shell, _| {
        shell.model.snapshot().active_session.is_none()
    }));
}

#[gpui::test]
fn no_project_turns_the_branch_pill_off_and_its_menu_stays_shut(cx: &mut TestAppContext) {
    let (rig, cx) = home_rig(cx);
    rig.updates
        .send(StoreUpdate::AgentNoProjectOffered(true))
        .expect("the store listens");
    cx.run_until_parked();
    choose_no_project(cx);
    click(cx, "home-start-branch");
    draw(cx);
    assert!(cx.debug_bounds("home-start-branch-popover").is_none());
    assert!(cx.debug_bounds("home-start-project-link").is_none());
}

#[gpui::test]
fn the_branch_pill_off_with_no_project_says_why_on_hover(cx: &mut TestAppContext) {
    use massorca_ui::tokens::DURATION_TOOLTIP_DELAY;
    let (rig, cx) = home_rig(cx);
    rig.updates
        .send(StoreUpdate::AgentNoProjectOffered(true))
        .expect("the store listens");
    cx.run_until_parked();
    choose_no_project(cx);
    draw(cx);
    let center = cx
        .debug_bounds("home-start-branch")
        .expect("the branch pill is drawn")
        .center();
    cx.simulate_mouse_move(center, None, gpui::Modifiers::none());
    cx.executor().advance_clock(DURATION_TOOLTIP_DELAY);
    cx.run_until_parked();
    draw(cx);
    assert!(
        cx.debug_bounds("tooltip-panel").is_some(),
        "the disabled branch pill names its reason on hover"
    );
}

#[gpui::test]
fn no_project_sends_while_no_project_is_listed_once_offered(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[]);
    rig.updates
        .send(StoreUpdate::AgentKindsResync(vec![agent_kind(
            "claude",
            "Claude",
            PackageState::Enabled,
        )]))
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::AgentNoProjectOffered(true))
        .expect("the store listens");
    cx.run_until_parked();
    click(cx, "button:sidebar-create");
    click(cx, "home-start-project");
    // With no project listed the rows are the caption, No project, a separator and Add a project.
    click(cx, "menu-item-1");
    rig.shell.read_with(cx, |shell, _| {
        assert_eq!(shell.home_project, Some(ProjectChoice::NoProject));
    });
    type_and_send(&rig, cx, "Fix the login bug");
    let (start, _reply) = expect_start(&mut rig.script);
    assert_eq!(start.workspace, None);
    assert_eq!(start.prompt.as_deref(), Some("Fix the login bug"));
}

/// The home page of a rig whose only project is `project`, which lists `workspaces`.
fn project_home(
    cx: &mut TestAppContext,
    project: Project,
    workspaces: Vec<Workspace>,
) -> (Rig, &mut VisualTestContext) {
    let (rig, cx) = rig(cx, &[("p0", "seed")]);
    rig.updates
        .send(StoreUpdate::Resync {
            projects: vec![project],
            workspaces,
        })
        .expect("the store listens");
    rig.updates
        .send(StoreUpdate::AgentKindsResync(vec![agent_kind(
            "claude",
            "Claude",
            PackageState::Enabled,
        )]))
        .expect("the store listens");
    cx.run_until_parked();
    click(cx, "button:sidebar-create");
    offer_chat(&rig);
    (rig, cx)
}

#[gpui::test]
fn a_folder_project_starts_in_its_existing_folder_workspace(cx: &mut TestAppContext) {
    let folder = Project {
        kind: ProjectKind::Folder,
        path: "/work/notes".to_owned(),
        ..project("p1", "notes")
    };
    let existing = Workspace {
        path: "/work/notes".to_owned(),
        ..workspace("w1", "p1", "notes", 1)
    };
    let (mut rig, cx) = project_home(cx, folder, vec![existing]);
    type_and_send(&rig, cx, "Summarise the notes");
    let (start, reply) = expect_start(&mut rig.script);
    assert_eq!(
        start.workspace.as_deref(),
        Some("w1"),
        "the start reuses the folder workspace, so no create runs first"
    );
    assert_eq!(start.prompt.as_deref(), Some("Summarise the notes"));
    reply
        .send(Ok(agent("s1", "w1", 1)))
        .expect("the shell waits for the start");
    cx.run_until_parked();
}

#[gpui::test]
fn a_folder_project_without_a_workspace_at_its_folder_creates_one(cx: &mut TestAppContext) {
    let folder = Project {
        kind: ProjectKind::Folder,
        path: "/work/notes".to_owned(),
        ..project("p1", "notes")
    };
    let (mut rig, cx) = project_home(cx, folder, Vec::new());
    type_and_send(&rig, cx, "Summarise the notes");
    let (create, _unique, reply) = expect_create(&mut rig.script);
    assert_eq!(create.project, "p1");
    let created = workspace("w9", "p1", "summarise-the-notes", 9);
    reply
        .send(Ok(created.clone()))
        .expect("the shell waits for the create");
    rig.updates
        .send(StoreUpdate::WorkspaceChanged(created))
        .expect("the store listens");
    cx.run_until_parked();
    let (start, _reply) = expect_start(&mut rig.script);
    assert_eq!(start.workspace.as_deref(), Some("w9"));
}

#[gpui::test]
fn a_git_project_still_creates_its_worktree_with_a_workspace_at_its_path(cx: &mut TestAppContext) {
    let git = project("p1", "notes");
    let existing = workspace("w1", "p1", "notes", 1);
    let (mut rig, cx) = project_home(cx, git, vec![existing]);
    type_and_send(&rig, cx, "Summarise the notes");
    let (create, _unique, _reply) = expect_create(&mut rig.script);
    assert_eq!(create.project, "p1", "a git project makes its own worktree");
}
