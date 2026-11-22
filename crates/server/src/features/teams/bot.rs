//! `team.bot_add`, `team.bot_update` and `team.bot_rm` (capability `teams/1`, decision 0072).

use massorca_core_store::{BotRow, Store, new_id, now_ms};
use massorca_protocol::methods::events::TOPIC_TEAM_CHANGED;
use massorca_protocol::methods::teams::{
    self, Bot, BotAddParams, BotRemoveParams, BotUpdateParams,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::workspace_domain::publish;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

use super::{blocking, non_blank, optional_text, selectors, store_error};

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    Ok(vec![
        methods.register(
            Owner::Core,
            teams::BOT_ADD,
            capabilities::TEAMS,
            Permission::Workspace,
            add,
        )?,
        methods.register(
            Owner::Core,
            teams::BOT_UPDATE,
            capabilities::TEAMS,
            Permission::Workspace,
            update,
        )?,
        methods.register(
            Owner::Core,
            teams::BOT_REMOVE,
            capabilities::TEAMS,
            Permission::Workspace,
            remove,
        )?,
    ])
}

async fn add(ctx: RequestCtx, params: BotAddParams) -> Result<Bot, ErrorBody> {
    let bot = blocking(&ctx, move |store, _agents| add_bot(store, params)).await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &bot.team_id);
    Ok(bot)
}

async fn update(ctx: RequestCtx, params: BotUpdateParams) -> Result<Bot, ErrorBody> {
    let bot = blocking(&ctx, move |store, _agents| update_bot(store, params)).await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &bot.team_id);
    Ok(bot)
}

async fn remove(ctx: RequestCtx, params: BotRemoveParams) -> Result<Bot, ErrorBody> {
    let bot = blocking(&ctx, move |store, _agents| remove_bot(store, params)).await?;
    publish(&ctx, TOPIC_TEAM_CHANGED, &bot.team_id);
    Ok(bot)
}

fn add_bot(store: &Store, params: BotAddParams) -> Result<Bot, ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let row = BotRow {
        id: new_id(),
        team_id: team.id.clone(),
        name: non_blank(&params.name, "the bot name")?,
        job: non_blank(&params.job, "the bot's job")?,
        kind: non_blank(&params.kind, "the agent kind")?,
        model: optional_text(params.model),
        effort: optional_text(params.effort),
        lead: params.lead,
        created_at: now_ms(),
    };
    store.insert_bot(&row).map_err(store_error)?;
    Ok(bot_view(&row))
}

fn update_bot(store: &Store, params: BotUpdateParams) -> Result<Bot, ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let mut row = selectors::bot(store, &team, &params.bot)?;
    if let Some(name) = params.name {
        row.name = non_blank(&name, "the bot name")?;
    }
    if let Some(job) = params.job {
        row.job = non_blank(&job, "the bot's job")?;
    }
    if let Some(kind) = params.kind {
        row.kind = non_blank(&kind, "the agent kind")?;
    }
    if let Some(model) = params.model {
        row.model = optional_text(Some(model));
    }
    if let Some(effort) = params.effort {
        row.effort = optional_text(Some(effort));
    }
    if let Some(lead) = params.lead {
        row.lead = lead;
    }
    if !store.update_bot(&row).map_err(store_error)? {
        return Err(ErrorBody::new(
            codes::INTERNAL,
            "the bot was removed by another call",
        ));
    }
    Ok(bot_view(&row))
}

fn remove_bot(store: &Store, params: BotRemoveParams) -> Result<Bot, ErrorBody> {
    let team = selectors::team(store, &params.team)?;
    let row = selectors::bot(store, &team, &params.bot)?;
    if !store.delete_bot(&row.id).map_err(store_error)? {
        return Err(ErrorBody::new(
            codes::INTERNAL,
            "the bot was removed by another call",
        ));
    }
    Ok(bot_view(&row))
}

fn bot_view(row: &BotRow) -> Bot {
    Bot {
        id: row.id.clone(),
        team_id: row.team_id.clone(),
        name: row.name.clone(),
        job: row.job.clone(),
        kind: row.kind.clone(),
        model: row.model.clone(),
        effort: row.effort.clone(),
        lead: row.lead,
        created_at_ms: row.created_at,
    }
}
