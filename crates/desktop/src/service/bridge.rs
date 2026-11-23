//! The bridge between the UI thread and the service (decision 0028). One tokio runtime on its own
//! thread owns the connection, every call and every subscription. The UI never touches a tokio
//! socket: it sends commands, awaits oneshot answers, and receives `StoreUpdate`s on an unbounded
//! channel, none of which needs a reactor.

use std::collections::BTreeSet;
use std::io;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use massorca_client::{Autostart, Client, ClientError, ConnectOptions, HostRecord, Subscription};
use massorca_profile::Profile;
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    AgentBringBackParams, AgentSession, AgentStartParams, BringBackResult,
};
use massorca_protocol::methods::config::{ConfigList, ConfigSet, ConfigValue};
use massorca_protocol::methods::devices::{
    DeviceInfo, DeviceList, DeviceRevokeParams, NetworkStatus, PairingCreateParams, PairingCreated,
};
use massorca_protocol::methods::events::{
    ChangeEvent, TOPIC_ACCOUNTS_CHANGED, TOPIC_AGENT_CHANGED, TOPIC_EXTENSION_DISABLED,
    TOPIC_EXTENSION_ENABLED, TOPIC_EXTENSION_INSTALLED, TOPIC_EXTENSION_REMOVED,
    TOPIC_PROJECT_ADDED, TOPIC_PROJECT_REMOVED, TOPIC_SECRET_CHANGED, TOPIC_TASK_CHANGED,
    TOPIC_TEAM_CHANGED, TOPIC_WORKSPACE_CREATED, TOPIC_WORKSPACE_REMOVED, TOPIC_WORKSPACE_REMOVING,
};
use massorca_protocol::methods::git::commit::{GitCommitParams, GitCommitted};
use massorca_protocol::methods::git::diff::{GitDiffFile, GitDiffFileParams};
use massorca_protocol::methods::git::index::{GitDiscardParams, GitDiscarded, GitPathsParams};
use massorca_protocol::methods::projects::{Project, ProjectAddParams, ProjectRef};
use massorca_protocol::methods::tasks::Task;
use massorca_protocol::methods::teams::{Bot, BotAddParams, Team, TeamCreateParams, TeamProject};
use massorca_protocol::methods::workspaces::{
    Workspace, WorkspaceCreateParams, WorkspaceListParams, WorkspaceRef, WorkspaceRemoveParams,
    WorkspaceRemoved,
};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::sleep;

use super::accounts::{self, AccountCommand, AccountsFreshness};
use super::agents;
use super::connection::{Backoff, ConnectionState, after_failed_attempt, after_loss, backoff};
use super::extensions::{self, ExtensionCommand};
use super::git::{self, Freshness, Watches};
use massorca_core_remote::bootstrap::RemoteMachine;

use super::hosts::{self, SshAdded, SshFailed, SshHost};
use super::refusal::Refusal;
use super::secrets::{self, SecretCommand, SecretsFreshness};
use super::teams::{self, Sequence, TeamCommand};
use super::terminals::{self, TerminalCommand};
use super::update::StoreUpdate;

pub(super) const BRIDGE_STOPPED: &str = "The link to the service has stopped.";

/// Who the desktop says it is in the handshake, and whether it may start the service.
pub fn connect_options(autostart: Option<Autostart>) -> ConnectOptions {
    let mut options = ConnectOptions::new(hosts::peer());
    options.autostart = autostart;
    options
}

