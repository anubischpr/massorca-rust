//! Event streams. `events.subscribe` returns a stream id; `event` frames on that id then carry [`ChangeEvent`]s.

use serde::{Deserialize, Serialize};

/// Params: none. Result: [`Subscribed`].
pub const SUBSCRIBE: &str = "events.subscribe";

pub const TOPIC_WORKSPACE_CREATED: &str = "workspace.created";
pub const TOPIC_WORKSPACE_REMOVING: &str = "workspace.removing";
pub const TOPIC_WORKSPACE_REMOVED: &str = "workspace.removed";
pub const TOPIC_PROJECT_ADDED: &str = "project.added";
pub const TOPIC_PROJECT_REMOVED: &str = "project.removed";
/// An agent session was started, changed, stopped or forgotten; the id is the session id.
pub const TOPIC_AGENT_CHANGED: &str = "agent.changed";
/// Topic of `agent.accounts_changed`, the account change of decision 0059; its id is the kind.
pub const TOPIC_ACCOUNTS_CHANGED: &str = "agent.accounts_changed";

/// Result of `events.subscribe`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscribed {
    pub stream: u64,
}

/// One change notification: the topic that fired and the id it concerns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeEvent {
    pub topic: String,
    pub id: String,
}
