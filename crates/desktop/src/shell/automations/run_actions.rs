//! The Automations page's run actions on the shell (decision 0125): Run now, and the run table of one automation. The
//! table is read when it opens and after a run now, and again every few seconds while one of its runs is going.

use std::time::Duration;

use gpui::Context;
use massorca_protocol::methods::automations::{AutomationRun, AutomationRunList};

use super::super::Shell;
use crate::service::Refusal;

/// How long the page waits before it reads a table whose run is going.
const POLL_EVERY: Duration = Duration::from_secs(5);

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
        self.plan_runs_poll(id.to_owned(), cx);
        cx.notify();
    }

    fn automation_run_answered(
        &mut self,
        answer: Result<AutomationRun, Refusal>,
        cx: &mut Context<Self>,
    ) {
        match answer {
            Ok(run) => {
                let id = run.automation_id.clone();
                self.automations.put_run(run);
                self.plan_runs_poll(id, cx);
            }
            Err(refusal) => self.automations.set_notice(Some(refusal.message())),
        }
        cx.notify();
    }

    /// Reads the table again after [`POLL_EVERY`] while one of its runs is going and it is still open.
    fn plan_runs_poll(&mut self, id: String, cx: &mut Context<Self>) {
        let planned = self
            .automations
            .history_of(&id)
            .is_some_and(|history| history.plan_poll());
        if !planned {
            return;
        }
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(POLL_EVERY).await;
            this.update(cx, |shell, cx| {
                if let Some(history) = shell.automations.history_of(&id) {
                    history.poll_done();
                    shell.read_automation_runs(id, cx);
                }
            })
            .ok();
        })
        .detach();
    }
}
