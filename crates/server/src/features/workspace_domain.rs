//! The workspace domain as the handlers use it: blocking calls on the pool, the default root read at call time,
//! and the change events published after each commit.

use std::path::PathBuf;
use std::sync::Arc;

use massorca_core_workspace::{BranchPrefix, DomainError, Workspaces};
use massorca_protocol::methods::events::ChangeEvent;
use massorca_protocol::{ErrorBody, codes};
use serde_json::Value;

use crate::context::{AppState, RequestCtx};
use crate::features::config::{
    WORKSPACES_BRANCH_PREFIX, WORKSPACES_BRANCH_PREFIX_CUSTOM, WORKSPACES_DEFAULT_ROOT,
};
use crate::owner::Owner;
use crate::store_blocking;

/// The domain handle for one mutating call: the root and branch prefix the settings hold now.
pub(crate) async fn for_mutation(ctx: &RequestCtx) -> Result<Workspaces, ErrorBody> {
    for_mutation_in(&ctx.state).await
}

/// [`for_mutation`] for a caller that holds the service state rather than a request, such as a background title run.
pub(crate) async fn for_mutation_in(state: &AppState) -> Result<Workspaces, ErrorBody> {
    let value = core_setting(state, WORKSPACES_DEFAULT_ROOT).await?;
    let root = value.as_str().map(PathBuf::from).ok_or_else(|| {
        ErrorBody::new(
            codes::CONFIG_INVALID_VALUE,
            "workspaces.default_root must be a path",
        )
    })?;
    let source = core_setting(state, WORKSPACES_BRANCH_PREFIX).await?;
    let custom = core_setting(state, WORKSPACES_BRANCH_PREFIX_CUSTOM).await?;
    let prefix = BranchPrefix::from_setting(
        source.as_str().unwrap_or_default(),
        custom.as_str().unwrap_or_default(),
    )?;
    Ok(state
        .workspaces
        .with_default_root(root)
        .with_branch_prefix(prefix))
}

/// The value the core setting `key` holds now, or its declared default.
pub(crate) async fn core_setting(state: &AppState, key: &'static str) -> Result<Value, ErrorBody> {
    let stored = store_blocking::run(state, move |store| store.get_setting("core", key)).await?;
    match stored {
        Some(json) => serde_json::from_str::<Value>(&json).map_err(|error| {
            ErrorBody::new(
                codes::INTERNAL,
                format!("a stored setting is not valid JSON: {error}"),
            )
        }),
        None => state
            .registries
            .settings
            .find(&Owner::Core, key)
            .map(|decl| decl.default)
            .ok_or_else(|| ErrorBody::new(codes::INTERNAL, format!("{key} is not declared"))),
    }
}

/// The shared domain handle, for reads that take no default root.
pub(crate) fn shared(ctx: &RequestCtx) -> Arc<Workspaces> {
    Arc::clone(&ctx.state.workspaces)
}

/// Runs one domain call on the blocking pool and maps its refusal to the protocol error.
pub(crate) async fn blocking<T, F>(call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, DomainError> + Send + 'static,
{
    match tokio::task::spawn_blocking(call).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error.into()),
        Err(error) => Err(ErrorBody::new(
            codes::INTERNAL,
            format!("the workspace task failed: {error}"),
        )),
    }
}

/// Publishes one change event. Called only after the change is committed.
pub(crate) fn publish(ctx: &RequestCtx, topic: &str, id: &str) {
    ctx.state.events.publish(ChangeEvent {
        topic: topic.to_owned(),
        id: id.to_owned(),
    });
}
