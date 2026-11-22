//! `task.create`, `task.list`, `task.show`, `task.update`, `task.done`, `task.reopen` and `task.rm` (capability `teams/1`,
//! decision 0072). `task.start` is not built yet: it will create the team's folder workspace, start the bot's agent with the
//! prompt, and record the session on the task once delivery succeeded.

use massorca_core_agents::Agents;
use massorca_core_store::{
    Store, TaskOrigin as StoredOrigin, TaskOutcome, TaskRow, TeamRow, new_id, now_ms,
};
use massorca_protocol::methods::events::TOPIC_TASK_CHANGED;
use massorca_protocol::methods::tasks::{
    self, Task, TaskCreateParams, TaskList, TaskListParams, TaskOrigin, TaskRef, TaskUpdateParams,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::workspace_domain::publish;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

use super::task_state::{self, Session, is_running, outcome_of, standing};
use super::{blocking, invalid, non_blank, optional_text, selectors, store_error};

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    Ok(vec![
        methods.register(
            Owner::Core,
            tasks::CREATE,
            capabilities::TEAMS,
            Permission::Workspace,
            create,
        )?,
        methods.register(
            Owner::Core,
            tasks::LIST,
            capabilities::TEAMS,
            Permission::Read,
            list,
        )?,
        methods.register(
            Owner::Core,
            tasks::SHOW,
            capabilities::TEAMS,
            Permission::Read,
            show,
        )?,
        methods.register(
            Owner::Core,
            tasks::UPDATE,
            capabilities::TEAMS,
            Permission::Workspace,
            update,
        )?,
        methods.register(
            Owner::Core,
            tasks::DONE,
            capabilities::TEAMS,
            Permission::Workspace,
            done,
        )?,
        methods.register(
            Owner::Core,
            tasks::REOPEN,
            capabilities::TEAMS,
            Permission::Workspace,
            reopen,
        )?,
        methods.register(
            Owner::Core,
            tasks::REMOVE,
            capabilities::TEAMS,
            Permission::Workspace,
            remove,
        )?,
    ])
}

async fn create(ctx: RequestCtx, params: TaskCreateParams) -> Result<Task, ErrorBody> {
    let task = blocking(&ctx, move |store, _agents| create_task(store, params)).await?;
    publish(&ctx, TOPIC_TASK_CHANGED, &task.id);
    Ok(task)
}

async fn list(ctx: RequestCtx, params: TaskListParams) -> Result<TaskList, ErrorBody> {
    blocking(&ctx, move |store, agents| list_tasks(store, agents, params)).await
}

async fn show(ctx: RequestCtx, params: TaskRef) -> Result<Task, ErrorBody> {
    blocking(&ctx, move |store, agents| {
        show_task(store, agents, &params.task)
    })
    .await
}

async fn update(ctx: RequestCtx, params: TaskUpdateParams) -> Result<Task, ErrorBody> {
    let task = blocking(&ctx, move |store, agents| {
        update_task(store, agents, params)
    })
    .await?;
    publish(&ctx, TOPIC_TASK_CHANGED, &task.id);
    Ok(task)
}

async fn done(ctx: RequestCtx, params: TaskRef) -> Result<Task, ErrorBody> {
    let task = blocking(&ctx, move |store, agents| {
        finish_task(store, agents, &params.task)
    })
    .await?;
    publish(&ctx, TOPIC_TASK_CHANGED, &task.id);
    Ok(task)
}

async fn reopen(ctx: RequestCtx, params: TaskRef) -> Result<Task, ErrorBody> {
    let task = blocking(&ctx, move |store, agents| {
        reopen_task(store, agents, &params.task)
    })
    .await?;
    publish(&ctx, TOPIC_TASK_CHANGED, &task.id);
    Ok(task)
}

async fn remove(ctx: RequestCtx, params: TaskRef) -> Result<Task, ErrorBody> {
    let task = blocking(&ctx, move |store, agents| {
        remove_task(store, agents, &params.task)
    })
    .await?;
    publish(&ctx, TOPIC_TASK_CHANGED, &task.id);
    Ok(task)
}

/// Whether any of `rows` has an agent session that still runs.
pub(crate) fn any_running(agents: &Agents, rows: &[TaskRow]) -> Result<bool, ErrorBody> {
    for row in rows {
        if row.session_id.is_some() && is_running(&task_state::load(agents, row)?) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn create_task(store: &Store, params: TaskCreateParams) -> Result<Task, ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let bot_id = optional_text(params.bot)
        .map(|selector| selectors::bot(store, &team, &selector))
        .transpose()?
        .map(|bot| bot.id);
    let project_id = optional_text(params.project)
        .map(|selector| selectors::team_project(store, &team, &selector))
        .transpose()?
        .map(|project| project.id);
    let now = now_ms();
    let row = TaskRow {
        id: new_id(),
        team_id: team.id.clone(),
        project_id,
        bot_id,
        title: non_blank(&params.title, "the task title")?,
        body: params.body.unwrap_or_default(),
        origin: stored_origin(params.origin.unwrap_or_default())?,
        session_id: None,
        outcome: None,
        error: None,
        created_at: now,
        updated_at: now,
    };
    store.insert_task(&row).map_err(store_error)?;
    Ok(view_of(&row, &Session::Unstarted))
}

fn list_tasks(
    store: &Store,
    agents: &Agents,
    params: TaskListParams,
) -> Result<TaskList, ErrorBody> {
    let team = optional_text(params.team)
        .map(|selector| selectors::team(store, &selector))
        .transpose()?;
    let project = optional_text(params.project)
        .map(|selector| {
            selectors::team_project(store, required_team(team.as_ref(), "project")?, &selector)
        })
        .transpose()?;
    let bot = optional_text(params.bot)
        .map(|selector| selectors::bot(store, required_team(team.as_ref(), "bot")?, &selector))
        .transpose()?;
    let rows = store
        .list_tasks(team.as_ref().map(|team| team.id.as_str()))
        .map_err(store_error)?;
    let mut tasks = Vec::new();
    for row in rows {
        if project
            .as_ref()
            .is_some_and(|project| row.project_id.as_deref() != Some(project.id.as_str()))
            || bot
                .as_ref()
                .is_some_and(|bot| row.bot_id.as_deref() != Some(bot.id.as_str()))
        {
            continue;
        }
        let view = view_of(&row, &task_state::load(agents, &row)?);
        if params.column.is_some_and(|column| column != view.column) {
            continue;
        }
        tasks.push(view);
    }
    Ok(TaskList { tasks })
}

/// A project or bot filter means something only inside a team, so it needs one.
fn required_team<'a>(team: Option<&'a TeamRow>, what: &str) -> Result<&'a TeamRow, ErrorBody> {
    team.ok_or_else(|| invalid(format!("a {what} filter needs a team")))
}

