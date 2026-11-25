//! `marketplace.service.*` (capability `services/1`, SKL-35, decision 0094). GitLab signs `glab` in; Bitbucket, Azure
//! DevOps and Gitea keep a token, a base URL and an email in the vault, write-only; Linear and Jira are status-only rows.
//! No reply, error or log carries a token.

use std::sync::Arc;

use massorca_core_vault::{SecretValue, ServiceConnection, Vault, VaultError};
use massorca_protocol::methods::hosted_services::{
    self as wire, SERVICE_IDS, ServiceConnect, ServiceList, ServiceName, ServiceSource,
    ServiceState, ServiceStatus, ServiceStatusParams,
};
use massorca_protocol::{ErrorBody, capabilities, codes};

use super::glab_login::{self, GlabOutcome};
use super::secret_params::Redacted;
use super::secret_refusals::refusal;
use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

/// GitLab personal access tokens are short; the cap keeps the stdin write within one pipe buffer.
const MAX_TOKEN_BYTES: usize = 1024;
const MAX_URL_CHARS: usize = 2048;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    let cap = capabilities::SERVICES;
    Ok(vec![
        registries.capabilities.register(cap, Owner::Core)?,
        methods.register(Owner::Core, wire::STATUS, cap, Permission::Read, status)?,
        // Connecting writes a credential, as `secret.set` does.
        methods.register(
            Owner::Core,
            wire::CONNECT,
            cap,
            Permission::HostAdmin,
            |ctx, params: Redacted<ServiceConnect>| connect(ctx, params.0),
        )?,
        methods.register(
            Owner::Core,
            wire::DISCONNECT,
            cap,
            Permission::HostAdmin,
            disconnect,
        )?,
    ])
}

fn display_name(id: &str) -> &'static str {
    match id {
        "gitlab" => "GitLab",
        "bitbucket" => "Bitbucket",
        "azure-devops" => "Azure DevOps",
        "gitea" => "Gitea",
        "linear" => "Linear",
        _ => "Jira",
    }
}

fn known(id: &str) -> Result<&'static str, ErrorBody> {
    SERVICE_IDS
        .into_iter()
        .find(|known| *known == id)
        .ok_or_else(|| {
            ErrorBody::new(
                codes::SERVICE_UNKNOWN,
                format!("unknown service; services: {}", SERVICE_IDS.join(", ")),
            )
        })
}

fn is_status_only(id: &str) -> bool {
    matches!(id, "linear" | "jira")
}

fn row(id: &str, state: ServiceState, source: ServiceSource) -> ServiceStatus {
    ServiceStatus {
        service: id.to_owned(),
        name: display_name(id).to_owned(),
        state,
        source,
        account: None,
        base_url: None,
        updated_at_ms: None,
        cli_installed: None,
        detail: None,
    }
}

async fn status(ctx: RequestCtx, params: ServiceStatusParams) -> Result<ServiceList, ErrorBody> {
    let ids = match params.service.as_deref() {
        Some(id) => vec![known(id)?],
        None => SERVICE_IDS.to_vec(),
    };
    let mut services = Vec::with_capacity(ids.len());
    for id in ids {
        services.push(status_of(&ctx, id).await);
    }
    Ok(ServiceList { services })
}

async fn status_of(ctx: &RequestCtx, id: &'static str) -> ServiceStatus {
    if id == "gitlab" {
        return gitlab_status(ctx).await;
    }
    if is_status_only(id) {
        return ServiceStatus {
            detail: Some("not connected from MassOrca yet".to_owned()),
            ..row(id, ServiceState::Unknown, ServiceSource::None)
        };
    }
    match on_vault(ctx, move |vault| vault.service(id)).await {
        Ok(Some(stored)) => ServiceStatus {
            account: stored.email,
            base_url: stored.base_url,
            updated_at_ms: Some(stored.updated_at),
            ..row(id, ServiceState::Connected, ServiceSource::Stored)
        },
        Ok(None) => row(id, ServiceState::NotConnected, ServiceSource::None),
        Err(VaultError::Locked(_) | VaultError::KeyMismatch { .. }) => ServiceStatus {
            detail: Some("the vault is locked".to_owned()),
            ..row(id, ServiceState::Unknown, ServiceSource::Stored)
        },
        Err(_) => ServiceStatus {
            detail: Some("the vault could not be read".to_owned()),
            ..row(id, ServiceState::Unknown, ServiceSource::None)
        },
    }
}

async fn gitlab_status(ctx: &RequestCtx) -> ServiceStatus {
    let base = row("gitlab", ServiceState::Unknown, ServiceSource::None);
    match on_programs(ctx, glab_login::status).await {
        GlabOutcome::Ok => ServiceStatus {
            state: ServiceState::Connected,
            source: ServiceSource::External,
            cli_installed: Some(true),
            ..base
        },
        GlabOutcome::Refused => ServiceStatus {
            state: ServiceState::NotConnected,
            cli_installed: Some(true),
            ..base
        },
        GlabOutcome::NotInstalled => ServiceStatus {
            state: ServiceState::NotConnected,
            cli_installed: Some(false),
            detail: Some("glab is not installed".to_owned()),
            ..base
        },
        GlabOutcome::Failed => ServiceStatus {
            detail: Some("glab did not answer".to_owned()),
            ..base
        },
    }
}

