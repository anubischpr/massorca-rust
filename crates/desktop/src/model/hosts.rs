//! The paired hosts as the Settings page and the sidebar show them (D-HO-01, D-HO-07, X-CN-01). A host
//! shows the state its last check left, with the age of its last good answer. No word says "stopped",
//! "offline" or "done": losing contact is not proof that the host stopped. Free of GPUI, so the rules
//! test without a window.

use std::time::{Duration, Instant};

use crate::model::marketplace::{Host, HostState as MarketHostState, HostTarget};
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
        HostLink::Reconnecting {
            reason,
            attempt,
            retry_at,
        } => {
            let known = match entry.last_connected {
                Some(at) => last_known(now.saturating_duration_since(at)),
                None => reason.trim_end_matches('.').to_owned(),
            };
            let wait = retry_at.saturating_duration_since(now);
            let next = if wait.is_zero() {
                "retrying now".to_owned()
            } else {
                format!("retrying in {} s", wait.as_secs_f64().ceil())
            };
            (
                "Unreachable",
                HostTone::Unreachable,
                Some(format!("{known}. Attempt {attempt}, {next}.")),
            )
        }
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

impl HostState {
    /// Whether the row offers "Retry now": the host is out of reach, or failed in a way a later try may fix.
    pub fn offers_retry(&self) -> bool {
        matches!(self.tone, HostTone::Unreachable | HostTone::Failed)
    }
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

/// The hosts the Marketplace lists while a service answers: This PC first, then each connected paired host.
pub fn marketplace_hosts(state: &StoreState) -> Vec<Host> {
    let this_pc = if state.connection.is_connected() {
        MarketHostState::Connected
    } else {
        MarketHostState::Unreachable
    };
    let mut hosts = vec![Host {
        id: HostTarget::Local.id(),
        name: LOCAL_HOST.to_owned(),
        state: this_pc,
    }];
    hosts.extend(connected_hosts(state).map(|entry| Host {
        id: HostTarget::Paired(entry.record.name.clone()).id(),
        name: entry.record.name.clone(),
        state: MarketHostState::Connected,
    }));
    hosts
}

/// The targets the Marketplace can be aimed at while a service answers: This computer, then each connected paired host.
pub fn offered_targets(state: &StoreState) -> Vec<HostTarget> {
    std::iter::once(HostTarget::Local)
        .chain(connected_hosts(state).map(|entry| HostTarget::Paired(entry.record.name.clone())))
        .collect()
}

/// The paired hosts whose last check reached them; only these are offered to the Marketplace.
fn connected_hosts(state: &StoreState) -> impl Iterator<Item = &HostEntry> {
    state
        .hosts
        .iter()
        .filter(|entry| matches!(entry.link, HostLink::Connected { .. }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::marketplace::HostId;
    use massorca_client::HostRecord;

    fn entry(link: HostLink, last_connected: Option<Instant>) -> HostEntry {
        HostEntry {
            record: HostRecord {
                addresses: vec!["100.64.0.10:7347".into()],
                scopes: vec!["read".into()],
                ..crate::service::fixtures::base_host_record()
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
    fn a_reconnecting_host_is_unreachable_with_its_age_and_the_next_attempt() {
        let now = Instant::now();
        let answered = now
            .checked_sub(Duration::from_secs(5 * 60))
            .expect("five minutes back");
        let link = HostLink::Reconnecting {
            reason: "The host is unreachable: refused".into(),
            attempt: 3,
            retry_at: now + Duration::from_millis(7_200),
        };
        let state = host_state(&entry(link.clone(), Some(answered)), now);
        assert_eq!(state.word, "Unreachable");
        assert_eq!(state.tone, HostTone::Unreachable);
        assert_eq!(
            state.detail.as_deref(),
            Some("Last known state, 5 min ago. Attempt 3, retrying in 8 s.")
        );
        assert!(state.offers_retry());

        let never = host_state(&entry(link, None), now);
        assert_eq!(
            never.detail.as_deref(),
            Some("The host is unreachable: refused. Attempt 3, retrying in 8 s.")
        );
        let due = HostLink::Reconnecting {
            reason: "The host is unreachable: refused.".into(),
            attempt: 4,
            retry_at: now,
        };
        assert_eq!(
            host_state(&entry(due, None), now).detail.as_deref(),
            Some("The host is unreachable: refused. Attempt 4, retrying now.")
        );
    }

    #[test]
    fn retry_now_is_offered_only_to_a_host_out_of_reach_or_failed() {
        let now = Instant::now();
        let offered = |link: HostLink| host_state(&entry(link, None), now).offers_retry();
        assert!(offered(HostLink::Unreachable("x".into())));
        assert!(offered(HostLink::Failed(
            "The host's identity changed.".into()
        )));
        assert!(!offered(HostLink::Checking));
        assert!(!offered(HostLink::Connected {
            server: "vps-server".into(),
            version: "0.1.0".into(),
        }));
        assert!(!offered(HostLink::SignInNeeded("Pair again.".into())));
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
            HostLink::Reconnecting {
                reason: "x".into(),
                attempt: 1,
                retry_at: now,
            },
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

    fn connected_host(name: &str) -> HostEntry {
        let mut host = entry(
            HostLink::Connected {
                server: "vps-server".into(),
                version: "0.1.0".into(),
            },
            None,
        );
        host.record.name = name.into();
        host
    }

    fn state_with(hosts: Vec<HostEntry>) -> StoreState {
        use crate::service::ConnectionState;
        StoreState {
            connection: ConnectionState::Connected {
                server: "massorca".into(),
                version: "0.1.0".into(),
            },
            hosts,
            ..StoreState::default()
        }
    }

    fn id_of(hosts: &[Host], name: &str) -> HostId {
        hosts
            .iter()
            .find(|host| host.name == name)
            .expect("the host is listed")
            .id
            .clone()
    }

    #[test]
    fn the_marketplace_lists_this_pc_then_each_connected_paired_host() {
        use crate::model::marketplace::HostState as MarketState;
        let mut checking = entry(HostLink::Checking, None);
        checking.record.name = "nas".into();
        let state = state_with(vec![connected_host("vps"), checking]);
        let hosts = marketplace_hosts(&state);
        let names: Vec<&str> = hosts.iter().map(|host| host.name.as_str()).collect();
        assert_eq!(names, ["This PC", "vps"]);
        assert_eq!(hosts[0].state, MarketState::Connected);
        assert_eq!(hosts[1].state, MarketState::Connected);
    }

    #[test]
    fn a_paired_host_keeps_its_key_when_another_host_sorts_before_it() {
        let vps = HostTarget::Paired("vps".to_owned()).id();
        let alone = marketplace_hosts(&state_with(vec![connected_host("vps")]));
        let sorted_after = marketplace_hosts(&state_with(vec![
            connected_host("aaa"),
            connected_host("vps"),
        ]));
        assert_eq!(id_of(&alone, "vps"), vps);
        assert_eq!(id_of(&sorted_after, "vps"), vps);
    }

    #[test]
    fn only_connected_hosts_are_offered_and_this_computer_comes_first() {
        let mut gone = entry(HostLink::Unreachable("timed out".into()), None);
        gone.record.name = "gone".into();
        let state = state_with(vec![connected_host("vps"), gone]);
        assert_eq!(
            offered_targets(&state),
            vec![HostTarget::Local, HostTarget::Paired("vps".to_owned())]
        );
    }
}
