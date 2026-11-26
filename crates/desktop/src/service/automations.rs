//! Scheduled automations for the shell (decisions 0123 and 0125): each `automation.*` call the shell makes, answered on
//! its oneshot. The service keeps the definitions and runs; the shell keeps what it draws and reads it again after a
//! change.

use massorca_client::Client;
use massorca_protocol::methods::automations::{
    Automation, AutomationCreateParams, AutomationList, AutomationRef, AutomationRun,
    AutomationRunList, AutomationRunsParams, AutomationUpdateParams,
};
use tokio::sync::oneshot;

use super::bridge::{Command, ServiceHandle};
use super::refusal::Refusal;

/// One `automation.*` call, answered on its oneshot with the service's answer or the reason it was refused.
pub(crate) enum AutomationCommand {
    List {
        reply: oneshot::Sender<Result<AutomationList, Refusal>>,
    },
    Create {
        params: AutomationCreateParams,
        reply: oneshot::Sender<Result<Automation, Refusal>>,
    },
    Update {
        params: AutomationUpdateParams,
        reply: oneshot::Sender<Result<Automation, Refusal>>,
    },
    Enable {
        automation: String,
        reply: oneshot::Sender<Result<Automation, Refusal>>,
    },
    Disable {
        automation: String,
        reply: oneshot::Sender<Result<Automation, Refusal>>,
    },
    Remove {
        automation: String,
        reply: oneshot::Sender<Result<Automation, Refusal>>,
    },
    RunNow {
        automation: String,
        reply: oneshot::Sender<Result<AutomationRun, Refusal>>,
    },
    Runs {
        params: AutomationRunsParams,
        reply: oneshot::Sender<Result<AutomationRunList, Refusal>>,
    },
}

/// Runs one automation call on its own task and answers it.
pub(super) fn spawn_call(client: &Client, command: AutomationCommand) {
    let client = client.clone();
    tokio::spawn(async move {
        match command {
            AutomationCommand::List { reply } => {
                let answer = client.automation_list().await;
                let _ = reply.send(answer.map_err(|error| Refusal::from(&error)));
            }
            AutomationCommand::Create { params, reply } => {
                let answer = client.automation_create(&params).await;
                let _ = reply.send(answer.map_err(|error| Refusal::from(&error)));
            }
            AutomationCommand::Update { params, reply } => {
                let answer = client.automation_update(&params).await;
                let _ = reply.send(answer.map_err(|error| Refusal::from(&error)));
            }
            AutomationCommand::Enable { automation, reply } => {
                let answer = client
                    .automation_enable(&AutomationRef { automation })
                    .await;
                let _ = reply.send(answer.map_err(|error| Refusal::from(&error)));
            }
            AutomationCommand::Disable { automation, reply } => {
                let answer = client
                    .automation_disable(&AutomationRef { automation })
                    .await;
                let _ = reply.send(answer.map_err(|error| Refusal::from(&error)));
            }
            AutomationCommand::Remove { automation, reply } => {
                let answer = client
                    .automation_remove(&AutomationRef { automation })
                    .await;
                let _ = reply.send(answer.map_err(|error| Refusal::from(&error)));
            }
            AutomationCommand::RunNow { automation, reply } => {
                let answer = client
                    .automation_run_now(&AutomationRef { automation })
                    .await;
                let _ = reply.send(answer.map_err(|error| Refusal::from(&error)));
            }
            AutomationCommand::Runs { params, reply } => {
                let answer = client.automation_runs(&params).await;
                let _ = reply.send(answer.map_err(|error| Refusal::from(&error)));
            }
        }
    });
}

/// Answers a call the service cannot take, with the reason.
pub(super) fn refuse(command: AutomationCommand, refusal: Refusal) {
    match command {
        AutomationCommand::List { reply } => {
            let _ = reply.send(Err(refusal));
        }
        AutomationCommand::Create { reply, .. }
        | AutomationCommand::Update { reply, .. }
        | AutomationCommand::Enable { reply, .. }
        | AutomationCommand::Disable { reply, .. }
        | AutomationCommand::Remove { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        AutomationCommand::RunNow { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        AutomationCommand::Runs { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
    }
}

impl ServiceHandle {
    /// `automation.list`: every automation, in creation order.
    pub async fn automation_list(&self) -> Result<AutomationList, Refusal> {
        self.ask(|reply| Command::Automation(AutomationCommand::List { reply }))
            .await
    }

    /// `automation.create`: defines an automation in a project.
    pub async fn automation_create(
        &self,
        params: AutomationCreateParams,
    ) -> Result<Automation, Refusal> {
        self.ask(|reply| Command::Automation(AutomationCommand::Create { params, reply }))
            .await
    }

    /// `automation.update`: changes the fields the params give.
    pub async fn automation_update(
        &self,
        params: AutomationUpdateParams,
    ) -> Result<Automation, Refusal> {
        self.ask(|reply| Command::Automation(AutomationCommand::Update { params, reply }))
            .await
    }

    /// `automation.enable`: enables an automation and schedules its next run from now.
    pub async fn automation_enable(&self, automation: String) -> Result<Automation, Refusal> {
        self.ask(|reply| Command::Automation(AutomationCommand::Enable { automation, reply }))
            .await
    }

    /// `automation.disable`: disables an automation.
    pub async fn automation_disable(&self, automation: String) -> Result<Automation, Refusal> {
        self.ask(|reply| Command::Automation(AutomationCommand::Disable { automation, reply }))
            .await
    }

    /// `automation.rm`: removes an automation; the removed automation comes back.
    pub async fn automation_remove(&self, automation: String) -> Result<Automation, Refusal> {
        self.ask(|reply| Command::Automation(AutomationCommand::Remove { automation, reply }))
            .await
    }

    /// `automation.run_now`: runs an automation now; the run it recorded comes back, also one that could not start.
    pub async fn automation_run_now(&self, automation: String) -> Result<AutomationRun, Refusal> {
        self.ask(|reply| Command::Automation(AutomationCommand::RunNow { automation, reply }))
            .await
    }

    /// `automation.runs`: the newest runs of an automation.
    pub async fn automation_runs(&self, automation: String) -> Result<AutomationRunList, Refusal> {
        let params = AutomationRunsParams {
            automation,
            limit: None,
            before: None,
        };
        self.ask(|reply| Command::Automation(AutomationCommand::Runs { params, reply }))
            .await
    }
}
