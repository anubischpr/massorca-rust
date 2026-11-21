//! The workspace domain as the handlers use it: blocking calls on the pool, the default root read at call time,
//! and the change events published after each commit.

use std::path::PathBuf;
use std::sync::Arc;

use massorca_core_workspace::{DomainError, Workspaces};
use massorca_protocol::methods::events::ChangeEvent;
use massorca_protocol::{ErrorBody, codes};
use serde_json::Value;

use crate::context::RequestCtx;
use crate::features::config::WORKSPACES_DEFAULT_ROOT;
use crate::owner::Owner;
use crate::store_blocking;

/// The domain handle for one mutating call: the default root the setting holds now.
pub(crate) async fn for_mutation(ctx: &RequestCtx) -> Result<Workspaces, ErrorBody> {
    let stored = store_blocking::run(&ctx.state, |store| {
        store.get_setting("core", WORKSPACES_DEFAULT_ROOT)
    })
    .await?;
    let value = match stored {
        Some(json) => serde_json::from_str::<Value>(&json).map_err(|error| {
            ErrorBody::new(
                codes::INTERNAL,
                format!("a stored setting is not valid JSON: {error}"),
            )
        })?,
        None => ctx
            .state
            .registries
            .settings
            .find(&Owner::Core, WORKSPACES_DEFAULT_ROOT)
            .map(|decl| decl.default)
            .ok_or_else(|| {
                ErrorBody::new(codes::INTERNAL, "workspaces.default_root is not declared")
            })?,
    };
    let root = value.as_str().map(PathBuf::from).ok_or_else(|| {
        ErrorBody::new(
            codes::CONFIG_INVALID_VALUE,
            "workspaces.default_root must be a path",
        )
    })?;
    Ok(ctx.state.workspaces.with_default_root(root))
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
