//! Bot Teams (capability `teams/1`, decision 0072): teams, their bots, projects and memory, and the tasks on their Boards.
//! Store calls run on the blocking pool, and each change publishes its event after it is committed.

mod board;
mod bot;
mod memory;
mod room;
mod room_card;
mod room_quote;
mod room_schedule;
mod routine_run;
mod routine_timer;
mod routines;
mod selectors;
mod task;
mod task_outcome;
mod task_start;

pub(crate) use routine_timer::follow_routines;
pub(crate) use task_start::start_gate;
mod task_state;
mod team;
mod team_project;

use massorca_core_agents::Agents;
use massorca_core_store::{Store, StoreError};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{AppState, Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::registry::RegistrationHandle;

pub(crate) use task_outcome::follow_task_outcomes;

/// Registers the Bot Teams capability and its methods.
pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let mut handles = vec![
        registries
            .capabilities
            .register(capabilities::TEAMS, Owner::Core)?,
    ];
    handles.extend(team::register(registries)?);
    handles.extend(bot::register(registries)?);
    handles.extend(team_project::register(registries)?);
    handles.extend(memory::register(registries)?);
    handles.extend(task::register(registries)?);
    handles.extend(task_start::register(registries)?);
    handles.extend(board::register(registries)?);
    handles.extend(routines::register(registries)?);
    handles.extend(room::register(registries)?);
    Ok(handles)
}

/// Runs one Bot Teams call on the blocking pool, with the store and the agents domain.
pub(crate) async fn blocking<T, F>(ctx: &RequestCtx, call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce(&Store, &Agents) -> Result<T, ErrorBody> + Send + 'static,
{
    blocking_state(&ctx.state, call).await
}

/// The same for a caller without a request, such as the task outcome listener.
pub(crate) async fn blocking_state<T, F>(state: &AppState, call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce(&Store, &Agents) -> Result<T, ErrorBody> + Send + 'static,
{
    let store = state.store.clone();
    let agents = state.agents.clone();
    let work = state.activity.enter();
    match tokio::task::spawn_blocking(move || {
        let _work = work;
        call(&store, &agents)
    })
    .await
    {
        Ok(result) => result,
        Err(error) => Err(ErrorBody::new(
            codes::INTERNAL,
            format!("the teams task failed: {error}"),
        )),
    }
}

/// The protocol error for a failed store call. A unique constraint stands for the refusal it names.
pub(crate) fn store_error(error: StoreError) -> ErrorBody {
    match error {
        StoreError::Conflict { constraint } => conflict(&constraint),
        other => ErrorBody::new(codes::INTERNAL, other.to_string()),
    }
}

fn conflict(constraint: &str) -> ErrorBody {
    match constraint {
        "teams.name" => ErrorBody::new(codes::TEAM_NAME_IN_USE, "another team has this name"),
        "team_projects.team_id, team_projects.name" => ErrorBody::new(
            codes::TEAM_PROJECT_NAME_IN_USE,
            "the team already has a project with this name",
        ),
        "bots.team_id, bots.name" => ErrorBody::new(
            codes::BOT_NAME_IN_USE,
            "the team already has a bot with this name",
        ),
        "bots.team_id" => ErrorBody::new(
            codes::BOT_LEAD_TAKEN,
            "the team already has a lead bot; clear its lead first",
        ),
        other => ErrorBody::new(
            codes::INTERNAL,
            format!("the store refused the write ({other})"),
        ),
    }
}

/// A request whose values cannot be used as sent.
pub(crate) fn invalid(message: impl Into<String>) -> ErrorBody {
    ErrorBody::new(codes::REQUEST_INVALID_PARAMS, message)
}

/// The value without its surrounding spaces; a blank value is refused.
pub(crate) fn non_blank(value: &str, what: &str) -> Result<String, ErrorBody> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(invalid(format!("{what} must not be empty")));
    }
    Ok(trimmed.to_owned())
}

/// An optional text without its surrounding spaces; a blank text counts as none.
pub(crate) fn optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}
