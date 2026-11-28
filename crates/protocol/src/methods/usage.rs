//! Usage and limits of each agent account, read on the host that runs the agent (capability `usage/1`, decision 0154).
//!
//! Each host answers for itself only. Off unless the host's setting [`SETTING_ENABLED`] is on: then nothing is read or
//! kept, and every answer says `collection: "off"`. When on, the service keeps one cached reading per account and refreshes it on its
//! own cadence. Reads answer from that cache and never fetch; a reader subscribes to [`TOPIC_USAGE_CHANGED`] and reads
//! again when it fires.

use serde::{Deserialize, Serialize};

/// Params: [`UsageParams`]. Result: [`UsageList`], from the cache.
pub const LIST: &str = "usage.list";
/// Params: [`UsageParams`]. Result: [`UsageList`], after one fetch of each selected account. A fetch is skipped for an
/// account read in the last few seconds, and concurrent refreshes of one account share one fetch.
pub const REFRESH: &str = "usage.refresh";
/// Event topic: an account's usage reading changed, or was dropped because usage was turned off. The id is
/// `<kind>/<account id>`.
pub const TOPIC_USAGE_CHANGED: &str = "usage.changed";
/// The core setting (owner `core`) that turns usage on. A boolean, off by default.
pub const SETTING_ENABLED: &str = "usage.enabled";

/// Selects accounts: every account of every kind with a usage reader when both are absent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// An account id; the kind is then optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

/// Result of `usage.list` and `usage.refresh`: one entry per account, by kind then account creation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageList {
    /// Whether this host collects usage. `off` is an answer of its own, never an empty list of accounts.
    pub collection: Collection,
    /// The accounts; always empty when `collection` is `off`.
    #[serde(default)]
    pub accounts: Vec<AccountUsage>,
}

/// Whether a host collects usage: its [`SETTING_ENABLED`] setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Collection {
    On,
    Off,
    #[serde(other)]
    Unknown,
}

impl UsageList {
    /// The answer of a host that collects usage.
    pub fn on(accounts: Vec<AccountUsage>) -> Self {
        Self {
            collection: Collection::On,
            accounts,
        }
    }

    /// The answer of a host whose collection is off.
    pub fn off() -> Self {
        Self {
            collection: Collection::Off,
            accounts: Vec::new(),
        }
    }
}

/// The last reading of one account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountUsage {
    pub kind: String,
    pub account: String,
    pub label: String,
    pub state: UsageState,
    /// The windows the reading found; empty unless `state` is `ok`.
    #[serde(default)]
    pub windows: Vec<UsageWindow>,
    /// When the service last read the account, successful or not. Absent before the first read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fetched_at_ms: Option<i64>,
    /// The time of the newest record the reading used: how fresh the figures are, not when they were read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at_ms: Option<i64>,
    /// When the service reads the account next on its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_fetch_at_ms: Option<i64>,
    /// Why the last read failed; set only when `state` is `error`. Never holds file contents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub source: UsageSource,
}

/// The state of an account's reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageState {
    /// Not read yet; the service reads it shortly.
    Pending,
    Ok,
    /// The account's folder holds no usage records the reader knows.
    NoData,
    /// The last read failed; `error` says why. The service tries again later, backing off.
    Error,
    /// A state this client does not know.
    #[serde(other)]
    Unknown,
}

/// Where a reading comes from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageSource {
    /// The vendor CLI's own logs and state files in the account's folder; nothing is fetched.
    #[default]
    LocalLogs,
    #[serde(other)]
    Unknown,
}

/// One limit window of an account.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageWindow {
    pub window: WindowKind,
    /// The window's length, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_minutes: Option<u32>,
    /// Percent of the window's limit used, 0 to 100, when the vendor recorded it. Absent means unknown, never 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub used_percent: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at_ms: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at_ms: Option<i64>,
    /// Tokens the account's logs record in the window, when the reader counts them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
    /// The vendor reported the limit as reached in this window.
    #[serde(default)]
    pub limited: bool,
}

/// Which limit a window is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowKind {
    /// The short window: Claude's 5-hour session, Codex's primary limit.
    Session,
    /// The 7-day window.
    Weekly,
    #[serde(other)]
    Unknown,
}

impl AccountUsage {
    /// An account not read yet.
    pub fn pending(kind: &str, account: &str, label: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            account: account.to_owned(),
            label: label.to_owned(),
            state: UsageState::Pending,
            windows: Vec::new(),
            fetched_at_ms: None,
            observed_at_ms: None,
            next_fetch_at_ms: None,
            error: None,
            source: UsageSource::LocalLogs,
        }
    }

    /// The id of this account's [`TOPIC_USAGE_CHANGED`] events.
    pub fn event_id(&self) -> String {
        format!("{}/{}", self.kind, self.account)
    }
}

impl UsageWindow {
    /// A window with nothing known but its kind.
    pub fn new(window: WindowKind) -> Self {
        Self {
            window,
            window_minutes: None,
            used_percent: None,
            started_at_ms: None,
            resets_at_ms: None,
            tokens: None,
            limited: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AccountUsage, UsageList, UsageState, UsageWindow, WindowKind};
    use serde_json::json;

    #[test]
    fn a_reading_has_its_wire_names() {
        let mut usage = AccountUsage::pending("codex", "a-1", "work");
        usage.state = UsageState::Ok;
        usage.fetched_at_ms = Some(2_000);
        usage.windows = vec![UsageWindow {
            window_minutes: Some(300),
            used_percent: Some(12.5),
            resets_at_ms: Some(9_000),
            ..UsageWindow::new(WindowKind::Session)
        }];
        let value = serde_json::to_value(&usage).unwrap();
        assert_eq!(
            value,
            json!({
                "kind": "codex", "account": "a-1", "label": "work", "state": "ok",
                "windows": [{
                    "window": "session", "window_minutes": 300, "used_percent": 12.5,
                    "resets_at_ms": 9000, "limited": false
                }],
                "fetched_at_ms": 2000, "source": "local_logs"
            })
        );
        assert_eq!(
            serde_json::from_value::<AccountUsage>(value).unwrap(),
            usage
        );
        assert_eq!(usage.event_id(), "codex/a-1");
    }

    #[test]
    fn unknown_names_from_a_newer_peer_still_decode() {
        let usage: AccountUsage = serde_json::from_value(json!({
            "kind": "claude", "account": "a-2", "label": "x", "state": "throttled",
            "windows": [{ "window": "monthly" }], "source": "vendor_api"
        }))
        .unwrap();
        assert_eq!(usage.state, UsageState::Unknown);
        assert_eq!(usage.windows[0].window, WindowKind::Unknown);
        assert!(!usage.windows[0].limited);
        assert_eq!(
            serde_json::to_value(UsageList::off()).unwrap(),
            json!({ "collection": "off", "accounts": [] })
        );
    }
}
