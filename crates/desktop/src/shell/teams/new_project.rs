//! The New project dialog of a team (pv96): a name, a goal and, in a team with Workspaces, the code's source.
//! The service keeps the project; sample data keeps its name only.

use super::super::Shell;
use super::dialogs::{TeamDialog, watched_field};
use crate::model::teams::{NewProject, ProjectDraft, TeamError, TeamId};
use crate::service::ServiceHandle;
use gpui::{
    App, Context, Entity, Focusable, SharedString, Subscription, WeakEntity, Window, div,
    prelude::*, px,
};
use massorca_protocol::methods::teams::{CodeSource, TeamProjectAddParams};
use massorca_ui::components::button::ButtonSize;
use massorca_ui::components::dialog::Dialog;
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::components::select::{SelectEvent, SelectView};
use massorca_ui::components::text_field::TextField;
use massorca_ui::tokens::{SPACE_3, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, IconName, TextStyled};

/// The code sources a team with Workspaces offers, in the order the form lists them (pv96).
const CODE_SOURCES: [CodeSource; 2] =
    [CodeSource::ExistingRepository, CodeSource::NewFolderWithGit];

/// The New project form: a name and a goal, and the code source when the team has Workspaces on.
#[derive(Debug)]
pub(in crate::shell) struct NewProjectDialog {
    team: TeamId,
    name: Entity<TextField>,
    goal: Entity<TextField>,
    workspaces: bool,
    source_select: Entity<SelectView>,
    source: CodeSource,
    pub(super) error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl NewProjectDialog {
    /// The form's values, checked: a team without Workspaces records no code source.
    fn draft(&self, cx: &App) -> Result<ProjectDraft, TeamError> {
        NewProject {
            name: self.name.read(cx).text().to_owned(),
            goal: self.goal.read(cx).text().to_owned(),
            code_source: Some(self.source),
        }
        .check(self.workspaces)
    }
}

impl Shell {
    /// "New project" in the Sidebar's PROJECTS header (4.1). Nothing opens without a connected service.
    fn open_new_project(&mut self, team: TeamId, window: &mut Window, cx: &mut Context<Self>) {
        if !self.model.teams().is_connected() {
            return;
        }
        let workspaces = self
            .model
            .teams()
            .team(team)
            .is_some_and(|team| team.workspaces);
        let (name, name_events) = watched_field("Name", "Project name", window, cx);
        let (goal, goal_events) =
            watched_field("Goal", "What the project is for (optional)", window, cx);
        let source_select = cx.new(|cx| {
            SelectView::new(CODE_SOURCES.map(CodeSource::label), cx)
                .label("Code source")
                .selected(Some(0))
                .layer(OverlayLayer::Dialog)
        });
        let source_events = cx.subscribe_in(
            &source_select,
            window,
            |shell, _, event: &SelectEvent, _, cx| match event {
                SelectEvent::Changed(index) => shell.set_new_project_source(*index, cx),
            },
        );
        self.team_dialog = Some(TeamDialog::NewProject(NewProjectDialog {
            team,
            name,
            goal,
            workspaces,
            source_select,
            source: CodeSource::ExistingRepository,
            error: None,
            _subscriptions: vec![name_events, goal_events, source_events],
        }));
        cx.notify();
    }

    /// Chooses the code source at `index`; an index the form does not offer keeps the choice.
    fn set_new_project_source(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(TeamDialog::NewProject(dialog)) = &mut self.team_dialog
            && let Some(source) = CODE_SOURCES.get(index)
        {
            dialog.source = *source;
        }
        cx.notify();
    }