pub(crate) enum Command {
    RetryNow,
    Create {
        params: WorkspaceCreateParams,
        reply: oneshot::Sender<Result<Workspace, Refusal>>,
    },
    Remove {
        params: WorkspaceRemoveParams,
        reply: oneshot::Sender<Result<WorkspaceRemoved, Refusal>>,
    },
    AddProject {
        params: ProjectAddParams,
        reply: oneshot::Sender<Result<Project, Refusal>>,
    },
    ListConfig {
        reply: oneshot::Sender<Result<ConfigList, Refusal>>,
    },
    Terminal(TerminalCommand),
    /// A Bot Teams write, answered on its oneshot.
    Team(TeamCommand),
    /// A Marketplace extension call, answered on its oneshot.
    Extension(ExtensionCommand),
    Secret(SecretCommand),
    /// An agent account write, answered on its oneshot.
    Account(AccountCommand),
    SetConfig {
        params: ConfigSet,
        reply: oneshot::Sender<Result<ConfigValue, Refusal>>,
    },
    BringBack {
        params: AgentBringBackParams,
        reply: oneshot::Sender<Result<BringBackResult, Refusal>>,
    },
    /// `agent.start`, answered on its oneshot once the service has started the session.
    AgentStart {
        params: AgentStartParams,
        reply: oneshot::Sender<Result<AgentSession, Refusal>>,
    },
    GitDiffFile {
        params: GitDiffFileParams,
        reply: oneshot::Sender<Result<GitDiffFile, Refusal>>,
    },
    GitStage {
        params: GitPathsParams,
        reply: oneshot::Sender<Result<(), Refusal>>,
    },
    GitUnstage {
        params: GitPathsParams,
        reply: oneshot::Sender<Result<(), Refusal>>,
    },
    GitDiscard {
        params: GitDiscardParams,
        reply: oneshot::Sender<Result<GitDiscarded, Refusal>>,
    },
    GitCommit {
        params: GitCommitParams,
        reply: oneshot::Sender<Result<GitCommitted, Refusal>>,
    },
    /// Follows a workspace's git state until `GitUnwatch`, across reconnects.
    GitWatch {
        workspace: String,
    },
    GitUnwatch {
        workspace: String,
    },
    /// Re-reads the paired hosts and checks each one. Needs no connection to this computer's service.
    RefreshHosts,
    /// Installs the service on an SSH host and pairs with it. Needs no connection to this computer's service.
    AddSshHost {
        host: SshHost,
        reply: oneshot::Sender<Result<SshAdded, SshFailed>>,
    },
    /// Checks an SSH host's machine, installing nothing. Needs no connection to this computer's service.
    ProbeSshHost {
        target: String,
        ssh_args: Vec<String>,
        reply: oneshot::Sender<Result<RemoteMachine, SshFailed>>,
    },
    /// Pairs with the server of an offer and keeps it as a host. Needs no connection to this computer's service.
    AcceptOffer {
        offer: String,
        name: Option<String>,
        reply: oneshot::Sender<Result<HostRecord, Refusal>>,
    },
    ListDevices {
        reply: oneshot::Sender<Result<DeviceList, Refusal>>,
    },
    RevokeDevice {
        params: DeviceRevokeParams,
        reply: oneshot::Sender<Result<DeviceInfo, Refusal>>,
    },
    CreateOffer {
        params: PairingCreateParams,
        reply: oneshot::Sender<Result<PairingCreated, Refusal>>,
    },
    NetworkStatus {
        reply: oneshot::Sender<Result<NetworkStatus, Refusal>>,
    },
    Stop,
}

/// The service's side of a handle made for tests: the calls the UI sent, to answer by hand.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct Script {
    commands: mpsc::UnboundedReceiver<Command>,
    connection: watch::Sender<ConnectionState>,
}

#[cfg(test)]
impl Script {
    /// The next call the UI sent, if one is waiting.
    pub(crate) fn next(&mut self) -> Option<Command> {
        self.commands.try_recv().ok()
    }

    /// Moves the link's state, as the bridge does when it connects or loses the service.
    pub(crate) fn set_connection(&self, state: ConnectionState) {
        self.connection.send_replace(state);
    }
}

/// The UI's side of the link. Clones share it.
#[derive(Clone, Debug)]
pub struct ServiceHandle {
    pub(super) commands: mpsc::UnboundedSender<Command>,
    connection: watch::Receiver<ConnectionState>,
}

impl ServiceHandle {
    /// A handle with no bridge behind it, so a test answers each call itself.
    #[cfg(test)]
    pub(crate) fn scripted() -> (Self, Script) {
        let (commands, command_rx) = mpsc::unbounded_channel();
        let (connection_tx, connection) = watch::channel(ConnectionState::Connecting);
        (
            Self {
                commands,
                connection,
            },
            Script {
                commands: command_rx,
                connection: connection_tx,
            },
        )
    }

