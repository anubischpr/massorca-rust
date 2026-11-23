//! `massorca host memory`: the memory of the service and of its terminals, read on the host that runs them
//! (decision 0081). A service that cannot be reached makes the answer `unverifiable`, never zero.

use massorca_protocol::methods::host::{HostMemory, MemoryMetric};
use serde_json::json;

use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::output::Output;
use crate::target::Target;
use crate::workspace_text;

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;

/// Runs `host memory` against the service `target` names.
pub async fn execute(target: &Target, autostart: bool, output: Output) -> Result<(), CliError> {
    let memory = read(target, autostart).await.map_err(unverifiable)?;
    output.success(&memory, &lines(&memory));
    Ok(())
}

async fn read(target: &Target, autostart: bool) -> Result<HostMemory, CliError> {
    Ok(connect(target, autostart).await?.host_memory().await?)
}

/// The failures that mean the service could not be reached or was lost mid-call.
const LOST_CONTACT: [&str; 4] = [
    "service.unavailable",
    "service.disconnected",
    "host.unreachable",
    "client.io",
];

/// Losing contact with the service leaves the figure unknown: it becomes `memory.unverifiable` with exit 4. Anything
/// else keeps its own code: an answer the service gave (a refusal, an unknown method), an identity that does not match,
/// an untrusted endpoint, or a service that failed to start.
fn unverifiable(error: CliError) -> CliError {
    if !LOST_CONTACT.contains(&error.code.as_str()) {
        return error;
    }
    let mut failure = CliError::new(
        "memory.unverifiable",
        format!("memory unverifiable: {}", error.message),
        exit::UNAVAILABLE,
    );
    failure.data = Some(Box::new(
        json!({ "state": "unverifiable", "cause": error.code }),
    ));
    failure
}

fn lines(memory: &HostMemory) -> Vec<String> {
    let metric = match memory.metric {
        MemoryMetric::WorkingSet => "working set",
        MemoryMetric::Resident => "resident",
    };
    let mut lines = vec![
        format!("memory: {} ({metric})", human_bytes(memory.total_bytes)),
        format!(
            "service: pid {}, {}",
            memory.service.pid,
            human_bytes(memory.service.bytes)
        ),
    ];
    if memory.terminals.is_empty() {
        lines.push("no live terminals".to_owned());
    } else {
        let rows = memory
            .terminals
            .iter()
            .map(|terminal| {
                // An unverifiable terminal's counts are 0 and mean nothing: it is shown as unknown.
                let (processes, bytes) = if terminal.unverifiable {
                    ("-".to_owned(), "unverifiable".to_owned())
                } else {
                    (terminal.processes.to_string(), human_bytes(terminal.bytes))
                };
                vec![
                    terminal.terminal_id.clone(),
                    terminal.workspace_id.clone(),
                    terminal
                        .session_id
                        .clone()
                        .unwrap_or_else(|| "-".to_owned()),
                    processes,
                    bytes,
                ]
            })
            .collect();
        lines.extend(workspace_text::table(
            &["TERMINAL", "WORKSPACE", "SESSION", "PROCESSES", "MEMORY"],
            rows,
        ));
    }
    let unverifiable = memory
        .terminals
        .iter()
        .filter(|terminal| terminal.unverifiable)
        .count();
    if unverifiable > 0 {
        lines.push(format!(
            "unverifiable terminals: {unverifiable} (not counted)"
        ));
    }
    if memory.unreadable > 0 {
        lines.push(format!(
            "unreadable processes: {} (not counted)",
            memory.unreadable
        ));
    }
    lines
}

