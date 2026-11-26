//! `usage.list` and `usage.refresh` (capability `usage/1`, decision 0154): each agent account's usage, read on this host
//! from its local logs into one cache that a refresher fills while `usage.enabled` is on. Reads never fetch.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use massorca_core_agents::paths::valid_account_home;
use massorca_core_agents::usage;
use massorca_core_store::{AgentAccountRow, now_ms};
use massorca_protocol::methods::events::{ChangeEvent, TOPIC_CONFIG_CHANGED};
use massorca_protocol::methods::usage::{
    self as wire, AccountUsage, SETTING_ENABLED, TOPIC_USAGE_CHANGED, UsageList, UsageParams,
    UsageState,
};
use massorca_protocol::{ErrorBody, capabilities, codes};
use tokio::sync::Notify;
use tokio::time::MissedTickBehavior;

use crate::context::{AppState, Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;
use crate::shutdown::ShutdownSignal;
use crate::store_blocking;

/// How often an account is read when its last read worked.
const INTERVAL_MS: i64 = 60_000;
/// The longest wait after failed reads; each failure doubles the wait from `INTERVAL_MS` up to it.
const MAX_BACKOFF_MS: i64 = 15 * 60_000;
/// A refresh this soon after a fetch answers the cache.
const MIN_GAP_MS: i64 = 5_000;
/// How often the refresher looks for due accounts.
const TICK: Duration = Duration::from_secs(5);

type Key = (String, String);

/// The cached readings, by kind and account, and the per-account fetch gates.
#[derive(Debug, Default)]
pub(crate) struct UsageBook {
    entries: Mutex<BTreeMap<Key, Entry>>,
    gates: Mutex<HashMap<Key, Arc<tokio::sync::Mutex<()>>>>,
    /// Wakes the refresher early: an account was seen for the first time, or the setting changed.
    wake: Notify,
}

#[derive(Debug)]
struct Entry {
    usage: AccountUsage,
    failures: u32,
}

impl UsageBook {
    fn entries(&self) -> MutexGuard<'_, BTreeMap<Key, Entry>> {
        self.entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn gate(&self, key: &Key) -> Arc<tokio::sync::Mutex<()>> {
        let mut gates = self
            .gates
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Arc::clone(gates.entry(key.clone()).or_default())
    }

    /// The cached reading of `row`, or a pending one that the refresher is woken to read.
    fn snapshot(&self, row: &AgentAccountRow) -> AccountUsage {
        let key = (row.kind.clone(), row.id.clone());
        let mut entries = self.entries();
        if let Some(entry) = entries.get_mut(&key) {
            entry.usage.label.clone_from(&row.label);
            return entry.usage.clone();
        }
        let pending = AccountUsage::pending(&row.kind, &row.id, &row.label);
        entries.insert(
            key,
            Entry {
                usage: pending.clone(),
                failures: 0,
            },
        );
        drop(entries);
        self.wake.notify_one();
        pending
    }

    /// Drops every reading whose account is not in `keep`, returning the dropped event ids.
    fn retain(&self, keep: &HashSet<Key>) -> Vec<String> {
        let mut dropped = Vec::new();
        self.entries().retain(|key, entry| {
            let kept = keep.contains(key);
            if !kept {
                dropped.push(entry.usage.event_id());
            }
            kept
        });
        self.gates
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retain(|key, _| keep.contains(key));
        dropped
    }

    /// Whether `key` has no reading or its next fetch is due at `now`.
    fn due(&self, key: &Key, now: i64) -> bool {
        self.entries().get(key).is_none_or(|entry| {
            entry.usage.state == UsageState::Pending
                || entry.usage.next_fetch_at_ms.is_none_or(|next| next <= now)
        })
    }

    fn fetched_within(&self, key: &Key, now: i64, gap: i64) -> bool {
        self.entries()
            .get(key)
            .and_then(|entry| entry.usage.fetched_at_ms)
            .is_some_and(|at| now - at < gap)
    }

    /// Stores a fetch's outcome; returns whether anything a reader shows changed.
    fn record(
        &self,
        row: &AgentAccountRow,
        outcome: Result<Option<usage::UsageReading>, String>,
        now: i64,
    ) -> bool {
        let key = (row.kind.clone(), row.id.clone());
        let mut entries = self.entries();
        let entry = entries.entry(key).or_insert_with(|| Entry {
            usage: AccountUsage::pending(&row.kind, &row.id, &row.label),
            failures: 0,
        });
        let before = shown(&entry.usage);
        let usage = &mut entry.usage;
        usage.label.clone_from(&row.label);
        usage.fetched_at_ms = Some(now);
        match outcome {
            Ok(reading) => {
                usage.state = if reading.is_some() {
                    UsageState::Ok
                } else {
                    UsageState::NoData
                };
                let reading = reading.unwrap_or(usage::UsageReading {
                    source: usage.source,
                    windows: Vec::new(),
                    observed_at_ms: None,
                });
                usage.source = reading.source;
                usage.windows = reading.windows;
                usage.observed_at_ms = reading.observed_at_ms;
                usage.error = None;
                entry.failures = 0;
            }
            Err(error) => {
                // The last good figures are dropped: an error state never shows stale numbers as current.
                usage.state = UsageState::Error;
                usage.windows.clear();
                usage.error = Some(error);
                entry.failures = entry.failures.saturating_add(1);
            }
        }
        entry.usage.next_fetch_at_ms = Some(now + wait_ms(entry.failures));
        before != shown(&entry.usage)
    }
}

/// The wait before the next fetch after `failures` failures in a row.
fn wait_ms(failures: u32) -> i64 {
    let factor = 1_i64 << failures.min(10);
    INTERVAL_MS.saturating_mul(factor).min(MAX_BACKOFF_MS)
}

/// What a reader shows: the reading without its fetch times.
fn shown(usage: &AccountUsage) -> AccountUsage {
    AccountUsage {
        fetched_at_ms: None,
        next_fetch_at_ms: None,
        ..usage.clone()
    }
}

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![
        registries
            .capabilities
            .register(capabilities::USAGE, Owner::Core)?,
        registries.methods.register(
            Owner::Core,
            wire::LIST,
            capabilities::USAGE,
            Permission::Read,
            |ctx: RequestCtx, params: Option<UsageParams>| async move {
                list(&ctx.state, &params.unwrap_or_default()).await
            },
        )?,
        registries.methods.register(
            Owner::Core,
            wire::REFRESH,
            capabilities::USAGE,
            Permission::Read,
            |ctx: RequestCtx, params: Option<UsageParams>| async move {
                refresh(&ctx.state, &params.unwrap_or_default()).await
            },
        )?,
    ])
}

