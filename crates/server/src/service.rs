//! Starting and stopping the service of one profile (ADR 0003): take the lock, open the store, bind the endpoint, serve.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use interprocess::local_socket::{ListenerOptions, tokio::Listener};
use massorca_core_agents::{Agents, AgentsConfig, Change, Listener as ChangeListener};
use massorca_core_git::Git;
use massorca_core_store::{Store, now_ms};
use massorca_core_terminal::shell::EnvSnapshot;
use massorca_core_terminal::{ManagerConfig, TerminalManager};
use massorca_core_workspace::Workspaces;
#[cfg(windows)]
use massorca_profile::owner_only_security_descriptor;
#[cfg(unix)]
use massorca_profile::restrict_socket;
use massorca_profile::{Profile, RootAccess, ensure_dir, local_socket_name, secure_root};
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::events::{ChangeEvent, TOPIC_AGENT_CHANGED};
use massorca_protocol::methods::service::ServiceInfo;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep};

use crate::accept_loop;
use crate::activity::Activity;
use crate::context::{AppState, Registries};
use crate::error::ServiceError;
use crate::event_bus::EventBus;
use crate::features;
use crate::instance_lock;
use crate::service_json;
use crate::shutdown::{self, ShutdownHandle};

/// What the service needs to start.
#[derive(Debug)]
pub struct ServiceConfig {
    pub profile: Profile,
    pub server_name: String,
    pub server_version: String,
}

/// Starts services.
#[derive(Debug)]
pub struct Service;

impl Service {
    /// Takes the profile lock, opens the store, binds the endpoint and starts serving. Fails with `AlreadyRunning` while another service holds the lock.
    pub async fn start(config: ServiceConfig) -> Result<RunningService, ServiceError> {
        let ServiceConfig {
            profile,
            server_name,
            server_version,
        } = config;
        // Before anything in the profile is opened, so planted content is never trusted (ADR 0073).
        if secure_root(&profile)? == RootAccess::Repaired {
            tracing::warn!(
                path = %profile.dir().display(),
                "profile directory access was repaired: it is now private to its owner"
            );
        }
        ensure_dir(&profile)?;
        let lock = instance_lock::acquire(&profile).await?;
        instance_lock::remove_stale_socket(&profile)?;

        let store = open_store(profile.store_path()).await?;
        let listener = bind_endpoint_when_released(&profile).await?;

        let registries = Registries::default();
        let registrations = features::register_core(&registries)?;
        let (shutdown, signal) = shutdown::channel();
        let started_at_ms = now_ms();
        // A missing git is reported per call as git.unavailable, so the service still starts.
        let git = Git::discover().unwrap_or_else(|_| Git::with_binary("git"));
        let default_root = PathBuf::from(features::default_workspace_root()?);
        let workspaces = Arc::new(Workspaces::new(store.clone(), git, default_root));
        let mut terminal_config =
            ManagerConfig::new(EnvSnapshot::current(), server_version.clone());
        terminal_config.massorca_home = profile.home().to_str().map(str::to_owned);
        terminal_config.massorca_profile = Some(profile.name().to_owned());
        let terminals = TerminalManager::new(terminal_config);
        let agents = Agents::new(AgentsConfig {
            store: store.clone(),
            terminals: terminals.clone(),
            workspaces: Arc::clone(&workspaces),
            profile_dir: profile.dir().to_path_buf(),
            massorca_bin: std::env::current_exe()?,
        });
        // No run survives the previous service (0051); nothing has started one yet in this service.
        let marker = agents.clone();
        tokio::task::spawn_blocking(move || marker.mark_stale_on_startup())
            .await
            .map_err(io::Error::other)?
            .map_err(|error| io::Error::other(error.to_string()))?;
        let events = EventBus::new();
        let (agent_changes, _) = broadcast::channel(AGENT_CHANGES_CAPACITY);
        agents.set_listener(agent_listener(events.clone(), agent_changes.clone()));
        let state = Arc::new(AppState {
            profile: profile.clone(),
            store,
            events,
            agent_changes,
            shutdown: shutdown.clone(),
            registries,
            workspaces,
            terminals: terminals.clone(),
            agents,
            server: PeerInfo {
                name: server_name,
                version: server_version.clone(),
            },
            started_at_ms,
            activity: Activity::default(),
        });
        service_json::write(&profile, &server_version, started_at_ms)?;

        // Terminals end as soon as shutdown is requested, alongside the connections' grace period.
        let mut terminals_signal = signal.clone();
        let terminals_stop = tokio::spawn(async move {
            terminals_signal.wait().await;
            let _ = tokio::task::spawn_blocking(move || terminals.shutdown()).await;
        });
        let accept_state = Arc::clone(&state);
        let task = tokio::spawn(async move {
            accept_loop::run(listener, accept_state, signal, lock, registrations).await;
            let _ = terminals_stop.await;
        });
        Ok(RunningService {
            state,
            shutdown,
            task: Some(task),
        })
    }
}

