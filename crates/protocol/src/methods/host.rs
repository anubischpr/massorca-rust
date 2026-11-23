//! Host methods: figures read on the host that runs the service (capability `host/1`, decision 0081).
//!
//! Sampled when asked and shared for about a second; nothing is pushed. A client polls while it shows them.

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

/// Params: none. Result: [`HostMemory`].
pub const MEMORY: &str = "host.memory";
/// Params: none. Result: [`HostPorts`].
pub const PORTS: &str = "host.ports";

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

/// One live terminal's processes and their memory: its shell and what the shell started, as its platform counts them
/// (decision 0081, D6). Windows counts the job, exactly. Linux counts the session, which misses a process that started a
/// session of its own. macOS counts the parent walk from the shell, which misses a process reparented away from it. The
/// service's own process is never counted.
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

/// Result of `host.ports`: the TCP sockets in the LISTEN state that the processes of each live terminal hold (agent runs
/// included), read on this host when asked (decision 0081). One entry per socket: a process listening on IPv4 and IPv6
/// has two.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPorts {
    pub ports: Vec<HostPort>,
    /// Live terminals, by creation time.
    pub terminals: Vec<PortTerminal>,
    /// The socket table cannot be read on this host, so nothing is known about any port: `ports` is then empty and
    /// every terminal is `unverifiable`. Never read as "no ports".
    pub unverifiable: bool,
    pub sampled_at_ms: i64,
}

/// One listening socket of a live terminal's processes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPort {
    pub port: u16,
    /// The address the socket is bound to: a wildcard (`0.0.0.0`, `::`) or one interface's address.
    pub address: IpAddr,
    /// Where a client connects: `localhost` for a wildcard bind, otherwise the address.
    pub connect_host: String,
    pub pid: u32,
    pub process_name: String,
    pub terminal_id: String,
    pub workspace_id: String,
    /// The agent session whose current run uses the terminal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

/// One live terminal's ports status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortTerminal {
    pub terminal_id: String,
    pub workspace_id: String,
    /// The terminal's processes could not be listed, or none of their sockets could be read: its ports are unknown, not
    /// none. Such a terminal lists no port.
    #[serde(default)]
    pub unverifiable: bool,
    /// Some of the terminal's processes could not be read, or the process table was incomplete: its list may miss ports.
    #[serde(default)]
    pub partial: bool,
}

#[cfg(test)]
mod ports_tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::{HostPort, HostPorts, PortTerminal};
    use serde_json::json;

    #[test]
    fn the_ports_answer_has_its_wire_names() {
        let answer = HostPorts {
            ports: vec![HostPort {
                port: 3000,
                address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                connect_host: "127.0.0.1".to_owned(),
                pid: 42,
                process_name: "node".to_owned(),
                terminal_id: "t-1".to_owned(),
                workspace_id: "w-1".to_owned(),
                session_id: Some("s-1".to_owned()),
            }],
            terminals: vec![
                PortTerminal {
                    terminal_id: "t-1".to_owned(),
                    workspace_id: "w-1".to_owned(),
                    unverifiable: false,
                    partial: false,
                },
                PortTerminal {
                    terminal_id: "t-2".to_owned(),
                    workspace_id: "w-1".to_owned(),
                    unverifiable: true,
                    partial: false,
                },
            ],
            unverifiable: false,
            sampled_at_ms: 1_700_000_000_000,
        };
        let value = serde_json::to_value(&answer).unwrap();
        assert_eq!(
            value,
            json!({
                "ports": [{
                    "port": 3000, "address": "127.0.0.1", "connect_host": "127.0.0.1", "pid": 42,
                    "process_name": "node", "terminal_id": "t-1", "workspace_id": "w-1", "session_id": "s-1"
                }],
                "terminals": [
                    { "terminal_id": "t-1", "workspace_id": "w-1", "unverifiable": false, "partial": false },
                    { "terminal_id": "t-2", "workspace_id": "w-1", "unverifiable": true, "partial": false }
                ],
                "unverifiable": false,
                "sampled_at_ms": 1_700_000_000_000_i64
            })
        );
        assert_eq!(serde_json::from_value::<HostPorts>(value).unwrap(), answer);

        let bare = HostPort {
            session_id: None,
            ..answer.ports[0].clone()
        };
        assert!(
            !serde_json::to_value(&bare)
                .unwrap()
                .as_object()
                .unwrap()
                .contains_key("session_id")
        );
        let older: PortTerminal =
            serde_json::from_value(json!({ "terminal_id": "t-3", "workspace_id": "w-1" })).unwrap();
        assert!(!older.unverifiable && !older.partial);
    }
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
