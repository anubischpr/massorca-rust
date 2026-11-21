//! Starting and stopping the service of one profile (ADR 0003): take the lock, open the store, bind the endpoint, serve.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use interprocess::local_socket::{ListenerOptions, tokio::Listener};
use massorca_core_git::Git;
use massorca_core_store::{Store, now_ms};
use massorca_core_workspace::Workspaces;
#[cfg(windows)]
use massorca_profile::owner_only_security_descriptor;
#[cfg(unix)]
use massorca_profile::restrict_socket;
use massorca_profile::{Profile, ensure_dir, local_socket_name};
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::service::ServiceInfo;
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
        let state = Arc::new(AppState {
            profile: profile.clone(),
            store,
            events: EventBus::new(),
            shutdown: shutdown.clone(),
            registries,
            workspaces,
            server: PeerInfo {
                name: server_name,
                version: server_version.clone(),
            },
            started_at_ms,
            activity: Activity::default(),
        });
        service_json::write(&profile, &server_version, started_at_ms)?;

        let task = tokio::spawn(accept_loop::run(
            listener,
            Arc::clone(&state),
            signal,
            lock,
            registrations,
        ));
        Ok(RunningService {
            state,
            shutdown,
            task: Some(task),
        })
    }
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
