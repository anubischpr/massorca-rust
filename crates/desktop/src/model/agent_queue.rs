//! A session's prompt queue as the dock draws it (decision 0084): the service's prompts become the dock's rows, and
//! a row's number leads back to its prompt's id.

use massorca_protocol::methods::agents::{
    QUEUE_REASON_ASKING, QUEUE_REASON_BEHIND_FAILED, QUEUE_REASON_PAUSED_RESTARTED,
    QUEUE_REASON_PAUSED_STOPPED, QueueState, QueuedPrompt,
};
use massorca_ui::components::queued_message::{QueueStatus, QueuedItem, WaitReason};

/// The failure shown when the service gives none.
const FAILED_WITHOUT_REASON: &str = "the delivery failed";

/// The dock's number for a prompt id: stable while the prompt is queued, so an open edit stays on its row.
pub fn row_id(id: &str) -> u64 {
    // FNV-1a: deterministic across runs, unlike the std hasher's contract.
    id.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The dock's rows for a session's queue, head first.
pub fn queued_items(prompts: &[QueuedPrompt]) -> Vec<QueuedItem> {
    prompts
        .iter()
        .map(|prompt| QueuedItem {
            id: row_id(&prompt.id),
            text: prompt.text.clone().into(),
            status: status(prompt),
        })
        .collect()
}

/// The id of the prompt drawn as row `row`.
pub fn prompt_id(prompts: &[QueuedPrompt], row: u64) -> Option<String> {
    prompts
        .iter()
        .find(|prompt| row_id(&prompt.id) == row)
        .map(|prompt| prompt.id.clone())
}

fn status(prompt: &QueuedPrompt) -> QueueStatus {
    match prompt.state {
        QueueState::Sending => QueueStatus::Sending,
        QueueState::Failed => QueueStatus::Failed(
            prompt
                .reason
                .clone()
                .unwrap_or_else(|| FAILED_WITHOUT_REASON.to_owned())
                .into(),
        ),
        // A state from a newer service reads as an ordinary wait; the service refuses what it does not allow.
        QueueState::Waiting | QueueState::Unknown => {
            QueueStatus::Waiting(match prompt.reason.as_deref() {
                Some(QUEUE_REASON_ASKING) => WaitReason::AgentAsking,
                Some(QUEUE_REASON_PAUSED_STOPPED) => WaitReason::PausedByStop,
                Some(QUEUE_REASON_PAUSED_RESTARTED) => WaitReason::PausedByRestart,
                Some(QUEUE_REASON_BEHIND_FAILED) => WaitReason::BehindFailed,
                _ => WaitReason::AgentBusy,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::agents::QUEUE_REASON_BUSY;

    use super::*;

    fn prompt(id: &str, state: QueueState, reason: Option<&str>) -> QueuedPrompt {
        QueuedPrompt {
            id: id.into(),
            text: format!("text {id}"),
            state,
            reason: reason.map(str::to_owned),
            ..crate::service::fixtures::base_queued_prompt()
        }
    }

    #[test]
    fn each_reason_and_state_becomes_the_docks_status() {
        let prompts = [
            prompt("a", QueueState::Waiting, Some(QUEUE_REASON_BUSY)),
            prompt("b", QueueState::Waiting, Some(QUEUE_REASON_ASKING)),
            prompt("c", QueueState::Waiting, Some(QUEUE_REASON_PAUSED_STOPPED)),
            prompt(
                "d",
                QueueState::Waiting,
                Some(QUEUE_REASON_PAUSED_RESTARTED),
            ),
            prompt("e", QueueState::Waiting, Some(QUEUE_REASON_BEHIND_FAILED)),
            prompt("f", QueueState::Sending, None),
            prompt("g", QueueState::Failed, Some("the run exited")),
            prompt("h", QueueState::Failed, None),
            prompt("i", QueueState::Unknown, Some("later")),
        ];
        let statuses: Vec<QueueStatus> = queued_items(&prompts)
            .into_iter()
            .map(|item| item.status)
            .collect();
        assert_eq!(
            statuses,
            [
                QueueStatus::Waiting(WaitReason::AgentBusy),
                QueueStatus::Waiting(WaitReason::AgentAsking),
                QueueStatus::Waiting(WaitReason::PausedByStop),
                QueueStatus::Waiting(WaitReason::PausedByRestart),
                QueueStatus::Waiting(WaitReason::BehindFailed),
                QueueStatus::Sending,
                QueueStatus::Failed("the run exited".into()),
                QueueStatus::Failed(FAILED_WITHOUT_REASON.into()),
                QueueStatus::Waiting(WaitReason::AgentBusy),
            ]
        );
    }

    #[test]
    fn a_rows_number_leads_back_to_its_prompt_and_keeps_its_text() {
        let prompts = [
            prompt("q1", QueueState::Waiting, None),
            prompt("q2", QueueState::Waiting, None),
        ];
        let items = queued_items(&prompts);
        assert_eq!(items[1].text.as_ref(), "text q2");
        assert_ne!(items[0].id, items[1].id);
        assert_eq!(prompt_id(&prompts, items[1].id).as_deref(), Some("q2"));
        assert_eq!(row_id("q2"), items[1].id, "the number is stable");
        assert_eq!(prompt_id(&prompts, 0), None);
    }
}
