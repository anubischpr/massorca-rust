//! The Bot Teams dialogs: New team (screens 4.5), Add bot (D-AG-33) and the confirms for a
//! project or a memory entry. A form reads its fields when it is submitted; a refusal keeps it open
//! with the reason, and nothing changes until the state accepts the change.

use super::super::Shell;
use crate::model::shell::Agent;
use crate::model::teams::{HostChoice, NewBot, NewTeam, ProjectId, TeamId, TeamPage};
use gpui::{
    App, Context, Entity, Focusable, Subscription, WeakEntity, Window, div, prelude::*, px,
};
use massorca_ui::components::dialog::{Dialog, DialogVariant};
use massorca_ui::components::segmented_control::{Segment, SegmentedControl};
use massorca_ui::components::switch::Switch;
use massorca_ui::components::text_field::{TextField, TextFieldEvent};
use massorca_ui::tokens::{SPACE_1, SPACE_3, TYPE_BODY_SM, TYPE_CAPTION};
use massorca_ui::{ActiveTheme, TextStyled};

/// The agents an Add bot form offers, in the order of its choice (D-AG-33).
pub(super) const BOT_AGENTS: [Agent; 3] = [Agent::Claude, Agent::Codex, Agent::Gemini];

/// The name a bot's agent is called in the form.
fn agent_name(agent: Agent) -> &'static str {
    match agent {
        Agent::Claude => "Claude",
        Agent::Codex => "Codex",
        Agent::Gemini => "Gemini",
        Agent::Antigravity => "Antigravity",
        Agent::GentleShell => "Gentle Shell",
        Agent::OpenCode => "OpenCode",
        Agent::Cursor => "Cursor",
    }
}