    /// The connection state right now.
    pub fn connection(&self) -> ConnectionState {
        self.connection.borrow().clone()
    }

    /// A receiver that wakes on each connection change.
    pub fn watch_connection(&self) -> watch::Receiver<ConnectionState> {
        self.connection.clone()
    }

    /// Resets the backoff and tries to connect at once.
    pub fn retry_now(&self) {
        let _ = self.commands.send(Command::RetryNow);
    }

    /// `workspace.create`. The store changes through the resulting event, not from this answer.
    pub async fn create_workspace(
        &self,
        params: WorkspaceCreateParams,
    ) -> Result<Workspace, Refusal> {
        self.ask(|reply| Command::Create { params, reply }).await
    }

    /// `project.add`. The project list changes through the resulting event, not from this answer.
    pub async fn add_project(&self, params: ProjectAddParams) -> Result<Project, Refusal> {
        let (reply, answer) = oneshot::channel();
        self.commands
            .send(Command::AddProject { params, reply })
            .map_err(|_| Refusal::unavailable(BRIDGE_STOPPED))?;
        answer
            .await
            .unwrap_or_else(|_| Err(Refusal::unavailable(BRIDGE_STOPPED)))
    }

    /// `team.create`. The teams change through the read that follows the answer, not from the answer.
    pub async fn create_team(&self, params: TeamCreateParams) -> Result<Team, Refusal> {
        self.ask(|reply| Command::Team(TeamCommand::CreateTeam { params, reply }))
            .await
    }

    /// `team.bot_add`. The bot is listed by the read that follows the answer.
    pub async fn add_bot(&self, params: BotAddParams) -> Result<Bot, Refusal> {
        self.ask(|reply| Command::Team(TeamCommand::AddBot { params, reply }))
            .await
    }

    /// `task.done`. The card moves to Done when the read that follows the answer lands.
    pub async fn finish_task(&self, task: String) -> Result<Task, Refusal> {
        self.ask(|reply| Command::Team(TeamCommand::TaskDone { task, reply }))
            .await
    }

    /// `task.reopen`. The card moves back to Open when the read that follows the answer lands.
    pub async fn reopen_task(&self, task: String) -> Result<Task, Refusal> {
        self.ask(|reply| Command::Team(TeamCommand::TaskReopen { task, reply }))
            .await
    }

    /// `team.project_update` with a new brief. The brief shows when the read that follows the answer lands.
    pub async fn save_brief(
        &self,
        team: String,
        project: String,
        brief: String,
    ) -> Result<TeamProject, Refusal> {
        self.ask(|reply| {
            Command::Team(TeamCommand::SaveBrief {
                team,
                project,
                brief,
                reply,
            })
        })
        .await
    }

    /// `team.project_rm`. The project leaves the team when the read that follows the answer lands.
    pub async fn delete_project(
        &self,
        team: String,
        project: String,
    ) -> Result<TeamProject, Refusal> {
        self.ask(|reply| {
            Command::Team(TeamCommand::DeleteProject {
                team,
                project,
                reply,
            })
        })
        .await
    }

    /// `workspace.remove`. Pass the flags of the waivers the user confirmed.
    pub async fn remove_workspace(
        &self,
        params: WorkspaceRemoveParams,
    ) -> Result<WorkspaceRemoved, Refusal> {
        self.ask(|reply| Command::Remove { params, reply }).await
    }

    /// `config.list`: every declared setting with its value and default.
    pub async fn config_list(&self) -> Result<ConfigList, Refusal> {
        self.ask(|reply| Command::ListConfig { reply }).await
    }

    /// `config.set`: the service checks the value and stores it. The answer is the value in effect,
    /// and the caller shows "Saved" only from that answer.
    pub async fn config_set(&self, params: ConfigSet) -> Result<ConfigValue, Refusal> {
        self.ask(|reply| Command::SetConfig { params, reply }).await
    }

