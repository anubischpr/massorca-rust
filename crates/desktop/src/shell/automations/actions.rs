//! The Automations page's actions on the shell (decision 0123): opening and closing the editor, saving it, switching an
//! automation on or off, and removing one. Each service call runs on its own task, and the list follows its answer.

use gpui::Context;
use massorca_protocol::methods::automations::{
    Automation, AutomationCreateParams, AutomationList, AutomationUpdateParams,
};

use super::super::Shell;
use super::{AutomationEditor, AutomationsFrame};
use crate::model::automations::Editor;
use crate::model::canvas::Canvas;
use crate::model::navigation::Page;
use crate::service::Refusal;

/// The save the editor sends: a new automation, or the changes to one.
enum Save {
    Create(AutomationCreateParams),
    Update(AutomationUpdateParams),
}

/// What the page says when no service backs it: the definitions live there.
const NOT_CONNECTED: &str = "Connect to MassOrca to change automations.";

impl Shell {
    /// Opens the editor on a new automation, or on the automation `editing` names.
    pub(in crate::shell) fn open_automation_editor(
        &mut self,
        editing: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let target = match &editing {
            Some(id) => Editor::Edit(id.clone()),
            None => Editor::New,
        };
        let automation = editing
            .as_deref()
            .and_then(|id| self.automations.find(id))
            .cloned();
        self.automations.open_editor(target);
        let workspaces = self.workspace_choices();
        let supported = self.store.read(cx).state().automation_destinations;
        self.automation_editor = Some(AutomationEditor::new(
            automation.as_ref(),
            &workspaces,
            supported,
            cx,
        ));
        cx.notify();
    }

    /// The workspaces the editor's select offers, as (label, workspace id); each label names its project.
    fn workspace_choices(&self) -> Vec<(String, String)> {
        let mut choices = Vec::new();
        for project in &self.model.snapshot().projects {
            for host in &project.hosts {
                for workspace in &host.workspaces {
                    choices.push((
                        format!("{} / {}", project.name, workspace.name),
                        workspace.id.clone(),
                    ));
                }
            }
        }
        choices
    }

    pub(in crate::shell) fn close_automation_editor(&mut self, cx: &mut Context<Self>) {
        self.automations.close_editor();
        self.automation_editor = None;
        cx.notify();
    }

    /// Sends the editor's values: a create for a new automation, an edit for the one it names. A value the form
    /// cannot send keeps the editor open with the field named.
    pub(in crate::shell) fn submit_automation_editor(&mut self, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            if let Some(editor) = self.automation_editor.as_mut() {
                editor.error = Some(NOT_CONNECTED.to_owned());
            }
            cx.notify();
            return;
        };
        let Some(editor) = self.automation_editor.as_mut() else {
            return;
        };
        let form = editor.form(cx);
        let saving = match editor.editing.clone() {
            Some(id) => form.update_params(&id).map(Save::Update),
            None => form.create_params(None).map(Save::Create),
        };
        match saving {
            Ok(save) => {
                editor.error = None;
                cx.spawn(async move |this, cx| {
                    let answer = match save {
                        Save::Create(params) => service.automation_create(params).await,
                        Save::Update(params) => service.automation_update(params).await,
                    };
                    this.update(cx, |shell, cx| shell.automation_saved(answer, cx))
                        .ok();
                })
                .detach();
            }
            Err(error) => editor.error = Some(error.message),
        }
        cx.notify();
    }

    /// Switches an automation on or off. Its row follows the service's answer.
    pub(in crate::shell) fn set_automation_enabled(
        &mut self,
        id: String,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(service) = self.service.clone() else {
            self.automations.set_notice(Some(NOT_CONNECTED.to_owned()));
            cx.notify();
            return;
        };
        cx.spawn(async move |this, cx| {
            let answer = if enabled {
                service.automation_enable(id).await
            } else {
                service.automation_disable(id).await
            };
            this.update(cx, |shell, cx| shell.automation_changed(answer, cx))
                .ok();
        })
        .detach();
    }

    /// Asks before an automation is removed.
    pub(in crate::shell) fn ask_automation_delete(&mut self, id: String, cx: &mut Context<Self>) {
        self.automations.ask_delete(id);
        cx.notify();
    }

    pub(in crate::shell) fn cancel_automation_delete(&mut self, cx: &mut Context<Self>) {
        self.automations.cancel_delete();
        cx.notify();
    }

    /// Removes the automation the person confirmed.
    pub(in crate::shell) fn confirm_automation_delete(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.automations.take_delete() else {
            return;
        };
        let Some(service) = self.service.clone() else {
            self.automations.set_notice(Some(NOT_CONNECTED.to_owned()));
            cx.notify();
            return;
        };
        cx.spawn(async move |this, cx| {
            let answer = service.automation_remove(id.clone()).await;
            this.update(cx, |shell, cx| shell.automation_removed(id, answer, cx))
                .ok();
        })
        .detach();
        cx.notify();
    }

    /// Reads the automations when the Automations page is on screen and nothing has read them yet.
    pub(in crate::shell) fn ensure_automations_loaded(&mut self, cx: &mut Context<Self>) {
        if self.model.canvas() != Canvas::Page(Page::Automations)
            || self.automations.is_read()
            || self.automations_loading
        {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        self.automations_loading = true;
        cx.spawn(async move |this, cx| {
            let answer = service.automation_list().await;
            this.update(cx, |shell, cx| shell.automations_read(answer, cx))
                .ok();
        })
        .detach();
    }

    /// The page's frame for this render.
    pub(in crate::shell) fn automations_frame(
        &self,
        shell: &gpui::WeakEntity<Shell>,
    ) -> AutomationsFrame {
        AutomationsFrame {
            state: self.automations.clone(),
            connected: self.service.is_some(),
            now_ms: now_ms(),
            shell: shell.clone(),
        }
    }

    pub(super) fn automations_read(
        &mut self,
        answer: Result<AutomationList, Refusal>,
        cx: &mut Context<Self>,
    ) {
        self.automations_loading = false;
        self.automations.set_list(
            answer
                .map(|list| list.automations)
                .map_err(|refusal| refusal.message()),
        );
        cx.notify();
    }

    /// A save the service answered: an accepted automation joins the list and closes the editor; a refused one
    /// keeps the editor open with the reason.
    fn automation_saved(&mut self, answer: Result<Automation, Refusal>, cx: &mut Context<Self>) {
        match answer {
            Ok(automation) => {
                self.automations.put(automation);
                self.automations.close_editor();
                self.automation_editor = None;
            }
            Err(refusal) => {
                let message = refusal.message();
                match self.automation_editor.as_mut() {
                    Some(editor) => editor.error = Some(message),
                    None => self.automations.set_notice(Some(message)),
                }
            }
        }
        cx.notify();
    }

    fn automation_changed(&mut self, answer: Result<Automation, Refusal>, cx: &mut Context<Self>) {
        match answer {
            Ok(automation) => self.automations.put(automation),
            Err(refusal) => self.automations.set_notice(Some(refusal.message())),
        }
        cx.notify();
    }

    fn automation_removed(
        &mut self,
        id: String,
        answer: Result<Automation, Refusal>,
        cx: &mut Context<Self>,
    ) {
        match answer {
            Ok(_) => self.automations.drop_automation(&id),
            Err(refusal) => self.automations.set_notice(Some(refusal.message())),
        }
        cx.notify();
    }
}

/// The current time in milliseconds since the Unix epoch, for the next-run wording.
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}
