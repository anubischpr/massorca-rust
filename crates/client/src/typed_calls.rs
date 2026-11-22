//! Typed helpers over the methods of the core features.

use massorca_protocol::methods::agents::{
    self, AgentHookParams, AgentKinds, AgentListParams, AgentPromptParams, AgentSession,
    AgentSessionList, AgentSessionRef, AgentStartParams, AgentStopParams, AgentWatchParams,
    CredentialsImportParams, CredentialsImported, CredentialsKindRef, CredentialsRemoved,
    CredentialsShown, PromptReceipt,
};
use massorca_protocol::methods::config::{self, ConfigKey, ConfigList, ConfigSet, ConfigValue};
use massorca_protocol::methods::events;
use massorca_protocol::methods::projects::{
    self, Project, ProjectAddParams, ProjectList, ProjectRef,
};
use massorca_protocol::methods::service::{self, ServiceInfo};
use massorca_protocol::methods::terminals::{
    self, Terminal, TerminalAttachParams, TerminalCreateParams, TerminalInputParams,
    TerminalKillParams, TerminalList, TerminalListParams, TerminalRef, TerminalResizeParams,
};
use massorca_protocol::methods::workspaces::{
    self, Workspace, WorkspaceCreateParams, WorkspaceList, WorkspaceListParams, WorkspaceRef,
    WorkspaceRemoveParams, WorkspaceRemoved,
};

