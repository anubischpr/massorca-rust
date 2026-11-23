//! The devices paired with this computer's service (D-PR-02): one row per device with its state and the
//! rule for revoking it, and the flow of a revoke that waits for the service's answer. Nothing changes on
//! screen before the service answers. Free of GPUI.

use massorca_protocol::methods::devices::{DeviceInfo, scopes};

use crate::service::Refusal;

/// The loss the revoke confirm lists: what the device can no longer do.
pub const REVOKE_LOSS: &str = "The device loses access to every host on this PC.";

/// The state word of a device row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceState {
    Active,
    Revoked,
}

impl DeviceState {
    pub fn word(self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Revoked => "Revoked",
        }
    }
}

/// One paired device as its row shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceRow {
    pub id: String,
    pub name: String,
    pub state: DeviceState,
    /// How long ago the device was paired and, when the service has seen it since, last seen, in words.
    pub paired: String,
    /// Shown only for a device with less than full access, naming what it holds: "Limited: read, workspace".
    pub limited: Option<String>,
    /// Whether the row offers Revoke: an active device, and not the one this computer runs as.
    pub revocable: bool,
}

/// What the Paired devices list shows: still being read, listed, or refused by the service.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DevicesView {
    Reading,
    Listed(Vec<DeviceRow>),
    Refused(String),
}

/// The list as the store holds it at `now_ms`.
pub fn devices_view(
    devices: Option<&Result<Vec<DeviceInfo>, Refusal>>,
    now_ms: i64,
) -> DevicesView {
    match devices {
        None => DevicesView::Reading,
        Some(Ok(list)) => DevicesView::Listed(
            list.iter()
                .map(|device| device_row(device, now_ms))
                .collect(),
        ),
        Some(Err(refusal)) => DevicesView::Refused(refusal.sentence()),
    }
}

/// The scopes a paired device holds with full access: the owner on another device.
pub const FULL_ACCESS: [&str; 4] = [
    scopes::READ,
    scopes::WORKSPACE,
    scopes::HOST_ADMIN,
    scopes::DEVICE_ADMIN,
];

/// "Paired 5 min ago", "Paired 3 h ago" or "Paired 2 days ago", counted from `now_ms`.
pub fn paired_text(paired_at_ms: i64, now_ms: i64) -> String {
    ago_text("Paired", paired_at_ms, now_ms)
}

/// The row's helper: when it was paired and, only when the service reports it, when it was last seen.
pub fn seen_text(paired_at_ms: i64, last_seen_ms: Option<i64>, now_ms: i64) -> String {
    let paired = paired_text(paired_at_ms, now_ms);
    match last_seen_ms {
        Some(seen) => format!("{paired} · {}", ago_text("Last seen", seen, now_ms)),
        None => paired,
    }
}

/// The label of a device with less than full access, or none for full access.
pub fn limited_label(held: &[String]) -> Option<String> {
    if FULL_ACCESS
        .iter()
        .all(|scope| held.iter().any(|h| h == scope))
    {
        return None;
    }
    if held.is_empty() {
        return Some("Limited: no access".to_owned());
    }
    Some(format!("Limited: {}", held.join(", ")))
}

fn ago_text(what: &str, at_ms: i64, now_ms: i64) -> String {
    let minutes = now_ms.saturating_sub(at_ms).max(0) / 60_000;
    match minutes {
        0..=59 => format!("{what} {} min ago", minutes.max(1)),
        60..=1_439 => format!("{what} {} h ago", minutes / 60),
        _ => match minutes / 1_440 {
            1 => format!("{what} 1 day ago"),
            days => format!("{what} {days} days ago"),
        },
    }
}

/// A revoke on screen: the device, whether the service is still answering, and its last refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevokeFlow {
    pub device: String,
    pub name: String,
    pub busy: bool,
    pub error: Option<String>,
}

impl RevokeFlow {
    pub fn new(row: &DeviceRow) -> Self {
        Self {
            device: row.id.clone(),
            name: row.name.clone(),
            busy: false,
            error: None,
        }
    }
}

/// The confirm's title, naming the device: "Revoke access for Pixel 8?".
pub fn revoke_title(name: &str) -> String {
    format!("Revoke access for {name}?")
}

