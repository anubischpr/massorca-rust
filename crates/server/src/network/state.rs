//! The network state shared by the listeners, the handshake and the device methods.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use massorca_net::Pin;
use massorca_protocol::methods::devices::ListenerInfo;
use tokio::sync::Notify;

use crate::network::listen::ListenSpec;
use crate::network::live::LiveDevices;
use crate::network::pairing::Pairing;

/// What one listener is doing now.
#[derive(Debug, Clone)]
pub(crate) enum ListenerState {
    Starting,
    Waiting {
        reason: String,
    },
    Listening {
        bound: Option<SocketAddr>,
        /// Another name for the same address (the MagicDNS name), offered after it.
        extra: Option<String>,
    },
}

/// The host name, lowercased and reduced to the characters of a host name on the client (decision 0046).
fn machine_name() -> String {
    let raw = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
        .or_else(|| std::fs::read_to_string("/proc/sys/kernel/hostname").ok())
        .unwrap_or_default();
    let name: String = raw
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .skip_while(|c| !c.is_ascii_alphanumeric())
        .take(32)
        .collect();
    if name.is_empty() {
        "server".to_owned()
    } else {
        name
    }
}

#[derive(Debug)]
pub struct NetworkState {
    /// The server's pin; `None` when the service has no network listener.
    pub(crate) server_pin: Option<Pin>,
    /// This machine's name, offered to new devices as the default host name.
    pub(crate) machine_name: String,
    listeners: Mutex<Vec<(ListenSpec, ListenerState)>>,
    pub(crate) pairing: Pairing,
    pub(crate) live: Arc<LiveDevices>,
    /// Woken whenever a listener changes state.
    changed: Notify,
}

impl NetworkState {
    pub(crate) fn new(server_pin: Option<Pin>, specs: &[ListenSpec]) -> Self {
        Self {
            server_pin,
            machine_name: machine_name(),
            listeners: Mutex::new(
                specs
                    .iter()
                    .map(|spec| (spec.clone(), ListenerState::Starting))
                    .collect(),
            ),
            pairing: Pairing::default(),
            live: Arc::new(LiveDevices::default()),
            changed: Notify::new(),
        }
    }

    pub(crate) fn set_listener(&self, index: usize, state: ListenerState) {
        if let Some(entry) = self.lock().get_mut(index) {
            entry.1 = state;
        }
        self.changed.notify_waiters();
    }

    /// True once every listener has made its first bind attempt: listening, or waiting with the reason.
    pub(crate) fn settled(&self) -> bool {
        self.lock()
            .iter()
            .all(|(_, state)| !matches!(state, ListenerState::Starting))
    }

    /// Resolves once [`Self::settled`] holds.
    pub(crate) async fn wait_settled(&self) {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            // Registered before the check, so a change between the check and the wait is not missed.
            changed.as_mut().enable();
            if self.settled() {
                return;
            }
            changed.await;
        }
    }

    /// Addresses a new device can dial: each bound listener, then its extra name.
    pub(crate) fn advertised_addresses(&self) -> Vec<String> {
        let mut addresses = Vec::new();
        for (_, state) in self.lock().iter() {
            if let ListenerState::Listening { bound, extra } = state {
                if let Some(bound) = bound.filter(|bound| !bound.ip().is_unspecified()) {
                    addresses.push(bound.to_string());
                }
                if let Some(extra) = extra {
                    addresses.push(extra.clone());
                }
            }
        }
        addresses
    }

    /// The port of the first bound listener, which an offer address without a port takes; 0 when none is bound.
    pub(crate) fn listening_port(&self) -> u16 {
        self.lock()
            .iter()
            .find_map(|(_, state)| match state {
                ListenerState::Listening {
                    bound: Some(bound), ..
                } => Some(bound.port()),
                _ => None,
            })
            .unwrap_or(0)
    }

    pub(crate) fn listener_infos(&self) -> Vec<ListenerInfo> {
        self.lock()
            .iter()
            .map(|(spec, state)| {
                let (state, bound, error) = match state {
                    ListenerState::Starting => ("waiting", None, None),
                    ListenerState::Waiting { reason } => ("waiting", None, Some(reason.clone())),
                    ListenerState::Listening { bound, .. } => {
                        ("listening", bound.map(|bound| bound.to_string()), None)
                    }
                };
                ListenerInfo {
                    requested: spec.to_string(),
                    bound,
                    state: state.to_owned(),
                    error,
                }
            })
            .collect()
    }

    fn lock(&self) -> MutexGuard<'_, Vec<(ListenSpec, ListenerState)>> {
        self.listeners
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}
