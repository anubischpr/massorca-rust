//! Host methods: figures read on the host that runs the service (capability `host/1`, decision 0081).
//!
//! Sampled when asked; nothing is pushed. A client polls while it shows them.

use serde::{Deserialize, Serialize};

/// Params: none. Result: [`HostMemory`].
pub const MEMORY: &str = "host.memory";

/// What the byte counts measure on the answering host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryMetric {
    /// Windows: pages of the process now in physical memory, shared ones included.
    WorkingSet,
    /// Linux and macOS: the resident set size, shared pages included.
    Resident,
}

/// Result of `host.memory`: the service process plus every process of each live terminal (agent runs included).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostMemory {
    pub metric: MemoryMetric,
    /// The service plus every terminal that is not `unverifiable`, as a plain sum: a page shared by two processes
    /// counts twice. A `partial` terminal counts the processes it read.
    pub total_bytes: u64,
    pub service: ProcessMemory,
    /// Live terminals, by creation time.
    pub terminals: Vec<TerminalMemory>,
    /// Processes that were listed but could not be read (they exited, or access was denied). Not in any sum.
    pub unreadable: u32,
    pub sampled_at_ms: i64,
}

/// One process and its memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessMemory {
    pub pid: u32,
    pub bytes: u64,
}

/// One live terminal's processes and their memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalMemory {
    pub terminal_id: String,
    pub workspace_id: String,
    /// The agent session whose current run uses this terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// How many of the terminal's processes were read.
    pub processes: u32,
    pub bytes: u64,
    /// The terminal's processes could not be listed or none could be read: its memory is unknown, not zero. Then
    /// `processes` and `bytes` are 0 and must not be shown as a size, and the terminal is not in `total_bytes`.
    #[serde(default)]
    pub unverifiable: bool,
    /// Some of the terminal's processes were read and some were not (they exited, or access was denied). `processes`
    /// and `bytes` cover only the ones read, so the sum is a lower bound and must be shown as partial. It is counted in
    /// `total_bytes` with what it read. Never set together with `unverifiable`.
    #[serde(default)]
    pub partial: bool,
}

#[cfg(test)]
mod tests {
    use super::{HostMemory, MemoryMetric, ProcessMemory, TerminalMemory};
    use serde_json::json;

    #[test]
    fn the_answer_has_its_wire_names() {
        let memory = HostMemory {
            metric: MemoryMetric::WorkingSet,
            total_bytes: 30,
            service: ProcessMemory { pid: 7, bytes: 10 },
            terminals: vec![
                TerminalMemory {
                    terminal_id: "t-1".to_owned(),
                    workspace_id: "w-1".to_owned(),
                    session_id: Some("s-1".to_owned()),
                    processes: 2,
                    bytes: 20,
                    unverifiable: false,
                    partial: false,
                },
                TerminalMemory {
                    terminal_id: "t-2".to_owned(),
                    workspace_id: "w-1".to_owned(),
                    session_id: None,
                    processes: 0,
                    bytes: 0,
                    unverifiable: true,
                    partial: false,
                },
                TerminalMemory {
                    terminal_id: "t-3".to_owned(),
                    workspace_id: "w-1".to_owned(),
                    session_id: None,
                    processes: 1,
                    bytes: 4,
                    unverifiable: false,
                    partial: true,
                },
            ],
            unreadable: 1,
            sampled_at_ms: 1_700_000_000_000,
        };
        let value = serde_json::to_value(&memory).unwrap();
        assert_eq!(
            value,
            json!({
                "metric": "working_set",
                "total_bytes": 30,
                "service": { "pid": 7, "bytes": 10 },
                "terminals": [
                    { "terminal_id": "t-1", "workspace_id": "w-1", "session_id": "s-1", "processes": 2, "bytes": 20, "unverifiable": false, "partial": false },
                    { "terminal_id": "t-2", "workspace_id": "w-1", "processes": 0, "bytes": 0, "unverifiable": true, "partial": false },
                    { "terminal_id": "t-3", "workspace_id": "w-1", "processes": 1, "bytes": 4, "unverifiable": false, "partial": true }
                ],
                "unreadable": 1,
                "sampled_at_ms": 1_700_000_000_000_i64
            })
        );
        assert_eq!(serde_json::from_value::<HostMemory>(value).unwrap(), memory);
        assert_eq!(
            serde_json::to_value(MemoryMetric::Resident).unwrap(),
            json!("resident")
        );
        // An answer from a service that predates the flags reads as verified and complete.
        let older: TerminalMemory = serde_json::from_value(json!({
            "terminal_id": "t-4", "workspace_id": "w-1", "processes": 1, "bytes": 4
        }))
        .unwrap();
        assert!(!older.unverifiable);
        assert!(!older.partial);
    }
}
