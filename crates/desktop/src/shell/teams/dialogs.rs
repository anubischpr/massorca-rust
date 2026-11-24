//! The Bot Teams dialogs: New team (screens 4.5), Add bot (D-AG-33) and the confirms for a
//! project or a memory entry. A form reads its fields when it is submitted; a refusal keeps it open
//! with the reason, and nothing changes until the state accepts the change.

use super::super::Shell;
use crate::model::agent_kinds::{
    KindChoice, bot_kind_choices, choice_at, first_available, sample_bot_kind_choices,
};
use crate::model::marketplace::MarketplaceItemId;
use crate::model::navigation::MarketplacePage;
use crate::model::teams::{
    HostChoice, MemoryId, NewBot, NewTeam, ProjectId, TeamError, TeamId, TeamPage, check_name,
    new_team_access_hint,
};
use crate::service::ServiceHandle;
use gpui::{
    App, Context, Entity, Focusable, SharedString, Subscription, WeakEntity, Window, div,
    prelude::*, px,
};
use massorca_protocol::methods::teams::{BotAddParams, TeamCreateParams};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::dialog::{Dialog, DialogVariant, DialogWidth};
use massorca_ui::components::popover::OverlayLayer;
use massorca_ui::components::select::{SelectEvent, SelectView};
use massorca_ui::components::switch::Switch;
use massorca_ui::components::text_field::{TextField, TextFieldEvent};
use massorca_ui::tokens::{SPACE_1, SPACE_3, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, TextStyled};

/// Why an Add bot form cannot be sent when the service lists no kind a bot can run on.
const NO_AGENT_KIND: &str = "No agent kind is installed on this service.";
/// Why an Add bot form cannot be sent when every kind the service lists is unavailable on this host.
const NO_AVAILABLE_KIND: &str = "No agent kind is available on this host.";

