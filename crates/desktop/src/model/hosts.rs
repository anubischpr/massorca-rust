//! The paired hosts as the Settings page and the sidebar show them (D-HO-01, D-HO-07, X-CN-01). A host
//! shows the state its last check left, with the age of its last good answer. No word says "stopped",
//! "offline" or "done": losing contact is not proof that the host stopped. Free of GPUI, so the rules
//! test without a window.

use std::time::{Duration, Instant};

use crate::model::marketplace::{Host, HostId, HostState as MarketHostState};
use crate::model::service_view::LOCAL_HOST;
use crate::service::{HostEntry, HostLink, StoreState};

/// A host's state as its row shows it: a word, its colour family and an optional line under the name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostState {
    pub word: &'static str,
    pub tone: HostTone,
    pub detail: Option<String>,
}

/// The colour family of a host's state. An unreachable host is drawn dimmed, not as a failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostTone {
    Connected,
    Checking,
    Warning,
    Unreachable,
    Failed,
}

/// The state of one paired host at `now`, from the link its last check left it in.
pub fn host_state(entry: &HostEntry, now: Instant) -> HostState {
    let (word, tone, detail) = match &entry.link {
        HostLink::Checking => ("Checking", HostTone::Checking, None),
        HostLink::Connected { server, version } => (
            "Connected",
            HostTone::Connected,
            Some(join_words(server, version)).filter(|text| !text.is_empty()),
        ),
        HostLink::Unreachable(reason) => (
            "Unreachable",
            HostTone::Unreachable,
            Some(match entry.last_connected {
                Some(at) => last_known(now.saturating_duration_since(at)),
                None => reason.clone(),
            }),
        ),
        HostLink::SignInNeeded(reason) => {
            ("Sign-in needed", HostTone::Warning, Some(reason.clone()))
        }
        HostLink::UpdateNeeded(reason) => {
            ("Update needed", HostTone::Warning, Some(reason.clone()))
        }
        HostLink::Failed(reason) => ("Failed", HostTone::Failed, Some(reason.clone())),
    };
    HostState { word, tone, detail }
}

/// "Last known state, 5 min ago": the age in whole minutes, and at least one minute.
pub fn last_known(elapsed: Duration) -> String {
    let minutes = (elapsed.as_secs() / 60).max(1);
    format!("Last known state, {minutes} min ago")
}