fn device_row(device: &DeviceInfo, now_ms: i64) -> DeviceRow {
    let state = if device.revoked_at_ms.is_some() {
        DeviceState::Revoked
    } else {
        DeviceState::Active
    };
    DeviceRow {
        id: device.id.clone(),
        name: device.name.clone(),
        state,
        paired: seen_text(device.paired_at_ms, device.last_seen_ms, now_ms),
        limited: limited_label(&device.scopes),
        revocable: state == DeviceState::Active && !device.current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, revoked: bool, current: bool) -> DeviceInfo {
        DeviceInfo {
            id: id.into(),
            name: "Pixel 8".into(),
            scopes: vec!["read".into()],
            key_pin: "sha256:test".into(),
            paired_at_ms: 0,
            last_seen_ms: None,
            revoked_at_ms: revoked.then_some(1),
            current,
        }
    }

    #[test]
    fn a_revoked_device_says_revoked_and_is_not_offered_revoke_again() {
        let view = devices_view(Some(&Ok(vec![device("d1", true, false)])), 0);
        let DevicesView::Listed(rows) = view else {
            panic!("the list is shown");
        };
        assert_eq!(rows[0].state.word(), "Revoked");
        assert!(!rows[0].revocable);
    }

    #[test]
    fn an_active_device_other_than_this_one_can_be_revoked() {
        let view = devices_view(Some(&Ok(vec![device("d1", false, false)])), 0);
        let DevicesView::Listed(rows) = view else {
            panic!("the list is shown");
        };
        assert_eq!(rows[0].state.word(), "Active");
        assert!(rows[0].revocable);
    }

    #[test]
    fn the_device_this_computer_runs_as_is_not_offered_revoke() {
        let view = devices_view(Some(&Ok(vec![device("me", false, true)])), 0);
        let DevicesView::Listed(rows) = view else {
            panic!("the list is shown");
        };
        assert!(!rows[0].revocable);
    }

    #[test]
    fn an_unread_list_says_reading_and_a_refusal_keeps_its_sentence() {
        assert_eq!(devices_view(None, 0), DevicesView::Reading);
        let refusal = Refusal::unavailable("the service is not running");
        let sentence = refusal.sentence();
        assert_eq!(
            devices_view(Some(&Err(refusal)), 0),
            DevicesView::Refused(sentence)
        );
    }

    #[test]
    fn paired_text_counts_minutes_hours_and_days() {
        let minute = 60_000;
        assert_eq!(paired_text(0, 10 * minute), "Paired 10 min ago");
        assert_eq!(paired_text(0, 0), "Paired 1 min ago");
        assert_eq!(paired_text(0, 3 * 60 * minute), "Paired 3 h ago");
        assert_eq!(paired_text(0, 24 * 60 * minute), "Paired 1 day ago");
        assert_eq!(paired_text(0, 2 * 24 * 60 * minute), "Paired 2 days ago");
    }

    #[test]
    fn a_device_with_full_access_has_no_label_and_a_limited_one_names_its_scopes() {
        let full: Vec<String> = FULL_ACCESS.iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(limited_label(&full), None);
        let limited = vec!["read".to_owned(), "workspace".to_owned()];
        assert_eq!(
            limited_label(&limited).as_deref(),
            Some("Limited: read, workspace")
        );
        assert_eq!(limited_label(&[]).as_deref(), Some("Limited: no access"));
    }

    #[test]
    fn last_seen_shows_only_when_the_service_reports_it() {
        let minute = 60_000;
        assert_eq!(seen_text(0, None, 10 * minute), "Paired 10 min ago");
        assert_eq!(
            seen_text(0, Some(8 * minute), 10 * minute),
            "Paired 10 min ago · Last seen 2 min ago"
        );
        let mut seen = device("d1", false, false);
        seen.last_seen_ms = Some(9 * minute);
        let DevicesView::Listed(rows) = devices_view(Some(&Ok(vec![seen])), 10 * minute) else {
            panic!("the list is shown");
        };
        assert_eq!(rows[0].paired, "Paired 10 min ago · Last seen 1 min ago");
        assert_eq!(rows[0].limited.as_deref(), Some("Limited: read"));
    }

    #[test]
    fn the_revoke_confirm_names_the_device() {
        assert_eq!(revoke_title("Pixel 8"), "Revoke access for Pixel 8?");
        assert_eq!(
            REVOKE_LOSS,
            "The device loses access to every host on this PC."
        );
    }
}