#[derive(Debug)]
pub(in crate::shell) struct NewTeamDialog {
    name: Entity<TextField>,
    purpose: Entity<TextField>,
    host: usize,
    workspaces: bool,
    error: Option<&'static str>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug)]
pub(in crate::shell) struct AddBotDialog {
    team: TeamId,
    name: Entity<TextField>,
    role: Entity<TextField>,
    agent: usize,
    error: Option<&'static str>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug)]
pub(in crate::shell) struct EditBriefDialog {
    team: TeamId,
    project: ProjectId,
    field: Entity<TextField>,
    error: Option<&'static str>,
    _subscriptions: Vec<Subscription>,
}

/// The team dialog on screen, if any.
#[derive(Debug)]
pub(in crate::shell) enum TeamDialog {
    NewTeam(NewTeamDialog),
    AddBot(AddBotDialog),
    EditBrief(EditBriefDialog),
}

/// A confirm before a Bot Teams item is deleted (X-DLG-01, destructive where it loses data).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::shell) enum TeamConfirm {
    DeleteProject {
        team: TeamId,
        project: ProjectId,
        name: String,
    },
}

/// A field whose changes redraw the dialog, so its buttons follow what is typed. Enter submits.
fn watched_field(
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> (Entity<TextField>, Subscription) {
    let field = cx.new(|cx| TextField::new(cx).placeholder(placeholder));
    let subscription = cx.subscribe_in(
        &field,
        window,
        |shell, _, event: &TextFieldEvent, _, cx| match event {
            TextFieldEvent::Changed => cx.notify(),
            TextFieldEvent::Submitted => shell.submit_team_dialog(cx),
        },
    );
    (field, subscription)
}

impl Shell {
    /// "New team" on the Bot teams page. Nothing opens without a connected service.
    pub(super) fn open_new_team(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.model.teams().is_connected() {
            return;
        }
        let (name, name_events) = watched_field("Team name", window, cx);
        let (purpose, purpose_events) =
            watched_field("What the team is for (optional)", window, cx);
        self.team_dialog = Some(TeamDialog::NewTeam(NewTeamDialog {
            name,
            purpose,
            host: 0,
            workspaces: false,
            error: None,
            _subscriptions: vec![name_events, purpose_events],
        }));
        cx.notify();
    }

    /// "Add bot" on a team card (D-AG-33).
    pub(super) fn open_add_bot(
        &mut self,
        team: TeamId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.model.teams().is_connected() {
            return;
        }
        let (name, name_events) = watched_field("Name", window, cx);
        let (role, role_events) = watched_field("What this bot is responsible for", window, cx);
        self.team_dialog = Some(TeamDialog::AddBot(AddBotDialog {
            team,
            name,
            role,
            agent: 0,
            error: None,
            _subscriptions: vec![name_events, role_events],
        }));
        cx.notify();
    }

    pub(super) fn set_new_team_host(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(TeamDialog::NewTeam(dialog)) = &mut self.team_dialog {
            dialog.host = index;
        }
        cx.notify();
    }

    pub(super) fn set_new_team_workspaces(&mut self, on: bool, cx: &mut Context<Self>) {
        if let Some(TeamDialog::NewTeam(dialog)) = &mut self.team_dialog {
            dialog.workspaces = on;
        }
        cx.notify();
    }

    pub(super) fn set_add_bot_agent(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(TeamDialog::AddBot(dialog)) = &mut self.team_dialog {
            dialog.agent = index.min(BOT_AGENTS.len() - 1);
        }
        cx.notify();
    }

    /// Creates a team or adds a bot from the open form. A refusal keeps the form open with why.
    pub(super) fn submit_team_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.team_dialog.take() else {
            return;
        };
        self.team_dialog = match dialog {
            TeamDialog::NewTeam(mut dialog) => {
                let result = self.model.teams_mut().create_team(NewTeam {
                    name: dialog.name.read(cx).text().to_owned(),
                    purpose: dialog.purpose.read(cx).text().to_owned(),
                    host: dialog.host,
                    workspaces: dialog.workspaces,
                });
                match result {
                    Ok(_) => None,
                    Err(error) => {
                        dialog.error = Some(error.message());
                        Some(TeamDialog::NewTeam(dialog))
                    }
                }
            }
            TeamDialog::EditBrief(mut dialog) => {
                let brief = dialog.field.read(cx).text().to_owned();
                match self
                    .model
                    .teams_mut()
                    .save_brief(dialog.team, dialog.project, &brief)
                {
                    Ok(()) => None,
                    Err(error) => {
                        dialog.error = Some(error.message());
                        Some(TeamDialog::EditBrief(dialog))
                    }
                }
            }
            TeamDialog::AddBot(mut dialog) => {
                let result = self.model.teams_mut().add_bot(
                    dialog.team,
                    NewBot {
                        name: dialog.name.read(cx).text().to_owned(),
                        role: dialog.role.read(cx).text().to_owned(),
                        agent: BOT_AGENTS[dialog.agent],
                    },
                );
                match result {
                    Ok(_) => None,
                    Err(error) => {
                        dialog.error = Some(error.message());
                        Some(TeamDialog::AddBot(dialog))
                    }
                }
            }
        };
        cx.notify();
    }

    pub(super) fn close_team_dialog(&mut self, cx: &mut Context<Self>) {
        self.team_dialog = None;
        cx.notify();
    }

    /// "Edit brief" on a project page: the brief opens in a field, ready to change.
    pub(super) fn open_edit_brief(
        &mut self,
        team: TeamId,
        project: ProjectId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let brief = self
            .model
            .teams()
            .team(team)
            .and_then(|team| team.project(project))
            .map(|project| project.brief.clone())
            .unwrap_or_default();
        let field = cx.new(|cx| {
            TextField::new(cx)
                .placeholder("What this project is for")
                .with_text(brief)
        });
        let events =
            cx.subscribe_in(
                &field,
                window,
                |shell, _, event: &TextFieldEvent, _, cx| match event {
                    TextFieldEvent::Changed => cx.notify(),
                    TextFieldEvent::Submitted => shell.submit_team_dialog(cx),
                },
            );
        self.team_dialog = Some(TeamDialog::EditBrief(EditBriefDialog {
            team,
            project,
            field,
            error: None,
            _subscriptions: vec![events],
        }));
        cx.notify();
    }

    /// Links a project's Board to a Workspaces project, or to none (4.4).
    pub(super) fn set_project_link(
        &mut self,
        team: TeamId,
        project: ProjectId,
        link: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let _ = self.model.teams_mut().link_project(team, project, link);
        cx.notify();
    }

    pub(super) fn open_team_confirm(&mut self, confirm: TeamConfirm, cx: &mut Context<Self>) {
        self.team_confirm = Some(confirm);
        cx.notify();
    }

    pub(super) fn close_team_confirm(&mut self, cx: &mut Context<Self>) {
        self.team_confirm = None;
        cx.notify();
    }

    /// Deletes what the confirm names. The project's page goes back to its team's Board.
    pub(super) fn confirm_team_delete(&mut self, cx: &mut Context<Self>) {
        let Some(confirm) = self.team_confirm.take() else {
            return;
        };
        match confirm {
            TeamConfirm::DeleteProject { team, project, .. } => {
                if self.model.teams_mut().delete_project(team, project).is_ok() {
                    self.model.open_team_page(TeamPage::Board(team));
                }
            }
        }
        cx.notify();
    }
}

/// The team dialog on screen: New team or Add bot.
pub(in crate::shell) fn team_dialog_view(
    dialog: &TeamDialog,
    hosts: &[HostChoice],
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> Dialog {
    match dialog {
        TeamDialog::NewTeam(form) => new_team_view(form, hosts, shell, cx),
        TeamDialog::AddBot(form) => add_bot_view(form, shell, cx),
        TeamDialog::EditBrief(form) => edit_brief_view(form, shell, cx),
    }
}

fn new_team_view(
    form: &NewTeamDialog,
    hosts: &[HostChoice],
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> Dialog {
    let theme = *cx.theme();
    let host = {
        let shell = shell.clone();
        SegmentedControl::new(
            "new-team-host",
            "Host",
            hosts
                .iter()
                .map(|host| Segment::text(host.name.clone()))
                .collect(),
        )
        .selected(form.host)
        .on_change(move |index, _, cx| {
            let _ = shell.update(cx, |shell, cx| shell.set_new_team_host(index, cx));
        })
    };
    let workspaces = {
        let shell = shell.clone();
        Switch::new("new-team-workspaces", form.workspaces)
            .label("Turn on Workspaces in this team")
            .helper("A Development team can use Workspaces depth. You can change this later.")
            .on_change(move |on, _, cx| {
                let _ = shell.update(cx, |shell, cx| shell.set_new_team_workspaces(on, cx));
            })
    };
    let body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(form.name.clone())
        .child(form.purpose.clone())
        .child(caption("Host", &theme))
        .child(host)
        .child(workspaces);
    let can_submit = !form.name.read(cx).text().trim().is_empty();
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    let mut dialog = Dialog::new("new-team", "New team")
        .open(true)
        .child(body)
        .field(form.name.read(cx).focus_handle(cx))
        .action("Create team")
        .action_disabled(!can_submit)
        .on_action(move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.submit_team_dialog(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.close_team_dialog(cx));
        });
    if let Some(error) = form.error {
        dialog = dialog.error(error);
    }
    dialog
}

fn add_bot_view(form: &AddBotDialog, shell: &WeakEntity<Shell>, cx: &App) -> Dialog {
    let theme = *cx.theme();
    let agent = {
        let shell = shell.clone();
        SegmentedControl::new(
            "add-bot-agent",
            "Agent",
            BOT_AGENTS
                .iter()
                .map(|agent| Segment::text(agent_name(*agent)))
                .collect(),
        )
        .selected(form.agent)
        .on_change(move |index, _, cx| {
            let _ = shell.update(cx, |shell, cx| shell.set_add_bot_agent(index, cx));
        })
    };
    let body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(form.name.clone())
        .child(form.role.clone())
        .child(caption("Agent", &theme))
        .child(agent);
    let can_submit = !form.name.read(cx).text().trim().is_empty();
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    let mut dialog = Dialog::new("add-bot", "Add bot")
        .open(true)
        .child(body)
        .field(form.name.read(cx).focus_handle(cx))
        .action("Add bot")
        .action_disabled(!can_submit)
        .on_action(move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.submit_team_dialog(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.close_team_dialog(cx));
        });
    if let Some(error) = form.error {
        dialog = dialog.error(error);
    }
    dialog
}

fn edit_brief_view(form: &EditBriefDialog, shell: &WeakEntity<Shell>, cx: &App) -> Dialog {
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    let mut dialog = Dialog::new("edit-brief", "Edit brief")
        .open(true)
        .child(form.field.clone())
        .field(form.field.read(cx).focus_handle(cx))
        .action("Save brief")
        .on_action(move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.submit_team_dialog(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.close_team_dialog(cx));
        });
    if let Some(error) = form.error {
        dialog = dialog.error(error);
    }
    dialog
}

