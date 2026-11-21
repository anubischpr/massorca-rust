//! The bridge between the UI thread and the service (decision 0028). One tokio runtime on its own
//! thread owns the connection, every call and every subscription. The UI never touches a tokio
//! socket: it sends commands, awaits oneshot answers, and receives `StoreUpdate`s on an unbounded
//! channel, none of which needs a reactor.

use std::io;
use std::thread;
use std::time::{Duration, Instant};

use massorca_client::{Autostart, Client, ClientError, ConnectOptions, Subscription};
use massorca_profile::Profile;
use massorca_protocol::methods::config::{ConfigList, ConfigSet, ConfigValue};
use massorca_protocol::methods::events::{
    ChangeEvent, TOPIC_PROJECT_ADDED, TOPIC_PROJECT_REMOVED, TOPIC_WORKSPACE_CREATED,
    TOPIC_WORKSPACE_REMOVED, TOPIC_WORKSPACE_REMOVING,
};
use massorca_protocol::methods::projects::{Project, ProjectAddParams, ProjectRef};
use massorca_protocol::methods::workspaces::{
    Workspace, WorkspaceCreateParams, WorkspaceListParams, WorkspaceRef, WorkspaceRemoveParams,
    WorkspaceRemoved,
};
use massorca_protocol::{PeerInfo, codes};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::sleep;

use super::connection::{Backoff, ConnectionState, after_failed_attempt, after_loss};
use super::refusal::Refusal;
use super::update::StoreUpdate;

const BRIDGE_STOPPED: &str = "The link to the service has stopped.";

/// Who the desktop says it is in the handshake, and whether it may start the service.
pub fn connect_options(autostart: Option<Autostart>) -> ConnectOptions {
    let mut options = ConnectOptions::new(PeerInfo {
        name: "massorca-desktop".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    });
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
    SetConfig {
        params: ConfigSet,
        reply: oneshot::Sender<Result<ConfigValue, Refusal>>,
    },
    Stop,
}

/// The service's side of a handle made for tests: the calls the UI sent, to answer by hand.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct Script {
    commands: mpsc::UnboundedReceiver<Command>,
    _connection: watch::Sender<ConnectionState>,
}

#[cfg(test)]
impl Script {
    /// The next call the UI sent, if one is waiting.
    pub(crate) fn next(&mut self) -> Option<Command> {
        self.commands.try_recv().ok()
    }
}

/// The UI's side of the link. Clones share it.
#[derive(Clone, Debug)]
pub struct ServiceHandle {
    commands: mpsc::UnboundedSender<Command>,
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
                _connection: connection_tx,
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

    /// Sends one command and waits for its answer. A bridge that has stopped answers as unavailable.
    async fn ask<T>(
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
}

impl Link {
    /// Connects, serves while connected, and reconnects with backoff, until stopped.
    async fn supervise(mut self) {
        let mut backoff = Backoff::default();
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
                    Some(command) => refuse_while_down(command),
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
                    Some(command) => refuse_while_down(command),
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
        Ok(events)
    }

    /// Serves one connection: events in, commands out, until it is lost or the UI stops.
    async fn session(&mut self, client: &Client) -> SessionEnd {
        let Ok(mut events) = self.subscribe_and_list(client).await else {
            return SessionEnd::Lost;
        };
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
                        Some(command) => spawn_call(client, command),
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
            TOPIC_WORKSPACE_REMOVED => Ok(StoreUpdate::WorkspaceGone(id)),
            TOPIC_PROJECT_REMOVED => Ok(StoreUpdate::ProjectGone(id)),
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
fn spawn_call(client: &Client, command: Command) {
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
        Command::RetryNow | Command::Stop => {}
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
        Command::RetryNow | Command::Stop => {}
    }
}
