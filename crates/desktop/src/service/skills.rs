//! Discovered skills for the Skills section (SKL-01): This computer's service, or a paired host's on its own
//! connection, the way its Marketplace lists are read. The newest answer per host is kept.

use massorca_client::Client;
use massorca_protocol::capabilities;
use massorca_protocol::methods::skill_bundled::{
    BundledSkill, SkillsBundledInstallParams, SkillsBundledNameParams,
};
use massorca_protocol::methods::skill_placement::{
    ManagedSkill, PlacementAgent, SkillPlaceParams, SkillPlacementResult, SkillUnplaceParams,
    SkillsManagedParams,
};
use massorca_protocol::methods::skills::{SkillDiscovery, SkillsDiscoverParams};
use tokio::sync::mpsc;

use super::bridge::{Command, ServiceHandle};
use super::extensions::{HostListing, connect_listing, predates};
use super::live_hosts::LiveHosts;
use super::refusal::Refusal;
use super::update::StoreUpdate;

/// What a discovery answered: the skills and roots, or why the host cannot show them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkillsListing {
    Found(SkillDiscovery),
    /// The host's MassOrca predates skill discovery: it lacks `skills/1` or does not know `skills.discover`.
    UpdateNeeded,
    /// The host could not be asked; the refusal says why.
    Refused(Refusal),
}

/// The answer to a discovery of This computer (`host` is None) or of a paired host, tagged with its request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillsRead {
    pub host: Option<String>,
    pub request: u64,
    pub listing: SkillsListing,
}

impl ServiceHandle {
    /// Discovers This computer's skills; the answer arrives as a store update tagged with `request`.
    pub fn read_skills(&self, request: u64, refresh: bool) {
        self.send(Command::Skills { request, refresh });
    }

    /// Discovers a paired host's skills on its live connection; the answer arrives tagged with `request`.
    pub fn read_host_skills(&self, host: String, request: u64, refresh: bool) {
        self.send(Command::HostSkills {
            host,
            request,
            refresh,
        });
    }
}

/// Whether a service's welcome offers the `skills/1` capability discovery needs.
fn has_skills(offered: &[String]) -> bool {
    offered
        .iter()
        .any(|name| name.as_str() == capabilities::SKILLS)
}

/// Whether a service's welcome offers the `skills-bundled/1` capability the bundled skills need (SKL-04).
fn has_bundled(offered: &[String]) -> bool {
    offered
        .iter()
        .any(|name| name.as_str() == capabilities::SKILLS_BUNDLED)
}

/// Asks a service for its skills; a service without the capability predates discovery.
async fn discover(client: &Client, refresh: bool) -> SkillsListing {
    if !has_skills(&client.welcome().capabilities) {
        return SkillsListing::UpdateNeeded;
    }
    let params = SkillsDiscoverParams {
        refresh,
        source_kinds: Vec::new(),
    };
    match client.skills_discover(&params).await {
        Ok(found) => SkillsListing::Found(found),
        Err(error) if predates(&error) => SkillsListing::UpdateNeeded,
        Err(error) => SkillsListing::Refused(Refusal::from(&error)),
    }
}

/// Discovers This computer's skills on its own task, so a slow scan never holds up events.
pub(super) fn spawn_read(
    client: &Client,
    request: u64,
    refresh: bool,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    let client = client.clone();
    let updates = updates.clone();
    tokio::spawn(async move {
        let listing = discover(&client, refresh).await;
        let _ = updates.send(StoreUpdate::Skills(SkillsRead {
            host: None,
            request,
            listing,
        }));
    });
}

/// Discovers a paired host's skills on the host's live connection, as its Marketplace lists are read.
pub(super) fn spawn_host_read(
    live: LiveHosts,
    host: String,
    request: u64,
    refresh: bool,
    updates: mpsc::UnboundedSender<StoreUpdate>,
) {
    tokio::spawn(async move {
        let listing = match live.client(&host).await {
            Ok(client) => discover(&client, refresh).await,
            Err(error) => match connect_listing(&error) {
                HostListing::Refused(refusal) => SkillsListing::Refused(refusal),
                HostListing::Lists(_) | HostListing::UpdateNeeded => SkillsListing::UpdateNeeded,
            },
        };
        let _ = updates.send(StoreUpdate::Skills(SkillsRead {
            host: Some(host),
            request,
            listing,
        }));
    });
}

