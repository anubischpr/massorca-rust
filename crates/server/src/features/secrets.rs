//! The secret vault methods (capability `secrets/1`, decision 0071). Values travel only inward, in `secret.set`;
//! no reply, error or event carries one.

use std::sync::Arc;

use massorca_core_store::SecretRequestRow;
use massorca_core_vault::{Protection, SecretValue, Vault, VaultEntry, VaultError};
use massorca_protocol::methods::events::{
    ChangeEvent, TOPIC_SECRET_CHANGED, TOPIC_SECRET_REQUEST_CHANGED, TOPIC_SECRET_REQUESTED,
};
use massorca_protocol::methods::secrets::{
    self, SecretDecline, SecretEntry, SecretLink, SecretList, SecretListParams, SecretName,
    SecretRemove, SecretRemoved, SecretRequest, SecretRequestParams, SecretRequests,
    SecretRequestsParams, SecretSet, SecretStatus,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use super::secret_params::Redacted;
use super::secret_refusals::{parse_scope, refusal};
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::NoParams;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    let cap = capabilities::SECRETS;
    Ok(vec![
        registries.capabilities.register(cap, Owner::Core)?,
        methods.register(Owner::Core, secrets::STATUS, cap, Permission::Read, status)?,
        methods.register(Owner::Core, secrets::LIST, cap, Permission::Read, list)?,
        methods.register(
            Owner::Core,
            secrets::REQUESTS,
            cap,
            Permission::Read,
            requests,
        )?,
        methods.register(
            Owner::Core,
            secrets::SET,
            cap,
            Permission::HostAdmin,
            |ctx, params: Redacted<SecretSet>| set(ctx, params.0),
        )?,
        methods.register(Owner::Core, secrets::LINK, cap, Permission::HostAdmin, link)?,
        methods.register(
            Owner::Core,
            secrets::UNLINK,
            cap,
            Permission::HostAdmin,
            unlink,
        )?,
        methods.register(
            Owner::Core,
            secrets::REMOVE,
            cap,
            Permission::HostAdmin,
            remove,
        )?,
        methods.register(
            Owner::Core,
            secrets::DECLINE,
            cap,
            Permission::HostAdmin,
            decline,
        )?,
        methods.register(
            Owner::Core,
            secrets::RESET_KEY,
            cap,
            Permission::HostAdmin,
            reset_key,
        )?,
        // Asking changes no secret, and agent sessions working in a workspace are the ones who ask.
        methods.register(
            Owner::Core,
            secrets::REQUEST,
            cap,
            Permission::Workspace,
            request,
        )?,
    ])
}

async fn reset_key(ctx: RequestCtx, _params: NoParams) -> Result<SecretStatus, ErrorBody> {
    blocking(&ctx, Vault::reset_key).await?;
    status(ctx, NoParams {}).await
}

async fn status(ctx: RequestCtx, _params: NoParams) -> Result<SecretStatus, ErrorBody> {
    let status = blocking(&ctx, Vault::status).await?;
    let protection = match status.protection {
        None => "none",
        Some(Protection::Dpapi) => "dpapi",
        Some(Protection::SecretService) => "secret_service",
        Some(Protection::File) => "file",
    };
    Ok(SecretStatus {
        protection: protection.to_owned(),
        state: if status.locked { "locked" } else { "ready" }.to_owned(),
        count: status.count,
    })
}

async fn list(ctx: RequestCtx, params: SecretListParams) -> Result<SecretList, ErrorBody> {
    let scope = params.scope.as_deref().map(parse_scope).transpose()?;
    let entries = blocking(&ctx, move |vault| vault.list(scope.as_ref())).await?;
    Ok(SecretList {
        secrets: entries.into_iter().map(entry_view).collect(),
    })
}

async fn set(ctx: RequestCtx, params: SecretSet) -> Result<SecretEntry, ErrorBody> {
    let SecretSet {
        scope,
        name,
        value,
        replace,
    } = params;
    let scope = parse_scope(&scope)?;
    let value = SecretValue::new(value).map_err(|_| refusal(VaultError::TooLarge))?;
    let written = blocking(&ctx, move |vault| vault.set(&scope, &name, &value, replace)).await?;
    publish_changed(&ctx, &written.entry.scope, &written.entry.name);
    publish_resolved(&ctx, &written.resolved_requests);
    Ok(entry_view(written.entry))
}

async fn link(ctx: RequestCtx, params: SecretLink) -> Result<SecretEntry, ErrorBody> {
    let scope = parse_scope(&params.scope)?;
    let from = parse_scope(&params.from_scope)?;
    let name = params.name;
    let written = blocking(&ctx, move |vault| vault.link(&scope, &name, &from)).await?;
    publish_changed(&ctx, &written.entry.scope, &written.entry.name);
    publish_resolved(&ctx, &written.resolved_requests);
    Ok(entry_view(written.entry))
}