    /// Sends the New project form. A form the checks refuse reopens at once; the service's refusal reopens it with
    /// its message, and an accepted project shows once the teams are read again.
    pub(super) fn send_new_project(
        &mut self,
        service: ServiceHandle,
        dialog: NewProjectDialog,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
        let team = dialog.team;
        let draft = dialog.draft(cx);
        let form = TeamDialog::NewProject(dialog);
        let draft = match draft {
            Ok(draft) => draft,
            Err(error) => return self.finish_team_dialog(form, Some(error.message().into()), cx),
        };
        let Some(team) = self.team_key(team, cx) else {
            let missing = TeamError::Missing.message();
            return self.finish_team_dialog(form, Some(missing.into()), cx);
        };
        let params = TeamProjectAddParams {
            team,
            name: draft.name,
            goal: draft.goal,
            brief: None,
            source_project: None,
            code_source: draft.code_source,
        };
        cx.spawn(async move |this, cx| {
            let answer = service.add_team_project(params).await;
            this.update(cx, |shell, cx| {
                let message = answer.err().map(|refusal| refusal.message().into());
                shell.finish_team_dialog(form, message, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Adds the project of the New project form on sample data.
    pub(super) fn save_project_locally(
        &mut self,
        dialog: &NewProjectDialog,
        cx: &App,
    ) -> Result<(), TeamError> {
        let draft = dialog.draft(cx)?;
        self.model
            .teams_mut()
            .create_project(dialog.team, &draft.name)
            .map(|_| ())
    }
}

/// The PROJECTS header's New project control: opens the New project form for the team in view.
pub(super) fn new_project_button(team: TeamId, shell: &WeakEntity<Shell>) -> IconButton {
    let shell = shell.clone();
    IconButton::new("teams-new-project", IconName::Plus, "New project")
        .size(ButtonSize::Compact)
        .on_click(move |window, cx| {
            let _ = shell.update(cx, |shell, cx| shell.open_new_project(team, window, cx));
        })
}

/// The New project form on screen.
pub(super) fn new_project_view(
    form: &NewProjectDialog,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> Dialog {
    let theme = *cx.theme();
    let helper = div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.secondary)
        .child("A project holds the why. Its tasks hold the how.");
    let mut body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(helper)
        .child(form.name.clone())
        .child(form.goal.clone());
    if form.workspaces {
        body = body.child(form.source_select.clone());
    }
    let can_submit = !form.name.read(cx).text().trim().is_empty();
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    let mut dialog = Dialog::new("new-project", "New project")
        .open(true)
        .child(body)
        .field(form.name.read(cx).focus_handle(cx))
        .action("Create project")
        .action_disabled(!can_submit)
        .on_action(move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.submit_team_dialog(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.close_team_dialog(cx));
        });
    if let Some(error) = form.error.clone() {
        dialog = dialog.error(error);
    }
    dialog
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::fixtures::team;
    use crate::service::{Command, StoreUpdate, TeamCommand, TeamsRead};
    use crate::shell::service_tests::rig;
    use gpui::TestAppContext;

    #[gpui::test]
    fn a_project_is_sent_with_its_goal_and_a_code_source_when_the_team_has_workspaces(
        cx: &mut TestAppContext,
    ) {
        let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
        let mut record = team("t1", "Desarrollo");
        record.team.workspaces = true;
        rig.updates
            .send(StoreUpdate::Teams(TeamsRead {
                number: 1,
                teams: Ok(vec![record]),
            }))
            .expect("the store listens");
        cx.run_until_parked();
        rig.shell.update_in(cx, |shell, window, cx| {
            shell.open_new_project(TeamId(1), window, cx);
        });
        cx.run_until_parked();
        let (name, goal) = rig
            .shell
            .read_with(cx, |shell, _| match &shell.team_dialog {
                Some(TeamDialog::NewProject(form)) => (form.name.clone(), form.goal.clone()),
                other => panic!("the New project form should open, found {other:?}"),
            });
        name.update_in(cx, |field, window, cx| {
            field.set_text("Checkout", window, cx)
        });
        goal.update_in(cx, |field, window, cx| {
            field.set_text("Ship the cart", window, cx)
        });
        rig.shell
            .update(cx, |shell, cx| shell.submit_team_dialog(cx));
        cx.run_until_parked();
        let Some(Command::Team(TeamCommand::AddProject { params, .. })) = rig.script.next() else {
            panic!("the project goes to the service");
        };
        assert_eq!(
            (params.team.as_str(), params.name.as_str()),
            ("t1", "Checkout")
        );
        assert_eq!(params.goal.as_deref(), Some("Ship the cart"));
        assert_eq!(params.code_source, Some(CodeSource::ExistingRepository));
    }
}
