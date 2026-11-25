//! What the service reported, and the reducer that folds it into one state (decision 0028).
//! The store changes only through these updates, never optimistically.

use std::collections::BTreeMap;
use std::time::Instant;

use massorca_client::{ClientError, HostRecord};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    AccountSummary, AgentKind, AgentQueueList, AgentSession, QueuedPrompt,
};
use massorca_protocol::methods::config::{ConfigValue, DEFAULT_OWNER};
use massorca_protocol::methods::devices::{DeviceInfo, NetworkStatus};
use massorca_protocol::methods::extensions::ExtensionItem;
use massorca_protocol::methods::git::log::GitLog;
use massorca_protocol::methods::git::status::GitStatus;
use massorca_protocol::methods::projects::Project;
use massorca_protocol::methods::secrets::{SecretEntry, SecretRequest, SecretStatus};
use massorca_protocol::methods::terminals::Terminal;
use massorca_protocol::methods::workspaces::Workspace;

use super::agent_chat::KindChat;
use super::connection::ConnectionState;
use super::extensions::{ExtensionsRead, HostMarketplaceRead};
use super::host_sessions::{HostSessions, HostSessionsRead};
use super::refusal::Refusal;
use super::skills::SkillsRead;
use super::teams::{TeamIds, TeamRecord, TeamsRead};

/// One change the bridge sends to the UI thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreUpdate {
    Connection(ConnectionState),
    /// Every project and workspace, after a connect or an overflow. Replaces what was held.
    Resync {
        projects: Vec<Project>,
        workspaces: Vec<Workspace>,
    },
    ProjectChanged(Project),
    ProjectGone(String),
    WorkspaceChanged(Workspace),
    WorkspaceGone(String),
    /// Every terminal the service lists, after a connect. Replaces what was held.
    TerminalsResync(Vec<Terminal>),
    /// A terminal the service created, killed or reported; keyed by its id.
    TerminalChanged(Terminal),
    TerminalGone(String),
    /// Every agent session the service lists, after a connect. Replaces what was held.
    AgentsResync(Vec<AgentSession>),
    /// An agent session the service started or changed; keyed by its id.
    AgentChanged(AgentSession),
    /// An agent session the service forgot.
    AgentGone(String),
    /// The agent kind chosen when this app created a workspace; kept until the workspace has a session (A97).
    WorkspaceKindChosen {
        workspace: String,
        kind: String,
    },
    /// Every agent kind the service declares, after a connect. Replaces what was held.
    AgentKindsResync(Vec<AgentKind>),
    /// Every session's prompt queue after a connect, or `None` when the service offers no queue. Replaces what was held.
    AgentQueuesResync(Option<Vec<AgentQueueList>>),
    /// One session's prompt queue as the service lists it now.
    AgentQueueChanged(AgentQueueList),
    /// Whether the service offers seen state (`agent-seen/1`), read on each connect.
    AgentSeenOffered(bool),
    /// Whether the service offers starting a session with no workspace (`agent-no-project/1`), read on each connect.
    AgentNoProjectOffered(bool),
    /// Whether the service offers a model and an effort on `agent.start` (`agent-options/1`), read on each connect.
    AgentOptionsOffered(bool),
    /// Whether the service offers chat sessions (`agent-chat/1`), read on each connect.
    AgentChatOffered(bool),
    /// Whether the service offers a fast speed on `agent.start` (`agent-fast/1`), read on each connect.
    AgentFastOffered(bool),
    /// Whether the service makes a taken workspace name unique itself (`workspace-unique-name/1`), read on each connect.
    WorkspaceUniqueNameOffered(bool),
    /// Whether the service offers `agent-archive/1` on `agent.forget`, read on each connect.
    AgentArchiveOffered(bool),
    /// Each agent kind's chat support as `agent.kinds` lists it, by kind id, after a connect. Replaces what was held.
    AgentKindChatResync(BTreeMap<String, KindChat>),
    /// What the git watch and the git calls learned about one workspace.
    Git(GitUpdate),
    /// What a Bot Teams read answered: every team with its Board, or why none could be listed.
    Teams(TeamsRead),
    /// What the extension reads learned: the installed packages and the catalogue, each replaced as a whole.
    Extensions(ExtensionsRead),
    /// What a paired host's Marketplace reads answered; the newest answer per host is kept.
    HostMarketplace(HostMarketplaceRead),
    /// What a discovery of skills answered, for This computer or a paired host; the newest answer per host is kept.
    Skills(SkillsRead),
    /// Every setting the service declares with its value, after a connect. Replaces what was held.
    ConfigResync(Vec<ConfigValue>),
    /// One setting the service changed or stored, read again by its owner and key.
    ConfigChanged(ConfigValue),
    /// The hosts this computer paired with, after a refresh or a change. Replaces what was held.
    HostsResync(Vec<HostRecord>),
    /// What one check of a paired host learned about its agent sessions; `None` answers mean it was not reached.
    HostSessions(HostSessionsRead),
    /// What a probe or a call learned about one paired host's connection.
    HostLinkChanged {
        name: String,
        link: HostLink,
    },
    /// The devices paired with this computer's service, after a list or a revoke.
    Devices(Result<Vec<DeviceInfo>, Refusal>),
    /// The listeners and counts of this computer's service, after a read.
    Network(Result<NetworkStatus, Refusal>),
    /// What the vault reads learned; each part replaces what it names.
    Secrets(SecretsUpdate),
    /// One kind's accounts, or why they could not be listed. Replaces what was held for that kind.
    Accounts {
        kind: String,
        result: Result<Vec<AccountSummary>, Refusal>,
    },
}

/// How a paired host's connection stands, as the last probe or call found it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HostLink {
    /// Not checked yet, or a check is running.
    Checking,
    Connected {
        server: String,
        version: String,
    },
    /// Not reachable; the sentence says why.
    Unreachable(String),
    /// The host no longer accepts this computer's device: pair again.
    SignInNeeded(String),
    /// The host speaks another protocol version than this app.
    UpdateNeeded(String),
    /// Any other failure, with its sentence.
    Failed(String),
}

