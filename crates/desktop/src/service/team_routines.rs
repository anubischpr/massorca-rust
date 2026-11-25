//! The routine calls of the Bot Teams pages: add, edit, remove, pause or resume, run now, and a routine's run history.
//! A write answers its caller first and then reads the teams again, so the pages follow the service.

use std::sync::Arc;

use massorca_client::Client;
use massorca_protocol::methods::teams::{
    Routine, RoutineAddParams, RoutineRef, RoutineRun, RoutineRunsParams, RoutineUpdateParams,
};
use tokio::sync::{mpsc, oneshot};

use super::bridge::{Command, ServiceHandle};
use super::refusal::Refusal;
use super::teams::{Sequence, spawn_write};
use super::update::StoreUpdate;

/// A routine call the pages ask the bridge to send. Each one answers through its oneshot.
pub(crate) enum RoutineCommand {
    Add {
        params: RoutineAddParams,
        reply: oneshot::Sender<Result<Routine, Refusal>>,
    },
    Update {
        params: RoutineUpdateParams,
        reply: oneshot::Sender<Result<Routine, Refusal>>,
    },
    Remove {
        team: String,
        routine: String,
        reply: oneshot::Sender<Result<Routine, Refusal>>,
    },
    SetPaused {
        team: String,
        routine: String,
        paused: bool,
        reply: oneshot::Sender<Result<Routine, Refusal>>,
    },
    /// `team.routine_run`: one run now, without moving the schedule. Answers with the run it made.
    Run {
        team: String,
        routine: String,
        reply: oneshot::Sender<Result<RoutineRun, Refusal>>,
    },
    /// `team.routine_runs`: the newest runs, answered to the caller and not kept by the store.
    Runs {
        team: String,
        routine: String,
        reply: oneshot::Sender<Result<Vec<RoutineRun>, Refusal>>,
    },
}

/// Runs one routine call on its own task.
pub(super) fn spawn_call(
    client: &Client,
    command: RoutineCommand,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    sequence: &Arc<Sequence>,
) {
    match command {
        RoutineCommand::Add { params, reply } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client.team_routine_add(&params).await
            });
        }
        RoutineCommand::Update { params, reply } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client.team_routine_update(&params).await
            });
        }
        RoutineCommand::Remove {
            team,
            routine,
            reply,
        } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client
                    .team_routine_remove(&RoutineRef { team, routine })
                    .await
            });
        }
        RoutineCommand::SetPaused {
            team,
            routine,
            paused,
            reply,
        } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                let target = RoutineRef { team, routine };
                if paused {
                    client.team_routine_pause(&target).await
                } else {
                    client.team_routine_resume(&target).await
                }
            });
        }
        RoutineCommand::Run {
            team,
            routine,
            reply,
        } => {
            spawn_write(client, updates, sequence, reply, move |client| async move {
                client.team_routine_run(&RoutineRef { team, routine }).await
            });
        }
        RoutineCommand::Runs {
            team,
            routine,
            reply,
        } => {
            let client = client.clone();
            tokio::spawn(async move {
                let answer = client
                    .team_routine_runs(&RoutineRunsParams {
                        team,
                        routine,
                        limit: None,
                    })
                    .await
                    .map(|list| list.runs)
                    .map_err(|error| Refusal::from(&error));
                let _ = reply.send(answer);
            });
        }
    }
}

/// Answers a routine call with `refusal`, while there is no connection to ask.
pub(super) fn refuse(command: RoutineCommand, refusal: Refusal) {
    match command {
        RoutineCommand::Add { reply, .. }
        | RoutineCommand::Update { reply, .. }
        | RoutineCommand::Remove { reply, .. }
        | RoutineCommand::SetPaused { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        RoutineCommand::Run { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        RoutineCommand::Runs { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
    }
}

impl ServiceHandle {
    /// `team.routine_add`. The routine shows when the read that follows the answer lands.
    pub async fn add_routine(&self, params: RoutineAddParams) -> Result<Routine, Refusal> {
        self.ask(|reply| Command::Routine(RoutineCommand::Add { params, reply }))
            .await
    }

    /// `team.routine_update`. Only the fields given change; the routine shows them when the read lands.
    pub async fn update_routine(&self, params: RoutineUpdateParams) -> Result<Routine, Refusal> {
        self.ask(|reply| Command::Routine(RoutineCommand::Update { params, reply }))
            .await
    }

    /// `team.routine_rm`. The routine leaves the team when the read that follows the answer lands.
    pub async fn remove_routine(&self, team: String, routine: String) -> Result<Routine, Refusal> {
        self.ask(|reply| {
            Command::Routine(RoutineCommand::Remove {
                team,
                routine,
                reply,
            })
        })
        .await
    }

    /// `team.routine_pause` when `paused`, else `team.routine_resume`.
    pub async fn set_routine_paused(
        &self,
        team: String,
        routine: String,
        paused: bool,
    ) -> Result<Routine, Refusal> {
        self.ask(|reply| {
            Command::Routine(RoutineCommand::SetPaused {
                team,
                routine,
                paused,
                reply,
            })
        })
        .await
    }

    /// `team.routine_run`: makes one run of a routine now. The run shows in the history and on the card when the read lands.
    pub async fn run_routine_now(
        &self,
        team: String,
        routine: String,
    ) -> Result<RoutineRun, Refusal> {
        self.ask(|reply| {
            Command::Routine(RoutineCommand::Run {
                team,
                routine,
                reply,
            })
        })
        .await
    }

    /// `team.routine_runs`: a routine's runs, newest first.
    pub async fn routine_runs(
        &self,
        team: String,
        routine: String,
    ) -> Result<Vec<RoutineRun>, Refusal> {
        self.ask(|reply| {
            Command::Routine(RoutineCommand::Runs {
                team,
                routine,
                reply,
            })
        })
        .await
    }
}
