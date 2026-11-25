//! The terminals of a host, read once, so the app can offer one to attach to (decision 0080, screen M-TM-01).

use massorca_protocol::methods::terminals::Terminal;

use crate::error::FfiError;
use crate::massorca::{Host, run_on};
use crate::wire::word;

/// A terminal as the app lists it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TerminalSummary {
    pub id: String,
    pub workspace_id: String,
    /// `live`, `unverifiable` or `exited`; `unknown` for a state this build does not know.
    pub state: String,
    pub title: Option<String>,
    pub cols: u16,
    pub rows: u16,
    /// The program the terminal runs.
    pub shell: String,
    pub cwd: String,
    pub created_at_ms: i64,
    /// Attaching needs host-admin scope because the program got vault secrets.
    pub holds_secrets: bool,
}

impl From<Terminal> for TerminalSummary {
    fn from(terminal: Terminal) -> Self {
        Self {
            id: terminal.id,
            workspace_id: terminal.workspace_id,
            state: word(terminal.state),
            title: terminal.title,
            cols: terminal.cols,
            rows: terminal.rows,
            shell: terminal.shell,
            cwd: terminal.cwd,
            created_at_ms: terminal.created_at_ms,
            holds_secrets: terminal.holds_secrets,
        }
    }
}

#[uniffi::export]
impl Host {
    /// The terminals of every workspace, or only those of `workspace` (its id), live and exited.
    pub async fn terminals(
        &self,
        workspace: Option<String>,
    ) -> Result<Vec<TerminalSummary>, FfiError> {
        let client = self.client.clone();
        let name = self.name.clone();
        run_on(&self.runtime, async move {
            let list = client
                .terminal_list(workspace.as_deref())
                .await
                .map_err(|error| FfiError::from_client(error, &name))?;
            Ok(list
                .terminals
                .into_iter()
                .map(TerminalSummary::from)
                .collect())
        })
        .await
    }
}
