//! Starting and stopping the service of one profile (ADR 0003): take the lock, open the store, bind the endpoint, serve.

use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use interprocess::local_socket::{ListenerOptions, tokio::Listener};
use massorca_core_agents::{
    Agents, AgentsConfig, Change, EnvProvider, Listener as ChangeListener, ProvidedEnv,
    ProviderError,
};
use massorca_core_extensions::Extensions;
use massorca_core_git::Git;
use massorca_core_store::{Store, now_ms};
use massorca_core_terminal::shell::EnvSnapshot;
use massorca_core_terminal::{ManagerConfig, TerminalManager};
use massorca_core_vault::Vault;
use massorca_core_workspace::Workspaces;
#[cfg(windows)]
use massorca_profile::owner_only_security_descriptor;
#[cfg(unix)]
use massorca_profile::restrict_socket;
use massorca_profile::{Profile, RootAccess, ensure_dir, local_socket_name, secure_root};
use massorca_protocol::PeerInfo;
use massorca_protocol::methods::events::{
    ChangeEvent, TOPIC_ACCOUNTS_CHANGED, TOPIC_AGENT_CHANGED, TOPIC_AGENT_QUEUE_CHANGED,
};
use massorca_protocol::methods::service::ServiceInfo;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tokio::time::{Instant, sleep};
use zeroize::Zeroizing;

use crate::accept_loop;
use crate::activity::Activity;
use crate::context::{AppState, Registries};
use crate::error::ServiceError;
use crate::event_bus::EventBus;
use crate::features;
use crate::features::extension_process::HostPrograms;
use crate::instance_lock;
use crate::kind_packages::{ExtensionKindPackages, prepare_builtin_agents};
use crate::network::{self, NetworkConfig};
use crate::service_json;
use crate::shutdown::{self, ShutdownHandle};
use crate::start_timing::StartTimings;

/// What the service needs to start.
#[derive(Debug)]
pub struct ServiceConfig {
    pub profile: Profile,
    pub server_name: String,
    pub server_version: String,
}

/// Where the vault keeps a new key (decision 0071).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VaultOptions {
    /// Keeps a new key in an owner-only file instead of the OS store.
    pub force_file: bool,
}

/// Selects the file protection; tests set it so no profile writes into a real keyring.
pub const VAULT_PROTECTION_ENV: &str = "MASSORCA_VAULT_PROTECTION";

impl VaultOptions {
    /// `force_file` when `MASSORCA_VAULT_PROTECTION=file`.
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            force_file: std::env::var_os(VAULT_PROTECTION_ENV).is_some_and(|value| value == "file"),
        }
    }
}

/// Starts services.
#[derive(Debug)]
pub struct Service;

impl Service {
    /// Takes the profile lock, opens the store, binds the endpoint and starts serving. Fails with `AlreadyRunning` while another service holds the lock.
    pub async fn start(config: ServiceConfig) -> Result<RunningService, ServiceError> {
        Self::start_with_network(config, NetworkConfig::default()).await
    }

    /// [`Service::start`] with explicit vault options instead of the environment's.
    pub async fn start_with(
        config: ServiceConfig,
        vault_options: VaultOptions,
    ) -> Result<RunningService, ServiceError> {
        Self::start_inner(config, NetworkConfig::default(), vault_options, None).await
    }

    /// As `start`, and also listens on the network as `network` says (decision 0045).
    pub async fn start_with_network(
        config: ServiceConfig,
        network: NetworkConfig,
    ) -> Result<RunningService, ServiceError> {
        Self::start_inner(config, network, VaultOptions::from_env(), None).await
    }

    /// As `start_with_network`, for a caller that has just checked the profile root itself with `secure_root` and got
    /// `checked` (`service run` does, before it writes its logs), so the check is not repeated. Nothing in the profile
    /// was trusted between that check and this call.
    pub async fn start_checked(
        config: ServiceConfig,
        network: NetworkConfig,
        checked: RootAccess,
    ) -> Result<RunningService, ServiceError> {
        Self::start_inner(config, network, VaultOptions::from_env(), Some(checked)).await
    }