async fn list(state: &AppState, params: &UsageParams) -> Result<UsageList, ErrorBody> {
    if !enabled(state).await? {
        return Ok(UsageList::off());
    }
    let rows = selected(state, params).await?;
    Ok(UsageList::on(
        rows.iter().map(|row| state.usage.snapshot(row)).collect(),
    ))
}

async fn refresh(state: &AppState, params: &UsageParams) -> Result<UsageList, ErrorBody> {
    if !enabled(state).await? {
        return Ok(UsageList::off());
    }
    for row in selected(state, params).await? {
        fetch(state, &row, MIN_GAP_MS).await;
    }
    list(state, params).await
}

/// Whether the `usage.enabled` setting is on.
async fn enabled(state: &AppState) -> Result<bool, ErrorBody> {
    let stored = store_blocking::run(state, |store| {
        store.get_setting(Owner::Core.to_string().as_str(), SETTING_ENABLED)
    })
    .await?;
    Ok(stored.is_some_and(|json| json.trim() == "true"))
}

/// The accounts of kinds with a usage reader, filtered by `params`; an account asked for by id must exist.
async fn selected(
    state: &AppState,
    params: &UsageParams,
) -> Result<Vec<AgentAccountRow>, ErrorBody> {
    let rows = accounts(state).await?;
    let picked: Vec<AgentAccountRow> = rows
        .into_iter()
        .filter(|row| params.kind.as_ref().is_none_or(|kind| *kind == row.kind))
        .filter(|row| {
            params
                .account
                .as_ref()
                .is_none_or(|account| *account == row.id)
        })
        .collect();
    if let Some(account) = &params.account
        && picked.is_empty()
    {
        return Err(ErrorBody::new(
            codes::REQUEST_INVALID_PARAMS,
            format!("no account {account} of a kind with usage"),
        ));
    }
    Ok(picked)
}

