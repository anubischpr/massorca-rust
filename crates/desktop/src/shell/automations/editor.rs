//! The automation editor (decision 0123): the fields of a new or an edited automation, read into the form the model
//! checks before it is sent. Cadence and weekday are choices; the cron and time fields apply to their own cadences.

use super::super::Shell;
use crate::model::automations::{
    AutomationForm, Cadence, FormDestination, WEEKDAYS, weekday_label,
};
use gpui::{
    App, Context, Entity, Focusable, ParentElement, Styled, WeakEntity, div, prelude::*, px,
};
use massorca_protocol::methods::automations::{Automation, AutomationWeekday};
use massorca_ui::components::dialog::Dialog;
use massorca_ui::components::select::SelectView;
use massorca_ui::components::text_field::TextField;
use massorca_ui::tokens::SPACE_3;

/// The editor's fields while it is open. `editing` is the automation's id; `None` is a new automation.
#[derive(Debug)]
pub(in crate::shell) struct AutomationEditor {
    pub editing: Option<String>,
    /// Why the last save was refused, shown in the editor until the next attempt.
    pub error: Option<String>,
    name: Entity<TextField>,
    project: Entity<TextField>,
    destination: Entity<SelectView>,
    workspace: Entity<TextField>,
    base_branch: Entity<TextField>,
    prompt: Entity<TextField>,
    agent: Entity<TextField>,
    cadence: Entity<SelectView>,
    time: Entity<TextField>,
    weekday: Entity<SelectView>,
    cron: Entity<TextField>,
    zone: Entity<TextField>,
}

impl AutomationEditor {
    /// The editor on `automation`'s values, or on a new automation's starting values.
    pub(in crate::shell) fn new(automation: Option<&Automation>, cx: &mut Context<Shell>) -> Self {
        let form = automation.map_or_else(AutomationForm::starting, AutomationForm::of);
        let cadence_index = Cadence::ALL
            .iter()
            .position(|cadence| *cadence == form.cadence);
        let weekday_index = WEEKDAYS.iter().position(|day| *day == form.weekday);
        Self {
            editing: automation.map(|automation| automation.id.clone()),
            error: None,
            name: text_field(cx, "Name", "Morning PRs", &form.name),
            project: text_field(cx, "Project", "Name, id or path", &form.project),
            destination: cx.new(|cx| {
                SelectView::new(FormDestination::LABELS, cx)
                    .selected(Some(form.destination.index()))
            }),
            workspace: text_field(
                cx,
                "Workspace",
                "Its id, name or path",
                match &form.destination {
                    FormDestination::Workspace { workspace } => workspace.as_str(),
                    _ => "",
                },
            ),
            base_branch: text_field(
                cx,
                "Base branch",
                "Blank for the project's base",
                match &form.destination {
                    FormDestination::NewWorktree { base_branch } => base_branch.as_str(),
                    _ => "",
                },
            ),
            prompt: text_field(
                cx,
                "Prompt",
                "What the agent does on each run",
                &form.prompt,
            ),
            agent: text_field(cx, "Agent", "claude", &form.agent),
            cadence: cx.new(|cx| {
                SelectView::new(Cadence::ALL.iter().map(|cadence| cadence.label()), cx)
                    .selected(cadence_index)
            }),
            time: text_field(cx, "Time", "09:00", &form.time),
            weekday: cx.new(|cx| {
                SelectView::new(WEEKDAYS.iter().map(|day| weekday_label(*day)), cx)
                    .selected(weekday_index)
            }),
            cron: text_field(cx, "Cron expression", "0 9 * * 1-5", &form.cron),
            zone: text_field(
                cx,
                "Zone",
                "Blank for the host's zone, such as Europe/Madrid",
                &form.zone,
            ),
        }
    }

    /// The values the fields hold now, as the form the model checks.
    pub(in crate::shell) fn form(&self, cx: &App) -> AutomationForm {
        let cadence = self
            .cadence
            .read(cx)
            .selected_index()
            .and_then(|index| Cadence::ALL.get(index).copied())
            .unwrap_or_default();
        let weekday = self
            .weekday
            .read(cx)
            .selected_index()
            .and_then(|index| WEEKDAYS.get(index).copied())
            .unwrap_or(AutomationWeekday::Monday);
        AutomationForm {
            project: self.project.read(cx).text().to_owned(),
            name: self.name.read(cx).text().to_owned(),
            prompt: self.prompt.read(cx).text().to_owned(),
            agent: self.agent.read(cx).text().to_owned(),
            cadence,
            time: self.time.read(cx).text().to_owned(),
            weekday,
            cron: self.cron.read(cx).text().to_owned(),
            zone: self.zone.read(cx).text().to_owned(),
            destination: FormDestination::from_choice(
                self.destination.read(cx).selected_index(),
                self.base_branch.read(cx).text().to_owned(),
                self.workspace.read(cx).text().to_owned(),
            ),
        }
    }

    fn first_field(&self) -> Entity<TextField> {
        self.name.clone()
    }
}

/// The editor on screen: its fields in order, its save action, and what a dismiss does.
pub(in crate::shell) fn editor_dialog(
    editor: &AutomationEditor,
    shell: &WeakEntity<Shell>,
    cx: &App,
) -> Dialog {
    let (title, action) = match editor.editing {
        Some(_) => ("Edit automation", "Save"),
        None => ("New automation", "Create"),
    };
    let (on_action, on_dismiss) = (shell.clone(), shell.clone());
    let body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(editor.name.clone())
        .child(editor.project.clone())
        .child(editor.destination.clone())
        .child(editor.workspace.clone())
        .child(editor.base_branch.clone())
        .child(editor.prompt.clone())
        .child(editor.agent.clone())
        .child(editor.cadence.clone())
        .child(editor.time.clone())
        .child(editor.weekday.clone())
        .child(editor.cron.clone())
        .child(editor.zone.clone());
    let mut dialog = Dialog::new("automation-editor", title)
        .open(true)
        .child(body)
        .field(editor.first_field().read(cx).focus_handle(cx))
        .action(action)
        .on_action(move |_, cx| {
            let _ = on_action.update(cx, |shell, cx| shell.submit_automation_editor(cx));
        })
        .on_dismiss(move |_, cx| {
            let _ = on_dismiss.update(cx, |shell, cx| shell.close_automation_editor(cx));
        });
    if let Some(error) = &editor.error {
        dialog = dialog.error(error.clone());
    }
    dialog
}

fn text_field(
    cx: &mut Context<Shell>,
    label: &str,
    placeholder: &str,
    text: &str,
) -> Entity<TextField> {
    cx.new(|cx| {
        TextField::new(cx)
            .label(label.to_owned())
            .placeholder(placeholder.to_owned())
            .with_text(text)
    })
}
