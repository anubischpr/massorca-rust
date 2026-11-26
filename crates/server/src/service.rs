//! Starting and stopping the service of one profile (ADR 0003): take the lock, open the store, bind the endpoint, serve.

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, Weak};
use std::time::Duration;

use interprocess::local_socket::{ListenerOptions, tokio::Listener};
use massorca_core_agents::{
    Agents, AgentsConfig, Change, EnvProvider, Listener as ChangeListener, ProvidedEnv,
    ProviderError,
};
use massorca_core_extensions::skills::SkillRootEnv;
use massorca_core_extensions::{Extensions, HookError};
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
use crate::extension_points::{ExtensionPoints, sync_extension_points};
use crate::features;
use crate::features::extension_process::HostPrograms;
use crate::instance_lock;
use crate::kind_packages::{ExtensionKindPackages, prepare_builtin_agents};
use crate::network::{self, NetworkConfig};
use crate::readiness::Readiness;
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

/// Where skill discovery looks on this host (SKL-01): the user's home folder and the folder overrides agent tools read
/// from the environment. Tests point `home` at a temporary folder.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscoveryOptions {
    /// The user's home folder. `None` leaves nothing to scan, so `skills.discover` fails.
    pub home: Option<PathBuf>,
    /// `CLAUDE_CONFIG_DIR`, `GROK_HOME` and `HERMES_HOME`, as the agent tools read them.
    pub env: SkillRootEnv,
}

/// Overrides the user's home folder that skill discovery reads; tests set it so no scan reaches the real home.
pub const SKILL_HOME_ENV: &str = "MASSORCA_SKILL_HOME";

