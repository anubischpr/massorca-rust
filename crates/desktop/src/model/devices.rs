//! The devices paired with this computer's service (D-PR-02): one row per device with its state and the
//! rule for revoking it, and the flow of a revoke that waits for the service's answer. Nothing changes on
//! screen before the service answers. Free of GPUI.

use massorca_protocol::methods::devices::DeviceInfo;

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
    /// How long ago the device was paired, in words.
    pub paired: String,
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

/// "Paired 5 min ago", "Paired 3 h ago" or "Paired 2 days ago", counted from `now_ms`.
pub fn paired_text(paired_at_ms: i64, now_ms: i64) -> String {
    let minutes = now_ms.saturating_sub(paired_at_ms).max(0) / 60_000;
    match minutes {
        0..=59 => format!("Paired {} min ago", minutes.max(1)),
        60..=1_439 => format!("Paired {} h ago", minutes / 60),
        _ => match minutes / 1_440 {
            1 => "Paired 1 day ago".to_owned(),
            days => format!("Paired {days} days ago"),
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
        paired: paired_text(device.paired_at_ms, now_ms),
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
    fn the_revoke_confirm_names_the_device() {
        assert_eq!(revoke_title("Pixel 8"), "Revoke access for Pixel 8?");
        assert_eq!(
            REVOKE_LOSS,
            "The device loses access to every host on this PC."
        );
    }
}
