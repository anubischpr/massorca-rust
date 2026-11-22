//! The state one connection shares between its request tasks and stream forwarders.

use std::collections::HashSet;
use std::sync::Arc;

use massorca_protocol::Frame;
use tokio::sync::mpsc;

use crate::context::AppState;
use crate::permission::Principal;
use crate::streams::StreamTable;

pub(crate) struct Session {
    pub(crate) state: Arc<AppState>,
    pub(crate) connection_id: String,
    /// The capabilities this connection's `welcome` offered; methods of any other capability are refused.
    pub(crate) capabilities: HashSet<String>,
    /// The capabilities the client listed in its `hello`, shared by the request contexts of this connection.
    pub(crate) client_capabilities: Arc<HashSet<String>>,
    /// Who the connection acts as, decided once at the handshake.
    pub(crate) principal: Principal,
    pub(crate) outbound: mpsc::Sender<Frame>,
    /// The largest frame body this connection's transport carries.
    pub(crate) max_frame_len: u32,
    pub(crate) streams: StreamTable,
}
