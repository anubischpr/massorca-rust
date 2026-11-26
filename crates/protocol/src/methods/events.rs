//! Event streams. `events.subscribe` returns a stream id; `event` frames on that id then carry [`ChangeEvent`]s.

use serde::{Deserialize, Serialize};

/// Params: none. Result: [`Subscribed`].
pub const SUBSCRIBE: &str = "events.subscribe";

pub const TOPIC_WORKSPACE_CREATED: &str = "workspace.created";
pub const TOPIC_WORKSPACE_REMOVING: &str = "workspace.removing";
pub const TOPIC_WORKSPACE_REMOVED: &str = "workspace.removed";
/// A workspace's name, branch or comment changed (`workspace.update`, `workspace.rename`); the id is the workspace id.
pub const TOPIC_WORKSPACE_UPDATED: &str = "workspace.updated";
pub const TOPIC_PROJECT_ADDED: &str = "project.added";
pub const TOPIC_PROJECT_REMOVED: &str = "project.removed";
/// A project's base was set or cleared; the id is the project id.
pub const TOPIC_PROJECT_CHANGED: &str = "project.changed";
/// An agent session was started, changed, stopped or forgotten; the id is the session id.
pub const TOPIC_AGENT_CHANGED: &str = "agent.changed";
/// A session's prompt queue changed; the id is the session id (decision 0084). Not sent to `agent.watch` streams.
pub const TOPIC_AGENT_QUEUE_CHANGED: &str = "agent.queue_changed";
/// A setting's value changed. The id is `<owner>/<key>`; the event carries no value.
pub const TOPIC_CONFIG_CHANGED: &str = "config.changed";
/// A team, bot or team project changed; the id is the team id.
pub const TOPIC_TEAM_CHANGED: &str = "team.changed";
/// A line was posted to a team's Conference room; the id is the team id (capability `team-room/1`).
pub const TOPIC_TEAM_ROOM_CHANGED: &str = "team.room_changed";
/// A task was created, changed, finished, reopened or removed; the id is the task id.
pub const TOPIC_TASK_CHANGED: &str = "task.changed";
/// A secret was set, rotated, linked, unlinked or removed. The id is `<scope>/<name>`; never a value.
pub const TOPIC_SECRET_CHANGED: &str = "secret.changed";
/// A secret request opened. The id is the request id.
pub const TOPIC_SECRET_REQUESTED: &str = "secret.requested";
/// A secret request left pending. The id is the request id.
pub const TOPIC_SECRET_REQUEST_CHANGED: &str = "secret.request_changed";
/// An extension was installed, enabled, disabled or removed; the id is the package id (decision 0062).
pub const TOPIC_EXTENSION_INSTALLED: &str = "extension.installed";
pub const TOPIC_EXTENSION_ENABLED: &str = "extension.enabled";
pub const TOPIC_EXTENSION_DISABLED: &str = "extension.disabled";
pub const TOPIC_EXTENSION_REMOVED: &str = "extension.removed";

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
/// Topic of `agent.accounts_changed`, the account change of decision 0059; its id is the kind.
pub const TOPIC_ACCOUNTS_CHANGED: &str = "agent.accounts_changed";