/// Agent changes a watcher may fall behind by before it is given a fresh snapshot.
const AGENT_CHANGES_CAPACITY: usize = 1024;

/// Announces each agent status change on the event bus (`agent.changed`) and to the watchers, without blocking.
fn agent_listener(events: EventBus, changes: broadcast::Sender<Change>) -> ChangeListener {
    Box::new(move |change: Change| {
        let id = match &change {
            Change::Updated(session) => session.id.clone(),
            Change::Forgotten(id) => id.clone(),
        };
        events.publish(ChangeEvent {
            topic: TOPIC_AGENT_CHANGED.to_owned(),
            id,
        });
        let _ = changes.send(change);
    })
}

async fn open_store(path: PathBuf) -> Result<Store, ServiceError> {
    let opened = tokio::task::spawn_blocking(move || Store::open(path))
        .await
        .map_err(io::Error::other)?;
    Ok(opened?)
}

/// A service that is serving. Dropping it requests shutdown; `wait` resolves once the service has stopped.
#[derive(Debug)]
pub struct RunningService {
    state: Arc<AppState>,
    shutdown: ShutdownHandle,
    task: Option<JoinHandle<()>>,
}

impl RunningService {
    /// What `service.info` reports.
    pub fn info(&self) -> ServiceInfo {
        features::info_of(&self.state)
    }

    pub fn shutdown_handle(&self) -> ShutdownHandle {
        self.shutdown.clone()
    }

    /// The registries that core and extensions contribute to.
    pub fn registries(&self) -> &Registries {
        &self.state.registries
    }

    /// The in-process event bus that publishes change events to subscribers.
    pub fn events(&self) -> &EventBus {
        &self.state.events
    }

    /// Waits until the service has stopped: the listener is closed, connections are gone and the profile lock is released.
    pub async fn wait(mut self) -> Result<(), ServiceError> {
        if let Some(task) = self.task.take() {
            task.await.map_err(io::Error::other)?;
        }
        Ok(())
    }
}

impl Drop for RunningService {
    fn drop(&mut self) {
        self.shutdown.shutdown();
    }
}

/// How long a bind is retried while the operating system still holds the previous service's endpoint. Windows refuses a
/// new first pipe instance until the last instance of the old service is closed.
const BIND_WAIT: Duration = Duration::from_secs(5);
const BIND_POLL: Duration = Duration::from_millis(25);

async fn bind_endpoint_when_released(profile: &Profile) -> Result<Listener, ServiceError> {
    let deadline = Instant::now() + BIND_WAIT;
    loop {
        match bind_endpoint(profile) {
            Err(ServiceError::Io(error))
                if error.kind() == io::ErrorKind::PermissionDenied && Instant::now() < deadline =>
            {
                sleep(BIND_POLL).await;
            }
            bound => return bound,
        }
    }
}

/// Binds the endpoint so that only the current user can reach it. On Windows the pipe is created with an owner-only
/// descriptor; on Unix the socket file is restricted to its owner once bound.
#[cfg(windows)]
fn bind_endpoint(profile: &Profile) -> Result<Listener, ServiceError> {
    use interprocess::os::windows::local_socket::ListenerOptionsExt;
    Ok(ListenerOptions::new()
        .name(local_socket_name(profile)?)
        .security_descriptor(owner_only_security_descriptor()?)
        .create_tokio()?)
}

#[cfg(unix)]
fn bind_endpoint(profile: &Profile) -> Result<Listener, ServiceError> {
    let listener = ListenerOptions::new()
        .name(local_socket_name(profile)?)
        .create_tokio()?;
    restrict_socket(profile)?;
    Ok(listener)
}