    /// `device.list` on this computer's service. The store's device list follows the answer too.
    pub async fn list_devices(&self) -> Result<DeviceList, Refusal> {
        self.ask(|reply| Command::ListDevices { reply }).await
    }

    /// `device.revoke`. The store's device list is read again after a revoke that worked.
    pub async fn revoke_device(&self, params: DeviceRevokeParams) -> Result<DeviceInfo, Refusal> {
        self.ask(|reply| Command::RevokeDevice { params, reply })
            .await
    }

    /// `pairing.create`: an offer another device can redeem once.
    pub async fn create_offer(
        &self,
        params: PairingCreateParams,
    ) -> Result<PairingCreated, Refusal> {
        self.ask(|reply| Command::CreateOffer { params, reply })
            .await
    }

    /// `network.status`: this computer's listeners and counts. The store keeps the answer too.
    pub async fn network_status(&self) -> Result<NetworkStatus, Refusal> {
        self.ask(|reply| Command::NetworkStatus { reply }).await
    }

    /// Sends one command without waiting for an answer. A bridge that has stopped takes it nowhere.
    pub(super) fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    /// Sends one command and waits for its answer. A bridge that has stopped answers as unavailable.
    pub(super) async fn ask<T>(
        &self,
        command: impl FnOnce(oneshot::Sender<Result<T, Refusal>>) -> Command,
    ) -> Result<T, Refusal> {
        let (reply, answer) = oneshot::channel();
        self.commands
            .send(command(reply))
            .map_err(|_| Refusal::unavailable(BRIDGE_STOPPED))?;
        answer
            .await
            .unwrap_or_else(|_| Err(Refusal::unavailable(BRIDGE_STOPPED)))
    }
}

/// The runtime thread and the updates it produces. Dropping it stops the thread's work; a service
/// it started keeps running.
#[derive(Debug)]
pub struct ServiceBridge {
    handle: ServiceHandle,
    updates: Option<mpsc::UnboundedReceiver<StoreUpdate>>,
}

impl ServiceBridge {
    /// Starts the runtime thread and begins connecting to the service of `profile`.
    pub fn start(profile: Profile, options: ConnectOptions) -> io::Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let (commands, command_rx) = mpsc::unbounded_channel();
        let (updates_tx, updates) = mpsc::unbounded_channel();
        let (connection_tx, connection) = watch::channel(ConnectionState::Connecting);
        let link = Link {
            profile,
            options,
            commands: command_rx,
            updates: updates_tx,
            connection: connection_tx,
            watching: BTreeSet::new(),
            freshness: Arc::new(Freshness::default()),
            team_reads: Arc::new(Sequence::default()),
            extension_reads: Arc::new(Sequence::default()),
            secrets_order: Arc::new(SecretsFreshness::default()),
            accounts_order: Arc::new(AccountsFreshness::default()),
        };
        thread::Builder::new()
            .name("massorca-service".to_owned())
            .spawn(move || runtime.block_on(link.supervise()))?;
        Ok(Self {
            handle: ServiceHandle {
                commands,
                connection,
            },
            updates: Some(updates),
        })
    }

    pub fn handle(&self) -> ServiceHandle {
        self.handle.clone()
    }

    /// The updates channel. It can be taken once; the UI's foreground task owns it.
    pub fn take_updates(&mut self) -> Option<mpsc::UnboundedReceiver<StoreUpdate>> {
        self.updates.take()
    }
}

impl Drop for ServiceBridge {
    fn drop(&mut self) {
        let _ = self.handle.commands.send(Command::Stop);
    }
}

enum SessionEnd {
    Stop,
    Lost,
}

enum Outcome {
    Continue,
    Resync,
    Lost,
}

/// Everything the runtime thread owns.
struct Link {
    profile: Profile,
    options: ConnectOptions,
    commands: mpsc::UnboundedReceiver<Command>,
    updates: mpsc::UnboundedSender<StoreUpdate>,
    connection: watch::Sender<ConnectionState>,
    /// The workspaces the UI follows. Kept across connections, so a reconnect watches them again.
    watching: BTreeSet<String>,
    /// Orders the git answers across the connections of this bridge.
    freshness: Arc<Freshness>,
    /// Numbers the Bot Teams reads across the connections of this bridge.
    team_reads: Arc<Sequence>,
    /// Numbers the extension reads across the connections of this bridge.
    extension_reads: Arc<Sequence>,
    /// Orders the vault reads across the connections of this bridge.
    secrets_order: Arc<SecretsFreshness>,
    /// Orders the account reads, per kind, across the connections of this bridge.
    accounts_order: Arc<AccountsFreshness>,
}