#[derive(Debug)]
pub(in crate::shell) struct NewTeamDialog {
    name: Entity<TextField>,
    purpose: Entity<TextField>,
    lead: Entity<SelectView>,
    host_select: Entity<SelectView>,
    host: usize,
    workspaces: bool,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

#[derive(Debug)]
pub(in crate::shell) struct AddBotDialog {
    team: TeamId,
    name: Entity<TextField>,
    role: Entity<TextField>,
    /// The kinds the form offers, in the order the service lists them (`agent.kinds`).
    kinds: Vec<KindChoice>,
    agent: usize,
    agent_select: Entity<SelectView>,
    host_select: Entity<SelectView>,
    /// The offered kinds this host cannot run, each with the Marketplace item its link opens.
    unavailable: Vec<UnavailableKind>,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

/// A kind the Add bot form lists as unavailable on this host, and the Marketplace item of that kind.
#[derive(Debug)]
struct UnavailableKind {
    id: String,
    name: String,
    item: MarketplaceItemId,
}

#[derive(Debug)]
pub(in crate::shell) struct EditBriefDialog {
    team: TeamId,
    project: ProjectId,
    field: Entity<TextField>,
    error: Option<SharedString>,
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
        /// Why the service refused the last delete, shown until the confirm closes.
        error: Option<String>,
    },
    DeleteMemory {
        team: TeamId,
        memory: MemoryId,
    },
}

/// A field whose changes redraw the dialog, so its buttons follow what is typed. Enter submits.
fn watched_field(
    label: &'static str,
    placeholder: &'static str,
    window: &mut Window,
    cx: &mut Context<Shell>,
) -> (Entity<TextField>, Subscription) {
    let field = cx.new(|cx| TextField::new(cx).label(label).placeholder(placeholder));
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
    pub(in crate::shell) fn open_new_team(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.model.teams().is_connected() {
            return;
        }
        let (name, name_events) = watched_field("Name", "Team name", window, cx);
        let (purpose, purpose_events) =
            watched_field("Purpose", "What the team is for (optional)", window, cx);
        // A new team has no bots yet, so there is no lead bot to choose (4.5).
        let lead = cx.new(|cx| {
            SelectView::new(Vec::<String>::new(), cx)
                .label("Lead bot")
                .placeholder("No bots yet")
                .disabled(true)
                .layer(OverlayLayer::Dialog)
        });
        let hosts: Vec<String> = self
            .model
            .teams()
            .hosts()
            .iter()
            .map(|host| host.name.clone())
            .collect();
        let host_select = cx.new(|cx| {
            SelectView::new(hosts, cx)
                .label("Host")
                .selected(Some(0))
                .layer(OverlayLayer::Dialog)
        });
        let host_events = cx.subscribe_in(
            &host_select,
            window,
            |shell, _, event: &SelectEvent, _, cx| match event {
                SelectEvent::Changed(index) => shell.set_new_team_host(*index, cx),
            },
        );
        self.team_dialog = Some(TeamDialog::NewTeam(NewTeamDialog {
            name,
            purpose,
            lead,
            host_select,
            host: 0,
            workspaces: false,
            error: None,
            _subscriptions: vec![name_events, purpose_events, host_events],
        }));
        cx.notify();
    }

    /// "Add bot" on a team card (D-AG-33).
    pub(in crate::shell) fn open_add_bot(
        &mut self,
        team: TeamId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.model.teams().is_connected() {
            return;
        }
        let (name, name_events) = watched_field("Name", "Bot name", window, cx);
        let (role, role_events) =
            watched_field("Role", "What this bot is responsible for", window, cx);
        // A connected service offers the kinds it lists as installed; sample data offers the four built-ins.
        let kinds = match &self.service {
            Some(_) => bot_kind_choices(&self.store.read(cx).state().agent_kinds),
            None => sample_bot_kind_choices(),
        };
        let agent = first_available(&kinds).unwrap_or(0);
        // A kind this host cannot run, or that is turned off, is listed disabled, with why; the form cannot choose it.
        let agent_select = cx.new(|cx| {
            let mut select = SelectView::new(kinds.iter().map(|kind| kind.name.clone()), cx)
                .label("Agent")
                .selected(first_available(&kinds))
                .layer(OverlayLayer::Dialog);
            for (index, reason) in kinds
                .iter()
                .enumerate()
                .filter_map(|(index, kind)| kind.unavailable.map(|reason| (index, reason)))
            {
                select = select.option_unavailable(index, reason);
            }
            select
        });
        let marketplace = self.model.marketplace();
        let unavailable: Vec<UnavailableKind> = kinds
            .iter()
            .filter(|kind| kind.unavailable.is_some())
            .filter_map(|kind| {
                let item = marketplace
                    .items
                    .iter()
                    .find(|item| item.agent_kind.as_deref() == Some(kind.id.as_str()))?;
                Some(UnavailableKind {
                    id: kind.id.clone(),
                    name: kind.name.clone(),
                    item: item.id,
                })
            })
            .collect();
        let agent_events = cx.subscribe_in(
            &agent_select,
            window,
            |shell, _, event: &SelectEvent, _, cx| match event {
                SelectEvent::Changed(index) => shell.set_add_bot_agent(*index, cx),
            },
        );
        // Bots run on the team's host; choosing another host waits for per-bot hosts.
        let team_host = self
            .model
            .teams()
            .team(team)
            .map(|team| team.host.clone())
            .unwrap_or_default();
        let host_select = cx.new(|cx| {
            SelectView::new([team_host], cx)
                .label("Host")
                .selected(Some(0))
                .disabled(true)
                .layer(OverlayLayer::Dialog)
        });
        self.team_dialog = Some(TeamDialog::AddBot(AddBotDialog {
            team,
            name,
            role,
            kinds,
            agent,
            agent_select,
            host_select,
            unavailable,
            error: None,
            _subscriptions: vec![name_events, role_events, agent_events],
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

    /// Chooses the kind at `index`. A kind this host cannot run is refused, so the form keeps its choice.
    pub(super) fn set_add_bot_agent(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(TeamDialog::AddBot(dialog)) = &mut self.team_dialog
            && choice_at(&dialog.kinds, index).is_some()
        {
            dialog.agent = index;
        }
        cx.notify();
    }

    /// Creates a team or adds a bot from the open form. A refusal keeps the form open with why.
    pub(super) fn submit_team_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.team_dialog.take() else {
            return;
        };
        // With a service connected, the form goes to it; sample data changes locally, as before.
        let dialog = match (dialog, self.service.clone()) {
            (TeamDialog::NewTeam(dialog), Some(service)) => {
                return self.send_new_team(service, dialog, cx);
            }
            (TeamDialog::AddBot(dialog), Some(service)) => {
                return self.send_add_bot(service, dialog, cx);
            }
            (TeamDialog::EditBrief(dialog), Some(service)) => {
                return self.send_edit_brief(service, dialog, cx);
            }
            (dialog, _) => dialog,
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
                        dialog.error = Some(error.message().into());
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
                        dialog.error = Some(error.message().into());
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
                        kind: dialog
                            .kinds
                            .get(dialog.agent)
                            .map(|kind| kind.id.clone())
                            .unwrap_or_default(),
                    },
                );
                match result {
                    Ok(_) => None,
                    Err(error) => {
                        dialog.error = Some(error.message().into());
                        Some(TeamDialog::AddBot(dialog))
                    }
                }
            }
        };
        cx.notify();
    }

    /// Sends the New team form. A name the form cannot use reopens it at once; the service's refusal reopens it
    /// with its message. An accepted team shows once the teams are read again.
    fn send_new_team(
        &mut self,
        service: ServiceHandle,
        dialog: NewTeamDialog,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
        let name = dialog.name.read(cx).text().to_owned();
        let purpose = dialog.purpose.read(cx).text().to_owned();
        let workspaces = dialog.workspaces;
        let form = TeamDialog::NewTeam(dialog);
        let name = match check_name(&name) {
            Ok(name) => name,
            Err(error) => return self.finish_team_dialog(form, Some(error.message().into()), cx),
        };
        let params = TeamCreateParams {
            name,
            purpose: Self::non_empty(&purpose),
            character: None,
            // The service picks: full for a host-admin caller, default for a standard device, which may not ask for full.
            access: None,
            workspaces: Some(workspaces),
        };
        cx.spawn(async move |this, cx| {
            let answer = service.create_team(params).await;
            this.update(cx, |shell, cx| {
                let message = answer.err().map(|refusal| refusal.message().into());
                shell.finish_team_dialog(form, message, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Sends the Add bot form, as `send_new_team` sends a team. The bot's team is the service's id for it.
    fn send_add_bot(
        &mut self,
        service: ServiceHandle,
        dialog: AddBotDialog,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
        let name = dialog.name.read(cx).text().to_owned();
        let job = dialog.role.read(cx).text().trim().to_owned();
        let kind = choice_at(&dialog.kinds, dialog.agent).map(|kind| kind.id.clone());
        let no_kinds = dialog.kinds.is_empty();
        let team = self.team_key(dialog.team, cx);
        let form = TeamDialog::AddBot(dialog);
        let name = match check_name(&name) {
            Ok(name) => name,
            Err(error) => return self.finish_team_dialog(form, Some(error.message().into()), cx),
        };
        let Some(team) = team else {
            let missing = TeamError::Missing.message();
            return self.finish_team_dialog(form, Some(missing.into()), cx);
        };
        // The bot's kind is the service's id for the kind the form chose, never one the desktop made up.
        let Some(kind) = kind else {
            let message = if no_kinds {
                NO_AGENT_KIND
            } else {
                NO_AVAILABLE_KIND
            };
            return self.finish_team_dialog(form, Some(message.into()), cx);
        };
        let params = BotAddParams {
            team,
            name,
            job,
            kind,
            model: None,
            effort: None,
            lead: false,
        };
        cx.spawn(async move |this, cx| {
            let answer = service.add_bot(params).await;
            this.update(cx, |shell, cx| {
                let message = answer.err().map(|refusal| refusal.message().into());
                shell.finish_team_dialog(form, message, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Sends the brief of the Edit brief form, as `send_add_bot` sends a bot. A refusal reopens the form with its
    /// message; an accepted brief shows once the teams are read again.
    fn send_edit_brief(
        &mut self,
        service: ServiceHandle,
        dialog: EditBriefDialog,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
        let brief = dialog.field.read(cx).text().trim().to_owned();
        let team = self.team_key(dialog.team, cx);
        let project = self.service_key(dialog.project.0, cx);
        let form = TeamDialog::EditBrief(dialog);
        let (Some(team), Some(project)) = (team, project) else {
            let missing = TeamError::Missing.message();
            return self.finish_team_dialog(form, Some(missing.into()), cx);
        };
        cx.spawn(async move |this, cx| {
            let answer = service.save_brief(team, project, brief).await;
            this.update(cx, |shell, cx| {
                let message = answer.err().map(|refusal| refusal.message().into());
                shell.finish_team_dialog(form, message, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Sends a project delete to the service, as `send_add_bot` sends a bot. The project's page goes back to its
    /// team's Board once the service accepts; a refusal reopens the confirm with its message.
    fn send_delete_project(
        &mut self,
        service: ServiceHandle,
        confirm: TeamConfirm,
        cx: &mut Context<Self>,
    ) {
        cx.notify();
        let TeamConfirm::DeleteProject { team, project, .. } = &confirm else {
            return;
        };
        let (team, project) = (*team, *project);
        let (Some(team_key), Some(project_key)) =
            (self.team_key(team, cx), self.service_key(project.0, cx))
        else {
            let missing = TeamError::Missing.message();
            return self.finish_team_confirm(confirm, Some(missing.into()), cx);
        };
        cx.spawn(async move |this, cx| {
            let answer = service.delete_project(team_key, project_key).await;
            this.update(cx, |shell, cx| {
                match answer {
                    Ok(_) => shell.model.open_team_page(TeamPage::Board(team)),
                    Err(refusal) => {
                        shell.finish_team_confirm(confirm, Some(refusal.message().into()), cx);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Reopens a confirm the service refused, with its message, unless another confirm has opened meanwhile.
    fn finish_team_confirm(
        &mut self,
        confirm: TeamConfirm,
        message: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let Some(message) = message else {
            return cx.notify();
        };
        if self.team_confirm.is_some() {
            return cx.notify();
        }
        self.team_confirm = Some(match confirm {
            TeamConfirm::DeleteProject {
                team,
                project,
                name,
                ..
            } => TeamConfirm::DeleteProject {
                team,
                project,
                name,
                error: Some(message.to_string()),
            },
            other => other,
        });
        cx.notify();
    }

    /// Closes a form the service accepted (`message` is `None`). Otherwise the form comes back with the message,
    /// unless another form has been opened meanwhile.
    fn finish_team_dialog(
        &mut self,
        mut form: TeamDialog,
        message: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let Some(message) = message else {
            return cx.notify();
        };
        if self.team_dialog.is_some() {
            return cx.notify();
        }
        match &mut form {
            TeamDialog::NewTeam(dialog) => dialog.error = Some(message),
            TeamDialog::AddBot(dialog) => dialog.error = Some(message),
            TeamDialog::EditBrief(dialog) => dialog.error = Some(message),
        }
        self.team_dialog = Some(form);
        cx.notify();
    }

    /// The service's id for a team of this shell, as the store numbered it.
    pub(super) fn team_key(&self, team: TeamId, cx: &App) -> Option<String> {
        self.service_key(team.0, cx)
    }

    /// The service's id for the item the store numbered `number`: a team, a project or a card.
    fn service_key(&self, number: u32, cx: &App) -> Option<String> {
        self.store
            .read(cx)
            .state()
            .team_ids
            .key(number)
            .map(str::to_owned)
    }

    /// The text without its surrounding spaces, or none when it is blank.
    fn non_empty(text: &str) -> Option<String> {
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
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
                .label("Brief")
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
        match (confirm, self.service.clone()) {
            (confirm @ TeamConfirm::DeleteProject { .. }, Some(service)) => {
                self.send_delete_project(service, confirm, cx);
            }
            (TeamConfirm::DeleteProject { team, project, .. }, None) => {
                if self.model.teams_mut().delete_project(team, project).is_ok() {
                    self.model.open_team_page(TeamPage::Board(team));
                }
            }
            (TeamConfirm::DeleteMemory { team, memory }, _) => {
                let _ = self.model.teams_mut().delete_memory(team, memory);
            }
        }
        cx.notify();
    }
}

/// The team dialog on screen: New team or Add bot.
pub(in crate::shell) fn team_dialog_view(
    dialog: &TeamDialog,
    _hosts: &[HostChoice],
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> Dialog {
    match dialog {
        TeamDialog::NewTeam(form) => new_team_view(form, shell, cx),
        TeamDialog::AddBot(form) => add_bot_view(form, shell, cx),
        TeamDialog::EditBrief(form) => edit_brief_view(form, shell, cx),
    }
}

fn new_team_view(form: &NewTeamDialog, shell: &WeakEntity<Shell>, cx: &App) -> Dialog {
    let theme = *cx.theme();
    let workspaces = {
        let shell = shell.clone();
        Switch::new("new-team-workspaces", form.workspaces)
            .label("Turn on Workspaces in this team")
            .on_change(move |on, _, cx| {
                let _ = shell.update(cx, |shell, cx| shell.set_new_team_workspaces(on, cx));
            })
    };
    // Plain text wraps; the Switch's own helper line does not wrap inside a dialog.
    let helper = div()
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.secondary)
        .child("A Development team can use Workspaces depth. You can change this later.");
    let body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(form.name.clone())
        .child(form.purpose.clone())
        .child(form.lead.clone())
        .child(form.host_select.clone())
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(SPACE_1))
                .child(workspaces)
                .child(helper),
        )
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(new_team_access_hint()),
        );
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
    if let Some(error) = form.error.clone() {
        dialog = dialog.error(error);
    }
    dialog
}

fn add_bot_view(form: &AddBotDialog, shell: &WeakEntity<Shell>, cx: &App) -> Dialog {
    let body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(form.name.clone())
        .child(form.role.clone())
        .child(form.agent_select.clone())
        .child(form.host_select.clone())
        .children(
            form.unavailable
                .iter()
                .map(|kind| unavailable_link(kind, shell)),
        );
    let can_submit = !form.name.read(cx).text().trim().is_empty();
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    let mut dialog = Dialog::new("add-bot", "Add bot")
        .open(true)
        .width(DialogWidth::Large)
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
    if let Some(error) = form.error.clone() {
        dialog = dialog.error(error);
    }
    dialog
}

/// The link to an unavailable kind's Marketplace page. Following it closes the form.
fn unavailable_link(kind: &UnavailableKind, shell: &WeakEntity<Shell>) -> impl IntoElement {
    let (shell, item) = (shell.clone(), kind.item);
    Button::new(
        format!("add-bot-marketplace-{}", kind.id),
        format!("Open {} in Marketplace", kind.name),
    )
    .variant(ButtonVariant::Ghost)
    .on_click(move |_, cx| {
        let _ = shell.update(cx, |shell, cx| {
            shell.close_team_dialog(cx);
            shell.open_marketplace(MarketplacePage::Item(item), cx);
        });
    })
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
    if let Some(error) = form.error.clone() {
        dialog = dialog.error(error);
    }
    dialog
}

/// The confirm on screen: a project's Board goes with it; a memory entry is forgotten.
pub(in crate::shell) fn team_confirm_view(
    confirm: &TeamConfirm,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> Dialog {
    let theme = *cx.theme();
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    let error = match confirm {
        TeamConfirm::DeleteProject { error, .. } => error.clone(),
        TeamConfirm::DeleteMemory { .. } => None,
    };
    let (dialog, body, action) = match confirm {
        TeamConfirm::DeleteProject { name, .. } => (
            Dialog::new("delete-project", format!("Delete project {name}?"))
                .variant(DialogVariant::Destructive),
            "Its tasks are removed from the Board. This cannot be undone.",
            "Delete project",
        ),
        TeamConfirm::DeleteMemory { .. } => (
            Dialog::new("delete-memory", "Delete this memory?"),
            "The team's bots stop using this note.",
            "Delete memory",
        ),
    };
    let dialog = dialog
        .open(true)
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.secondary)
                .child(body),
        )
        .action(action)
        .on_action(move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.confirm_team_delete(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.close_team_confirm(cx));
        });
    match error {
        Some(message) => dialog.error(SharedString::from(message)),
        None => dialog,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::SampleDataSource;
    use crate::model::marketplace::MarketplaceItemId;
    use crate::model::navigation::{MarketplacePage, Mode, Page};
    use crate::model::teams::{ProjectId, SampleTeamsSource, TeamId, TeamPage};
    use crate::service::fixtures::{agent_kind, team};
    use crate::service::{
        Command, ProjectRecord, Refusal, StoreUpdate, TeamCommand, TeamRecord, TeamsRead,
    };
    use crate::shell::service_tests::{click, draw, rig};
    use gpui::{Entity, TestAppContext, VisualTestContext};
    use massorca_protocol::methods::agents::PackageState;
    use massorca_protocol::methods::teams::{Board, TeamProject};
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
                assert_eq!(form.error.as_deref(), Some("Name is required."));
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
    fn add_bot_offers_the_agents_and_the_teams_host(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update_in(cx, |shell, window, cx| {
            shell.open_add_bot(TeamId(1), window, cx);
        });
        shell.read_with(cx, |shell, cx| match &shell.team_dialog {
            Some(TeamDialog::AddBot(form)) => {
                let agent = form
                    .agent_select
                    .read(cx)
                    .selected_label()
                    .map(ToString::to_string);
                let host = form
                    .host_select
                    .read(cx)
                    .selected_label()
                    .map(ToString::to_string);
                assert_eq!(agent.as_deref(), Some("Claude"));
                assert_eq!(host.as_deref(), Some("VPS"));
            }
            other => panic!("the Add bot form should open, found {other:?}"),
        });
    }

    #[gpui::test]
    fn new_team_offers_the_hosts_with_this_pc_chosen(cx: &mut TestAppContext) {
        let (shell, cx) = shell(cx);
        shell.update_in(cx, |shell, window, cx| shell.open_new_team(window, cx));
        shell.read_with(cx, |shell, cx| match &shell.team_dialog {
            Some(TeamDialog::NewTeam(form)) => {
                let host = form
                    .host_select
                    .read(cx)
                    .selected_label()
                    .map(ToString::to_string);
                assert_eq!(host.as_deref(), Some("This PC"));
                assert_eq!(form.host, 0);
            }
            other => panic!("the New team form should open, found {other:?}"),
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

    #[gpui::test]
    fn a_kind_the_host_does_not_detect_is_listed_unavailable_and_its_link_opens_its_page(
        cx: &mut TestAppContext,
    ) {
        let (rig, cx) = rig(cx, &[("p1", "alpha")]);
        let mut codex = agent_kind("codex", "Codex", PackageState::Enabled);
        codex.detected = false;
        rig.updates
            .send(StoreUpdate::AgentKindsResync(vec![
                agent_kind("claude", "Claude", PackageState::Enabled),
                codex,
            ]))
            .expect("the store listens");
        rig.updates
            .send(StoreUpdate::Teams(TeamsRead {
                number: 1,
                teams: Ok(vec![team("t1", "Desarrollo")]),
            }))
            .expect("the store listens");
        cx.run_until_parked();
        rig.shell.update_in(cx, |shell, window, cx| {
            shell.open_add_bot(TeamId(1), window, cx);
        });
        cx.run_until_parked();
        draw(cx);
        rig.shell
            .read_with(cx, |shell, cx| match &shell.team_dialog {
                Some(TeamDialog::AddBot(form)) => {
                    let select = form.agent_select.read(cx);
                    assert_eq!(
                        select.selected_label().map(ToString::to_string).as_deref(),
                        Some("Claude")
                    );
                    assert_eq!(
                        select.option_reason(1).map(ToString::to_string).as_deref(),
                        Some("Not available on this host")
                    );
                }
                other => panic!("the Add bot form should open, found {other:?}"),
            });
        rig.shell
            .update(cx, |shell, cx| shell.set_add_bot_agent(1, cx));
        rig.shell
            .read_with(cx, |shell, _| match &shell.team_dialog {
                Some(TeamDialog::AddBot(form)) => {
                    assert_eq!(form.agent, 0, "the unavailable kind is refused")
                }
                other => panic!("the Add bot form should stay open, found {other:?}"),
            });
        click(cx, "button:add-bot-marketplace-codex");
        rig.shell.read_with(cx, |shell, _| {
            assert!(shell.team_dialog.is_none(), "the form closes for the page");
            assert_eq!(
                shell.model().navigation().page,
                Page::Marketplace(MarketplacePage::Item(MarketplaceItemId(2)))
            );
        });
    }

    #[gpui::test]
    fn a_turned_off_kind_is_listed_turned_off_and_its_link_opens_its_page(cx: &mut TestAppContext) {
        let (rig, cx) = rig(cx, &[("p1", "alpha")]);
        rig.updates
            .send(StoreUpdate::AgentKindsResync(vec![
                agent_kind("claude", "Claude", PackageState::Enabled),
                agent_kind("codex", "Codex", PackageState::Disabled),
            ]))
            .expect("the store listens");
        rig.updates
            .send(StoreUpdate::Teams(TeamsRead {
                number: 1,
                teams: Ok(vec![team("t1", "Desarrollo")]),
            }))
            .expect("the store listens");
        cx.run_until_parked();
        rig.shell.update_in(cx, |shell, window, cx| {
            shell.open_add_bot(TeamId(1), window, cx);
        });
        cx.run_until_parked();
        draw(cx);
        rig.shell
            .read_with(cx, |shell, cx| match &shell.team_dialog {
                Some(TeamDialog::AddBot(form)) => {
                    let select = form.agent_select.read(cx);
                    assert_eq!(
                        select.selected_label().map(ToString::to_string).as_deref(),
                        Some("Claude")
                    );
                    assert_eq!(
                        select.option_reason(1).map(ToString::to_string).as_deref(),
                        Some("Turned off")
                    );
                }
                other => panic!("the Add bot form should open, found {other:?}"),
            });
        rig.shell
            .update(cx, |shell, cx| shell.set_add_bot_agent(1, cx));
        rig.shell
            .read_with(cx, |shell, _| match &shell.team_dialog {
                Some(TeamDialog::AddBot(form)) => {
                    assert_eq!(form.agent, 0, "the turned-off kind is refused")
                }
                other => panic!("the Add bot form should stay open, found {other:?}"),
            });
        click(cx, "button:add-bot-marketplace-codex");
        rig.shell.read_with(cx, |shell, _| {
            assert!(shell.team_dialog.is_none(), "the form closes for the page");
            assert_eq!(
                shell.model().navigation().page,
                Page::Marketplace(MarketplacePage::Item(MarketplaceItemId(2)))
            );
        });
    }

    /// Team t1 with one project, p1, named Checkout and briefed "Old".
    fn team_with_checkout() -> TeamRecord {
        let mut record = team("t1", "Desarrollo");
        record.projects.push(ProjectRecord {
            project: checkout(),
            board: Board {
                team_id: "t1".into(),
                project_id: Some("p1".into()),
                columns: Vec::new(),
            },
        });
        record
    }

    fn checkout() -> TeamProject {
        TeamProject {
            id: "p1".into(),
            team_id: "t1".into(),
            name: "Checkout".into(),
            goal: String::new(),
            brief: "Old".into(),
            created_at_ms: 1,
            source_project: None,
            source_project_name: None,
            source_kind: None,
            code_source: None,
        }
    }

    #[gpui::test]
    fn a_brief_on_a_connected_service_is_sent_and_its_form_closes_once_accepted(
        cx: &mut TestAppContext,
    ) {
        let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
        rig.updates
            .send(StoreUpdate::Teams(TeamsRead {
                number: 1,
                teams: Ok(vec![team_with_checkout()]),
            }))
            .expect("the store listens");
        cx.run_until_parked();
        rig.shell.update_in(cx, |shell, window, cx| {
            shell.open_edit_brief(TeamId(1), ProjectId(2), window, cx);
        });
        cx.run_until_parked();
        rig.shell
            .update(cx, |shell, cx| shell.submit_team_dialog(cx));
        cx.run_until_parked();
        let Some(Command::Team(TeamCommand::SaveBrief {
            team,
            project,
            brief,
            reply,
        })) = rig.script.next()
        else {
            panic!("the brief goes to the service");
        };
        assert_eq!(
            (team.as_str(), project.as_str(), brief.as_str()),
            ("t1", "p1", "Old")
        );
        let _ = reply.send(Ok(checkout()));
        cx.run_until_parked();
        rig.shell.read_with(cx, |shell, _| {
            assert!(shell.team_dialog.is_none(), "the form closes once accepted")
        });
    }

    #[gpui::test]
    fn a_brief_the_service_refuses_keeps_its_form_open_with_the_reason(cx: &mut TestAppContext) {
        let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
        rig.updates
            .send(StoreUpdate::Teams(TeamsRead {
                number: 1,
                teams: Ok(vec![team_with_checkout()]),
            }))
            .expect("the store listens");
        cx.run_until_parked();
        rig.shell.update_in(cx, |shell, window, cx| {
            shell.open_edit_brief(TeamId(1), ProjectId(2), window, cx);
        });
        cx.run_until_parked();
        rig.shell
            .update(cx, |shell, cx| shell.submit_team_dialog(cx));
        cx.run_until_parked();
        let Some(Command::Team(TeamCommand::SaveBrief { reply, .. })) = rig.script.next() else {
            panic!("the brief goes to the service");
        };
        let _ = reply.send(Err(Refusal::Failed {
            code: "team.project_not_found".into(),
            message: "That item is gone. Nothing was changed.".into(),
        }));
        cx.run_until_parked();
        rig.shell
            .read_with(cx, |shell, _| match &shell.team_dialog {
                Some(TeamDialog::EditBrief(form)) => assert_eq!(
                    form.error.as_deref(),
                    Some("That item is gone. Nothing was changed.")
                ),
                other => {
                    panic!("the Edit brief form should stay open with the reason, found {other:?}")
                }
            });
    }

    #[gpui::test]
    fn deleting_a_project_on_a_connected_service_opens_its_team_board_once_accepted(
        cx: &mut TestAppContext,
    ) {
        let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
        rig.updates
            .send(StoreUpdate::Teams(TeamsRead {
                number: 1,
                teams: Ok(vec![team_with_checkout()]),
            }))
            .expect("the store listens");
        cx.run_until_parked();
        rig.shell.update(cx, |shell, cx| {
            shell.model.press_mode_switch();
            shell
                .model
                .open_team_page(TeamPage::Project(TeamId(1), ProjectId(2)));
            cx.notify();
        });
        cx.run_until_parked();
        rig.shell.update(cx, |shell, cx| {
            shell.open_team_confirm(
                TeamConfirm::DeleteProject {
                    team: TeamId(1),
                    project: ProjectId(2),
                    name: "Checkout".into(),
                    error: None,
                },
                cx,
            );
        });
        rig.shell
            .update(cx, |shell, cx| shell.confirm_team_delete(cx));
        cx.run_until_parked();
        let Some(Command::Team(TeamCommand::DeleteProject {
            team,
            project,
            reply,
        })) = rig.script.next()
        else {
            panic!("the delete goes to the service");
        };
        assert_eq!((team.as_str(), project.as_str()), ("t1", "p1"));
        let _ = reply.send(Ok(checkout()));
        cx.run_until_parked();
        rig.shell.read_with(cx, |shell, _| {
            assert_eq!(
                shell.model().navigation().page,
                Page::Team(TeamPage::Board(TeamId(1)))
            )
        });
    }
}