async fn accounts(state: &AppState) -> Result<Vec<AgentAccountRow>, ErrorBody> {
    let rows = store_blocking::run(state, |store| store.list_all_agent_accounts()).await?;
    let mut rows: Vec<AgentAccountRow> = rows
        .into_iter()
        .filter(|row| usage::supports(&row.kind))
        .collect();
    rows.sort_by(|a, b| (&a.kind, a.created_at, &a.id).cmp(&(&b.kind, b.created_at, &b.id)));
    Ok(rows)
}

/// Reads `row`'s usage unless another fetch of it ran within `gap`, and announces a change. One fetch per account at a
/// time: a caller that waited for another's fetch finds it within the gap and answers the cache.
async fn fetch(state: &AppState, row: &AgentAccountRow, gap: i64) {
    let key = (row.kind.clone(), row.id.clone());
    let gate = state.usage.gate(&key);
    let _held = gate.lock().await;
    if state.usage.fetched_within(&key, now_ms(), gap) {
        return;
    }
    let home = valid_account_home(state.profile.dir(), &row.kind, &row.id);
    let kind = row.kind.clone();
    let work = state.activity.enter();
    let outcome = tokio::task::spawn_blocking(move || {
        let _work = work;
        let Some(home) = home else {
            return Err("the account id is not valid".to_owned());
        };
        // The error names its kind only: never a path or file contents.
        usage::read(&kind, &home, now_ms())
            .map_err(|error| format!("the account's logs could not be read: {}", error.kind()))
    })
    .await
    .unwrap_or_else(|error| Err(format!("the usage read did not finish: {error}")));
    if state.usage.record(row, outcome, now_ms()) {
        publish(state, format!("{}/{}", row.kind, row.id));
    }
}

fn publish(state: &AppState, id: String) {
    state.events.publish(ChangeEvent {
        topic: TOPIC_USAGE_CHANGED.to_owned(),
        id,
    });
}

/// The refresher: while usage is on, fetches each account when it is due, one at a time; while it is off, keeps nothing.
/// Wakes on its tick, on a newly seen account and on a change of the setting.
pub(crate) async fn follow_usage(state: Arc<AppState>, mut signal: ShutdownSignal) {
    let mut events = state.events.subscribe();
    let mut ticks = tokio::time::interval(TICK);
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let setting_id = format!("{}/{SETTING_ENABLED}", Owner::Core);
    loop {
        tokio::select! {
            biased;
            () = signal.wait() => return,
            _ = ticks.tick() => {}
            () = state.usage.wake.notified() => {}
            event = events.next() => match event {
                Some(event) if event.topic == TOPIC_CONFIG_CHANGED && event.id == setting_id => {}
                Some(_) => continue,
                None => return,
            },
        }
        sweep(&state).await;
    }
}

async fn sweep(state: &AppState) {
    let on = match enabled(state).await {
        Ok(on) => on,
        Err(error) => {
            tracing::warn!(code = %error.code, "the usage setting could not be read: {}", error.message);
            return;
        }
    };
    let rows = if on {
        match accounts(state).await {
            Ok(rows) => rows,
            Err(error) => {
                tracing::warn!(code = %error.code, "the accounts could not be listed for usage: {}", error.message);
                return;
            }
        }
    } else {
        Vec::new()
    };
    let keep: HashSet<Key> = rows
        .iter()
        .map(|row| (row.kind.clone(), row.id.clone()))
        .collect();
    for id in state.usage.retain(&keep) {
        publish(state, id);
    }
    for row in &rows {
        let key = (row.kind.clone(), row.id.clone());
        if state.usage.due(&key, now_ms()) {
            fetch(state, row, 0).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{INTERVAL_MS, MAX_BACKOFF_MS, wait_ms};

    #[test]
    fn failures_back_off_up_to_the_cap() {
        assert_eq!(wait_ms(0), INTERVAL_MS);
        assert_eq!(wait_ms(1), 2 * INTERVAL_MS);
        assert_eq!(wait_ms(3), 8 * INTERVAL_MS);
        assert_eq!(wait_ms(4), MAX_BACKOFF_MS);
        assert_eq!(wait_ms(u32::MAX), MAX_BACKOFF_MS);
    }
}
