//! Typed helpers over the methods of the core features.

use massorca_protocol::methods::config::{self, ConfigKey, ConfigList, ConfigSet, ConfigValue};
use massorca_protocol::methods::events;
use massorca_protocol::methods::projects::{
    self, Project, ProjectAddParams, ProjectList, ProjectRef,
};
use massorca_protocol::methods::service::{self, ServiceInfo};
use massorca_protocol::methods::workspaces::{
    self, Workspace, WorkspaceCreateParams, WorkspaceList, WorkspaceListParams, WorkspaceRef,
    WorkspaceRemoveParams, WorkspaceRemoved,
};

use crate::client::Client;
use crate::error::ClientError;
use crate::subscription::Subscription;

impl Client {
    /// `service.info`: the service's identity, protocol version, capabilities and process.
    pub async fn service_info(&self) -> Result<ServiceInfo, ClientError> {
        self.call(service::INFO, &()).await
    }

    /// `service.shutdown`: asks the service to stop. The request is answered before the service winds down.
    pub async fn shutdown(&self) -> Result<(), ClientError> {
        self.call(service::SHUTDOWN, &()).await
    }

    /// `events.subscribe`: the change events the service publishes.
    pub async fn subscribe_events(&self) -> Result<Subscription, ClientError> {
        self.subscribe(events::SUBSCRIBE, &()).await
    }

    /// `config.get`: one setting, its value and its default.
    pub async fn config_get(&self, params: &ConfigKey) -> Result<ConfigValue, ClientError> {
        self.call(config::GET, params).await
    }

    /// `config.set`: changes one setting after validating it.
    pub async fn config_set(&self, params: &ConfigSet) -> Result<ConfigValue, ClientError> {
        self.call(config::SET, params).await
    }

    /// `config.list`: every declared setting with its value.
    pub async fn config_list(&self) -> Result<ConfigList, ClientError> {
        self.call(config::LIST, &()).await
    }

    /// `project.add`: registers a repository or a folder.
    pub async fn project_add(&self, params: &ProjectAddParams) -> Result<Project, ClientError> {
        self.call(projects::ADD, params).await
    }

    /// `project.list`: every registered project, ordered by name.
    pub async fn project_list(&self) -> Result<ProjectList, ClientError> {
        self.call(projects::LIST, &()).await
    }

    /// `project.show`: one project, selected by id, name or path.
    pub async fn project_show(&self, params: &ProjectRef) -> Result<Project, ClientError> {
        self.call(projects::SHOW, params).await
    }

    /// `project.remove`: unregisters a project that has no workspaces. Its files stay.
    pub async fn project_remove(&self, params: &ProjectRef) -> Result<Project, ClientError> {
        self.call(projects::REMOVE, params).await
    }

    /// `workspace.create`: creates a worktree, or registers a folder, for a project.
    pub async fn workspace_create(
        &self,
        params: &WorkspaceCreateParams,
    ) -> Result<Workspace, ClientError> {
        self.call(workspaces::CREATE, params).await
    }

    /// `workspace.list`: every workspace, or only those of one project.
    pub async fn workspace_list(
        &self,
        params: &WorkspaceListParams,
    ) -> Result<WorkspaceList, ClientError> {
        self.call(workspaces::LIST, params).await
    }

    /// `workspace.show`: one workspace, selected by id, name or path.
    pub async fn workspace_show(&self, params: &WorkspaceRef) -> Result<Workspace, ClientError> {
        self.call(workspaces::SHOW, params).await
    }

    /// `workspace.remove`: removes a workspace; `force` discards uncommitted changes.
    pub async fn workspace_remove(
        &self,
        params: &WorkspaceRemoveParams,
    ) -> Result<WorkspaceRemoved, ClientError> {
        self.call(workspaces::REMOVE, params).await
    }
}
