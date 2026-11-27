//! Typed helpers over the methods of the core features.

use massorca_protocol::capabilities;
use massorca_protocol::methods::agents::{
    self, AccountSummary, AccountsActivateParams, AccountsAddParams, AccountsAdded,
    AccountsCheckParams, AccountsImportParams, AccountsImported, AccountsList, AccountsListParams,
    AccountsRemoveParams, AccountsRemoved, AccountsSignInParams, AgentArchive,
    AgentArchiveSessionParams, AgentBringBackParams, AgentDelegateParams, AgentDelegated,
    AgentForgetParams, AgentForgetPreview, AgentForgotten, AgentForkParams, AgentHookParams,
    AgentKinds, AgentListParams, AgentMarkSeenParams, AgentModeSet, AgentModels, AgentModelsParams,
    AgentOptionsSet, AgentPromptParams, AgentQueueEditParams, AgentQueueItemRef, AgentQueueList,
    AgentQueueParams, AgentRenameParams, AgentResumeParams, AgentRetainParams, AgentSession,
    AgentSessionList, AgentSessionRef, AgentSetModeParams, AgentSetOptionsParams, AgentStartParams,
    AgentStopParams, AgentTree, AgentWatchParams, BringBackResult, CredentialsImportParams,
    CredentialsImported, CredentialsKindRef, CredentialsRemoved, CredentialsShown,
    GenerateTitleParams, GenerateTitleResult, ImportCandidates, ImportCandidatesParams,
    PromptReceipt, QueuedPrompt,
};
use massorca_protocol::methods::chat;
use massorca_protocol::methods::commands::{self, CommandTable};
use massorca_protocol::methods::config::{self, ConfigKey, ConfigList, ConfigSet, ConfigValue};
use massorca_protocol::methods::events;
use massorca_protocol::methods::host::{self, HostMemory, HostPorts};
use massorca_protocol::methods::projects::{
    self, Project, ProjectAddParams, ProjectList, ProjectRef,
};
use massorca_protocol::methods::service::{self, ServiceInfo, ToolList, ToolsParams};
use massorca_protocol::methods::terminals::{
    self, Terminal, TerminalAttachParams, TerminalCreateParams, TerminalInputParams,
    TerminalKillParams, TerminalList, TerminalListParams, TerminalRef, TerminalResizeParams,
    TerminalWait, TerminalWaitParams,
};
use massorca_protocol::methods::workspaces::{
    self, Workspace, WorkspaceCheck, WorkspaceCreateParams, WorkspaceCurrentParams, WorkspaceList,
    WorkspaceListParams, WorkspaceRef, WorkspaceRemoveParams, WorkspaceRemoved,
    WorkspaceRenameParams, WorkspaceRunsStopped, WorkspaceUpdateParams,
};

use crate::client::Client;
use crate::error::ClientError;
use crate::subscription::Subscription;
use crate::terminal_attach::TerminalAttachment;

/// Refuses a request the service cannot read: it offers no `capability`, so the request is never sent (decision 0094). A
/// peer that predates the capability would misread the request, so the client does not send it at all.
fn require_offered(offered: bool, capability: &str, what: &str) -> Result<(), ClientError> {
    if offered {
        Ok(())
    } else {
        Err(ClientError::CapabilityMissing {
            what: what.to_owned(),
            capability: capability.to_owned(),
        })
    }
}

impl Client {
    /// `service.info`: the service's identity, protocol version, capabilities and process.
    pub async fn service_info(&self) -> Result<ServiceInfo, ClientError> {
        self.call(service::INFO, &()).await
    }

    /// `service.tools`: the developer tools on the service's host. `refresh` detects them again instead of answering
    /// from the service's last detection.
    pub async fn service_tools(&self, refresh: bool) -> Result<ToolList, ClientError> {
        require_offered(
            self.has_capability(capabilities::SERVICE_TOOLS),
            capabilities::SERVICE_TOOLS,
            "service.tools",
        )?;
        self.call(service::TOOLS, &ToolsParams { refresh }).await
    }

    /// `service.shutdown`: asks the service to stop. The request is answered before the service winds down.
    pub async fn shutdown(&self) -> Result<(), ClientError> {
        self.call(service::SHUTDOWN, &()).await
    }

    /// `commands.describe`: every command the service runs, with its method, capability, permission and parameters.
    pub async fn commands_describe(&self) -> Result<CommandTable, ClientError> {
        self.call(commands::DESCRIBE, &()).await
    }

