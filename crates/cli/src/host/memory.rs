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

/// A failure to reach the service, or losing it mid-call, leaves the figure unknown: it becomes `memory.unverifiable`
/// with exit 4. An answer the service gave (a refusal, an unknown method) keeps its own code.
fn unverifiable(error: CliError) -> CliError {
    if error.exit_code != exit::UNAVAILABLE && error.code != "client.io" {
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
                vec![
                    terminal.terminal_id.clone(),
                    terminal.workspace_id.clone(),
                    terminal
                        .session_id
                        .clone()
                        .unwrap_or_else(|| "-".to_owned()),
                    terminal.processes.to_string(),
                    human_bytes(terminal.bytes),
                ]
            })
            .collect();
        lines.extend(workspace_text::table(
            &["TERMINAL", "WORKSPACE", "SESSION", "PROCESSES", "MEMORY"],
            rows,
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

/// Binary units: `N KB` under 1 MiB, `x.y MB` under 1 GiB, `x.yz GB` above.
fn human_bytes(bytes: u64) -> String {
    // Display only: f64 holds any byte count to well within the shown precision.
    let value = bytes as f64;
    if bytes < MIB {
        format!("{:.0} KB", value / KIB as f64)
    } else if bytes < GIB {
        format!("{:.1} MB", value / MIB as f64)
    } else {
        format!("{:.2} GB", value / GIB as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::{GIB, KIB, MIB, human_bytes};

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
