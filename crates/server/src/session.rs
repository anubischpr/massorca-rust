//! The state one connection shares between its request tasks and stream forwarders.

use std::collections::HashSet;
use std::sync::Arc;

use massorca_protocol::Message;
use tokio::sync::mpsc;

use crate::context::AppState;
use crate::streams::StreamTable;

pub(crate) struct Session {
    pub(crate) state: Arc<AppState>,
    pub(crate) connection_id: String,
    /// The capabilities this connection's `welcome` offered; methods of any other capability are refused.
    pub(crate) capabilities: HashSet<String>,
    pub(crate) outbound: mpsc::Sender<Message>,
    pub(crate) streams: StreamTable,
}
