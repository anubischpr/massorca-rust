//! `massorca host ports`: the TCP ports that the processes of each live terminal listen on, read on the host that runs
//! them (decision 0081). A service that cannot be reached, or a host whose socket table cannot be read, makes the answer
//! `unverifiable`, never "no ports".

use massorca_protocol::methods::host::HostPorts;
use serde_json::json;

use super::LOST_CONTACT;
use crate::connect::connect;
use crate::error::{CliError, exit};
use crate::output::Output;
use crate::target::Target;
use crate::workspace_text;

/// Runs `host ports` against the service `target` names.
pub async fn execute(target: &Target, autostart: bool, output: Output) -> Result<(), CliError> {
    let ports = read(target, autostart).await.map_err(unverifiable)?;
    let ports = answer_or_error(ports)?;
    output.success(&ports, &lines(&ports));
    Ok(())
}

async fn read(target: &Target, autostart: bool) -> Result<HostPorts, CliError> {
    Ok(connect(target, autostart).await?.host_ports().await?)
}

/// Losing contact with the service leaves the ports unknown: it becomes `ports.unverifiable` with exit 4. Anything else
/// keeps its own code, as `host memory` does.
fn unverifiable(error: CliError) -> CliError {
    if !LOST_CONTACT.contains(&error.code.as_str()) {
        return error;
    }
    let mut failure = CliError::new(
        "ports.unverifiable",
        format!("ports unverifiable: {}", error.message),
        exit::UNAVAILABLE,
    );
    failure.data = Some(Box::new(
        json!({ "state": "unverifiable", "cause": error.code }),
    ));
    failure
}

/// A host whose socket table cannot be read knows no port at all: that is `unverifiable` with exit 4, not an empty list.
fn answer_or_error(ports: HostPorts) -> Result<HostPorts, CliError> {
    if !ports.unverifiable {
        return Ok(ports);
    }
    let mut failure = CliError::new(
        "ports.unverifiable",
        "ports unverifiable: this host's socket table cannot be read",
        exit::UNAVAILABLE,
    );
    failure.data = Some(Box::new(
        json!({ "state": "unverifiable", "cause": "socket_table" }),
    ));
    Err(failure)
}