    async fn start_inner(
        config: ServiceConfig,
        network: NetworkConfig,
        vault_options: VaultOptions,
        checked: Option<RootAccess>,
    ) -> Result<RunningService, ServiceError> {
        let mut timings = StartTimings::new();
        let ServiceConfig {
            profile,
            server_name,
            server_version,
        } = config;
        // Before anything in the profile is opened, so planted content is never trusted (ADR 0073).
        // A caller that checked the root itself has already reported a repair.
        if checked.is_none() && secure_root(&profile)? == RootAccess::Repaired {
            tracing::warn!(
                path = %profile.dir().display(),
                "profile directory access was repaired: it is now private to its owner"
            );
        }
        timings.mark("root check");
        ensure_dir(&profile)?;
        let lock = instance_lock::acquire(&profile).await?;
        instance_lock::remove_stale_socket(&profile)?;
        timings.mark("lock");

        let store = open_store(profile.store_path()).await?;
        timings.mark("store");
        let listener = bind_endpoint_when_released(&profile).await?;
        timings.mark("endpoint");

        let (network_state, acceptor) = network::prepare(&profile, &network, &server_name)?;
        let registries = Registries::default();
        let registrations = features::register_core(&registries)?;
        let (shutdown, signal) = shutdown::channel();
        let started_at_ms = now_ms();
        // A missing git is reported per call as git.unavailable, so the start does not run git to find out.
        let git = Git::from_env();
        let default_root = PathBuf::from(features::default_workspace_root()?);
        let workspaces = Arc::new(Workspaces::new(store.clone(), git, default_root));
        let mut terminal_config =
            ManagerConfig::new(EnvSnapshot::current(), server_version.clone());
        terminal_config.massorca_home = profile.home().to_str().map(str::to_owned);
        terminal_config.massorca_profile = Some(profile.name().to_owned());
        let terminals = TerminalManager::new(terminal_config);
        let vault = Arc::new(Vault::new(
            store.clone(),
            profile.dir().join("vault"),
            vault_options.force_file,
        ));
        let host_programs = Arc::new(HostPrograms::default());
        let extensions = Arc::new(Extensions::new(
            &profile,
            host_programs.clone(),
            host_programs,
        ));
        let agents = Agents::new(AgentsConfig {
            store: store.clone(),
            terminals: terminals.clone(),
            workspaces: Arc::clone(&workspaces),
            profile_dir: profile.dir().to_path_buf(),
            massorca_bin: std::env::current_exe()?,
            env_provider: Some(vault_env_provider(Arc::clone(&vault))),
            kind_packages: Arc::new(ExtensionKindPackages::new(profile.clone())),
        });
        // No run survives the previous service (0051); nothing has started one yet in this service.
        let marker = agents.clone();
        tokio::task::spawn_blocking(move || marker.mark_stale_on_startup())
            .await
            .map_err(io::Error::other)?
            .map_err(|error| io::Error::other(error.to_string()))?;
        timings.mark("stale runs");
        // Credential homes and account rows are put in order before requests are served (decision 0059).
        let sweeper = agents.clone();
        tokio::task::spawn_blocking(move || sweeper.startup_sweep())
            .await
            .map_err(io::Error::other)?;
        timings.mark("agent homes");
        // After the sweep, so the kinds its accounts name count as used (decision 0063).
        let (adopter, adopter_store, adopter_profile) =
            (Arc::clone(&extensions), store.clone(), profile.clone());
        tokio::task::spawn_blocking(move || {
            prepare_builtin_agents(&adopter, &adopter_store, &adopter_profile);
        })
        .await
        .map_err(io::Error::other)?;
        timings.mark("built-in agents");
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
            vault,
            terminals: terminals.clone(),
            agents,
            extensions,
            network: network_state,
            server: PeerInfo {
                name: server_name,
                version: server_version.clone(),
            },
            started_at_ms,
            activity: Activity::default(),
        });
        // Subscribed before the first request is served, so no agent change is missed (decision 0072).
        let outcome_changes = state.agent_changes.subscribe();
        service_json::write(&profile, &server_version, started_at_ms)?;

        // Terminals end as soon as shutdown is requested, alongside the connections' grace period.
        let mut terminals_signal = signal.clone();
        let terminals_stop = tokio::spawn(async move {
            terminals_signal.wait().await;
            let _ = tokio::task::spawn_blocking(move || terminals.shutdown()).await;
        });
        let outcomes_stop = tokio::spawn(features::follow_task_outcomes(
            Arc::clone(&state),
            outcome_changes,
            signal.clone(),
        ));
        let routines_stop = tokio::spawn(features::follow_routines(
            Arc::clone(&state),
            signal.clone(),
        ));
        let listeners = network::spawn_listeners(&network, acceptor, &state, &signal);
        let accept_state = Arc::clone(&state);
        let task = tokio::spawn(async move {
            accept_loop::run(
                listener,
                listeners,
                accept_state,
                signal,
                lock,
                registrations,
            )
            .await;
            let _ = terminals_stop.await;
            let _ = outcomes_stop.await;
            let _ = routines_stop.await;
        });
        timings.mark("serving");
        timings.report();
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
            // An account change is announced under its kind; it is not a session change, so no watcher gets it.
            Change::Account(account) => {
                events.publish(ChangeEvent {
                    topic: TOPIC_ACCOUNTS_CHANGED.to_owned(),
                    id: account.kind.clone(),
                });
                return;
            }
            // A queue change is announced under its session; no session record changed, so no watcher gets it (0084).
            Change::Queue(session) => {
                events.publish(ChangeEvent {
                    topic: TOPIC_AGENT_QUEUE_CHANGED.to_owned(),
                    id: session.clone(),
                });
                return;
            }
        };
        events.publish(ChangeEvent {
            topic: TOPIC_AGENT_CHANGED.to_owned(),
            id,
        });
        let _ = changes.send(change);
    })
}