impl Link {
    /// Connects, serves while connected, and reconnects with backoff, until stopped.
    async fn supervise(mut self) {
        let mut backoff = backoff();
        let mut link_was_up = false;
        self.publish(ConnectionState::Connecting);
        loop {
            let (profile, options) = (self.profile.clone(), self.options.clone());
            let attempt = async move { Client::connect(&profile, options).await };
            let Some(outcome) = self.while_connecting(attempt).await else {
                return;
            };
            let pause = match outcome {
                Ok(client) => {
                    link_was_up = true;
                    backoff.reset();
                    let server = &client.welcome().server;
                    self.publish(ConnectionState::Connected {
                        server: server.name.clone(),
                        version: server.version.clone(),
                    });
                    match self.session(&client).await {
                        SessionEnd::Stop => return,
                        SessionEnd::Lost => {
                            let (state, delay) = after_loss(&mut backoff, Instant::now());
                            self.publish(state);
                            delay
                        }
                    }
                }
                Err(error) => {
                    let (state, delay) =
                        after_failed_attempt(link_was_up, &error, &mut backoff, Instant::now());
                    self.publish(state);
                    delay
                }
            };
            if !self.wait(pause, &mut backoff).await {
                return;
            }
        }
    }

    /// Host calls need no connection to this computer's service, so they run whatever the link is doing.
    /// Hands back every other command.
    fn host_call(&self, command: Command) -> Option<Command> {
        match command {
            Command::RefreshHosts
            | Command::AddSshHost { .. }
            | Command::ProbeSshHost { .. }
            | Command::AcceptOffer { .. } => {
                hosts::spawn(self.profile.clone(), command, self.updates.clone());
                None
            }
            other => Some(other),
        }
    }

    fn publish(&self, state: ConnectionState) {
        self.connection.send_replace(state.clone());
        self.emit(StoreUpdate::Connection(state));
    }

    fn emit(&self, update: StoreUpdate) {
        // A closed channel means the UI is gone; the loops notice that and stop.
        let _ = self.updates.send(update);
    }

    /// Runs a connect attempt while still answering commands. `None` means stop.
    async fn while_connecting(
        &mut self,
        attempt: impl Future<Output = Result<Client, ClientError>>,
    ) -> Option<Result<Client, ClientError>> {
        tokio::pin!(attempt);
        loop {
            tokio::select! {
                result = &mut attempt => return Some(result),
                command = self.commands.recv() => match command {
                    None | Some(Command::Stop) => return None,
                    Some(command) => {
                        if let Some(command) = self.host_call(command) {
                            decline_while_down(&mut self.watching, command);
                        }
                    }
                },
                () = self.updates.closed() => return None,
            }
        }
    }

    /// Sleeps `pause`, or less when "Retry now" arrives. Returns false to stop.
    async fn wait(&mut self, pause: Duration, backoff: &mut Backoff) -> bool {
        let timer = sleep(pause);
        tokio::pin!(timer);
        loop {
            tokio::select! {
                () = &mut timer => return true,
                command = self.commands.recv() => match command {
                    None | Some(Command::Stop) => return false,
                    Some(Command::RetryNow) => {
                        backoff.reset();
                        return true;
                    }
                    Some(command) => {
                        if let Some(command) = self.host_call(command) {
                            decline_while_down(&mut self.watching, command);
                        }
                    }
                },
                () = self.updates.closed() => return false,
            }
        }
    }