fn show_task(store: &Store, agents: &Agents, selector: &str) -> Result<Task, ErrorBody> {
    let row = selectors::task(store, selector)?;
    Ok(view_of(&row, &task_state::load(agents, &row)?))
}

fn update_task(
    store: &Store,
    agents: &Agents,
    params: TaskUpdateParams,
) -> Result<Task, ErrorBody> {
    let mut row = selectors::task(store, &params.task)?;
    let team = selectors::team(store, &row.team_id)?;
    if let Some(title) = params.title {
        row.title = non_blank(&title, "the task title")?;
    }
    if let Some(body) = params.body {
        row.body = body;
    }
    if let Some(bot) = params.bot {
        row.bot_id = optional_text(Some(bot))
            .map(|selector| selectors::bot(store, &team, &selector))
            .transpose()?
            .map(|bot| bot.id);
    }
    if let Some(project) = params.project {
        row.project_id = optional_text(Some(project))
            .map(|selector| selectors::team_project(store, &team, &selector))
            .transpose()?
            .map(|project| project.id);
    }
    row.updated_at = now_ms();
    save(store, &row)?;
    Ok(view_of(&row, &task_state::load(agents, &row)?))
}

/// Marks a task done by hand. Refused while its agent session runs.
fn finish_task(store: &Store, agents: &Agents, selector: &str) -> Result<Task, ErrorBody> {
    let mut row = selectors::task(store, selector)?;
    let session = task_state::load(agents, &row)?;
    refuse_while_running(&row, &session)?;
    row.outcome = Some(TaskOutcome::Success);
    row.updated_at = now_ms();
    save(store, &row)?;
    Ok(view_of(&row, &session))
}

/// Clears the outcome, the error and the session link, so the task returns to Open and can start again. Refused while its agent session runs.
fn reopen_task(store: &Store, agents: &Agents, selector: &str) -> Result<Task, ErrorBody> {
    let mut row = selectors::task(store, selector)?;
    let session = task_state::load(agents, &row)?;
    refuse_while_running(&row, &session)?;
    row.outcome = None;
    row.error = None;
    row.session_id = None;
    row.updated_at = now_ms();
    save(store, &row)?;
    Ok(view_of(&row, &Session::Unstarted))
}

fn remove_task(store: &Store, agents: &Agents, selector: &str) -> Result<Task, ErrorBody> {
    let row = selectors::task(store, selector)?;
    let session = task_state::load(agents, &row)?;
    refuse_while_running(&row, &session)?;
    let view = view_of(&row, &session);
    if !store.delete_task(&row.id).map_err(store_error)? {
        return Err(ErrorBody::new(
            codes::TASK_NOT_FOUND,
            "the task was removed by another call",
        ));
    }
    Ok(view)
}

fn refuse_while_running(row: &TaskRow, session: &Session) -> Result<(), ErrorBody> {
    if is_running(session) {
        return Err(ErrorBody::new(
            codes::TASK_RUNNING,
            format!("task {} is still running; stop its agent first", row.id),
        ));
    }
    Ok(())
}

fn save(store: &Store, row: &TaskRow) -> Result<(), ErrorBody> {
    if store.update_task(row).map_err(store_error)? {
        return Ok(());
    }
    Err(ErrorBody::new(
        codes::TASK_NOT_FOUND,
        "the task was removed by another call",
    ))
}

/// The protocol view of a stored task, with the state its session gives it.
fn view_of(row: &TaskRow, session: &Session) -> Task {
    let standing = standing(row, session);
    Task {
        id: row.id.clone(),
        team_id: row.team_id.clone(),
        project_id: row.project_id.clone(),
        bot_id: row.bot_id.clone(),
        title: row.title.clone(),
        body: row.body.clone(),
        origin: match row.origin {
            StoredOrigin::Room => TaskOrigin::Room,
            StoredOrigin::Cli => TaskOrigin::Cli,
        },
        session_id: row.session_id.clone(),
        outcome: row.outcome.map(outcome_of),
        error: row.error.clone(),
        column: standing.column,
        state: standing.state,
        last_known: standing.last_known,
        created_at_ms: row.created_at,
        updated_at_ms: row.updated_at,
    }
}

fn stored_origin(origin: TaskOrigin) -> Result<StoredOrigin, ErrorBody> {
    match origin {
        TaskOrigin::Room => Ok(StoredOrigin::Room),
        TaskOrigin::Cli => Ok(StoredOrigin::Cli),
        TaskOrigin::Unknown => Err(invalid("the task origin is not known")),
    }
}