fn join_words(server: &str, version: &str) -> String {
    [server, version]
        .into_iter()
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Each paired host by name, with the state its last check left at `now`, in the order the store holds them.
pub fn host_rows(entries: &[HostEntry], now: Instant) -> Vec<(String, HostState)> {
    entries
        .iter()
        .map(|entry| (entry.record.name.clone(), host_state(entry, now)))
        .collect()
}

/// The hosts the Marketplace lists while a service answers: This PC first, then each paired host.
pub fn marketplace_hosts(state: &StoreState, now: Instant) -> Vec<Host> {
    let this_pc = if state.connection.is_connected() {
        MarketHostState::Connected
    } else {
        MarketHostState::Unreachable
    };
    let mut hosts = vec![Host {
        id: HostId(1),
        name: LOCAL_HOST.to_owned(),
        state: this_pc,
    }];
    for (index, entry) in state.hosts.iter().enumerate() {
        let link = host_state(entry, now);
        let state = match link.tone {
            HostTone::Connected => MarketHostState::Connected,
            HostTone::Unreachable => MarketHostState::Unreachable,
            _ => MarketHostState::Other(link.word),
        };
        hosts.push(Host {
            id: HostId(u32::try_from(index + 2).unwrap_or(u32::MAX)),
            name: entry.record.name.clone(),
            state,
        });
    }
    hosts
}

#[cfg(test)]
mod tests {
    use super::*;
    use massorca_client::HostRecord;

    fn entry(link: HostLink, last_connected: Option<Instant>) -> HostEntry {
        HostEntry {
            record: HostRecord {
                name: "vps".into(),
                server: "vps-server".into(),
                addresses: vec!["100.64.0.10:7347".into()],
                pin: "sha256:test".into(),
                device_id: "dev-1".into(),
                device_name: "laptop".into(),
                scopes: vec!["read".into()],
                paired_at_ms: 1,
            },
            link,
            last_connected,
        }
    }

    #[test]
    fn a_checked_host_says_connected_and_names_its_server_and_version() {
        let link = HostLink::Connected {
            server: "vps-server".into(),
            version: "0.1.0".into(),
        };
        let state = host_state(&entry(link, None), Instant::now());
        assert_eq!(state.word, "Connected");
        assert_eq!(state.tone, HostTone::Connected);
        assert_eq!(state.detail.as_deref(), Some("vps-server 0.1.0"));
    }

    #[test]
    fn a_host_not_checked_yet_says_checking_with_no_detail() {
        let state = host_state(&entry(HostLink::Checking, None), Instant::now());
        assert_eq!(state.word, "Checking");
        assert_eq!(state.tone, HostTone::Checking);
        assert_eq!(state.detail, None);
    }

    #[test]
    fn an_unreachable_host_keeps_the_age_of_its_last_good_answer() {
        let now = Instant::now();
        let answered = now
            .checked_sub(Duration::from_secs(5 * 60))
            .expect("five minutes back");
        let link = HostLink::Unreachable("The host is unreachable: timed out".into());
        let state = host_state(&entry(link, Some(answered)), now);
        assert_eq!(state.word, "Unreachable");
        assert_eq!(state.tone, HostTone::Unreachable);
        assert_eq!(state.detail.as_deref(), Some("Last known state, 5 min ago"));
    }

    #[test]
    fn an_unreachable_host_that_never_answered_gives_the_reason_instead() {
        let reason = "The host is unreachable: timed out";
        let link = HostLink::Unreachable(reason.into());
        let state = host_state(&entry(link, None), Instant::now());
        assert_eq!(state.word, "Unreachable");
        assert_eq!(state.detail.as_deref(), Some(reason));
    }

    #[test]
    fn the_age_is_counted_in_whole_minutes_and_is_at_least_one() {
        assert_eq!(
            last_known(Duration::from_secs(10)),
            "Last known state, 1 min ago"
        );
        assert_eq!(
            last_known(Duration::from_secs(5 * 60 + 59)),
            "Last known state, 5 min ago"
        );
    }

    #[test]
    fn a_host_that_needs_sign_in_or_an_update_is_a_warning_with_the_sentence() {
        let sign_in = host_state(
            &entry(HostLink::SignInNeeded("Pair again.".into()), None),
            Instant::now(),
        );
        assert_eq!(sign_in.word, "Sign-in needed");
        assert_eq!(sign_in.tone, HostTone::Warning);
        assert_eq!(sign_in.detail.as_deref(), Some("Pair again."));

        let update = host_state(
            &entry(HostLink::UpdateNeeded("Update the host.".into()), None),
            Instant::now(),
        );
        assert_eq!(update.word, "Update needed");
        assert_eq!(update.tone, HostTone::Warning);
        assert_eq!(update.detail.as_deref(), Some("Update the host."));
    }

    #[test]
    fn a_failed_host_is_a_failure_with_its_sentence() {
        let state = host_state(
            &entry(
                HostLink::Failed("The host's identity changed.".into()),
                None,
            ),
            Instant::now(),
        );
        assert_eq!(state.word, "Failed");
        assert_eq!(state.tone, HostTone::Failed);
        assert_eq!(
            state.detail.as_deref(),
            Some("The host's identity changed.")
        );
    }

    #[test]
    fn no_state_word_claims_the_host_stopped_or_went_offline() {
        let now = Instant::now();
        let links = [
            HostLink::Checking,
            HostLink::Connected {
                server: "s".into(),
                version: "v".into(),
            },
            HostLink::Unreachable("x".into()),
            HostLink::SignInNeeded("x".into()),
            HostLink::UpdateNeeded("x".into()),
            HostLink::Failed("x".into()),
        ];
        for link in links {
            let word = host_state(&entry(link, None), now).word.to_lowercase();
            for banned in ["stop", "offline", "done"] {
                assert!(!word.contains(banned), "{word} says {banned}");
            }
        }
    }

    #[test]
    fn the_marketplace_lists_this_pc_then_each_paired_host_with_its_state() {
        use crate::model::marketplace::HostState as MarketState;
        use crate::service::ConnectionState;
        let mut checking = entry(HostLink::Checking, None);
        checking.record.name = "nas".into();
        let state = StoreState {
            connection: ConnectionState::Connected {
                server: "massorca".into(),
                version: "0.1.0".into(),
            },
            hosts: vec![
                entry(
                    HostLink::Connected {
                        server: "vps-server".into(),
                        version: "0.1.0".into(),
                    },
                    None,
                ),
                checking,
            ],
            ..StoreState::default()
        };
        let hosts = marketplace_hosts(&state, Instant::now());
        let names: Vec<&str> = hosts.iter().map(|host| host.name.as_str()).collect();
        assert_eq!(names, ["This PC", "vps", "nas"]);
        assert_eq!(hosts[0].state, MarketState::Connected);
        assert_eq!(hosts[1].state, MarketState::Connected);
        assert_eq!(hosts[2].state.word(), "Checking");
    }
}