/// The table of ports, then the terminals that could not be listed or are partial, so no count is read as complete when
/// it is not.
fn lines(answer: &HostPorts) -> Vec<String> {
    let unverifiable = answer
        .terminals
        .iter()
        .filter(|terminal| terminal.unverifiable)
        .count();
    let partial = answer
        .terminals
        .iter()
        .filter(|terminal| terminal.partial)
        .count();
    let mut lines = Vec::new();
    if answer.ports.is_empty() {
        // An empty table only speaks for the terminals it could verify: with none verified, nothing is known.
        let none_verified = !answer.terminals.is_empty() && unverifiable == answer.terminals.len();
        lines.push(if none_verified {
            "no terminal could be verified (their ports are not listed)".to_owned()
        } else if unverifiable > 0 {
            "no listening ports among the verified terminals".to_owned()
        } else {
            "no listening ports".to_owned()
        });
    } else {
        let rows = answer
            .ports
            .iter()
            .map(|port| {
                let process = if port.process_name.is_empty() {
                    format!("pid {}", port.pid)
                } else {
                    format!("{} ({})", port.process_name, port.pid)
                };
                vec![
                    port.port.to_string(),
                    port.address.to_string(),
                    process,
                    port.workspace_id.clone(),
                    port.session_id.clone().unwrap_or_else(|| "-".to_owned()),
                ]
            })
            .collect();
        lines.extend(workspace_text::table(
            &["PORT", "ADDRESS", "PROCESS", "WORKSPACE", "SESSION"],
            rows,
        ));
    }
    if unverifiable > 0 {
        lines.push(format!(
            "unverifiable terminals: {unverifiable} (their ports are not listed)"
        ));
    }
    if partial > 0 {
        lines.push(format!(
            "partial terminals: {partial} (their lists may miss ports)"
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use massorca_protocol::methods::host::{HostPort, HostPorts, PortTerminal};
    use serde_json::json;

    use super::{answer_or_error, lines, unverifiable};
    use crate::error::{CliError, exit};

    fn port(number: u16, address: IpAddr, pid: u32, session: Option<&str>) -> HostPort {
        HostPort {
            port: number,
            address,
            connect_host: "localhost".to_owned(),
            pid,
            process_name: "node".to_owned(),
            terminal_id: "t-1".to_owned(),
            workspace_id: "w-1".to_owned(),
            session_id: session.map(str::to_owned),
        }
    }

    fn terminal(id: &str, unverifiable: bool, partial: bool) -> PortTerminal {
        PortTerminal {
            terminal_id: id.to_owned(),
            workspace_id: "w-1".to_owned(),
            unverifiable,
            partial,
        }
    }

    fn answer(ports: Vec<HostPort>, terminals: Vec<PortTerminal>, unverifiable: bool) -> HostPorts {
        HostPorts {
            ports,
            terminals,
            unverifiable,
            sampled_at_ms: 1,
        }
    }

    #[test]
    fn each_port_is_a_row_with_its_address_process_workspace_and_session() {
        let text = lines(&answer(
            vec![
                port(3000, IpAddr::V4(Ipv4Addr::UNSPECIFIED), 42, Some("s-1")),
                port(5173, IpAddr::V4(Ipv4Addr::LOCALHOST), 43, None),
            ],
            vec![terminal("t-1", false, false)],
            false,
        ))
        .join("\n");
        let header = text.lines().next().unwrap_or_default();
        assert_eq!(
            header.split_whitespace().collect::<Vec<_>>(),
            ["PORT", "ADDRESS", "PROCESS", "WORKSPACE", "SESSION"],
            "{text}"
        );
        let first = text
            .lines()
            .find(|line| line.starts_with("3000"))
            .unwrap_or_else(|| panic!("{text}"));
        assert_eq!(
            first.split_whitespace().collect::<Vec<_>>(),
            ["3000", "0.0.0.0", "node", "(42)", "w-1", "s-1"],
            "{first}"
        );
        let second = text
            .lines()
            .find(|line| line.starts_with("5173"))
            .unwrap_or_else(|| panic!("{text}"));
        assert!(second.trim_end().ends_with("-"), "{second}");
    }

    #[test]
    fn an_empty_answer_says_no_port_only_for_the_terminals_it_could_verify() {
        let verified = lines(&answer(
            Vec::new(),
            vec![terminal("t-1", false, false)],
            false,
        ))
        .join("\n");
        assert!(verified.contains("no listening ports"), "{verified}");
        assert!(!verified.contains("verified terminals"), "{verified}");

        // One terminal verified and one not: the empty table speaks for the verified one only.
        let mixed = lines(&answer(
            Vec::new(),
            vec![terminal("t-1", false, false), terminal("t-2", true, false)],
            false,
        ))
        .join("\n");
        assert!(
            mixed.contains("no listening ports among the verified terminals"),
            "{mixed}"
        );
        assert!(
            mixed.contains("unverifiable terminals: 1 (their ports are not listed)"),
            "{mixed}"
        );

        // No terminal verified: nothing is known, so the answer must not claim anything about the verified ones.
        let none = lines(&answer(
            Vec::new(),
            vec![terminal("t-2", true, false), terminal("t-3", true, false)],
            false,
        ))
        .join("\n");
        assert!(
            none.contains("no terminal could be verified (their ports are not listed)"),
            "{none}"
        );
        assert!(!none.contains("no listening ports"), "{none}");
        assert!(!none.contains("among the verified terminals"), "{none}");
    }

    #[test]
    fn a_partial_terminal_is_said_to_maybe_miss_ports() {
        let text = lines(&answer(
            vec![port(8080, IpAddr::V4(Ipv4Addr::LOCALHOST), 42, None)],
            vec![terminal("t-1", false, true)],
            false,
        ))
        .join("\n");
        assert!(
            text.contains("partial terminals: 1 (their lists may miss ports)"),
            "{text}"
        );
    }

    #[test]
    fn an_unreadable_socket_table_is_unverifiable_never_an_empty_list() {
        let failed = answer_or_error(answer(Vec::new(), vec![terminal("t-1", true, false)], true))
            .expect_err("the socket table is unknown");
        assert_eq!(failed.code, "ports.unverifiable");
        assert_eq!(failed.exit_code, exit::UNAVAILABLE);
        assert_eq!(
            failed.data.as_deref(),
            Some(&json!({ "state": "unverifiable", "cause": "socket_table" }))
        );
        let verified =
            answer_or_error(answer(Vec::new(), Vec::new(), false)).expect("a verified answer");
        assert!(verified.ports.is_empty());
    }

    #[test]
    fn only_losing_the_service_is_unverifiable_for_ports() {
        for code in [
            "service.unavailable",
            "service.disconnected",
            "host.unreachable",
            "client.io",
        ] {
            let relabelled = unverifiable(CliError::new(code, "gone", exit::UNAVAILABLE));
            assert_eq!(relabelled.code, "ports.unverifiable", "{code}");
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
    }
}
