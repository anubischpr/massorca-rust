//! The agent watch: the agent sessions of a host and their changes, followed live (decision 0052, screen STAT-01).

use std::fmt;
use std::sync::Arc;

use massorca_client::{ClientError, StreamItem};
use massorca_protocol::methods::agents::{AgentSession, AgentWatchEvent};

use crate::error::FfiError;
use crate::massorca::Host;
use crate::watch::{Feed, LinkState, Watched};
use crate::wire::word;

/// One agent session as the first screens show it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AgentSessionSummary {
    pub id: String,
    /// The agent kind, as the host names it.
    pub kind: String,
    pub workspace_id: String,
    pub title: Option<String>,
    /// What the agent reports it is doing: `working`, `waiting`, `idle` or `done`. Absent until it reports.
    pub activity_state: Option<String>,
    /// `live`, `unverifiable` or `exited`. `unverifiable` means the host cannot prove the run is alive right now; it is
    /// never read as dead.
    pub execution_state: String,
    /// The latest time the host recorded for the session: its newest activity or exit, else its creation. Milliseconds
    /// on the host's clock.
    pub updated_at_ms: i64,
}

impl From<AgentSession> for AgentSessionSummary {
    fn from(session: AgentSession) -> Self {
        let updated_at_ms = session
            .activity
            .as_ref()
            .map(|activity| activity.at_ms)
            .into_iter()
            .chain(session.execution.exit.as_ref().map(|exit| exit.at_ms))
            .fold(session.created_at_ms, i64::max);
        Self {
            id: session.id,
            kind: session.kind,
            workspace_id: session.workspace_id,
            title: session.title,
            activity_state: session.activity.map(|activity| word(activity.state)),
            execution_state: word(session.execution.state),
            updated_at_ms,
        }
    }
}

/// What the host reports about the sessions: every selected session first, then each change.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AgentItem {
    /// Every selected session: the first item, and again after the watch fell behind.
    Snapshot { sessions: Vec<AgentSessionSummary> },
    /// One session changed; its state as of the change.
    Changed { session: AgentSessionSummary },
}

/// One step of an agent watch: an item from the host, or a change of link.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum AgentEvent {
    Item { item: AgentItem },
    Link { link: LinkState },
}

impl From<LinkState> for AgentEvent {
    fn from(link: LinkState) -> Self {
        Self::Link { link }
    }
}

/// A live view of the agent sessions of a host. `close`, or dropping it, ends the connection.
#[derive(uniffi::Object)]
pub struct AgentWatch {
    feed: Feed<AgentEvent>,
}

impl fmt::Debug for AgentWatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentWatch").finish_non_exhaustive()
    }
}

#[uniffi::export]
impl AgentWatch {
    /// The next event. `Ok(None)` once the watch is closed or the host ended it. An `Err` is final: the watch is over.
    pub async fn next(&self) -> Result<Option<AgentEvent>, FfiError> {
        self.feed.next().await
    }

    /// Ends the watch and its connection. A `next` in progress returns `None`.
    pub fn close(&self) {
        self.feed.close();
    }
}

#[uniffi::export]
impl Host {
    /// Follows the agent sessions of the host, or only those of `workspace` (its id). The first item is every selected
    /// session.
    pub fn watch_agents(&self, workspace: Option<String>) -> Arc<AgentWatch> {
        Arc::new(AgentWatch {
            feed: self.follow(Watched::Agents { workspace }, agent_event),
        })
    }
}

/// Reads one `agent.watch` item. An item of a kind this build does not know is skipped, so a newer host does not stop
/// the watch.
fn agent_event(item: StreamItem) -> Result<Option<AgentEvent>, ClientError> {
    let StreamItem::Event(value) = item else {
        return Ok(None);
    };
    let event: AgentWatchEvent =
        serde_json::from_value(value).map_err(|error| ClientError::Protocol(error.to_string()))?;
    let item = match event {
        AgentWatchEvent::Snapshot { sessions } => AgentItem::Snapshot {
            sessions: sessions
                .into_iter()
                .map(AgentSessionSummary::from)
                .collect(),
        },
        AgentWatchEvent::Changed { session } => AgentItem::Changed {
            session: AgentSessionSummary::from(*session),
        },
        AgentWatchEvent::Unknown => return Ok(None),
    };
    Ok(Some(AgentEvent::Item { item }))
}