    /// `host.memory`: the memory of the service and of each live terminal's processes, read now on the service's host.
    pub async fn host_memory(&self) -> Result<HostMemory, ClientError> {
        self.call(host::MEMORY, &()).await
    }

    /// `host.ports`: the listening sockets of each live terminal's processes, read now on the service's host.
    pub async fn host_ports(&self) -> Result<HostPorts, ClientError> {
        self.call(host::PORTS, &()).await
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

    /// `config.reset`: removes the stored value, so the default applies again.
    pub async fn config_reset(&self, params: &ConfigKey) -> Result<ConfigValue, ClientError> {
        self.call(config::RESET, params).await
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

    /// `workspace.current`: the workspace that contains `params.path`, an absolute path on the service's host.
    pub async fn workspace_current(
        &self,
        params: &WorkspaceCurrentParams,
    ) -> Result<Workspace, ClientError> {
        self.call(workspaces::CURRENT, params).await
    }

    /// `workspace.remove`: removes a workspace; `force` discards uncommitted changes.
    pub async fn workspace_remove(
        &self,
        params: &WorkspaceRemoveParams,
    ) -> Result<WorkspaceRemoved, ClientError> {
        self.call(workspaces::REMOVE, params).await
    }

    /// `workspace.update`: renames a workspace or sets its comment.
    pub async fn workspace_update(
        &self,
        params: &WorkspaceUpdateParams,
    ) -> Result<Workspace, ClientError> {
        self.call(workspaces::UPDATE, params).await
    }

    /// `workspace.rename`: replaces a name the service chose, and its branch (decision 0111).
    pub async fn workspace_rename(
        &self,
        params: &WorkspaceRenameParams,
    ) -> Result<Workspace, ClientError> {
        self.call(workspaces::RENAME, params).await
    }

    /// `workspace.check`: whether a workspace's folder is there and, for a worktree, still registered.
    pub async fn workspace_check(
        &self,
        params: &WorkspaceRef,
    ) -> Result<WorkspaceCheck, ClientError> {
        self.call(workspaces::CHECK, params).await
    }

    /// `workspace.stop_runs`: stops every agent run and terminal of a workspace; the workspace itself stays.
    pub async fn workspace_stop_runs(
        &self,
        params: &WorkspaceRef,
    ) -> Result<WorkspaceRunsStopped, ClientError> {
        self.call(workspaces::STOP_RUNS, params).await
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
        Ok(match params.since {
            Some(since) => TerminalAttachment::resumed(subscription, since),
            None => TerminalAttachment::new(subscription),
        })
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

    /// `terminal.wait`: answers once the terminal exits, or when the service's bounded wait ends; see [`TerminalWait`].
    pub async fn terminal_wait(
        &self,
        params: &TerminalWaitParams,
    ) -> Result<TerminalWait, ClientError> {
        self.call(terminals::WAIT, params).await
    }

    /// `agent.kinds`: every declared agent kind, its package state, whether a program is detected, and its problems.
    pub async fn agent_kinds(&self) -> Result<AgentKinds, ClientError> {
        self.call(agents::KINDS, &()).await
    }

    /// `agent.models`: the models a kind's picker offers for an account, the declared choices first and then what the
    /// kind's vendor CLI lists (decision 0166). A service that does not offer `agent-models/1` is refused before anything
    /// is sent.
    pub async fn agent_models(
        &self,
        params: &AgentModelsParams,
    ) -> Result<AgentModels, ClientError> {
        require_offered(
            self.has_capability(capabilities::AGENT_MODELS),
            capabilities::AGENT_MODELS,
            "the models of an agent kind",
        )?;
        self.call(agents::MODELS, params).await
    }

    /// `agent.start`: starts an agent session in a workspace, or with no workspace in the conversations folder (decision
    /// 0094). A start with no workspace needs capability `agent-no-project/1`, one with a `model` or an `effort` needs
    /// `agent-options/1`, and one with `fast` needs `agent-fast/1` (decision 0165); a service that offers none of them is
    /// refused before anything is sent.
    pub async fn agent_start(
        &self,
        params: &AgentStartParams,
    ) -> Result<AgentSession, ClientError> {
        if params.workspace.is_none() {
            require_offered(
                self.has_capability(capabilities::AGENT_NO_PROJECT),
                capabilities::AGENT_NO_PROJECT,
                "a session with no project",
            )?;
        }
        if params.model.is_some() || params.effort.is_some() {
            require_offered(
                self.has_capability(capabilities::AGENT_OPTIONS),
                capabilities::AGENT_OPTIONS,
                "a model or an effort",
            )?;
        }
        if params.fast.is_some() {
            require_offered(
                self.has_capability(capabilities::AGENT_FAST),
                capabilities::AGENT_FAST,
                "fast mode",
            )?;
        }
        self.call(agents::START, params).await
    }

    /// `agent.chat.start`: starts an agent session whose run is a structured chat (decision 0101). Needs capability
    /// `agent-chat/1`; a service that does not offer it is refused before anything is sent. A kind that cannot run as a
    /// chat is refused with `agent.chat_unsupported`, and the caller may start it with [`Self::agent_start`] instead.
    pub async fn agent_chat_start(
        &self,
        params: &AgentStartParams,
    ) -> Result<AgentSession, ClientError> {
        require_offered(
            self.has_capability(capabilities::AGENT_CHAT),
            capabilities::AGENT_CHAT,
            "a chat session",
        )?;
        let params = chat::AgentChatStartParams {
            start: params.clone(),
        };
        self.call(chat::START, &params).await
    }

    /// `agent.chat.approval.answer`: answers a chat session's permission request; the result is the approval item after
    /// the answer (decision 0101, D6).
    pub async fn agent_chat_answer(
        &self,
        params: &chat::ChatApprovalAnswerParams,
    ) -> Result<chat::ChatItem, ClientError> {
        self.call(chat::APPROVAL_ANSWER, params).await
    }

    /// `agent.chat.interrupt`: ends a chat session's running turn and keeps the session (decision 0101, D8).
    pub async fn agent_chat_interrupt(
        &self,
        session: &str,
    ) -> Result<chat::ChatInterrupted, ClientError> {
        let params = agents::AgentSessionRef {
            session: session.to_owned(),
        };
        self.call(chat::INTERRUPT, &params).await
    }

    /// `agent.list`: sessions newest first, or only those of one workspace (a selector).
    pub async fn agent_list(
        &self,
        workspace: Option<&str>,
    ) -> Result<AgentSessionList, ClientError> {
        let params = AgentListParams {
            workspace: workspace.map(str::to_owned),
            include_archived: false,
        };
        self.call(agents::LIST, &params).await
    }

    /// `agent.list` with every parameter. Asking for archived sessions needs capability `agent-archive/1`; a service
    /// without it is refused before anything is sent (decision 0163).
    pub async fn agent_list_with(
        &self,
        params: &AgentListParams,
    ) -> Result<AgentSessionList, ClientError> {
        if params.include_archived {
            require_offered(
                self.has_capability(capabilities::AGENT_ARCHIVE),
                capabilities::AGENT_ARCHIVE,
                "archived sessions",
            )?;
        }
        self.call(agents::LIST, params).await
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

    /// `agent.mark_seen`: marks a session's latest `waiting` or `done` as seen and answers with the session (capability
    /// `agent-seen/1`). `at_ms` names the activity the caller showed, so a newer one stays unseen.
    pub async fn agent_mark_seen(
        &self,
        session: &str,
        at_ms: Option<i64>,
    ) -> Result<AgentSession, ClientError> {
        let params = AgentMarkSeenParams {
            session: session.to_owned(),
            at_ms,
        };
        self.call(agents::MARK_SEEN, &params).await
    }

    /// `agent.forget`: deletes the record of a session whose run has ended.
    pub async fn agent_forget(&self, session: &str) -> Result<AgentSession, ClientError> {
        self.call(agents::FORGET, &session_ref(session)).await
    }

    /// `agent.forget` with every parameter: with `remove_workspace`, the session's workspace goes too (decision 0163),
    /// which needs capability `agent-archive/1`; a service without it is refused before anything is sent.
    pub async fn agent_forget_with(
        &self,
        params: &AgentForgetParams,
    ) -> Result<AgentForgotten, ClientError> {
        if params.remove_workspace
            || params.force
            || params.discard_ignored
            || params.expect_dirty.is_some()
            || params.allow_failed_archive
        {
            require_offered(
                self.has_capability(capabilities::AGENT_ARCHIVE),
                capabilities::AGENT_ARCHIVE,
                "removing a session's workspace",
            )?;
        }
        self.call(agents::FORGET, params).await
    }

    /// `agent.forget_preview`: what deleting a session with its workspace would meet; nothing changes (capability
    /// `agent-archive/1`).
    pub async fn agent_forget_preview(
        &self,
        session: &str,
    ) -> Result<AgentForgetPreview, ClientError> {
        require_offered(
            self.has_capability(capabilities::AGENT_ARCHIVE),
            capabilities::AGENT_ARCHIVE,
            "a delete preview",
        )?;
        self.call(agents::FORGET_PREVIEW, &session_ref(session))
            .await
    }

    /// `agent.rename`: sets a live or exited session's title (capability `agent-archive/1`).
    pub async fn agent_rename(
        &self,
        session: &str,
        title: &str,
    ) -> Result<AgentSession, ClientError> {
        require_offered(
            self.has_capability(capabilities::AGENT_ARCHIVE),
            capabilities::AGENT_ARCHIVE,
            "renaming a session",
        )?;
        let params = AgentRenameParams {
            session: session.to_owned(),
            title: title.to_owned(),
        };
        self.call(agents::RENAME, &params).await
    }

    /// `agent.archive_session`: archives (hides) an exited session, or clears that (capability `agent-archive/1`).
    pub async fn agent_archive_session(
        &self,
        session: &str,
        archived: bool,
    ) -> Result<AgentSession, ClientError> {
        require_offered(
            self.has_capability(capabilities::AGENT_ARCHIVE),
            capabilities::AGENT_ARCHIVE,
            "archived sessions",
        )?;
        let params = AgentArchiveSessionParams {
            session: session.to_owned(),
            archived,
        };
        self.call(agents::ARCHIVE_SESSION, &params).await
    }

    /// `agent.release`: archives an exited session's output, frees its terminal and keeps the record (capability
    /// `agent-release/1`).
    pub async fn agent_release(&self, session: &str) -> Result<AgentSession, ClientError> {
        self.call(agents::RELEASE, &session_ref(session)).await
    }

    /// `agent.retain`: sets or clears a session's durable retain (capability `agent-release/1`).
    pub async fn agent_retain(
        &self,
        session: &str,
        retained: bool,
    ) -> Result<AgentSession, ClientError> {
        let params = AgentRetainParams {
            session: session.to_owned(),
            retained,
        };
        self.call(agents::RETAIN, &params).await
    }

    /// `agent.archive`: the archived output of a released session (capability `agent-release/1`).
    pub async fn agent_archive(&self, session: &str) -> Result<AgentArchive, ClientError> {
        self.call(agents::ARCHIVE, &session_ref(session)).await
    }

    /// `agent.prompt`: types a prompt into a session's terminal once it is ready; `force` types it at once.
    pub async fn agent_prompt(
        &self,
        params: &AgentPromptParams,
    ) -> Result<PromptReceipt, ClientError> {
        self.call(agents::PROMPT, params).await
    }

    /// `agent.queue`: queues a prompt behind a busy agent; it is typed once the agent is idle or done (capability
    /// `agent-queue/1`, decision 0084).
    pub async fn agent_queue(
        &self,
        params: &AgentQueueParams,
    ) -> Result<QueuedPrompt, ClientError> {
        self.call(agents::QUEUE, params).await
    }

    /// `agent.queue.list`: the queued prompts of a session, head first (decision 0084).
    pub async fn agent_queue_list(&self, session: &str) -> Result<AgentQueueList, ClientError> {
        self.call(agents::QUEUE_LIST, &session_ref(session)).await
    }

    /// `agent.queue.edit`: replaces the text of a waiting or failed queued prompt (decision 0084).
    pub async fn agent_queue_edit(
        &self,
        params: &AgentQueueEditParams,
    ) -> Result<QueuedPrompt, ClientError> {
        self.call(agents::QUEUE_EDIT, params).await
    }

    /// `agent.queue.send_now`: sends a queued prompt at once, with force; it leaves the queue (decision 0084).
    pub async fn agent_queue_send_now(&self, id: &str) -> Result<QueuedPrompt, ClientError> {
        let params = AgentQueueItemRef { id: id.to_owned() };
        self.call(agents::QUEUE_SEND_NOW, &params).await
    }

    /// `agent.queue.resume`: lifts a session's queue pauses and delivers its head if the agent is ready (decision 0084).
    pub async fn agent_queue_resume(&self, session: &str) -> Result<AgentQueueList, ClientError> {
        self.call(agents::QUEUE_RESUME, &session_ref(session)).await
    }

    /// `agent.queue.remove`: removes a waiting or failed queued prompt (decision 0084).
    pub async fn agent_queue_remove(&self, id: &str) -> Result<QueuedPrompt, ClientError> {
        let params = AgentQueueItemRef { id: id.to_owned() };
        self.call(agents::QUEUE_REMOVE, &params).await
    }

    /// `agent.resume`: starts a new run of an exited session; the session keeps its id.
    pub async fn agent_resume(&self, session: &str) -> Result<AgentSession, ClientError> {
        self.call(agents::RESUME, &session_ref(session)).await
    }

    /// `agent.resume` with its optional permission mode (capability `agent-modes/1`, decision 0083).
    pub async fn agent_resume_with(
        &self,
        params: &AgentResumeParams,
    ) -> Result<AgentSession, ClientError> {
        self.call(agents::RESUME, params).await
    }

    /// `agent.fork`: starts a new session from a copy of an exited session's conversation (decision 0168). Needs
    /// capability `agent-fork/1`; a service that does not offer it is refused before anything is sent.
    pub async fn agent_fork(&self, params: &AgentForkParams) -> Result<AgentSession, ClientError> {
        require_offered(
            self.has_capability(capabilities::AGENT_FORK),
            capabilities::AGENT_FORK,
            "a fork of a session",
        )?;
        self.call(agents::FORK, params).await
    }

    /// `agent.set_mode`: switches a session's permission mode at once where the kind has a live switch, or stores it for
    /// the next resume; the result says which (capability `agent-modes/1`, decision 0083).
    pub async fn agent_set_mode(
        &self,
        params: &AgentSetModeParams,
    ) -> Result<AgentModeSet, ClientError> {
        self.call(agents::SET_MODE, params).await
    }

    /// `agent.set_options`: changes a session's model, effort or fast mode, typed into a live run where the kind declares a
    /// live command for it, or kept for the session's next resume (capability `agent-options-live/1`, decision 0171). A
    /// service that does not offer it is refused before anything is sent.
    pub async fn agent_set_options(
        &self,
        params: &AgentSetOptionsParams,
    ) -> Result<AgentOptionsSet, ClientError> {
        require_offered(
            self.has_capability(capabilities::AGENT_OPTIONS_LIVE),
            capabilities::AGENT_OPTIONS_LIVE,
            "a change to a session's model, effort or fast mode",
        )?;
        self.call(agents::SET_OPTIONS, params).await
    }

    /// `agent.delegate`: starts a child session of a parent session; the child's record and an optional note.
    pub async fn agent_delegate(
        &self,
        params: &AgentDelegateParams,
    ) -> Result<AgentDelegated, ClientError> {
        self.call(agents::DELEGATE, params).await
    }

    /// `agent.tree`: a session's delegation tree, its root and the root's children.
    pub async fn agent_tree(&self, session: &str) -> Result<AgentTree, ClientError> {
        self.call(agents::TREE, &session_ref(session)).await
    }

    /// `agent.bring_back`: merges a finished child into its parent and tells the parent run about it.
    pub async fn agent_bring_back(
        &self,
        params: &AgentBringBackParams,
    ) -> Result<BringBackResult, ClientError> {
        self.call(agents::BRING_BACK, params).await
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

    /// `agent.accounts.list`: the kind's accounts, oldest first.
    pub async fn agent_accounts_list(&self, kind: &str) -> Result<AccountsList, ClientError> {
        let params = AccountsListParams {
            kind: kind.to_owned(),
        };
        self.call(agents::ACCOUNTS_LIST, &params).await
    }

    /// `agent.accounts.add`: starts a sign-in and returns the new account and its terminal.
    pub async fn agent_accounts_add(
        &self,
        params: &AccountsAddParams,
    ) -> Result<AccountsAdded, ClientError> {
        self.call(agents::ACCOUNTS_ADD, params).await
    }

    /// `agent.accounts.sign_in`: signs an existing account in again, and returns it and the terminal that runs the login.
    pub async fn agent_accounts_sign_in(
        &self,
        params: &AccountsSignInParams,
    ) -> Result<AccountsAdded, ClientError> {
        self.call(agents::ACCOUNTS_SIGN_IN, params).await
    }

    /// `agent.accounts.import`: imports the kind's declared files into a new account. Without `from`, each file comes from
    /// where the kind's CLI keeps it, which needs capability `agent-import-detect/1` (decision 0097); a service that does
    /// not offer it is refused before anything is sent.
    pub async fn agent_accounts_import(
        &self,
        params: &AccountsImportParams,
    ) -> Result<AccountsImported, ClientError> {
        if params.from.is_none() {
            require_offered(
                self.has_capability(capabilities::AGENT_IMPORT_DETECT),
                capabilities::AGENT_IMPORT_DETECT,
                "an import without a folder",
            )?;
        }
        self.call(agents::ACCOUNTS_IMPORT, params).await
    }

    /// `agent.accounts.import_candidates`: where an import without a folder would copy each kind's files from (decision
    /// 0097). Needs capability `agent-import-detect/1`; a service that does not offer it is refused before anything is
    /// sent.
    pub async fn agent_accounts_import_candidates(
        &self,
        kind: Option<&str>,
    ) -> Result<ImportCandidates, ClientError> {
        require_offered(
            self.has_capability(capabilities::AGENT_IMPORT_DETECT),
            capabilities::AGENT_IMPORT_DETECT,
            "import source detection",
        )?;
        let params = ImportCandidatesParams {
            kind: kind.map(str::to_owned),
        };
        self.call(agents::ACCOUNTS_IMPORT_CANDIDATES, &params).await
    }

    /// `agent.accounts.activate`: makes an account the kind's active one, or a project's choice.
    pub async fn agent_accounts_activate(
        &self,
        params: &AccountsActivateParams,
    ) -> Result<AccountSummary, ClientError> {
        self.call(agents::ACCOUNTS_ACTIVATE, params).await
    }

    /// `agent.accounts.remove`: deletes an account.
    pub async fn agent_accounts_remove(
        &self,
        params: &AccountsRemoveParams,
    ) -> Result<AccountsRemoved, ClientError> {
        self.call(agents::ACCOUNTS_REMOVE, params).await
    }

    /// `agent.accounts.check`: checks an account, or the kind's active one, and returns its state.
    pub async fn agent_accounts_check(
        &self,
        params: &AccountsCheckParams,
    ) -> Result<AccountSummary, ClientError> {
        self.call(agents::ACCOUNTS_CHECK, params).await
    }

    /// `agent.generate_title`: names a chat from its first prompt and reply, by one run of the kind's one-shot form
    /// (capability `agent-title/1`, decision 0167).
    pub async fn agent_generate_title(
        &self,
        params: &GenerateTitleParams,
    ) -> Result<GenerateTitleResult, ClientError> {
        self.call(agents::GENERATE_TITLE, params).await
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

#[cfg(test)]
mod require_offered_tests {
    use massorca_protocol::capabilities;

    use super::require_offered;
    use crate::error::ClientError;

    #[test]
    fn a_request_is_sent_only_when_the_service_offers_its_capability() {
        assert!(require_offered(true, capabilities::AGENT_NO_PROJECT, "x").is_ok());
        let refused = require_offered(
            false,
            capabilities::AGENT_NO_PROJECT,
            "a session with no project",
        )
        .expect_err("refused without the capability");
        match refused {
            ClientError::CapabilityMissing { what, capability } => {
                assert_eq!(what, "a session with no project");
                assert_eq!(capability, "agent-no-project/1");
            }
            other => panic!("unexpected refusal: {other}"),
        }
    }

    #[test]
    fn a_change_to_a_sessions_options_needs_agent_options_live() {
        let what = "a change to a session's model, effort or fast mode";
        let refused = require_offered(false, capabilities::AGENT_OPTIONS_LIVE, what)
            .expect_err("refused without the capability");
        match refused {
            ClientError::CapabilityMissing { what, capability } => {
                assert_eq!(what, "a change to a session's model, effort or fast mode");
                assert_eq!(capability, "agent-options-live/1");
            }
            other => panic!("unexpected refusal: {other}"),
        }
        assert!(require_offered(true, capabilities::AGENT_OPTIONS_LIVE, what).is_ok());
    }

    #[test]
    fn the_models_of_an_agent_kind_need_agent_models() {
        let refused = require_offered(
            false,
            capabilities::AGENT_MODELS,
            "the models of an agent kind",
        )
        .expect_err("refused without the capability");
        match refused {
            ClientError::CapabilityMissing { what, capability } => {
                assert_eq!(what, "the models of an agent kind");
                assert_eq!(capability, "agent-models/1");
            }
            other => panic!("unexpected refusal: {other}"),
        }
    }
}