/// The confirm on screen: deleting a project removes its Board.
pub(in crate::shell) fn team_confirm_view(
    confirm: &TeamConfirm,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> Dialog {
    let theme = *cx.theme();
    let TeamConfirm::DeleteProject { name, .. } = confirm;
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    Dialog::new("delete-project", format!("Delete project {name}?"))
        .variant(DialogVariant::Destructive)
        .open(true)
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child("Its tasks are removed from the Board. This cannot be undone."),
        )
        .action("Delete project")
        .on_action(move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.confirm_team_delete(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.close_team_confirm(cx));
        })
}

fn caption(text: &'static str, theme: &massorca_ui::Theme) -> impl IntoElement {
    div()
        .pb(px(SPACE_1))
        .text_style(TYPE_CAPTION)
        .text_color(theme.palette.text.secondary)
        .child(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::SampleDataSource;
    use crate::model::navigation::Mode;
    use crate::model::teams::{SampleTeamsSource, TeamId};
    use gpui::{Entity, TestAppContext, VisualTestContext};
    use massorca_ui::{Appearance, Theme};

    fn shell(cx: &mut TestAppContext) -> (Entity<Shell>, &mut VisualTestContext) {
        cx.update(|cx| {
            Theme::set(Appearance::Dark, cx);
            crate::shell::bind_keys(cx);
        });
        cx.add_window_view(|window, cx| {
            Shell::new(&SampleDataSource, &SampleTeamsSource, window, cx)
        })
    }

    fn team_count(shell: &Entity<Shell>, cx: &mut VisualTestContext) -> usize {
        shell.read_with(cx, |shell, _| shell.model().teams().teams().len())
    }

    #[gpui::test]
    fn an_empty_name_keeps_the_new_team_form_open_and_says_why(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update_in(cx, |shell, window, cx| shell.open_new_team(window, cx));
        shell.update(cx, |shell, cx| shell.submit_team_dialog(cx));
        shell.read_with(cx, |shell, _| match &shell.team_dialog {
            Some(TeamDialog::NewTeam(form)) => {
                assert_eq!(form.error, Some("Name is required."));
            }
            other => panic!("the New team form should stay open, found {other:?}"),
        });
        assert_eq!(team_count(&shell, cx), 1, "nothing is created");
    }

    #[gpui::test]
    fn add_bot_opens_its_form_for_the_team_and_a_refusal_keeps_it(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update_in(cx, |shell, window, cx| {
            shell.open_add_bot(TeamId(1), window, cx);
        });
        shell.read_with(cx, |shell, _| match &shell.team_dialog {
            Some(TeamDialog::AddBot(form)) => assert_eq!(form.team, TeamId(1)),
            other => panic!("the Add bot form should open, found {other:?}"),
        });
        shell.update(cx, |shell, cx| shell.submit_team_dialog(cx));
        shell.read_with(cx, |shell, _| {
            assert!(matches!(shell.team_dialog, Some(TeamDialog::AddBot(_))));
            assert_eq!(shell.model().teams().team(TeamId(1)).unwrap().bots.len(), 3);
        });
    }

    #[gpui::test]
    fn the_bot_teams_home_draws_its_team_cards_and_closes_a_form(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            cx.notify();
        });
        assert_eq!(
            shell.read_with(cx, |shell, _| shell.model().navigation().mode),
            Mode::BotTeams
        );
        cx.update(|window, cx| window.draw(cx).clear(cx));
        shell.update_in(cx, |shell, window, cx| shell.open_new_team(window, cx));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        shell.update(cx, |shell, cx| shell.close_team_dialog(cx));
        shell.read_with(cx, |shell, _| assert!(shell.team_dialog.is_none()));
    }
}