use crate::client::Client;
use crate::error::ClientError;
use crate::subscription::Subscription;
use crate::terminal_attach::TerminalAttachment;

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

    /// `terminal.create`: starts a terminal in a workspace.
    pub async fn terminal_create(
        &self,
        params: &TerminalCreateParams,
    ) -> Result<Terminal, ClientError> {
        self.call(terminals::CREATE, params).await
    }

    /// `terminal.list`: every terminal, or only those of one workspace (a selector).
    pub async fn terminal_list(
        &self,
        workspace: Option<&str>,
    ) -> Result<TerminalList, ClientError> {
        let params = TerminalListParams {
            workspace: workspace.map(str::to_owned),
        };
        self.call(terminals::LIST, &params).await
    }

    /// `terminal.show`: one terminal by id.
    pub async fn terminal_show(&self, terminal: &str) -> Result<Terminal, ClientError> {
        self.call(terminals::SHOW, &terminal_ref(terminal)).await
    }

    /// `terminal.attach`: follows a terminal's output; see [`TerminalAttachment`].
    pub async fn terminal_attach(
        &self,
        params: &TerminalAttachParams,
    ) -> Result<TerminalAttachment, ClientError> {
        let subscription = self.subscribe(terminals::ATTACH, params).await?;
        Ok(TerminalAttachment::new(subscription))
    }

    /// `terminal.input`: types `bytes` into a terminal.
    pub async fn terminal_input(&self, terminal: &str, bytes: &[u8]) -> Result<(), ClientError> {
        let params = TerminalInputParams::from_bytes(terminal, bytes);
        self.call(terminals::INPUT, &params).await
    }

    /// `terminal.resize`: sets the terminal's size.
    pub async fn terminal_resize(
        &self,
        params: &TerminalResizeParams,
    ) -> Result<Terminal, ClientError> {
        self.call(terminals::RESIZE, params).await
    }

    /// `terminal.kill`: ends the terminal's process tree; `force` skips the grace period.
    pub async fn terminal_kill(
        &self,
        terminal: &str,
        force: bool,
    ) -> Result<Terminal, ClientError> {
        let params = TerminalKillParams {
            terminal: terminal.to_owned(),
            force,
        };
        self.call(terminals::KILL, &params).await
    }

    /// `terminal.close`: kills the terminal if it is live, then forgets it.
    pub async fn terminal_close(&self, terminal: &str) -> Result<Terminal, ClientError> {
        self.call(terminals::CLOSE, &terminal_ref(terminal)).await
    }

    /// `agent.kinds`: every declared agent kind, its package state, whether a program is detected, and its problems.
    pub async fn agent_kinds(&self) -> Result<AgentKinds, ClientError> {
        self.call(agents::KINDS, &()).await
    }

    /// `agent.start`: starts an agent session in a workspace.
    pub async fn agent_start(
        &self,
        params: &AgentStartParams,
    ) -> Result<AgentSession, ClientError> {
        self.call(agents::START, params).await
    }

    /// `agent.list`: sessions newest first, or only those of one workspace (a selector).
    pub async fn agent_list(
        &self,
        workspace: Option<&str>,
    ) -> Result<AgentSessionList, ClientError> {
        let params = AgentListParams {
            workspace: workspace.map(str::to_owned),
        };
        self.call(agents::LIST, &params).await
    }

    /// `agent.show`: one session, by id or a unique id prefix of at least 8 characters.
    pub async fn agent_show(&self, session: &str) -> Result<AgentSession, ClientError> {
        self.call(agents::SHOW, &session_ref(session)).await
    }

    /// `agent.stop`: ends the session's run; `force` skips the grace period.
    pub async fn agent_stop(
        &self,
        session: &str,
        force: bool,
    ) -> Result<AgentSession, ClientError> {
        let params = AgentStopParams {
            session: session.to_owned(),
            force,
        };
        self.call(agents::STOP, &params).await
    }

    /// `agent.forget`: deletes the record of a session whose run has ended.
    pub async fn agent_forget(&self, session: &str) -> Result<AgentSession, ClientError> {
        self.call(agents::FORGET, &session_ref(session)).await
    }

    /// `agent.prompt`: types a prompt into a session's terminal once it is ready; `force` types it at once.
    pub async fn agent_prompt(
        &self,
        params: &AgentPromptParams,
    ) -> Result<PromptReceipt, ClientError> {
        self.call(agents::PROMPT, params).await
    }

    /// `agent.resume`: starts a new run of an exited session; the session keeps its id.
    pub async fn agent_resume(&self, session: &str) -> Result<AgentSession, ClientError> {
        self.call(agents::RESUME, &session_ref(session)).await
    }

    /// `agent.hook`: one status report from a run's hook, with the run's token.
    pub async fn agent_hook(&self, params: &AgentHookParams) -> Result<(), ClientError> {
        let _: serde_json::Value = self.call(agents::HOOK, params).await?;
        Ok(())
    }

    /// `agent.watch`: a `snapshot` of the selected sessions, then a `changed` item after every change. Read the items
    /// as [`massorca_protocol::methods::agents::AgentWatchEvent`].
    pub async fn agent_watch(
        &self,
        params: &AgentWatchParams,
    ) -> Result<Subscription, ClientError> {
        self.subscribe(agents::WATCH, params).await
    }

    /// `agent.credentials.import`: copies the kind's declared files from `params.from` into the profile's home for
    /// it. The result names the files and their sizes only.
    pub async fn agent_credentials_import(
        &self,
        params: &CredentialsImportParams,
    ) -> Result<CredentialsImported, ClientError> {
        self.call(agents::CREDENTIALS_IMPORT, params).await
    }

    /// `agent.credentials.show`: whether the kind's home exists and which declared files are in it.
    pub async fn agent_credentials_show(
        &self,
        kind: &str,
    ) -> Result<CredentialsShown, ClientError> {
        self.call(agents::CREDENTIALS_SHOW, &kind_ref(kind)).await
    }

    /// `agent.credentials.remove`: deletes the kind's home.
    pub async fn agent_credentials_remove(
        &self,
        kind: &str,
    ) -> Result<CredentialsRemoved, ClientError> {
        self.call(agents::CREDENTIALS_REMOVE, &kind_ref(kind)).await
    }
}

fn kind_ref(kind: &str) -> CredentialsKindRef {
    CredentialsKindRef {
        kind: kind.to_owned(),
    }
}

fn session_ref(session: &str) -> AgentSessionRef {
    AgentSessionRef {
        session: session.to_owned(),
    }
}

fn terminal_ref(terminal: &str) -> TerminalRef {
    TerminalRef {
        terminal: terminal.to_owned(),
    }
}
