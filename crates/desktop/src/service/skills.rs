//! Discovered skills for the Skills section (SKL-01): This computer's service, or a paired host's on its own
//! connection, the way its Marketplace lists are read. The newest answer per host is kept.

use massorca_client::Client;
use massorca_profile::Profile;
use massorca_protocol::capabilities;
use massorca_protocol::methods::skills::{SkillDiscovery, SkillsDiscoverParams};
use tokio::sync::mpsc;

use super::bridge::{Command, ServiceHandle};
use super::extensions::{HostListing, connect_host, connect_listing, predates};
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

    /// Discovers a paired host's skills on its own connection; the answer arrives tagged with `request`.
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

/// Discovers a paired host's skills on the host's own connection, as its Marketplace lists are read.
pub(super) fn spawn_host_read(
    profile: Profile,
    host: String,
    request: u64,
    refresh: bool,
    updates: mpsc::UnboundedSender<StoreUpdate>,
) {
    tokio::spawn(async move {
        let listing = match connect_host(&profile, &host).await {
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