impl DiscoveryOptions {
    /// `SKILL_HOME_ENV` or else the platform's home folder, and each folder override the environment sets to an absolute
    /// path.
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_lookup(|name| std::env::var_os(name))
    }

    /// `from_env` over any lookup. A relative path is ignored everywhere, so only absolute overrides take effect.
    fn from_lookup(lookup: impl Fn(&str) -> Option<OsString>) -> Self {
        let absolute = |name: &str| {
            lookup(name)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
        };
        Self {
            home: absolute(SKILL_HOME_ENV)
                .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf())),
            env: SkillRootEnv {
                claude_config_dir: absolute("CLAUDE_CONFIG_DIR"),
                grok_home: absolute("GROK_HOME"),
                hermes_home: absolute("HERMES_HOME"),
            },
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
        let discovery = DiscoveryOptions::from_env();
        Self::start_inner(
            config,
            NetworkConfig::default(),
            vault_options,
            discovery,
            None,
        )
        .await
    }

    /// [`Service::start`] with explicit skill discovery options instead of the environment's.
    pub async fn start_with_discovery(
        config: ServiceConfig,
        discovery: DiscoveryOptions,
    ) -> Result<RunningService, ServiceError> {
        Self::start_with_network_and_discovery(config, NetworkConfig::default(), discovery).await
    }

    /// As `start_with_network`, with explicit skill discovery options instead of the environment's.
    pub async fn start_with_network_and_discovery(
        config: ServiceConfig,
        network: NetworkConfig,
        discovery: DiscoveryOptions,
    ) -> Result<RunningService, ServiceError> {
        let vault = VaultOptions::from_env();
        Self::start_inner(config, network, vault, discovery, None).await
    }

    /// As `start_with_network`, with explicit vault options instead of the environment's.
    pub async fn start_with_network_and_vault(
        config: ServiceConfig,
        network: NetworkConfig,
        vault: VaultOptions,
    ) -> Result<RunningService, ServiceError> {
        Self::start_inner(config, network, vault, DiscoveryOptions::default(), None).await
    }

    /// As `start`, and also listens on the network as `network` says (decision 0045).
    pub async fn start_with_network(
        config: ServiceConfig,
        network: NetworkConfig,
    ) -> Result<RunningService, ServiceError> {
        let (vault, discovery) = (VaultOptions::from_env(), DiscoveryOptions::from_env());
        Self::start_inner(config, network, vault, discovery, None).await
    }

    /// As `start_with_network`, for a caller that has just checked the profile root itself with `secure_root` and got
    /// `checked` (`service run` does, before it writes its logs), so the check is not repeated. Nothing in the profile
    /// was trusted between that check and this call.
    pub async fn start_checked(
        config: ServiceConfig,
        network: NetworkConfig,
        checked: RootAccess,
    ) -> Result<RunningService, ServiceError> {
        let (vault, discovery) = (VaultOptions::from_env(), DiscoveryOptions::from_env());
        Self::start_inner(config, network, vault, discovery, Some(checked)).await
    }

    async fn start_inner(
        config: ServiceConfig,
        network: NetworkConfig,
        vault_options: VaultOptions,
        discovery_options: DiscoveryOptions,
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
        let workspaces = Arc::new(Workspaces::new(store.clone(), git.clone(), default_root));
        // A run identity the service inherited belongs to another session: no terminal of this service carries it
        // (decision 0160). Each agent run gets its own from its launch.
        let base_env = EnvSnapshot::from_pairs(std::env::vars_os().filter_map(|(name, value)| {
            let name = name.into_string().ok()?;
            // A vendor's session marker (an inherited CLAUDE_CODE_CHILD_SESSION, say) is dropped too, so no run reads it.
            (!massorca_core_agents::launch::is_run_identity(&name)
                && !massorca_core_agents::launch::is_inherited_vendor_marker(&name))
            .then_some((name, value.into_string().ok()?))
        }));
        let mut terminal_config = ManagerConfig::new(base_env, server_version.clone());
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
            host_programs.clone(),
        ));
        let agents = Agents::new(AgentsConfig {
            store: store.clone(),
            terminals: terminals.clone(),
            workspaces: Arc::clone(&workspaces),
            profile_dir: profile.dir().to_path_buf(),
            massorca_bin: std::env::current_exe()?,
            conversations_default: PathBuf::from(crate::features::default_conversations_root()?),
            env_provider: Some(vault_env_provider(Arc::clone(&vault))),
            kind_packages: Arc::new(ExtensionKindPackages::new(profile.clone())),
            // The service's own environment: where the vendor CLIs it starts save their conversations (decision 0160).
            vendor_env: Some(massorca_core_agents::VendorEnv::process()),
            // The host's search path, the one the extension install check uses (decision 0164).
            vendor_search: massorca_core_agents::VendorSearch::host(),
        });
        // A package whose kind a live or resumable session uses is not removed, disabled or updated away (decision 0062).
        let guarded = agents.clone();
        let kind_guard = extensions.register_remove_guard("agents", move |package| {
            guarded
                .refuse_removing_package(package)
                .map_err(|error| HookError::new(error.code, error.message))
        });
        // A package removed with its agent homes takes its kinds' accounts with it; nothing is signed out (decision 0098).
        let cleaned = agents.clone();
        let kind_cleanup = extensions.register_remove_cleanup("agents", move |removed| {
            cleaned
                .remove_package_accounts(&removed.id, &removed.agent_kinds)
                .map_err(|error| HookError::new(error.code, error.message))
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
        // Registered before the first sync, so a removal in between still drops its owner (decision 0153).
        let extension_points = Arc::new(ExtensionPoints::default());
        let dropped = Arc::clone(&extension_points);
        let points_listener =
            extensions.register_remove_listener("extension-points", move |removed| {
                dropped.drop_owner(&removed.id);
            });
        let (synced, packages) = (Arc::clone(&extension_points), Arc::clone(&extensions));
        tokio::task::spawn_blocking(move || sync_extension_points(&synced, &packages))
            .await
            .map_err(io::Error::other)?;
        timings.mark("extension points");
        let events = EventBus::new();
        let (agent_changes, _) = broadcast::channel(AGENT_CHANGES_CAPACITY);
        let (chat_changes, _) = broadcast::channel(CHAT_CHANGES_CAPACITY);
        agents.set_listener(agent_listener(
            events.clone(),
            agent_changes.clone(),
            chat_changes.clone(),
        ));
        // The prompt listener reaches the state once it is built, below (decision 0118).
        let first_prompt_state: Arc<OnceLock<Weak<AppState>>> = Arc::default();
        agents.set_prompt_listener(features::first_prompt_rename::prompt_listener(
            Arc::clone(&first_prompt_state),
            tokio::runtime::Handle::current(),
        ));
        let state = Arc::new(AppState {
            profile: profile.clone(),
            store,
            events,
            agent_changes,
            chat_changes,
            shutdown: shutdown.clone(),
            registries,
            workspaces,
            git,
            vault,
            terminals: terminals.clone(),
            agents,
            extensions,
            host_programs,
            skills: features::skills::SkillScans::new(discovery_options),
            _kind_guard: kind_guard,
            _kind_cleanup: kind_cleanup,
            extension_points,
            _points_listener: points_listener,
            network: network_state,
            server: PeerInfo {
                name: server_name,
                version: server_version.clone(),
            },
            started_at_ms,
            activity: Activity::default(),
            first_prompt_titles: Default::default(),
        });
        let _ = first_prompt_state.set(Arc::downgrade(&state));
        // Subscribed before the first request is served, so no agent change is missed (decision 0072).
        let outcome_changes = state.agent_changes.subscribe();
        service_json::write(&profile, &server_version, started_at_ms)?;

        // Terminals and chat runs end as soon as shutdown is requested, alongside the connections' grace period.
        let mut terminals_signal = signal.clone();
        let chat_runs = state.agents.clone();
        let terminals_stop = tokio::spawn(async move {
            terminals_signal.wait().await;
            let _ = tokio::task::spawn_blocking(move || {
                terminals.shutdown();
                chat_runs.stop_chat_runs();
            })
            .await;
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
/// Chat journal changes kept for chat streams that are behind; one that falls further behind is reset to a snapshot.
const CHAT_CHANGES_CAPACITY: usize = 4096;

/// Announces each agent status change on the event bus (`agent.changed`) and to the watchers, without blocking.
fn agent_listener(
    events: EventBus,
    changes: broadcast::Sender<Change>,
    chats: broadcast::Sender<Change>,
) -> ChangeListener {
    Box::new(move |change: Change| {
        let id = match &change {
            Change::Updated(session) => session.id.clone(),
            Change::Forgotten(id) => {
                // A forgotten session's chat streams end (decision 0101).
                let _ = chats.send(change.clone());
                id.clone()
            }
            // A chat journal change goes to the chat streams only; no session record changed (decision 0101).
            Change::Chat(_) => {
                let _ = chats.send(change);
                return;
            }
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
    /// Waits until the network listeners have made their first bind attempt, then reads the readiness record. `None` when
    /// shutdown is requested first, so a service that is stopping reports nothing as ready.
    pub async fn ready(&self) -> Option<Readiness> {
        tokio::select! {
            biased;
            () = self.shutdown.listener_closed() => None,
            () = self.state.network.wait_settled() => {
                Some(Readiness::of(
                    &self.state.profile,
                    &self.state.network,
                    self.state.terminals.live_count(),
                ))
            }
        }
    }

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
            let (chats, _) = broadcast::channel(8);
            let mut subscription = events.subscribe();
            let listener = agent_listener(events.clone(), changes, chats);

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

#[cfg(test)]
mod chat_change_tests {
    use massorca_core_agents::Change;
    use massorca_core_agents::chat::ChatChanged;
    use massorca_protocol::methods::chat::{ChatBatch, ChatCursor};
    use tokio::sync::broadcast;

    use super::{EventBus, agent_listener};

    /// A chat journal change reaches the chat streams only; a forgotten session reaches them and the watchers
    /// (decision 0101).
    #[test]
    fn a_chat_change_goes_to_the_chat_streams_and_a_forget_to_both() {
        let events = EventBus::new();
        let (changes, mut watcher) = broadcast::channel(8);
        let (chats, mut chat) = broadcast::channel(8);
        let listener = agent_listener(events, changes, chats);
        let changed = Change::Chat(Box::new(ChatChanged {
            session: "session-1".to_owned(),
            batch: ChatBatch {
                cursor: ChatCursor {
                    epoch: "e".to_owned(),
                    seq: 1,
                },
                updates: Vec::new(),
                activity: None,
            },
        }));

        listener(changed.clone());
        assert_eq!(chat.try_recv().ok(), Some(changed));
        assert!(
            watcher.try_recv().is_err(),
            "no watcher receives a chat change"
        );

        listener(Change::Forgotten("session-1".to_owned()));
        assert_eq!(
            chat.try_recv().ok(),
            Some(Change::Forgotten("session-1".to_owned()))
        );
        assert_eq!(
            watcher.try_recv().ok(),
            Some(Change::Forgotten("session-1".to_owned()))
        );
    }
}

#[cfg(test)]
mod discovery_option_tests {
    use std::ffi::OsString;
    use std::path::Path;

    use super::{DiscoveryOptions, SKILL_HOME_ENV};

    fn options(vars: &[(&str, &str)]) -> DiscoveryOptions {
        DiscoveryOptions::from_lookup(|name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| OsString::from(value))
        })
    }

    #[test]
    fn a_relative_override_is_ignored() {
        let options = options(&[
            ("CLAUDE_CONFIG_DIR", "relative/config"),
            (SKILL_HOME_ENV, "relative/home"),
        ]);

        assert_eq!(options.env.claude_config_dir, None);
        assert_ne!(options.home.as_deref(), Some(Path::new("relative/home")));
        assert!(options.home.as_deref().is_none_or(Path::is_absolute));
    }

    #[test]
    fn an_absolute_override_is_used() {
        let config = std::env::temp_dir().join("config");
        let home = std::env::temp_dir().join("home");

        let options = options(&[
            ("CLAUDE_CONFIG_DIR", config.to_str().expect("utf-8")),
            (SKILL_HOME_ENV, home.to_str().expect("utf-8")),
        ]);

        assert_eq!(options.env.claude_config_dir, Some(config));
        assert_eq!(options.home, Some(home));
    }
}