    /// Subscribes to events first, then lists, so no change falls between the two.
    async fn subscribe_and_list(&self, client: &Client) -> Result<Subscription, ClientError> {
        let events = client.subscribe_events().await?;
        let projects = client.project_list().await?.projects;
        let workspaces = client
            .workspace_list(&WorkspaceListParams { project: None })
            .await?
            .workspaces;
        self.emit(StoreUpdate::Resync {
            projects,
            workspaces,
        });
        let teams = teams::read(client, &self.team_reads).await?;
        self.emit(StoreUpdate::Teams(teams));
        let terminals = terminals::list_terminals(client).await?;
        self.emit(StoreUpdate::TerminalsResync(terminals));
        let sessions = agents::list_sessions(client).await?;
        self.emit(StoreUpdate::AgentsResync(sessions));
        // The protocol has no event for a change in the kinds, so they are read again on each connect.
        let kinds = agents::list_kinds(client).await?;
        for kind in &kinds {
            accounts::refresh_in_background(client, &kind.id, &self.updates, &self.accounts_order);
        }
        self.emit(StoreUpdate::AgentKindsResync(kinds));
        let read_extensions = extensions::read(client, &self.extension_reads).await?;
        self.emit(StoreUpdate::Extensions(read_extensions));
        secrets::refresh_in_background(client, &self.updates, &self.secrets_order);
        Ok(events)
    }

    /// Serves one connection: events in, commands out, until it is lost or the UI stops.
    async fn session(&mut self, client: &Client) -> SessionEnd {
        let Ok(mut events) = self.subscribe_and_list(client).await else {
            return SessionEnd::Lost;
        };
        let writes = terminals::spawn_writes(client.clone());
        // The watches end with this connection; the ones the UI still follows start again here.
        let mut watches = Watches::default();
        for workspace in &self.watching {
            watches.start(client, workspace, &self.updates, &self.freshness);
        }
        loop {
            let outcome = tokio::select! {
                event = events.next_as::<ChangeEvent>() => match event {
                    Some(Ok(event)) => self.on_event(client, event).await,
                    Some(Err(ClientError::Remote(body))) if body.code == codes::STREAM_OVERFLOW => {
                        Outcome::Resync
                    }
                    // An event this build cannot read changes nothing it shows.
                    Some(Err(ClientError::Protocol(_))) => Outcome::Continue,
                    Some(Err(_)) | None => Outcome::Lost,
                },
                command = self.commands.recv() => {
                    match command {
                        None | Some(Command::Stop) => return SessionEnd::Stop,
                        Some(command) => match self.host_call(command) {
                            None => {}
                            Some(Command::Terminal(command)) => {
                                terminals::spawn_call(client, &writes, &self.updates, command);
                            }
                            Some(Command::Team(command)) => {
                                teams::spawn_call(client, command, &self.updates, &self.team_reads);
                            }
                            Some(Command::Extension(command)) => {
                                extensions::spawn_call(
                                    client,
                                    command,
                                    &self.updates,
                                    &self.extension_reads,
                                );
                            }
                            Some(Command::Secret(command)) => {
                                secrets::spawn_call(
                                    client,
                                    command,
                                    &self.updates,
                                    &self.secrets_order,
                                );
                            }
                            Some(Command::Account(command)) => {
                                accounts::spawn_call(
                                    client,
                                    command,
                                    &self.updates,
                                    &self.accounts_order,
                                );
                            }
                            Some(Command::GitWatch { workspace }) => {
                                watches.start(client, &workspace, &self.updates, &self.freshness);
                                self.watching.insert(workspace);
                            }
                            Some(Command::GitUnwatch { workspace }) => {
                                watches.stop(&workspace);
                                self.watching.remove(&workspace);
                            }
                            Some(command) => spawn_call(client, command, &self.updates, &self.freshness),
                        },
                    }
                    Outcome::Continue
                }
                () = self.updates.closed() => return SessionEnd::Stop,
            };
            match outcome {
                Outcome::Continue => {}
                Outcome::Lost => return SessionEnd::Lost,
                Outcome::Resync => match self.subscribe_and_list(client).await {
                    Ok(fresh) => events = fresh,
                    Err(_) => return SessionEnd::Lost,
                },
            }
        }
    }

