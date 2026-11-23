//! `team.memory_list`, `team.memory_add` and `team.memory_rm` (capability `teams/1`, decision 0072): the lines a team and
//! its bots keep for later tasks.

use massorca_core_store::{Store, TeamMemoryRow, TeamRow, new_id, now_ms};
use massorca_protocol::methods::events::TOPIC_TEAM_CHANGED;
use massorca_protocol::methods::teams::{
    self, MemorySource, TeamMemoryAddParams, TeamMemoryLine, TeamMemoryList, TeamMemoryListParams,
    TeamMemoryRemoveParams,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::workspace_domain::publish;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

use super::{blocking, invalid, non_blank, selectors, store_error};

/// The longest memory text, in Unicode scalar values.
const MAX_TEXT_CHARS: usize = 500;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    Ok(vec![
        methods.register(
            Owner::Core,
            teams::MEMORY_LIST,
            capabilities::TEAMS,
            Permission::Read,
            list,
        )?,
        methods.register(
            Owner::Core,
            teams::MEMORY_ADD,
            capabilities::TEAMS,
            Permission::Workspace,
            add,
        )?,
        methods.register(
            Owner::Core,
            teams::MEMORY_REMOVE,
            capabilities::TEAMS,
            Permission::Workspace,
            remove,
        )?,
    ])
}

async fn list(ctx: RequestCtx, params: TeamMemoryListParams) -> Result<TeamMemoryList, ErrorBody> {
    blocking(&ctx, move |store, _agents| list_lines(store, &params)).await
}

async fn add(ctx: RequestCtx, params: TeamMemoryAddParams) -> Result<TeamMemoryLine, ErrorBody> {
    let row = blocking(&ctx, move |store, _agents| add_line(store, params)).await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &row.team_id);
    Ok(line_view(&row))
}

async fn remove(
    ctx: RequestCtx,
    params: TeamMemoryRemoveParams,
) -> Result<TeamMemoryLine, ErrorBody> {
    let row = blocking(&ctx, move |store, _agents| remove_line(store, &params)).await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &row.team_id);
    Ok(line_view(&row))
}

/// The team's lines newest first; with `bot`, only that bot's lines.
fn list_lines(store: &Store, params: &TeamMemoryListParams) -> Result<TeamMemoryList, ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let bot = params
        .bot
        .as_deref()
        .map(|selector| selectors::bot(store, &team, selector))
        .transpose()?;
    let rows = store
        .list_team_memory(&team.id, bot.as_ref().map(|row| row.id.as_str()), None)
        .map_err(store_error)?;
    Ok(TeamMemoryList {
        lines: rows.iter().map(line_view).collect(),
    })
}

fn add_line(store: &Store, params: TeamMemoryAddParams) -> Result<TeamMemoryRow, ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let text = memory_text(&params.text)?;
    let bot = params
        .bot
        .as_deref()
        .map(|selector| selectors::bot(store, &team, selector))
        .transpose()?;
    let task = params
        .task
        .as_deref()
        .map(|selector| task_of_team(store, &team, selector))
        .transpose()?;
    let source = if task.is_some() { "task" } else { "person" };
    let row = TeamMemoryRow {
        id: new_id(),
        team_id: team.id.clone(),
        bot_id: bot.map(|row| row.id),
        text,
        source: source.to_owned(),
        task_id: task,
        created_at: now_ms(),
    };
    store.insert_team_memory(&row).map_err(store_error)?;
    Ok(row)
}

fn remove_line(store: &Store, params: &TeamMemoryRemoveParams) -> Result<TeamMemoryRow, ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let row = store
        .get_team_memory(&params.line)
        .map_err(store_error)?
        .filter(|row| row.team_id == team.id)
        .ok_or_else(|| {
            ErrorBody::new(
                codes::TEAM_MEMORY_NOT_FOUND,
                format!("team {} has no memory line {:?}", team.name, params.line),
            )
        })?;
    if !store.delete_team_memory(&row.id).map_err(store_error)? {
        return Err(ErrorBody::new(
            codes::TEAM_MEMORY_NOT_FOUND,
            "the memory line was removed by another call",
        ));
    }
    Ok(row)
}

/// The text without its surrounding spaces. A blank text, or one longer than 500 characters, is refused.
fn memory_text(value: &str) -> Result<String, ErrorBody> {
    let text = non_blank(value, "the memory text")?;
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(invalid(format!(
            "the memory text is longer than {MAX_TEXT_CHARS} characters"
        )));
    }
    Ok(text)
}

/// The id of a task of `team`. A task of another team is not found.
fn task_of_team(store: &Store, team: &TeamRow, selector: &str) -> Result<String, ErrorBody> {
    let row = selectors::task(store, selector)?;
    if row.team_id != team.id {
        return Err(ErrorBody::new(
            codes::TASK_NOT_FOUND,
            format!("team {} has no task {selector:?}", team.name),
        ));
    }
    Ok(row.id)
}

fn line_view(row: &TeamMemoryRow) -> TeamMemoryLine {
    TeamMemoryLine {
        id: row.id.clone(),
        team_id: row.team_id.clone(),
        bot_id: row.bot_id.clone(),
        text: row.text.clone(),
        source: source_view(&row.source),
        task_id: row.task_id.clone(),
        created_at_ms: row.created_at,
    }
}

fn source_view(source: &str) -> MemorySource {
    match source {
        "task" => MemorySource::Task,
        "person" => MemorySource::Person,
        _ => MemorySource::Unknown,
    }
}