impl HostLink {
    /// The state a failed connection to a host leaves it in, with the service's own sentence for it.
    pub fn from_error(error: &ClientError) -> Self {
        let refusal = Refusal::from(error);
        let message = refusal.sentence();
        match refusal.code() {
            _ if matches!(refusal, Refusal::Unavailable { .. }) => Self::Unreachable(message),
            Some(code) if code.starts_with("auth.") => Self::SignInNeeded(message),
            Some(codes::PROTOCOL_UNSUPPORTED_VERSION) => Self::UpdateNeeded(message),
            _ => Self::Failed(message),
        }
    }
}

/// One paired host: its record, how its connection stands, and when it last answered in this run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostEntry {
    pub record: HostRecord,
    pub link: HostLink,
    pub last_connected: Option<Instant>,
}

/// One answer of the vault reads. Names and times only: no answer carries a secret's value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecretsUpdate {
    /// The key's state and the count of owned secrets, or why they could not be read.
    Status(Result<SecretStatus, Refusal>),
    /// Every name in every scope, owned and linked, or why they could not be read.
    List(Result<Vec<SecretEntry>, Refusal>),
    /// The requests still pending, or why they could not be read. Names, reasons and times only.
    Requests(Result<Vec<SecretRequest>, Refusal>),
}

/// One answer about a workspace's git state (decisions 0010 and 0012). Each update replaces the part
/// it names, so the latest answer wins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitUpdate {
    /// The status, or why it could not be read (`git.not_a_repository` for a folder workspace).
    Status {
        workspace: String,
        result: Result<GitStatus, Refusal>,
    },
    /// The first page of history.
    Log {
        workspace: String,
        result: Result<GitLog, Refusal>,
    },
    /// Whether working-tree edits are observed; `Some` carries why they are not.
    Watch {
        workspace: String,
        degraded: Option<String>,
    },
}

impl GitUpdate {
    pub fn workspace(&self) -> &str {
        match self {
            Self::Status { workspace, .. }
            | Self::Log { workspace, .. }
            | Self::Watch { workspace, .. } => workspace,
        }
    }
}

/// The git state the store holds for one workspace. A part is `None` until its first answer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceGit {
    pub status: Option<Result<GitStatus, Refusal>>,
    pub log: Option<Result<GitLog, Refusal>>,
    /// Why working-tree edits are not observed, while they are not.
    pub degraded: Option<String>,
}

/// The vault as the service last reported it. A part is `None` until its first answer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SecretsState {
    pub status: Option<Result<SecretStatus, Refusal>>,
    pub list: Option<Result<Vec<SecretEntry>, Refusal>>,
    pub requests: Option<Result<Vec<SecretRequest>, Refusal>>,
}

/// The projects and workspaces the service reported, and how the link to it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreState {
    pub connection: ConnectionState,
    pub projects: Vec<Project>,
    pub workspaces: Vec<Workspace>,
    /// Terminals of every workspace, oldest first (decision 0033).
    pub terminals: Vec<Terminal>,
    /// Agent sessions of every workspace, oldest first, children included (decision 0055).
    pub agents: Vec<AgentSession>,
    /// Every agent kind the service declares, built-ins first, as it lists them (`agent.kinds`).
    pub agent_kinds: Vec<AgentKind>,
    /// True once the first full list arrived, so an empty tree can tell "none" from "not yet".
    pub loaded: bool,
    /// True once the agent session list arrived, so a debug page for sessions waits for it.
    pub agents_answered: bool,
    /// When the link last stopped being connected, while it is not.
    pub down_since: Option<Instant>,
    /// The git state of each workspace the store holds, by workspace id.
    pub git: BTreeMap<String, WorkspaceGit>,
    /// Every team with its Board, once a read answered; `None` while the service cannot list them.
    pub teams: Option<Vec<TeamRecord>>,
    /// The number of the newest Bot Teams read applied.
    pub team_read: u64,
    /// The numbers the pages give the ids of the teams, their cards and their bots.
    pub team_ids: TeamIds,
    /// The hosts this computer paired with, by name.
    pub hosts: Vec<HostEntry>,
    /// The devices paired with this computer's service, once listed.
    pub devices: Option<Result<Vec<DeviceInfo>, Refusal>>,
    /// This computer's listeners and counts, once read.
    pub network: Option<Result<NetworkStatus, Refusal>>,
    /// The secret vault: its status and every name it lists.
    pub secrets: SecretsState,
    /// The number of the newest extension read applied; an older late answer changes nothing.
    pub extension_read: u64,
    /// The installed packages and the catalogue, once read; `None` while the service cannot be asked.
    pub extensions: Option<ExtensionsRead>,
    /// The newest Marketplace answer of each paired host, by host name.
    pub host_listings: BTreeMap<String, HostMarketplaceRead>,
    /// The agent sessions each paired host last reported, by host name (STAT-16).
    pub host_sessions: BTreeMap<String, HostSessions>,
    /// The newest skills answer of This computer (None) and of each paired host, by host name.
    pub skills: BTreeMap<Option<String>, SkillsRead>,
    /// Each kind's accounts as last listed, by kind id (decision 0059). Ids, labels and states only.
    pub accounts: BTreeMap<String, Result<Vec<AccountSummary>, Refusal>>,
    /// Each session's prompt queue, head first, by session id (decision 0084); a session with none is absent. `None`
    /// while the service offers no queue.
    pub agent_queues: Option<BTreeMap<String, Vec<QueuedPrompt>>>,
    /// Whether the service offers seen state (`agent-seen/1`); without it no session is unread.
    pub agent_seen: bool,
    /// Whether the service offers a session with no workspace (`agent-no-project/1`); without it "No project" is off.
    pub agent_no_project: bool,
    /// Whether the service offers `model` and `effort` on `agent.start` (`agent-options/1`); without it neither is sent.
    pub agent_options: bool,
    /// Whether the service offers chat sessions (`agent-chat/1`); without it every start page opens on Terminal.
    pub agent_chat: bool,
    /// Whether the service offers a fast speed (`agent-fast/1`); without it Fast is off, with its reason.
    pub agent_fast: bool,
    /// Whether the service makes a taken workspace name unique itself (`workspace-unique-name/1`); without it a derived
    /// name is retried with a suffix on the client.
    pub workspace_unique_name: bool,
    /// Whether the service offers `agent-archive/1` on `agent.forget`; without it no session can be deleted.
    pub agent_archive: bool,
    /// Each agent kind's chat support by kind id (decision 0101, D10); a kind absent from it cannot chat.
    pub agent_kind_chat: BTreeMap<String, KindChat>,
    /// The settings the service declares with their values, as the last read or change left them.
    pub config: Vec<ConfigValue>,
    /// Workspaces seen created with no session yet, by id, with the kind chosen for each (`None` when the service made
    /// it). In memory only; a session ends the entry, so no placeholder comes back once its session is archived (A97).
    pub awaiting_session: BTreeMap<String, Option<String>>,
}

