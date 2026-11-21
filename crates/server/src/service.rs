//! Starting and stopping the service of one profile (ADR 0003): take the lock, open the store, bind the endpoint, serve.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use interprocess::local_socket::ListenerOptions;
use massorca_core_store::{Store, now_ms};
use massorca_profile::{Profile, ensure_dir, local_socket_name};
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::service::ServiceInfo;
use tokio::task::JoinHandle;

use crate::accept_loop;
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
        let lock = instance_lock::acquire(&profile)?;
        instance_lock::remove_stale_socket(&profile)?;

        let store = open_store(profile.store_path()).await?;
        let listener = ListenerOptions::new()
            .name(local_socket_name(&profile)?)
            .create_tokio()?;

        let registries = Registries::default();
        let registrations = features::register_core(&registries)?;
        let (shutdown, signal) = shutdown::channel();
        let started_at_ms = now_ms();
        let state = Arc::new(AppState {
            profile: profile.clone(),
            store,
            events: EventBus::new(),
            shutdown: shutdown.clone(),
            registries,
            server: PeerInfo {
                name: server_name,
                version: server_version.clone(),
            },
            started_at_ms,
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
