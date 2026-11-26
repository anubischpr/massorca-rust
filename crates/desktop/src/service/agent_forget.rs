//! `agent.forget` with the options of capability `agent-archive/1`: forgets a session's record, and with
//! `remove_workspace` also removes its workspace through the workspace-remove path. The protocol's params name only the
//! session, so the desktop builds its own params until the protocol carries these. The agent's own saved conversation in
//! its account home is never touched.

use massorca_client::{Client, ClientError};
use massorca_protocol::methods::agents::{AgentSession, FORGET};
use serde::Serialize;

/// The capability that offers `remove_workspace` and `force` on `agent.forget`. The protocol does not name it yet.
pub const AGENT_ARCHIVE: &str = "agent-archive/1";

/// Params of `agent.forget` with the capability's options. A flag that is false is not sent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ForgetSessionParams {
    pub session: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub remove_workspace: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub force: bool,
}

/// Sends `agent.forget`. The answer is the forgotten record as the service had it.
pub(super) async fn forget(
    client: &Client,
    params: &ForgetSessionParams,
) -> Result<AgentSession, ClientError> {
    client.call(FORGET, params).await
}
