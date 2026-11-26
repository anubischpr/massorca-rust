//! One live link per paired host (REM-07), kept by the bridge: each link holds the one connection to its host,
//! reconnects with the network backoff, and its state reaches the store as `HostLinkChanged`. Everything else that
//! talks to a paired host borrows that connection: the session follower, terminal attaches and one-off calls. A host
//! that is forgotten has its link stopped, and its borrowers with it.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;

use massorca_client::{
    Backoff, Client, ClientError, HostDial, Hosts, LiveHost, LiveState, LiveWatch,
};
use massorca_profile::Profile;
use tokio::sync::{mpsc, watch};

use super::host_sessions;
use super::hosts::peer;
use super::refusal::Refusal;
use super::update::{HostLink, StoreUpdate};

type Start = Arc<dyn Fn(&str) -> LiveHost + Send + Sync>;

/// The live links of the paired hosts, by name. Clones share them.
#[derive(Clone)]
pub(super) struct LiveHosts {
    links: Arc<Mutex<BTreeMap<String, LiveHost>>>,
    start: Start,
    updates: mpsc::UnboundedSender<StoreUpdate>,
}

impl LiveHosts {
    /// Links that connect to the hosts of `profile` as this desktop, with the network backoff.
    pub(super) fn new(profile: &Profile, updates: mpsc::UnboundedSender<StoreUpdate>) -> Self {
        let hosts = Hosts::of(profile);
        let start: Start = Arc::new(move |name: &str| {
            LiveHost::start(
                HostDial::new(hosts.clone(), name, peer()),
                Backoff::default(),
            )
        });
        Self::with_start(start, updates)
    }

    fn with_start(start: Start, updates: mpsc::UnboundedSender<StoreUpdate>) -> Self {
        Self {
            links: Arc::new(Mutex::new(BTreeMap::new())),
            start,
            updates,
        }
    }

    /// Starts a link for each listed host that has none, and stops the links of hosts no longer listed. A link that
    /// runs keeps its connection and state. Needs a Tokio runtime.
    pub(super) fn follow(&self, names: impl IntoIterator<Item = String>) {
        let names: Vec<String> = names.into_iter().collect();
        let mut links = self.links.lock().unwrap_or_else(PoisonError::into_inner);
        links.retain(|name, _| names.contains(name));
        for name in names {
            if links.contains_key(&name) {
                continue;
            }
            let link = (self.start)(&name);
            tokio::spawn(forward(
                name.clone(),
                link.subscribe(),
                self.updates.clone(),
            ));
            tokio::spawn(host_sessions::follow(
                name.clone(),
                link.watch(),
                self.updates.clone(),
            ));
            links.insert(name, link);
        }
    }

    /// Follows the listed hosts, then tries `name` again at once: it was just paired, re-added or upgraded, so a link
    /// that had ended may now get through. The link alone reports the host's state.
    pub(super) fn follow_again(&self, names: impl IntoIterator<Item = String>, name: &str) {
        self.follow(names);
        self.reconnect(name);
    }

    /// Tries host `name` again at once. `false` when it has no link.
    pub(super) fn reconnect(&self, name: &str) -> bool {
        let links = self.links.lock().unwrap_or_else(PoisonError::into_inner);
        links.get(name).map(LiveHost::reconnect).is_some()
    }

    /// A view of host `name`'s link for a borrower of its connection. `None` when the host has no link.
    pub(super) fn watch(&self, name: &str) -> Option<LiveWatch> {
        let links = self.links.lock().unwrap_or_else(PoisonError::into_inner);
        links.get(name).map(LiveHost::watch)
    }

    /// Host `name`'s connection once its link's attempt under way has settled, or why there is none. A host this
    /// computer is not paired with is `HostNotFound`, as a dial would say.
    pub(super) async fn client(&self, name: &str) -> Result<Client, Arc<ClientError>> {
        let Some(mut watch) = self.watch(name) else {
            return Err(Arc::new(ClientError::HostNotFound(name.to_owned())));
        };
        watch.client().await
    }

    /// Stops the link of host `name`, closing its connection.
    pub(super) fn stop(&self, name: &str) {
        let removed = self
            .links
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(name);
        drop(removed);
    }
}

/// Sends each state of one link to the store until the link stops. The first `Connecting` is left out: the store
/// already shows a host it has not heard from as checking, and a pairing just reported it connected.
async fn forward(
    name: String,
    mut states: watch::Receiver<LiveState>,
    updates: mpsc::UnboundedSender<StoreUpdate>,
) {
    let mut first = true;
    loop {
        let link = {
            let state = states.borrow_and_update();
            let skip = first && matches!(*state, LiveState::Connecting);
            (!skip).then(|| host_link(&state, Instant::now()))
        };
        first = false;
        if let Some(link) = link {
            let update = StoreUpdate::HostLinkChanged {
                name: name.clone(),
                link,
            };
            if updates.send(update).is_err() {
                return;
            }
        }
        if states.changed().await.is_err() {
            return;
        }
    }
}

/// A link state in the store's words, at `now`.
pub(super) fn host_link(state: &LiveState, now: Instant) -> HostLink {
    match state {
        LiveState::Connecting => HostLink::Checking,
        LiveState::Connected { server, version } => HostLink::Connected {
            server: server.clone(),
            version: version.clone(),
        },
        LiveState::Unreachable {
            error,
            attempt,
            retry_in,
        } => HostLink::Reconnecting {
            reason: Refusal::from(&**error).sentence(),
            attempt: *attempt,
            retry_at: now + *retry_in,
        },
        LiveState::Ended { error } => HostLink::from_error(error),
    }
}