impl Default for StoreState {
    fn default() -> Self {
        Self {
            connection: ConnectionState::Connecting,
            projects: Vec::new(),
            workspaces: Vec::new(),
            terminals: Vec::new(),
            agents: Vec::new(),
            agent_kinds: Vec::new(),
            loaded: false,
            agents_answered: false,
            down_since: None,
            git: BTreeMap::new(),
            teams: None,
            team_read: 0,
            team_ids: TeamIds::default(),
            hosts: Vec::new(),
            devices: None,
            network: None,
            secrets: SecretsState::default(),
            extension_read: 0,
            extensions: None,
            host_listings: BTreeMap::new(),
            host_sessions: BTreeMap::new(),
            skills: BTreeMap::new(),
            accounts: BTreeMap::new(),
            agent_queues: None,
            agent_seen: false,
            agent_no_project: false,
            agent_options: false,
            agent_chat: false,
            agent_fast: false,
            workspace_unique_name: false,
            agent_archive: false,
            agent_kind_chat: BTreeMap::new(),
            config: Vec::new(),
            awaiting_session: BTreeMap::new(),
        }
    }
}

impl StoreState {
    /// Folds one update in. `now` stamps the moment a connected link goes down.
    pub fn apply(&mut self, update: StoreUpdate, now: Instant) {
        match update {
            StoreUpdate::Connection(state) => {
                if state.is_connected() {
                    self.down_since = None;
                } else if self.connection.is_connected() || self.down_since.is_none() {
                    self.down_since = Some(now);
                }
                if !state.is_connected() {
                    self.teams = None;
                    self.extensions = None;
                }
                self.connection = state;
            }
            StoreUpdate::Resync {
                projects,
                workspaces,
            } => {
                self.projects = projects;
                self.workspaces = workspaces;
                self.projects.sort_by(project_order);
                self.workspaces.sort_by(workspace_order);
                self.git
                    .retain(|id, _| self.workspaces.iter().any(|w| &w.id == id));
                self.awaiting_session
                    .retain(|id, _| self.workspaces.iter().any(|w| &w.id == id));
                self.loaded = true;
            }
            StoreUpdate::ProjectChanged(project) => {
                replace_by(&mut self.projects, project, |p| p.id.clone());
                self.projects.sort_by(project_order);
            }
            StoreUpdate::ProjectGone(id) => self.projects.retain(|p| p.id != id),
            StoreUpdate::WorkspaceChanged(workspace) => {
                // Only a workspace that appears after the first list was created while this app runs (A97).
                if self.loaded
                    && self.workspace(&workspace.id).is_none()
                    && !self.workspace_has_session(&workspace.id)
                {
                    self.awaiting_session
                        .entry(workspace.id.clone())
                        .or_insert(None);
                }
                replace_by(&mut self.workspaces, workspace, |w| w.id.clone());
                self.workspaces.sort_by(workspace_order);
            }
            StoreUpdate::WorkspaceGone(id) => {
                self.workspaces.retain(|w| w.id != id);
                self.git.remove(&id);
                self.awaiting_session.remove(&id);
            }
            StoreUpdate::TerminalsResync(mut terminals) => {
                terminals.sort_by(terminal_order);
                self.terminals = terminals;
            }
            StoreUpdate::TerminalChanged(terminal) => {
                replace_by(&mut self.terminals, terminal, |t| t.id.clone());
                self.terminals.sort_by(terminal_order);
            }
            StoreUpdate::TerminalGone(id) => self.terminals.retain(|t| t.id != id),
            StoreUpdate::AgentsResync(mut sessions) => {
                sessions.sort_by(agent_order);
                self.agents = sessions;
                self.agents_answered = true;
                self.awaiting_session
                    .retain(|id, _| !self.agents.iter().any(|s| &s.workspace_id == id));
            }
            StoreUpdate::AgentChanged(session) => {
                self.awaiting_session.remove(&session.workspace_id);
                replace_by(&mut self.agents, session, |s| s.id.clone());
                self.agents.sort_by(agent_order);
            }
            StoreUpdate::WorkspaceKindChosen { workspace, kind } => {
                if !self.workspace_has_session(&workspace) {
                    self.awaiting_session.insert(workspace, Some(kind));
                }
            }
            StoreUpdate::AgentGone(id) => {
                self.agents.retain(|s| s.id != id);
                if let Some(queues) = &mut self.agent_queues {
                    queues.remove(&id);
                }
            }
            StoreUpdate::AgentKindsResync(kinds) => self.agent_kinds = kinds,
            StoreUpdate::AgentQueuesResync(queues) => {
                self.agent_queues = queues.map(|lists| {
                    lists
                        .into_iter()
                        .filter(|list| !list.items.is_empty())
                        .map(|list| (list.session, list.items))
                        .collect()
                });
            }
            StoreUpdate::AgentSeenOffered(offered) => self.agent_seen = offered,
            StoreUpdate::AgentNoProjectOffered(offered) => self.agent_no_project = offered,
            StoreUpdate::AgentOptionsOffered(offered) => self.agent_options = offered,
            StoreUpdate::AgentChatOffered(offered) => self.agent_chat = offered,
            StoreUpdate::AgentFastOffered(offered) => self.agent_fast = offered,
            StoreUpdate::WorkspaceUniqueNameOffered(offered) => {
                self.workspace_unique_name = offered;
            }
            StoreUpdate::AgentArchiveOffered(offered) => self.agent_archive = offered,
            StoreUpdate::AgentKindChatResync(chats) => self.agent_kind_chat = chats,
            StoreUpdate::AgentQueueChanged(list) => {
                if let Some(queues) = &mut self.agent_queues {
                    if list.items.is_empty() {
                        queues.remove(&list.session);
                    } else {
                        queues.insert(list.session, list.items);
                    }
                }
            }
            StoreUpdate::Git(update) => self.apply_git(update),
            StoreUpdate::Teams(read) => self.apply_teams(read),
            StoreUpdate::Extensions(read) => self.apply_extensions(read),
            StoreUpdate::HostMarketplace(read) => self.apply_host_marketplace(read),
            StoreUpdate::HostSessions(read) => self.apply_host_sessions(read),
            StoreUpdate::Skills(read) => self.apply_skills(read),
            StoreUpdate::ConfigResync(values) => self.config = values,
            StoreUpdate::ConfigChanged(value) => {
                replace_by(&mut self.config, value, |held| {
                    format!("{}/{}", held.owner, held.key)
                });
            }
            StoreUpdate::HostsResync(records) => self.resync_hosts(records),
            StoreUpdate::HostLinkChanged { name, link } => {
                if let Some(entry) = self
                    .hosts
                    .iter_mut()
                    .find(|entry| entry.record.name == name)
                {
                    if matches!(link, HostLink::Connected { .. }) {
                        entry.last_connected = Some(now);
                    }
                    entry.link = link;
                }
            }
            StoreUpdate::Devices(result) => self.devices = Some(result),
            StoreUpdate::Network(result) => self.network = Some(result),
            StoreUpdate::Secrets(SecretsUpdate::Status(result)) => {
                self.secrets.status = Some(result);
            }
            StoreUpdate::Secrets(SecretsUpdate::List(result)) => {
                self.secrets.list = Some(result);
            }
            StoreUpdate::Secrets(SecretsUpdate::Requests(result)) => {
                self.secrets.requests = Some(result);
            }
            StoreUpdate::Accounts { kind, result } => {
                self.accounts.insert(kind, result);
            }
        }
    }