/// Binary units: `N KB` under 1 MiB, `x.y MB` under 1 GiB, `x.yz GB` above. A size that rounds up to the next unit's
/// first value is shown in that unit (`1.0 MB`, not `1024 KB`).
fn human_bytes(bytes: u64) -> String {
    // Display only: f64 holds any byte count to well within the shown precision.
    let value = bytes as f64;
    let kib = (value / KIB as f64).round();
    if kib < 1024.0 {
        return format!("{kib:.0} KB");
    }
    let mib = (value / MIB as f64 * 10.0).round() / 10.0;
    if mib < 1024.0 {
        return format!("{mib:.1} MB");
    }
    format!("{:.2} GB", value / GIB as f64)
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::host::{
        HostMemory, MemoryMetric, ProcessMemory, TerminalMemory,
    };

    use super::{GIB, KIB, MIB, human_bytes, lines, unverifiable};
    use crate::error::{CliError, exit};

    fn terminal(id: &str, bytes: u64, unverifiable: bool) -> TerminalMemory {
        TerminalMemory {
            terminal_id: id.to_owned(),
            workspace_id: "w-1".to_owned(),
            session_id: None,
            processes: if unverifiable { 0 } else { 2 },
            bytes,
            unverifiable,
        }
    }

    #[test]
    fn an_unverifiable_terminal_is_shown_as_unknown_never_as_zero() {
        let memory = HostMemory {
            metric: MemoryMetric::WorkingSet,
            total_bytes: 3 * MIB,
            service: ProcessMemory {
                pid: 7,
                bytes: 2 * MIB,
            },
            terminals: vec![terminal("t-read", MIB, false), terminal("t-gone", 0, true)],
            unreadable: 0,
            sampled_at_ms: 1,
        };
        let text = lines(&memory).join("\n");
        assert!(!text.contains("0 KB"), "{text}");
        let row = text
            .lines()
            .find(|line| line.starts_with("t-gone"))
            .unwrap_or_else(|| panic!("{text}"));
        let cells: Vec<&str> = row.split_whitespace().collect();
        assert_eq!(cells, ["t-gone", "w-1", "-", "-", "unverifiable"], "{row}");
        assert!(
            text.contains("unverifiable terminals: 1 (not counted)"),
            "{text}"
        );
    }

    #[test]
    fn only_losing_the_service_is_unverifiable() {
        for code in [
            "service.unavailable",
            "service.disconnected",
            "host.unreachable",
            "client.io",
        ] {
            let relabelled = unverifiable(CliError::new(code, "gone", exit::UNAVAILABLE));
            assert_eq!(relabelled.code, "memory.unverifiable", "{code}");
            assert_eq!(relabelled.exit_code, exit::UNAVAILABLE, "{code}");
        }
        for code in [
            "host.identity_mismatch",
            "service.untrusted_endpoint",
            "auth.device_revoked",
            "service.start_failed",
        ] {
            let kept = unverifiable(CliError::new(code, "refused", exit::UNAVAILABLE));
            assert_eq!(kept.code, code);
        }
        let kept = unverifiable(CliError::new("permission.denied", "no", exit::ERROR));
        assert_eq!(kept.code, "permission.denied");
    }

    #[test]
    fn a_size_that_rounds_up_takes_the_next_unit() {
        assert_eq!(human_bytes(MIB - 513), "1023 KB");
        assert_eq!(human_bytes(MIB - 512), "1.0 MB");
        assert_eq!(human_bytes(MIB - 1), "1.0 MB");
        assert_eq!(human_bytes(GIB - 1), "1.00 GB");
    }

    #[test]
    fn units_switch_at_one_mebibyte_and_one_gibibyte() {
        assert_eq!(human_bytes(0), "0 KB");
        assert_eq!(human_bytes(512 * KIB), "512 KB");
        assert_eq!(human_bytes(MIB - KIB), "1023 KB");
        assert_eq!(human_bytes(MIB), "1.0 MB");
        assert_eq!(human_bytes(1536 * KIB), "1.5 MB");
        assert_eq!(human_bytes(GIB - MIB), "1023.0 MB");
        assert_eq!(human_bytes(GIB), "1.00 GB");
        assert_eq!(human_bytes(GIB + GIB / 4), "1.25 GB");
    }
}