    /// Fetches the item an event names again, or drops it on a `*.removed` topic. Events carry
    /// only a topic and an id, and topics this build does not know are ignored (0002).
    async fn on_event(&self, client: &Client, event: ChangeEvent) -> Outcome {
        let ChangeEvent { topic, id } = event;
        let refetched = match topic.as_str() {
            TOPIC_SECRET_CHANGED => {
                // The event names a scope and a name; the store takes the whole list again, names only.
                secrets::refresh_in_background(client, &self.updates, &self.secrets_order);
                return Outcome::Continue;
            }
            TOPIC_ACCOUNTS_CHANGED => {
                // The event names the kind; its whole list is read again.
                accounts::refresh_in_background(client, &id, &self.updates, &self.accounts_order);
                return Outcome::Continue;
            }
            TOPIC_WORKSPACE_CREATED | TOPIC_WORKSPACE_REMOVING => client
                .workspace_show(&WorkspaceRef {
                    workspace: id.clone(),
                })
                .await
                .map(StoreUpdate::WorkspaceChanged)
                .or_else(|error| {
                    gone_if_missing(error, codes::WORKSPACE_NOT_FOUND, || {
                        StoreUpdate::WorkspaceGone(id.clone())
                    })
                }),
            TOPIC_PROJECT_ADDED => client
                .project_show(&ProjectRef {
                    project: id.clone(),
                })
                .await
                .map(StoreUpdate::ProjectChanged)
                .or_else(|error| {
                    gone_if_missing(error, codes::PROJECT_NOT_FOUND, || {
                        StoreUpdate::ProjectGone(id.clone())
                    })
                }),
            // A team or task change re-reads every team: the events carry only an id, and the read is cheap.
            TOPIC_TEAM_CHANGED | TOPIC_TASK_CHANGED => teams::read(client, &self.team_reads)
                .await
                .map(StoreUpdate::Teams),
            // The events carry only the package id, so both lists are read again (decision 0062).
            TOPIC_EXTENSION_INSTALLED
            | TOPIC_EXTENSION_ENABLED
            | TOPIC_EXTENSION_DISABLED
            | TOPIC_EXTENSION_REMOVED => extensions::read(client, &self.extension_reads)
                .await
                .map(StoreUpdate::Extensions),
            TOPIC_WORKSPACE_REMOVED => Ok(StoreUpdate::WorkspaceGone(id)),
            TOPIC_PROJECT_REMOVED => Ok(StoreUpdate::ProjectGone(id)),
            TOPIC_AGENT_CHANGED => client
                .agent_show(&id)
                .await
                .map(StoreUpdate::AgentChanged)
                .or_else(|error| {
                    gone_if_missing(error, codes::AGENT_NOT_FOUND, || {
                        StoreUpdate::AgentGone(id.clone())
                    })
                }),
            _ => return Outcome::Continue,
        };
        match refetched {
            Ok(update) => {
                self.emit(update);
                Outcome::Continue
            }
            Err(ClientError::Remote(_) | ClientError::Protocol(_)) => Outcome::Resync,
            Err(_) => Outcome::Lost,
        }
    }
}

/// A refetch that finds nothing means the item is gone; any other failure is passed on.
fn gone_if_missing(
    error: ClientError,
    missing_code: &str,
    gone: impl FnOnce() -> StoreUpdate,
) -> Result<StoreUpdate, ClientError> {
    match &error {
        ClientError::Remote(body) if body.code == missing_code => Ok(gone()),
        _ => Err(error),
    }
}