async fn connect(ctx: RequestCtx, params: ServiceConnect) -> Result<ServiceStatus, ErrorBody> {
    let ServiceConnect {
        service,
        token,
        base_url,
        email,
    } = params;
    let id = known(&service)?;
    let token = SecretValue::new(token).map_err(|_| invalid("the token is too long"))?;
    let text = token.expose();
    if text.is_empty() || text.len() > MAX_TOKEN_BYTES || text.chars().any(char::is_whitespace) {
        return Err(invalid("the token must be one word of at most 1024 bytes"));
    }
    let address = base_url.as_deref().map(parse_address).transpose()?;
    if email.is_some() && id != "bitbucket" {
        return Err(invalid("`email` applies to Bitbucket only"));
    }
    if is_status_only(id) {
        return Err(unavailable(id, "is not connected from MassOrca yet"));
    }
    if id == "gitlab" {
        let host = address.map_or_else(|| glab_login::DEFAULT_HOST.to_owned(), |(_, host)| host);
        let outcome = on_programs(&ctx, move |programs| {
            glab_login::login(programs, token.expose(), &host)
        })
        .await;
        return match outcome {
            GlabOutcome::Ok => Ok(gitlab_status(&ctx).await),
            GlabOutcome::Refused => Err(ErrorBody::new(
                codes::SERVICE_REJECTED,
                "glab refused the token",
            )),
            GlabOutcome::NotInstalled => Err(unavailable(id, "needs glab on this host")),
            GlabOutcome::Failed => Err(ErrorBody::new(
                codes::SERVICE_FAILED,
                "glab could not be run or did not finish",
            )),
        };
    }
    let base_url = address.map(|(url, _)| url);
    on_vault(&ctx, move |vault| {
        vault.set_service(
            id,
            &ServiceConnection {
                token: &token,
                base_url: base_url.as_deref(),
                email: email.as_deref(),
            },
        )
    })
    .await
    .map_err(refusal)?;
    Ok(status_of(&ctx, id).await)
}

async fn disconnect(ctx: RequestCtx, params: ServiceName) -> Result<ServiceStatus, ErrorBody> {
    let id = known(&params.service)?;
    if id == "gitlab" {
        return Err(unavailable(
            id,
            "keeps its sign-in in glab; run `glab auth logout` on the host",
        ));
    }
    if is_status_only(id) {
        return Err(unavailable(id, "is not connected from MassOrca yet"));
    }
    on_vault(&ctx, move |vault| vault.remove_service(id))
        .await
        .map_err(refusal)?;
    Ok(status_of(&ctx, id).await)
}

/// An `http` or `https` address, and its host with any port. The refusal never repeats the text.
fn parse_address(text: &str) -> Result<(String, String), ErrorBody> {
    let refused = || invalid("`base_url` must be an http or https address");
    let rest = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
        .ok_or_else(refused)?;
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let clean = text.chars().count() <= MAX_URL_CHARS
        && !text.chars().any(|c| c.is_whitespace() || c.is_control());
    if host.is_empty() || host.contains('@') || !clean {
        return Err(refused());
    }
    Ok((text.trim_end_matches('/').to_owned(), host.to_owned()))
}

fn invalid(message: &str) -> ErrorBody {
    ErrorBody::new(codes::REQUEST_INVALID_PARAMS, message)
}

fn unavailable(id: &str, reason: &str) -> ErrorBody {
    ErrorBody::new(
        codes::SERVICE_UNAVAILABLE,
        format!("{} {reason}", display_name(id)),
    )
}

/// Runs one vault call on the blocking pool, counted as activity until it returns.
async fn on_vault<T, F>(ctx: &RequestCtx, call: F) -> Result<T, VaultError>
where
    T: Send + 'static,
    F: FnOnce(&Vault) -> Result<T, VaultError> + Send + 'static,
{
    let vault = Arc::clone(&ctx.state.vault);
    let work = ctx.state.activity.enter();
    tokio::task::spawn_blocking(move || {
        let _work = work;
        call(&vault)
    })
    .await
    .unwrap_or(Err(VaultError::Cipher))
}

async fn on_programs<F>(ctx: &RequestCtx, call: F) -> GlabOutcome
where
    F: FnOnce(&super::extension_process::HostPrograms) -> GlabOutcome + Send + 'static,
{
    let programs = Arc::clone(&ctx.state.host_programs);
    let work = ctx.state.activity.enter();
    tokio::task::spawn_blocking(move || {
        let _work = work;
        call(&programs)
    })
    .await
    .unwrap_or(GlabOutcome::Failed)
}
