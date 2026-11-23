//! Debug only: a launch with `--page <name>` opens that page or dialog once the data it shows has
//! arrived. When the service has answered without that data, the page's parent opens instead and
//! stderr says why, so a launch never panics on a missing secret or team.

use std::io::Write;

use gpui::{Context, Window};

use super::Shell;
use crate::launch_options::DebugPage;
use crate::model::navigation::{Mode, Page};
use crate::model::settings_sections::SettingsSection;
use crate::model::teams::{TeamId, TeamPage};
use crate::service::StoreState;

/// What the shell holds now, for the request that is pending.
struct Facts {
    workspace: Option<String>,
    /// The first team, only while Bot Teams is connected.
    team: Option<TeamId>,
    teams_connected: bool,
    owned_secret: Option<OwnedSecret>,
    /// The first list of workspaces and projects has arrived (or the shell runs on sample data).
    workspaces_answered: bool,
    /// The Bot Teams read has answered, a refusal included.
    teams_answered: bool,
    /// The vault status or list has answered, a refusal included.
    vault_answered: bool,
    /// The newest agent session the store holds, which `--page session` opens.
    session: Option<String>,
    /// The agent session list has arrived (or the shell runs on sample data).
    sessions_answered: bool,
    /// Whether a service backs the shell: a delegate needs one to send to.
    service: bool,
    /// The first child session the store holds that is not brought back yet, which `--page bring-back` opens.
    child: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OwnedSecret {
    scope: String,
    name: String,
    linked_scopes: Vec<String>,
}

enum Target {
    Workspace(String),
    Session(String),
    Settings(SettingsSection),
    Pairing,
    AddHost,
    AddSecret,
    Board(TeamId),
    Rotate(OwnedSecret),
    Remove(OwnedSecret),
    NewTeam,
    AddBot(TeamId),
    Delegate(String),
    BringBack(String),
}

/// The page a request falls back to when its data is absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Parent {
    Workspaces,
    BotTeams,
    SettingsVault,
}

impl Parent {
    fn label(self) -> &'static str {
        match self {
            Parent::Workspaces => "the workspaces home",
            Parent::BotTeams => "the Bot Teams home",
            Parent::SettingsVault => "Settings, Vault",
        }
    }
}

enum Step {
    Open(Target),
    /// The service has not answered for this data yet; the next frame asks again.
    Wait,
    Fall(Parent, &'static str),
}

/// Decides one request from the data in hand. Only an answered gap falls back; an unanswered one waits.
fn plan(page: DebugPage, facts: &Facts) -> Step {
    match page {
        DebugPage::Workspace => match &facts.workspace {
            Some(id) => Step::Open(Target::Workspace(id.clone())),
            None => answered(
                facts.workspaces_answered,
                Parent::Workspaces,
                "the service lists no workspace",
            ),
        },
        DebugPage::Session => match (&facts.session, &facts.workspace) {
            (Some(session), _) => Step::Open(Target::Session(session.clone())),
            _ if !facts.sessions_answered => Step::Wait,
            (None, Some(workspace)) => Step::Open(Target::Workspace(workspace.clone())),
            (None, None) => answered(
                facts.workspaces_answered,
                Parent::Workspaces,
                "the service lists no workspace",
            ),
        },
        DebugPage::Delegate => match (&facts.session, facts.service) {
            (Some(session), true) => Step::Open(Target::Delegate(session.clone())),
            (Some(_), false) => Step::Fall(
                Parent::Workspaces,
                "no service is connected, so no delegate can be sent",
            ),
            (None, _) => answered(
                facts.sessions_answered,
                Parent::Workspaces,
                "the service lists no agent session",
            ),
        },
        DebugPage::BringBack => match &facts.child {
            Some(child) => Step::Open(Target::BringBack(child.clone())),
            None => answered(
                facts.sessions_answered,
                Parent::Workspaces,
                "the service lists no child session to bring back",
            ),
        },
        DebugPage::SettingsGeneral => Step::Open(Target::Settings(SettingsSection::General)),
        DebugPage::SettingsHosts => Step::Open(Target::Settings(SettingsSection::Hosts)),
        DebugPage::SettingsVault => Step::Open(Target::Settings(SettingsSection::Vault)),
        DebugPage::Pairing => Step::Open(Target::Pairing),
        DebugPage::AddHost => Step::Open(Target::AddHost),
        DebugPage::AddSecret => Step::Open(Target::AddSecret),
        DebugPage::Board => match &facts.team {
            Some(team) => Step::Open(Target::Board(*team)),
            None => answered(facts.teams_answered, Parent::BotTeams, "no team is listed"),
        },
        DebugPage::ReplaceSecret => match &facts.owned_secret {
            Some(secret) => Step::Open(Target::Rotate(secret.clone())),
            None => answered(
                facts.vault_answered,
                Parent::SettingsVault,
                "the vault holds no owned secret",
            ),
        },
        DebugPage::RemoveSecret => match &facts.owned_secret {
            Some(secret) => Step::Open(Target::Remove(secret.clone())),
            None => answered(
                facts.vault_answered,
                Parent::SettingsVault,
                "the vault holds no owned secret",
            ),
        },
        DebugPage::NewTeam => {
            if facts.teams_connected {
                Step::Open(Target::NewTeam)
            } else {
                answered(
                    facts.teams_answered,
                    Parent::BotTeams,
                    "Bot Teams is not connected",
                )
            }
        }
        DebugPage::AddBot => match &facts.team {
            Some(team) => Step::Open(Target::AddBot(*team)),
            None => answered(
                facts.teams_answered,
                Parent::BotTeams,
                "Bot Teams has no team to add a bot to",
            ),
        },
    }
}

fn answered(answered: bool, parent: Parent, reason: &'static str) -> Step {
    if answered {
        Step::Fall(parent, reason)
    } else {
        Step::Wait
    }
}

fn owned_secret(state: &StoreState) -> Option<OwnedSecret> {
    let entries = state.secrets.list.as_ref()?.as_ref().ok()?;
    entries
        .iter()
        .find(|entry| !entry.linked)
        .map(|entry| OwnedSecret {
            scope: entry.scope.clone(),
            name: entry.name.clone(),
            linked_scopes: entry.linked_scopes.clone(),
        })
}

impl Shell {
    /// Asks for `page` on the first frame that finds its data. The request is spent once it opens or falls back.
    pub fn start_debug_page(&mut self, page: DebugPage) {
        self.debug_page = Some(page);
    }

