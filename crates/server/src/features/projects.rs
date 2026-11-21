//! `project.add`, `project.list`, `project.show` and `project.remove` (capability `projects/1`) over the workspace domain.

use massorca_protocol::methods::events::{TOPIC_PROJECT_ADDED, TOPIC_PROJECT_REMOVED};
use massorca_protocol::methods::projects::{
    self, Project, ProjectAddParams, ProjectList, ProjectRef,
};
use massorca_protocol::{ErrorBody, capabilities};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::NoParams;
use crate::features::workspace_domain::{blocking, for_mutation, publish, shared};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::PROJECTS, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            projects::ADD,
            capabilities::PROJECTS,
            Permission::Workspace,
            add,
        )?,
        registries.methods.register(
            Owner::Core,
            projects::LIST,
            capabilities::PROJECTS,
            Permission::Read,
            list,
        )?,
        registries.methods.register(
            Owner::Core,
            projects::SHOW,
            capabilities::PROJECTS,
            Permission::Read,
            show,
        )?,
        registries.methods.register(
            Owner::Core,
            projects::REMOVE,
            capabilities::PROJECTS,
            Permission::Workspace,
            remove,
        )?,
    ])
}

async fn add(ctx: RequestCtx, params: ProjectAddParams) -> Result<Project, ErrorBody> {
    let domain = for_mutation(&ctx).await?;
    let project = blocking(move || domain.add_project(params)).await?;
    publish(&ctx, TOPIC_PROJECT_ADDED, &project.id);
    Ok(project)
}

async fn list(ctx: RequestCtx, _params: NoParams) -> Result<ProjectList, ErrorBody> {
    let domain = shared(&ctx);
    let projects = blocking(move || domain.list_projects()).await?;
    Ok(ProjectList { projects })
}

async fn show(ctx: RequestCtx, params: ProjectRef) -> Result<Project, ErrorBody> {
    let domain = shared(&ctx);
    blocking(move || domain.show_project(&params.project)).await
}

async fn remove(ctx: RequestCtx, params: ProjectRef) -> Result<Project, ErrorBody> {
    let domain = for_mutation(&ctx).await?;
    let removed = blocking(move || domain.remove_project(&params.project)).await?;
    publish(&ctx, TOPIC_PROJECT_REMOVED, &removed.id);
    Ok(removed)
}