    /// The value the service reports for a setting of the core owner, once a read has listed it.
    pub fn config_value(&self, key: &str) -> Option<&ConfigValue> {
        self.config
            .iter()
            .find(|value| value.owner == DEFAULT_OWNER && value.key == key)
    }

    /// Whether the held agent sessions include one in this workspace.
    pub fn workspace_has_session(&self, workspace_id: &str) -> bool {
        self.agents
            .iter()
            .any(|session| session.workspace_id == workspace_id)
    }

    /// Keeps the newest read: an older answer that arrives late changes nothing.
    fn apply_teams(&mut self, read: TeamsRead) {
        if read.number <= self.team_read {
            return;
        }
        self.team_read = read.number;
        self.teams = read.teams.ok();
        for record in self.teams.iter().flatten() {
            self.team_ids.register(record);
        }
    }

    /// The installed packages and the catalogue as the last read listed them; empty while a list is refused or unread.
    pub fn extension_lists(&self) -> (&[ExtensionItem], &[ExtensionItem]) {
        let Some(read) = &self.extensions else {
            return (&[], &[]);
        };
        let installed = read.installed.as_ref().map_or(&[][..], Vec::as_slice);
        let catalogue = read.catalogue.as_ref().map_or(&[][..], Vec::as_slice);
        (installed, catalogue)
    }

    /// Keeps the newest extension read, as `apply_teams` does for Bot Teams.
    fn apply_extensions(&mut self, read: ExtensionsRead) {
        if read.number <= self.extension_read {
            return;
        }
        self.extension_read = read.number;
        self.extensions = Some(read);
    }

    /// Keeps the newest answer of each host, so an answer to an older request never replaces a newer one.
    fn apply_host_marketplace(&mut self, read: HostMarketplaceRead) {
        if self
            .host_listings
            .get(&read.host)
            .is_some_and(|held| held.request > read.request)
        {
            return;
        }
        self.host_listings.insert(read.host.clone(), read);
    }

    /// Keeps the newest check of each host's sessions. A check that did not reach the host keeps the sessions it
    /// last saw, marked unreached, so they show as unverifiable rather than gone.
    fn apply_host_sessions(&mut self, read: HostSessionsRead) {
        let held = self.host_sessions.entry(read.host).or_default();
        if held.request > read.request {
            return;
        }
        held.request = read.request;
        match read.answer {
            Some(agents) => {
                held.reached = true;
                held.sessions = agents.sessions;
                held.workspaces = agents.workspaces;
            }
            None => held.reached = false,
        }
    }

    /// Keeps the newest skills answer of each host, so an answer to an older request never replaces a newer one.
    fn apply_skills(&mut self, read: SkillsRead) {
        if self
            .skills
            .get(&read.host)
            .is_some_and(|held| held.request > read.request)
        {
            return;
        }
        self.skills.insert(read.host.clone(), read);
    }

    /// Replaces the host records, keeping what each host's last check found. A host no longer listed is dropped.
    fn resync_hosts(&mut self, records: Vec<HostRecord>) {
        let mut hosts: Vec<HostEntry> = records
            .into_iter()
            .map(|record| {
                let previous = self
                    .hosts
                    .iter()
                    .find(|entry| entry.record.name == record.name);
                HostEntry {
                    link: previous.map_or(HostLink::Checking, |entry| entry.link.clone()),
                    last_connected: previous.and_then(|entry| entry.last_connected),
                    record,
                }
            })
            .collect();
        hosts.sort_by(|a, b| a.record.name.cmp(&b.record.name));
        self.host_sessions
            .retain(|name, _| hosts.iter().any(|entry| &entry.record.name == name));
        self.hosts = hosts;
    }

    /// An update for a workspace the store does not hold (removed, or not listed yet) changes nothing.
    fn apply_git(&mut self, update: GitUpdate) {
        if self.workspace(update.workspace()).is_none() {
            return;
        }
        let git = self.git.entry(update.workspace().to_owned()).or_default();
        match update {
            GitUpdate::Status { result, .. } => git.status = Some(result),
            GitUpdate::Log { result, .. } => git.log = Some(result),
            GitUpdate::Watch { degraded, .. } => git.degraded = degraded,
        }
    }

    /// The git state of workspace `id`, once the store has heard of it.
    pub fn git(&self, id: &str) -> Option<&WorkspaceGit> {
        self.git.get(id)
    }

    pub fn workspace(&self, id: &str) -> Option<&Workspace> {
        self.workspaces.iter().find(|w| w.id == id)
    }

    pub fn project(&self, id: &str) -> Option<&Project> {
        self.projects.iter().find(|p| p.id == id)
    }
}