async fn unlink(ctx: RequestCtx, params: SecretName) -> Result<SecretRemoved, ErrorBody> {
    let scope = parse_scope(&params.scope)?;
    let name = params.name.clone();
    blocking(&ctx, move |vault| vault.unlink(&scope, &name)).await?;
    publish_changed(&ctx, &params.scope, &params.name);
    Ok(SecretRemoved {
        scope: params.scope,
        name: params.name,
        links_removed: Vec::new(),
    })
}

async fn remove(ctx: RequestCtx, params: SecretRemove) -> Result<SecretRemoved, ErrorBody> {
    let scope = parse_scope(&params.scope)?;
    let name = params.name.clone();
    let (include_links, force) = (params.include_links, params.force);
    let links_removed = blocking(&ctx, move |vault| {
        vault.remove(&scope, &name, include_links, force)
    })
    .await?;
    publish_changed(&ctx, &params.scope, &params.name);
    for link_scope in &links_removed {
        publish_changed(&ctx, link_scope, &params.name);
    }
    Ok(SecretRemoved {
        scope: params.scope,
        name: params.name,
        links_removed,
    })
}

async fn request(ctx: RequestCtx, params: SecretRequestParams) -> Result<SecretRequest, ErrorBody> {
    let scope = parse_scope(&params.scope)?;
    let SecretRequestParams {
        name,
        reason,
        session,
        ..
    } = params;
    let requester = ctx
        .principal
        .device_id()
        .map_or_else(|| "owner".to_owned(), |id| format!("device:{id}"));
    let row = blocking(&ctx, move |vault| {
        vault.request(&scope, &name, &reason, session, &requester)
    })
    .await?;
    publish(&ctx, TOPIC_SECRET_REQUESTED, &row.id);
    Ok(request_view(row))
}

async fn requests(
    ctx: RequestCtx,
    params: SecretRequestsParams,
) -> Result<SecretRequests, ErrorBody> {
    let rows = blocking(&ctx, move |vault| vault.requests(params.all)).await?;
    Ok(SecretRequests {
        requests: rows.into_iter().map(request_view).collect(),
    })
}

async fn decline(ctx: RequestCtx, params: SecretDecline) -> Result<SecretRequest, ErrorBody> {
    let id = params.request;
    let row = blocking(&ctx, move |vault| vault.decline(&id)).await?;
    publish(&ctx, TOPIC_SECRET_REQUEST_CHANGED, &row.id);
    Ok(request_view(row))
}

/// Runs one vault call on the blocking pool, counted as activity until it returns.
async fn blocking<T, F>(ctx: &RequestCtx, call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce(&Vault) -> Result<T, VaultError> + Send + 'static,
{
    let vault = Arc::clone(&ctx.state.vault);
    let work = ctx.state.activity.enter();
    match tokio::task::spawn_blocking(move || {
        let _work = work;
        call(&vault)
    })
    .await
    {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(refusal(error)),
        Err(error) => Err(ErrorBody::new(
            codes::INTERNAL,
            format!("the vault task failed: {error}"),
        )),
    }
}

fn publish(ctx: &RequestCtx, topic: &str, id: &str) {
    ctx.state.events.publish(ChangeEvent {
        topic: topic.to_owned(),
        id: id.to_owned(),
    });
}

fn publish_changed(ctx: &RequestCtx, scope: &str, name: &str) {
    publish(ctx, TOPIC_SECRET_CHANGED, &format!("{scope}/{name}"));
}

fn publish_resolved(ctx: &RequestCtx, ids: &[String]) {
    for id in ids {
        publish(ctx, TOPIC_SECRET_REQUEST_CHANGED, id);
    }
}

fn entry_view(entry: VaultEntry) -> SecretEntry {
    SecretEntry {
        scope: entry.scope,
        name: entry.name,
        owner_scope: entry.owner_scope,
        linked: entry.linked,
        linked_scopes: entry.linked_scopes,
        created_at_ms: entry.created_at,
        rotated_at_ms: entry.rotated_at,
        used_at_ms: entry.used_at,
    }
}

fn request_view(row: SecretRequestRow) -> SecretRequest {
    SecretRequest {
        id: row.id,
        scope: row.scope,
        name: row.name,
        reason: row.reason,
        session: row.session,
        state: row.state,
        created_at_ms: row.created_at,
        resolved_at_ms: row.resolved_at,
    }
}
