//! The Automations page's run actions on the shell (decision 0125): Run now, and the run table of one automation. The
//! table is read when it opens, after a run now, and when the service says an automation changed.

use gpui::Context;
use massorca_protocol::methods::automations::{AutomationRun, AutomationRunList};

use super::super::Shell;
use crate::service::Refusal;

const NOT_CONNECTED: &str = "Connect to MassOrca to run automations.";

impl Shell {
    /// Runs an automation now. The run it recorded joins its table, and the list is read again for its last run.
    pub(in crate::shell) fn run_automation_now(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            self.automations.set_notice(Some(NOT_CONNECTED.to_owned()));
            cx.notify();
            return;
        };
        self.automations.set_notice(None);
        cx.spawn(async move |this, cx| {
            let answer = service.automation_run_now(id).await;
            let list = service.automation_list().await;
            this.update(cx, |shell, cx| {
                shell.automation_run_answered(answer, cx);
                shell.automations_read(list, cx);
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Opens the run table of an automation, or closes it when it is already open.
    pub(in crate::shell) fn toggle_automation_runs(&mut self, id: String, cx: &mut Context<Self>) {
        if self
            .automations
            .history()
            .is_some_and(|history| history.automation() == id)
        {
            self.automations.close_history();
        } else {
            self.automations.open_history(id.clone());
            self.read_automation_runs(id, cx);
        }
        cx.notify();
    }

    pub(in crate::shell) fn close_automation_runs(&mut self, cx: &mut Context<Self>) {
        self.automations.close_history();
        cx.notify();
    }

    fn read_automation_runs(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(service) = self.service.clone() else {
            self.automations
                .set_runs(&id, Err(NOT_CONNECTED.to_owned()));
            return;
        };
        cx.spawn(async move |this, cx| {
            let answer = service.automation_runs(id.clone()).await;
            this.update(cx, |shell, cx| shell.automation_runs_read(&id, answer, cx))
                .ok();
        })
        .detach();
    }

    fn automation_runs_read(
        &mut self,
        id: &str,
        answer: Result<AutomationRunList, Refusal>,
        cx: &mut Context<Self>,
    ) {
        self.automations.set_runs(
            id,
            answer
                .map(|list| list.runs)
                .map_err(|refusal| refusal.message()),
        );
        cx.notify();
    }

    fn automation_run_answered(
        &mut self,
        answer: Result<AutomationRun, Refusal>,
        cx: &mut Context<Self>,
    ) {
        match answer {
            Ok(run) => self.automations.put_run(run),
            Err(refusal) => self.automations.set_notice(Some(refusal.message())),
        }
        cx.notify();
    }

    /// Reads the list and the open run table again when the service announced an automation change since the last
    /// read. Called on each render of the shell.
    pub(super) fn follow_automation_changes(&mut self, cx: &mut Context<Self>) {
        let count = self.store.read(cx).state().automation_changes;
        if !self.automations.note_changes(count) || !self.automations.is_read() {
            return;
        }
        let Some(service) = self.service.clone() else {
            return;
        };
        if let Some(id) = self
            .automations
            .history()
            .map(|history| history.automation().to_owned())
        {
            self.read_automation_runs(id, cx);
        }
        cx.spawn(async move |this, cx| {
            let list = service.automation_list().await;
            this.update(cx, |shell, cx| shell.automations_read(list, cx))
                .ok();
        })
        .detach();
    }
}