    /// Acts on the pending request: opens it, falls back to its parent, or waits for the next frame.
    pub(super) fn drive_debug_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = self.debug_page else {
            return;
        };
        let facts = self.debug_facts(cx);
        match plan(page, &facts) {
            Step::Wait => {}
            Step::Open(target) => {
                self.debug_page = None;
                self.open_debug_target(target, window, cx);
            }
            Step::Fall(parent, reason) => {
                self.debug_page = None;
                let _ = writeln!(
                    std::io::stderr(),
                    "massorca-desktop: debug page {} fell back to {}: {reason}",
                    page.name(),
                    parent.label()
                );
                self.open_debug_parent(parent);
            }
        }
    }

    fn debug_facts(&self, cx: &Context<Self>) -> Facts {
        let store = self.store.read(cx);
        let state = store.state();
        let sample = store.is_sample();
        let teams_connected = self.model.teams().is_connected();
        Facts {
            workspace: self
                .model
                .snapshot()
                .projects
                .iter()
                .flat_map(|project| &project.hosts)
                .flat_map(|host| &host.workspaces)
                .next()
                .map(|row| row.id.clone()),
            team: teams_connected
                .then(|| self.model.teams().teams().first().map(|team| team.id))
                .flatten(),
            teams_connected,
            owned_secret: owned_secret(state),
            session: state
                .agents
                .iter()
                .max_by_key(|session| session.created_at_ms)
                .map(|session| session.id.clone()),
            sessions_answered: sample || state.agents_answered,
            service: self.service.is_some(),
            child: state
                .agents
                .iter()
                .find(|session| session.parent_id.is_some() && session.brought_back_at_ms.is_none())
                .map(|session| session.id.clone()),
            workspaces_answered: sample || state.loaded,
            teams_answered: sample || state.team_read > 0,
            vault_answered: sample
                || state.secrets.status.is_some()
                || state.secrets.list.is_some(),
        }
    }

    fn open_debug_target(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        match target {
            Target::Workspace(id) => {
                self.go_home(Mode::Workspaces);
                self.model.open_workspace(id);
            }
            Target::Session(id) => {
                self.go_home(Mode::Workspaces);
                self.model.open_session(&id);
            }
            Target::Delegate(id) => {
                self.go_home(Mode::Workspaces);
                self.model.open_session(&id);
                self.open_delegate(&id, window, cx);
            }
            Target::BringBack(child) => {
                self.go_home(Mode::Workspaces);
                self.model.open_session(&child);
                self.ask_bring_back(&child, cx);
            }
            Target::Settings(section) => self.open_debug_settings(section),
            Target::Pairing => self.model.open_page(Page::Pairing),
            Target::AddHost => {
                self.open_debug_settings(SettingsSection::Hosts);
                self.open_add_host(cx);
            }
            Target::AddSecret => {
                self.open_debug_settings(SettingsSection::Vault);
                self.open_add_secret(cx);
            }
            Target::Board(team) => {
                self.go_home(Mode::BotTeams);
                self.model.open_team_page(TeamPage::Board(team));
            }
            Target::Rotate(secret) => {
                self.open_debug_settings(SettingsSection::Vault);
                self.open_rotate_secret(secret.scope, secret.name, cx);
            }
            Target::Remove(secret) => {
                self.open_debug_settings(SettingsSection::Vault);
                self.open_remove_secret(secret.scope, secret.name, secret.linked_scopes, cx);
            }
            Target::NewTeam => {
                self.go_home(Mode::BotTeams);
                self.open_new_team(window, cx);
            }
            Target::AddBot(team) => {
                self.go_home(Mode::BotTeams);
                self.model.open_team_page(TeamPage::Board(team));
                self.open_add_bot(team, window, cx);
            }
        }
    }

    fn open_debug_parent(&mut self, parent: Parent) {
        match parent {
            Parent::Workspaces => self.go_home(Mode::Workspaces),
            Parent::BotTeams => self.go_home(Mode::BotTeams),
            Parent::SettingsVault => self.open_debug_settings(SettingsSection::Vault),
        }
    }

    /// Reaches the home page of `mode`: at most one mode press from a page, and one more to switch.
    fn go_home(&mut self, mode: Mode) {
        for _ in 0..2 {
            let navigation = self.model.navigation();
            if navigation.mode == mode && navigation.page == Page::Home {
                break;
            }
            self.model.press_mode_switch();
        }
    }

    fn open_debug_settings(&mut self, section: SettingsSection) {
        self.model.open_page(Page::Settings);
        self.settings.section = section;
    }
}