fn replace_by<T>(items: &mut Vec<T>, item: T, id: impl Fn(&T) -> String) {
    let wanted = id(&item);
    match items.iter().position(|existing| id(existing) == wanted) {
        Some(index) => items[index] = item,
        None => items.push(item),
    }
}

/// Projects by name, as `project.list` orders them.
fn project_order(a: &Project, b: &Project) -> std::cmp::Ordering {
    a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id))
}

/// Workspaces by creation time, then name, so a new one lands last and rows do not jump.
fn terminal_order(a: &Terminal, b: &Terminal) -> std::cmp::Ordering {
    a.created_at_ms
        .cmp(&b.created_at_ms)
        .then_with(|| a.id.cmp(&b.id))
}

/// Agent sessions by creation time, then id: a new session lands last and a child after its earlier siblings.
fn agent_order(a: &AgentSession, b: &AgentSession) -> std::cmp::Ordering {
    a.created_at_ms
        .cmp(&b.created_at_ms)
        .then_with(|| a.id.cmp(&b.id))
}

fn workspace_order(a: &Workspace, b: &Workspace) -> std::cmp::Ordering {
    a.created_at_ms
        .cmp(&b.created_at_ms)
        .then_with(|| a.name.cmp(&b.name))
        .then_with(|| a.id.cmp(&b.id))
}

#[cfg(test)]
pub(crate) mod fixtures {
    use massorca_protocol::methods::agents::{
        AgentKind, Execution, KindCredentials, KindPackage, PackageState, QUEUE_REASON_BUSY,
        QueueState, StatusMechanism,
    };
    use massorca_protocol::methods::projects::ProjectKind;
    use massorca_protocol::methods::teams::{Board, Team, TeamAccess, TeamCharacter};
    use massorca_protocol::methods::terminals::TerminalState;
    use massorca_protocol::methods::workspaces::{WorkspaceKind, WorkspaceState};

    use super::*;

    /// A live session of the `claude` kind with no report yet, in workspace `workspace_id`.
    pub fn agent(id: &str, workspace_id: &str, created: i64) -> AgentSession {
        AgentSession {
            id: id.into(),
            kind: "claude".into(),
            workspace_id: workspace_id.into(),
            title: None,
            read_only: false,
            parent_id: None,
            brought_back_at_ms: None,
            agent_ref: None,
            terminal_id: None,
            run: Default::default(),
            created_at_ms: created,
            execution: Execution {
                state: TerminalState::Live,
                exit: None,
            },
            activity: None,
            result: None,
            holds_secrets: false,
            secrets_withheld: None,
            permission_mode: None,
            mode_confirmed: false,
            mode_source: None,
            pending_permission_mode: None,
            retained: false,
            released: false,
            resume: None,
            delegation: None,
            notes: Vec::new(),
        }
    }

    /// An agent kind the service declares, its package in `state` and its program found, so detected.
    pub fn agent_kind(id: &str, name: &str, state: PackageState) -> AgentKind {
        AgentKind {
            id: id.into(),
            name: name.into(),
            icon: id.into(),
            owner: "core".into(),
            installed: true,
            detected: true,
            program_path: None,
            package: KindPackage {
                state,
                id: None,
                version: None,
            },
            resumable: true,
            status_mechanism: StatusMechanism::None,
            credentials: KindCredentials {
                supported: false,
                imported: false,
            },
            problems: Vec::new(),
            permission_modes: Vec::new(),
            default_permission_mode: None,
            chat: None,
            model: None,
            effort: None,
        }
    }

    pub fn project(id: &str, name: &str) -> Project {
        Project {
            icon: None,
            id: id.into(),
            kind: ProjectKind::Git,
            path: format!("/repos/{name}"),
            name: name.into(),
            workspaces_root: format!("/repos/{name}-worktrees"),
            added_at_ms: 1,
            base_ref: None,
        }
    }

    pub fn workspace(id: &str, project_id: &str, name: &str, created: i64) -> Workspace {
        Workspace {
            id: id.into(),
            project_id: project_id.into(),
            kind: WorkspaceKind::Worktree,
            name: name.into(),
            path: format!("/repos/{name}"),
            branch: Some(format!("feat/{name}")),
            base_ref: None,
            created_branch: true,
            state: WorkspaceState::Ready,
            missing: false,
            created_at_ms: created,
            locked: None,
            prunable: None,
            base_refresh: None,
            comment: None,
            setup: None,
        }
    }

    pub fn team(id: &str, name: &str) -> TeamRecord {
        TeamRecord {
            team: Team {
                id: id.into(),
                name: name.into(),
                purpose: String::new(),
                character: TeamCharacter::Triangle,
                access: TeamAccess::Full,
                workspaces: false,
                created_at_ms: 1,
            },
            board: Board {
                team_id: id.into(),
                project_id: None,
                columns: Vec::new(),
            },
            bots: Vec::new(),
            projects: Vec::new(),
        }
    }