/// A call about skills on This computer: a placement, a removal or the placed list (SKL-02), or a bundled skill (SKL-04).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlacementCall {
    /// What placing the skill for these agents would do. Changes nothing.
    Preview {
        skill_id: String,
        agents: Vec<PlacementAgent>,
    },
    Place {
        skill_id: String,
        agents: Vec<PlacementAgent>,
    },
    /// Removes the copies MassOrca placed for the skill; a copy the user changed stays.
    Unplace {
        name: String,
    },
    Managed,
    /// Lists the skills MassOrca ships and their state on This computer. Changes nothing.
    BundledList,
    /// Places a bundled skill for every agent folder on this computer.
    BundledInstall {
        name: String,
    },
    /// Brings a placed bundled skill to this build's text, for the agents it is placed for.
    BundledRefresh {
        name: String,
    },
}

/// What a placement call answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlacementOutcome {
    Preview(SkillPlacementResult),
    Changed(SkillPlacementResult),
    Managed(Vec<ManagedSkill>),
    /// The skills MassOrca ships; empty when the service does not offer them.
    Bundled(Vec<BundledSkill>),
    Refused(Refusal),
}

/// The answer to a placement call, tagged with its request; the newest request is the one shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementAnswer {
    pub request: u64,
    pub outcome: PlacementOutcome,
}

impl ServiceHandle {
    /// Previews, places or removes skills on This computer, or lists them; the answer arrives tagged with `request`.
    pub fn skill_placement(&self, request: u64, call: PlacementCall) {
        self.send(Command::SkillPlacement { request, call });
    }
}

/// Runs one placement call on its own task, so a copy of a large skill never holds up events.
pub(super) fn spawn_placement(
    client: &Client,
    request: u64,
    call: PlacementCall,
    updates: &mpsc::UnboundedSender<StoreUpdate>,
) {
    let client = client.clone();
    let updates = updates.clone();
    tokio::spawn(async move {
        let changes = matches!(
            call,
            PlacementCall::Place { .. }
                | PlacementCall::Unplace { .. }
                | PlacementCall::BundledInstall { .. }
                | PlacementCall::BundledRefresh { .. }
        );
        let outcome = placement(&client, call).await;
        let refresh = changes && matches!(outcome, PlacementOutcome::Changed(_));
        let _ = updates.send(StoreUpdate::SkillPlacement(PlacementAnswer {
            request,
            outcome,
        }));
        if refresh {
            // Why: a change also refreshes the placed and bundled lists, under the same request, so neither shows a stale copy.
            for list in [PlacementCall::Managed, PlacementCall::BundledList] {
                let outcome = placement(&client, list).await;
                let _ = updates.send(StoreUpdate::SkillPlacement(PlacementAnswer {
                    request,
                    outcome,
                }));
            }
        }
    });
}

async fn placement(client: &Client, call: PlacementCall) -> PlacementOutcome {
    let answered = match call {
        PlacementCall::Preview { skill_id, agents } => client
            .skills_place_preview(&SkillPlaceParams {
                skill_id,
                agents,
                target: Default::default(),
            })
            .await
            .map(PlacementOutcome::Preview),
        PlacementCall::Place { skill_id, agents } => client
            .skills_place(&SkillPlaceParams {
                skill_id,
                agents,
                target: Default::default(),
            })
            .await
            .map(PlacementOutcome::Changed),
        PlacementCall::Unplace { name } => client
            .skills_unplace(&SkillUnplaceParams {
                name,
                agents: Vec::new(),
                target: Default::default(),
            })
            .await
            .map(PlacementOutcome::Changed),
        PlacementCall::Managed => client
            .skills_managed(&SkillsManagedParams::default())
            .await
            .map(|managed| PlacementOutcome::Managed(managed.skills)),
        PlacementCall::BundledList => {
            if !has_bundled(&client.welcome().capabilities) {
                return PlacementOutcome::Bundled(Vec::new());
            }
            client
                .skills_bundled_list()
                .await
                .map(|listed| PlacementOutcome::Bundled(listed.skills))
        }
        PlacementCall::BundledInstall { name } => client
            .skills_bundled_install(&SkillsBundledInstallParams {
                name,
                agents: Vec::new(),
            })
            .await
            .map(PlacementOutcome::Changed),
        PlacementCall::BundledRefresh { name } => client
            .skills_bundled_refresh(&SkillsBundledNameParams { name })
            .await
            .map(PlacementOutcome::Changed),
    };
    answered.unwrap_or_else(|error| PlacementOutcome::Refused(Refusal::from(&error)))
}