async fn open_store(path: PathBuf) -> Result<Store, ServiceError> {
    let parts = StartTimings::part_logger();
    let opened = tokio::task::spawn_blocking(move || Store::open_observed(path, parts))
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

    /// The installed extensions of the profile. Other features register remove guards and cleanups on it.
    pub fn extensions(&self) -> &Arc<Extensions> {
        &self.state.extensions
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

/// Each agent run's variables from the vault (decision 0071): the global secrets and the project's own and linked ones.
/// The one place a value leaves the vault, copied into a zeroizing string at the last moment.
fn vault_env_provider(vault: Arc<Vault>) -> EnvProvider {
    EnvProvider::new(move |project_id| {
        let variables = vault
            .environment_for(project_id)
            .map_err(|error| ProviderError(error.to_string()))?;
        Ok(ProvidedEnv::new(
            variables
                .into_iter()
                .map(|(name, value)| (name, Zeroizing::new(value.expose().to_owned())))
                .collect(),
        ))
    })
}

#[cfg(test)]
mod agent_queue_event_tests {
    use std::time::Duration;

    use massorca_core_agents::Change;
    use massorca_protocol::methods::events::{ChangeEvent, TOPIC_AGENT_QUEUE_CHANGED};
    use tokio::sync::broadcast;

    use super::agent_listener;
    use crate::event_bus::EventBus;

    /// A queue change reaches the event bus as `agent.queue_changed` under its session id, and no `agent.watch` stream
    /// gets it (decision 0084).
    #[test]
    fn a_queue_change_reaches_the_event_bus_under_its_session() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("a runtime");
        runtime.block_on(async {
            let events = EventBus::new();
            let (changes, mut watcher) = broadcast::channel(8);
            let mut subscription = events.subscribe();
            let listener = agent_listener(events.clone(), changes);

            listener(Change::Queue("session-1".to_owned()));

            let event = tokio::time::timeout(Duration::from_secs(5), subscription.next())
                .await
                .expect("the event arrives")
                .expect("the bus is open");
            assert_eq!(
                event,
                ChangeEvent {
                    topic: TOPIC_AGENT_QUEUE_CHANGED.to_owned(),
                    id: "session-1".to_owned(),
                }
            );
            assert!(
                watcher.try_recv().is_err(),
                "no watcher receives a queue change"
            );
        });
    }
}
