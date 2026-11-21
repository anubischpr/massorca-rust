//! `workspace.create`, `workspace.list`, `workspace.show` and `workspace.remove` (capability `workspaces/1`).
//! Removal publishes `workspace.removing` between its two halves and `workspace.removed` after the row is gone.

use std::sync::Arc;

use massorca_protocol::methods::events::{
    TOPIC_WORKSPACE_CREATED, TOPIC_WORKSPACE_REMOVED, TOPIC_WORKSPACE_REMOVING,
};
use massorca_protocol::methods::workspaces::{
    self, Workspace, WorkspaceCreateParams, WorkspaceList, WorkspaceListParams, WorkspaceRef,
    WorkspaceRemoveParams, WorkspaceRemoved,
};
use massorca_protocol::{ErrorBody, capabilities};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::workspace_domain::{blocking, for_mutation, publish, shared};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::WORKSPACES, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            workspaces::CREATE,
            capabilities::WORKSPACES,
            Permission::Workspace,
            create,
        )?,
        registries.methods.register(
            Owner::Core,
            workspaces::LIST,
            capabilities::WORKSPACES,
            Permission::Read,
            list,
        )?,
        registries.methods.register(
            Owner::Core,
            workspaces::SHOW,
            capabilities::WORKSPACES,
            Permission::Read,
            show,
        )?,
        registries.methods.register(
            Owner::Core,
            workspaces::REMOVE,
            capabilities::WORKSPACES,
            Permission::Workspace,
            remove,
        )?,
    ])
}

async fn create(ctx: RequestCtx, params: WorkspaceCreateParams) -> Result<Workspace, ErrorBody> {
    let domain = for_mutation(&ctx).await?;
    let workspace = blocking(move || domain.create_workspace(params)).await?;
    publish(&ctx, TOPIC_WORKSPACE_CREATED, &workspace.id);
    Ok(workspace)
}

async fn list(ctx: RequestCtx, params: WorkspaceListParams) -> Result<WorkspaceList, ErrorBody> {
    let domain = shared(&ctx);
    let workspaces = blocking(move || domain.list_workspaces(params.project.as_deref())).await?;
    Ok(WorkspaceList { workspaces })
}

async fn show(ctx: RequestCtx, params: WorkspaceRef) -> Result<Workspace, ErrorBody> {
    let domain = shared(&ctx);
    blocking(move || domain.show_workspace(&params.workspace)).await
}

async fn remove(
    ctx: RequestCtx,
    params: WorkspaceRemoveParams,
) -> Result<WorkspaceRemoved, ErrorBody> {
    let domain = Arc::new(for_mutation(&ctx).await?);
    let force = params.force;
    let selector = params.workspace;
    let begun = {
        let domain = Arc::clone(&domain);
        blocking(move || domain.begin_remove(&selector, force)).await?
    };
    publish(&ctx, TOPIC_WORKSPACE_REMOVING, &begun.id);
    let id = begun.id.clone();
    let removed = blocking(move || domain.finish_remove(&id, force)).await?;
    publish(&ctx, TOPIC_WORKSPACE_REMOVED, &removed.workspace.id);
    Ok(removed)
}