/// Runs a call on its own task so a slow one never holds up events.
fn spawn_call(
    client: &Client,
    command: Command,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
    freshness: &Arc<Freshness>,
) {
    let client = client.clone();
    match command {
        Command::Create { params, reply } => {
            tokio::spawn(async move {
                let result = client
                    .workspace_create(&params)
                    .await
                    .map_err(|error| Refusal::from(&error));
                let _ = reply.send(result);
            });
        }
        Command::Remove { params, reply } => {
            tokio::spawn(async move {
                let result = client
                    .workspace_remove(&params)
                    .await
                    .map_err(|error| Refusal::from(&error));
                let _ = reply.send(result);
            });
        }
        Command::AddProject { params, reply } => {
            tokio::spawn(async move {
                let result = client
                    .project_add(&params)
                    .await
                    .map_err(|error| Refusal::from(&error));
                let _ = reply.send(result);
            });
        }
        Command::ListConfig { reply } => {
            tokio::spawn(async move {
                let result = client
                    .config_list()
                    .await
                    .map_err(|error| Refusal::from(&error));
                let _ = reply.send(result);
            });
        }
        Command::SetConfig { params, reply } => {
            tokio::spawn(async move {
                let result = client
                    .config_set(&params)
                    .await
                    .map_err(|error| Refusal::from(&error));
                let _ = reply.send(result);
            });
        }
        Command::ListDevices { reply } => {
            let updates = updates.clone();
            tokio::spawn(async move {
                let result = client
                    .device_list()
                    .await
                    .map_err(|error| Refusal::from(&error));
                let _ = updates.send(StoreUpdate::Devices(
                    result.clone().map(|list| list.devices),
                ));
                let _ = reply.send(result);
            });
        }
        Command::RevokeDevice { params, reply } => {
            let updates = updates.clone();
            tokio::spawn(async move {
                let result = client
                    .device_revoke(&params)
                    .await
                    .map_err(|error| Refusal::from(&error));
                if result.is_ok() {
                    let listed = client
                        .device_list()
                        .await
                        .map_err(|error| Refusal::from(&error));
                    let _ = updates.send(StoreUpdate::Devices(listed.map(|list| list.devices)));
                }
                let _ = reply.send(result);
            });
        }
        Command::CreateOffer { params, reply } => {
            tokio::spawn(async move {
                let result = client
                    .pairing_create(&params)
                    .await
                    .map_err(|error| Refusal::from(&error));
                let _ = reply.send(result);
            });
        }
        Command::NetworkStatus { reply } => {
            let updates = updates.clone();
            tokio::spawn(async move {
                let result = client
                    .network_status()
                    .await
                    .map_err(|error| Refusal::from(&error));
                let _ = updates.send(StoreUpdate::Network(result.clone()));
                let _ = reply.send(result);
            });
        }
        Command::AgentStart { params, reply } => {
            agents::spawn_start(&client, params, reply, updates);
        }
        Command::BringBack { params, reply } => {
            agents::spawn_bring_back(&client, params, reply, updates);
        }
        // Routed by the session, which owns the terminal writes and store updates.
        Command::Terminal(_)
        | Command::Team(_)
        | Command::Secret(_)
        | Command::Account(_)
        | Command::RetryNow
        | Command::Stop => {}
        other => git::spawn(&client, other, updates, freshness),
    }
}

/// While the link is down a watch still changes what the bridge follows; the next connection starts it.
fn decline_while_down(watching: &mut BTreeSet<String>, command: Command) {
    match command {
        Command::GitWatch { workspace } => {
            watching.insert(workspace);
        }
        Command::GitUnwatch { workspace } => {
            watching.remove(&workspace);
        }
        other => refuse_while_down(other),
    }
}

/// While there is no connection a call cannot run, and says so at once.
fn refuse_while_down(command: Command) {
    let refusal = Refusal::unavailable("The service is not connected.");
    match command {
        Command::Create { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        Command::Remove { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        Command::AddProject { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        Command::ListConfig { reply } => {
            let _ = reply.send(Err(refusal));
        }
        Command::SetConfig { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        Command::Team(command) => teams::refuse(command, refusal),
        Command::Extension(command) => extensions::refuse_while_down(command, refusal),
        Command::ListDevices { reply } => {
            let _ = reply.send(Err(refusal));
        }
        Command::RevokeDevice { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        Command::CreateOffer { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        Command::NetworkStatus { reply } => {
            let _ = reply.send(Err(refusal));
        }
        Command::AgentStart { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        Command::BringBack { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
        Command::Terminal(command) => terminals::refuse_while_down(command),
        Command::Secret(command) => secrets::refuse_while_down(command, refusal),
        Command::Account(command) => accounts::refuse_while_down(command, refusal),
        other => git::refuse(other, refusal),
    }
}