    /// Prompt `id` of `session`'s queue at `position`, waiting behind a busy agent.
    pub fn queued(id: &str, session: &str, position: u64) -> QueuedPrompt {
        QueuedPrompt {
            id: id.into(),
            session: session.into(),
            position,
            text: format!("prompt {id}"),
            state: QueueState::Waiting,
            reason: Some(QUEUE_REASON_BUSY.into()),
            created_at_ms: 1,
            updated_at_ms: 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::agents::{
        Activity, ActivitySource, ActivityState, PackageState,
    };

    use super::fixtures::{agent, agent_kind, project, queued, team, workspace};
    use super::*;
    use crate::service::extensions::HostListing;

    #[test]
    fn the_store_holds_whether_the_service_offers_agent_options() {
        let mut state = StoreState::default();
        assert!(
            !state.agent_options,
            "no read yet: neither model nor effort is sent"
        );
        state.apply(StoreUpdate::AgentOptionsOffered(true), Instant::now());
        assert!(state.agent_options);
    }

    #[test]
    fn the_store_holds_whether_the_service_offers_agent_chat() {
        let mut state = StoreState::default();
        assert!(
            !state.agent_chat,
            "no read yet: the start page opens on Terminal"
        );
        state.apply(StoreUpdate::AgentChatOffered(true), Instant::now());
        assert!(state.agent_chat);
    }

    #[test]
    fn the_store_holds_whether_the_service_makes_workspace_names_unique() {
        let mut state = StoreState::default();
        assert!(
            !state.workspace_unique_name,
            "no read yet: the client retries"
        );
        state.apply(
            StoreUpdate::WorkspaceUniqueNameOffered(true),
            Instant::now(),
        );
        assert!(state.workspace_unique_name);
    }

    #[test]
    fn the_store_holds_whether_the_service_offers_agent_fast() {
        let mut state = StoreState::default();
        assert!(!state.agent_fast, "no read yet: Fast stays off");
        state.apply(StoreUpdate::AgentFastOffered(true), Instant::now());
        assert!(state.agent_fast);
    }

    fn resynced() -> StoreState {
        let mut state = StoreState::default();
        state.apply(
            StoreUpdate::Resync {
                projects: vec![project("p2", "zeta"), project("p1", "alpha")],
                workspaces: vec![
                    workspace("w2", "p1", "later", 20),
                    workspace("w1", "p1", "first", 10),
                ],
            },
            Instant::now(),
        );
        state
    }

    #[test]
    fn a_resync_replaces_everything_and_orders_it() {
        let state = resynced();
        let names: Vec<&str> = state.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "zeta"]);
        let ids: Vec<&str> = state.workspaces.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, vec!["w1", "w2"]);
        assert!(state.loaded);
    }

    fn setting(key: &str, value: &str) -> ConfigValue {
        ConfigValue {
            owner: DEFAULT_OWNER.into(),
            key: key.into(),
            value: value.into(),
            ..Default::default()
        }
    }

    fn setting_text(state: &StoreState, key: &str) -> Option<String> {
        state
            .config_value(key)
            .and_then(|value| value.value.as_str().map(str::to_owned))
    }

    #[test]
    fn a_config_resync_replaces_the_values_the_store_held() {
        let mut state = StoreState::default();
        state.apply(
            StoreUpdate::ConfigResync(vec![setting("appearance.theme", "dark")]),
            Instant::now(),
        );
        state.apply(
            StoreUpdate::ConfigResync(vec![setting("appearance.theme", "light")]),
            Instant::now(),
        );
        assert_eq!(state.config.len(), 1);
        assert_eq!(
            setting_text(&state, "appearance.theme").as_deref(),
            Some("light")
        );
    }

    #[test]
    fn a_changed_setting_replaces_its_own_value_and_keeps_the_others() {
        let mut state = StoreState::default();
        state.apply(
            StoreUpdate::ConfigResync(vec![
                setting("appearance.theme", "system"),
                setting("workspaces.default_root", "C:/old"),
            ]),
            Instant::now(),
        );
        state.apply(
            StoreUpdate::ConfigChanged(setting("appearance.theme", "light")),
            Instant::now(),
        );
        assert_eq!(state.config.len(), 2);
        assert_eq!(
            setting_text(&state, "appearance.theme").as_deref(),
            Some("light")
        );
        assert_eq!(
            setting_text(&state, "workspaces.default_root").as_deref(),
            Some("C:/old")
        );
    }

    #[test]
    fn a_changed_setting_the_store_has_not_listed_is_added() {
        let mut state = StoreState::default();
        state.apply(
            StoreUpdate::ConfigChanged(setting("appearance.theme", "dark")),
            Instant::now(),
        );
        assert_eq!(
            setting_text(&state, "appearance.theme").as_deref(),
            Some("dark")
        );
    }

    #[test]
    fn a_setting_of_another_owner_is_not_the_core_setting() {
        let mut state = StoreState::default();
        let mut other = setting("appearance.theme", "light");
        other.owner = "plugin".into();
        state.apply(StoreUpdate::ConfigChanged(other), Instant::now());
        assert!(state.config_value("appearance.theme").is_none());
    }

    #[test]
    fn a_second_resync_drops_what_the_service_no_longer_has() {
        let mut state = resynced();
        state.apply(
            StoreUpdate::Resync {
                projects: vec![project("p1", "alpha")],
                workspaces: Vec::new(),
            },
            Instant::now(),
        );
        assert_eq!(state.projects.len(), 1);
        assert!(state.workspaces.is_empty());
    }

    #[test]
    fn a_changed_workspace_replaces_the_row_with_the_same_id() {
        let mut state = resynced();
        let mut renamed = workspace("w1", "p1", "first", 10);
        renamed.missing = true;
        state.apply(StoreUpdate::WorkspaceChanged(renamed), Instant::now());
        assert_eq!(state.workspaces.len(), 2);
        assert!(state.workspace("w1").is_some_and(|w| w.missing));
    }

    #[test]
    fn a_new_workspace_lands_after_older_ones() {
        let mut state = resynced();
        state.apply(
            StoreUpdate::WorkspaceChanged(workspace("w3", "p1", "newest", 30)),
            Instant::now(),
        );
        assert_eq!(state.workspaces.last().map(|w| w.id.as_str()), Some("w3"));
    }

    #[test]
    fn a_gone_workspace_or_project_is_dropped_and_an_unknown_id_is_ignored() {
        let mut state = resynced();
        state.apply(StoreUpdate::WorkspaceGone("w1".into()), Instant::now());
        state.apply(StoreUpdate::WorkspaceGone("nope".into()), Instant::now());
        state.apply(StoreUpdate::ProjectGone("p2".into()), Instant::now());
        assert_eq!(state.workspaces.len(), 1);
        assert_eq!(state.projects.len(), 1);
    }

    #[test]
    fn an_added_project_lands_in_name_order() {
        let mut state = resynced();
        state.apply(
            StoreUpdate::ProjectChanged(project("p3", "middle")),
            Instant::now(),
        );
        let names: Vec<&str> = state.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "middle", "zeta"]);
    }

    #[test]
    fn agent_sessions_resync_replaces_them_oldest_first() {
        let now = Instant::now();
        let mut state = StoreState::default();
        state.apply(
            StoreUpdate::AgentsResync(vec![agent("s2", "w1", 20), agent("s1", "w1", 10)]),
            now,
        );
        let ids: Vec<&str> = state.agents.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["s1", "s2"]);
        state.apply(StoreUpdate::AgentsResync(vec![agent("s3", "w1", 30)]), now);
        assert_eq!(
            state.agents.len(),
            1,
            "a resync drops what the service no longer lists"
        );
    }

    #[test]
    fn agent_kinds_resync_replaces_what_the_service_declared() {
        let now = Instant::now();
        let mut state = StoreState::default();
        assert!(state.agent_kinds.is_empty(), "no kinds before a connect");
        state.apply(
            StoreUpdate::AgentKindsResync(vec![agent_kind(
                "claude",
                "Claude",
                PackageState::Enabled,
            )]),
            now,
        );
        state.apply(
            StoreUpdate::AgentKindsResync(vec![agent_kind(
                "codex",
                "Codex",
                PackageState::Available,
            )]),
            now,
        );
        let ids: Vec<&str> = state.agent_kinds.iter().map(|k| k.id.as_str()).collect();
        assert_eq!(
            ids,
            ["codex"],
            "a resync drops the kinds it no longer lists"
        );
    }

    #[test]
    fn each_kinds_chat_support_is_held_as_the_service_lists_it() {
        let now = Instant::now();
        let mut state = StoreState::default();
        let claude = KindChat {
            supported: true,
            reason: None,
            version: Some("2.2.0".to_owned()),
            range: Some(">=2.1.0, <2.3.0".to_owned()),
        };
        state.apply(
            StoreUpdate::AgentKindChatResync(BTreeMap::from([(
                "claude".to_owned(),
                claude.clone(),
            )])),
            now,
        );
        assert_eq!(state.agent_kind_chat.get("claude"), Some(&claude));
        state.apply(StoreUpdate::AgentKindChatResync(BTreeMap::new()), now);
        assert!(
            state.agent_kind_chat.is_empty(),
            "a resync drops the chat support of the kinds it no longer lists"
        );
    }

    #[test]
    fn a_changed_session_replaces_its_record_and_a_gone_one_is_dropped() {
        let now = Instant::now();
        let mut state = StoreState::default();
        state.apply(StoreUpdate::AgentsResync(vec![agent("s1", "w1", 10)]), now);
        let mut working = agent("s1", "w1", 10);
        working.activity = Some(Activity {
            state: ActivityState::Working,
            outcome: None,
            source: ActivitySource::Hook,
            at_ms: 11,
            seen_at_ms: None,
        });
        state.apply(StoreUpdate::AgentChanged(working.clone()), now);
        assert_eq!(state.agents, vec![working]);
        state.apply(StoreUpdate::AgentChanged(agent("s2", "w1", 20)), now);
        state.apply(StoreUpdate::AgentGone("s1".into()), now);
        state.apply(StoreUpdate::AgentGone("nope".into()), now);
        let ids: Vec<&str> = state.agents.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["s2"]);
    }

    #[test]
    fn the_time_the_link_went_down_is_kept_until_it_is_back() {
        let mut state = StoreState::default();
        let up = ConnectionState::Connected {
            server: "s".into(),
            version: "1".into(),
        };
        let t0 = Instant::now();
        state.apply(StoreUpdate::Connection(up.clone()), t0);
        assert_eq!(state.down_since, None);
        let down = ConnectionState::Reconnecting {
            attempt: 1,
            retry_at: t0,
        };
        state.apply(StoreUpdate::Connection(down.clone()), t0);
        let later = t0 + std::time::Duration::from_secs(3);
        state.apply(StoreUpdate::Connection(down), later);
        assert_eq!(
            state.down_since,
            Some(t0),
            "a second down state keeps the first stamp"
        );
        state.apply(StoreUpdate::Connection(up), later);
        assert_eq!(state.down_since, None);
    }

    #[test]
    fn a_stale_teams_read_never_replaces_a_newer_one() {
        let mut state = StoreState::default();
        let read = |number, name: &str| TeamsRead {
            number,
            teams: Ok(vec![team("t1", name)]),
        };
        state.apply(StoreUpdate::Teams(read(2, "newer")), Instant::now());
        state.apply(StoreUpdate::Teams(read(1, "older")), Instant::now());
        let names: Vec<&str> = state
            .teams
            .iter()
            .flatten()
            .map(|record| record.team.name.as_str())
            .collect();
        assert_eq!(names, ["newer"]);
        let refused = TeamsRead {
            number: 3,
            teams: Err(Refusal::unavailable("no teams")),
        };
        state.apply(StoreUpdate::Teams(refused), Instant::now());
        assert!(state.teams.is_none());
    }

    fn host(name: &str) -> HostRecord {
        HostRecord {
            name: name.into(),
            server: "vps-server".into(),
            addresses: vec!["100.64.0.10:7347".into()],
            pin: "sha256:test".into(),
            device_id: "dev-1".into(),
            device_name: "laptop".into(),
            scopes: vec!["read".into()],
            paired_at_ms: 1,
        }
    }

    fn entry<'a>(state: &'a StoreState, name: &str) -> &'a HostEntry {
        state
            .hosts
            .iter()
            .find(|entry| entry.record.name == name)
            .expect("the host is listed")
    }

    #[test]
    fn a_host_check_changes_only_the_host_it_names_and_a_refresh_keeps_the_last_answer() {
        let mut state = StoreState::default();
        let t0 = Instant::now();
        state.apply(StoreUpdate::HostsResync(vec![host("vps"), host("nas")]), t0);
        assert!(state.hosts.iter().all(|e| e.link == HostLink::Checking));

        let connected = HostLink::Connected {
            server: "vps-server".into(),
            version: "0.1.0".into(),
        };
        state.apply(
            StoreUpdate::HostLinkChanged {
                name: "vps".into(),
                link: connected.clone(),
            },
            t0,
        );
        state.apply(
            StoreUpdate::HostLinkChanged {
                name: "unlisted".into(),
                link: HostLink::Unreachable("x".into()),
            },
            t0,
        );
        assert_eq!(entry(&state, "vps").link, connected);
        assert_eq!(entry(&state, "vps").last_connected, Some(t0));
        assert_eq!(entry(&state, "nas").link, HostLink::Checking);

        let later = t0 + std::time::Duration::from_secs(300);
        let unreachable = HostLink::Unreachable("The host is unreachable: timed out".into());
        state.apply(
            StoreUpdate::HostLinkChanged {
                name: "vps".into(),
                link: unreachable.clone(),
            },
            later,
        );
        state.apply(StoreUpdate::HostsResync(vec![host("vps")]), later);
        assert_eq!(state.hosts.len(), 1, "a host no longer listed is dropped");
        assert_eq!(entry(&state, "vps").link, unreachable);
        assert_eq!(
            entry(&state, "vps").last_connected,
            Some(t0),
            "the last time it answered is kept"
        );
    }

    #[test]
    fn a_failed_connection_says_what_the_host_refused_in_a_sentence() {
        assert_eq!(
            HostLink::from_error(&ClientError::HostUnreachable("timed out".into())),
            HostLink::Unreachable("The host is unreachable: timed out".into())
        );
        let revoked = ClientError::Handshake {
            error: Box::new(massorca_protocol::ErrorBody::new(
                codes::AUTH_DEVICE_REVOKED,
                "this device's access was revoked; pair it again to reconnect",
            )),
            supported: None,
        };
        assert_eq!(
            HostLink::from_error(&revoked),
            HostLink::SignInNeeded(
                "This device's access was revoked; pair it again to reconnect".into()
            )
        );
        assert!(matches!(
            HostLink::from_error(&ClientError::ServerIdentityMismatch(Box::new(
                massorca_client::IdentityMismatch {
                    host: "vps".into(),
                    trusted_from: massorca_client::TrustedFrom::Pairing,
                    expected: "sha256:old".into(),
                    actual: Some("sha256:new".into()),
                }
            ))),
            HostLink::Failed(message)
                if message.starts_with("Vps presented a different server key")
                    && message.contains("sha256:new")
        ));
    }

    #[test]
    fn devices_and_network_replace_what_was_read_before() {
        let mut state = StoreState::default();
        assert_eq!(state.devices, None);
        state.apply(StoreUpdate::Devices(Ok(Vec::new())), Instant::now());
        assert_eq!(state.devices, Some(Ok(Vec::new())));
        state.apply(
            StoreUpdate::Devices(Err(Refusal::unavailable("gone"))),
            Instant::now(),
        );
        assert_eq!(state.devices, Some(Err(Refusal::unavailable("gone"))));
        assert_eq!(state.network, None);
    }

    #[test]
    fn a_vault_answer_replaces_only_the_part_it_names() {
        let mut state = StoreState::default();
        let status = SecretStatus {
            protection: "file".into(),
            state: "ready".into(),
            count: 1,
            name_clashes: Vec::new(),
        };
        let now = Instant::now();
        state.apply(
            StoreUpdate::Secrets(SecretsUpdate::Status(Ok(status.clone()))),
            now,
        );
        state.apply(
            StoreUpdate::Secrets(SecretsUpdate::List(Ok(Vec::new()))),
            now,
        );
        assert_eq!(state.secrets.status, Some(Ok(status)));
        assert_eq!(state.secrets.list, Some(Ok(Vec::new())));

        let gone = Refusal::unavailable("the service cannot be reached right now");
        state.apply(
            StoreUpdate::Secrets(SecretsUpdate::Status(Err(gone.clone()))),
            now,
        );
        assert_eq!(state.secrets.status, Some(Err(gone)));
        assert_eq!(state.secrets.list, Some(Ok(Vec::new())), "the list stays");
    }

    fn queue(session: &str, items: Vec<QueuedPrompt>) -> AgentQueueList {
        AgentQueueList {
            session: session.into(),
            items,
        }
    }

    #[test]
    fn a_service_without_the_queue_holds_none_and_ignores_queue_changes() {
        let now = Instant::now();
        let mut state = StoreState::default();
        state.apply(StoreUpdate::AgentQueuesResync(None), now);
        state.apply(
            StoreUpdate::AgentQueueChanged(queue("s1", vec![queued("q1", "s1", 1)])),
            now,
        );
        assert_eq!(state.agent_queues, None);
    }

    #[test]
    fn a_queue_change_replaces_the_sessions_queue_and_an_empty_or_gone_one_leaves() {
        let now = Instant::now();
        let mut state = StoreState::default();
        state.apply(
            StoreUpdate::AgentQueuesResync(Some(vec![
                queue("s1", vec![queued("q1", "s1", 1)]),
                queue("s2", Vec::new()),
            ])),
            now,
        );
        let held = |state: &StoreState| {
            state
                .agent_queues
                .as_ref()
                .map(|queues| queues.keys().cloned().collect::<Vec<_>>())
        };
        assert_eq!(held(&state), Some(vec!["s1".to_owned()]));
        state.apply(
            StoreUpdate::AgentQueueChanged(queue(
                "s2",
                vec![queued("q2", "s2", 1), queued("q3", "s2", 2)],
            )),
            now,
        );
        assert_eq!(
            state
                .agent_queues
                .as_ref()
                .and_then(|q| q.get("s2"))
                .map(Vec::len),
            Some(2)
        );
        state.apply(StoreUpdate::AgentQueueChanged(queue("s1", Vec::new())), now);
        state.apply(StoreUpdate::AgentGone("s2".into()), now);
        assert_eq!(held(&state), Some(Vec::new()));
    }

    #[test]
    fn a_paired_host_keeps_the_newest_answer_not_the_last_one_to_arrive() {
        let now = Instant::now();
        let mut state = StoreState::default();
        let newest = HostMarketplaceRead {
            host: "vps".into(),
            request: 2,
            listing: HostListing::UpdateNeeded,
        };
        let older = HostMarketplaceRead {
            host: "vps".into(),
            request: 1,
            listing: HostListing::UpdateNeeded,
        };
        state.apply(StoreUpdate::HostMarketplace(newest.clone()), now);
        state.apply(StoreUpdate::HostMarketplace(older), now);
        assert_eq!(state.host_listings.get("vps"), Some(&newest));
    }

    #[test]
    fn marketplace_skills_keep_the_newest_answer_of_each_host() {
        use crate::service::SkillsListing;
        let now = Instant::now();
        let mut state = StoreState::default();
        let answer = |host: Option<&str>, request| SkillsRead {
            host: host.map(str::to_owned),
            request,
            listing: SkillsListing::UpdateNeeded,
        };
        state.apply(StoreUpdate::Skills(answer(None, 2)), now);
        state.apply(StoreUpdate::Skills(answer(None, 1)), now);
        state.apply(StoreUpdate::Skills(answer(Some("vps"), 3)), now);
        assert_eq!(state.skills.get(&None).map(|read| read.request), Some(2));
        assert_eq!(
            state
                .skills
                .get(&Some("vps".to_owned()))
                .map(|read| read.request),
            Some(3)
        );
    }
}
