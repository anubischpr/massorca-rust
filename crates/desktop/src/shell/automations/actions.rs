//! The Automations page's actions on the shell (decision 0123): switching an automation on or off, and removing one.
//! Each service call runs on its own task, and the list follows its answer.

use gpui::Context;
use massorca_protocol::methods::automations::{Automation, AutomationList};

use super::super::Shell;
use super::AutomationsFrame;
use crate::model::canvas::Canvas;
use crate::model::navigation::Page;
use crate::service::Refusal;

/// What the page says when no service backs it: the definitions live there.
const NOT_CONNECTED: &str = "Connect to MassOrca to change automations.";

impl Shell {
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

    fn automations_read(
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
