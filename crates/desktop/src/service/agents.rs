//! The agent sessions the bridge reads (decisions 0050 to 0057). The list comes on each connect and
//! each `agent.changed` event refetches the session it names, so the store follows the service without
//! optimistic updates.

use massorca_client::{Client, ClientError};
use massorca_protocol::methods::agents::AgentSession;

/// Lists every agent session. A service that predates agents answers with a remote error, which lists
/// none; a lost link is passed on so the session ends and reconnects.
pub(super) async fn list_sessions(client: &Client) -> Result<Vec<AgentSession>, ClientError> {
    match client.agent_list(None).await {
        Ok(list) => Ok(list.sessions),
        Err(ClientError::Remote(_)) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}
