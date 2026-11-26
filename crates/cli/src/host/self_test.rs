//! `massorca host self-test NAME`: runs the checks of one paired host in order and says which step fails, and how to fix it.

use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::str::FromStr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use massorca_client::{Client, ClientError, HostRecord, Hosts, SshInstall, Welcome};
use massorca_core_remote::bootstrap::UNIT_NAME;
use massorca_net::Pin;
use massorca_profile::Profile;
use massorca_profile::local_socket_name;
use massorca_protocol::codes;
use massorca_protocol::methods::devices::{self, ListenerInfo, NetworkStatus, WhoAmI};
use massorca_protocol::methods::service::{self, ServiceInfo};
use serde::Serialize;
use serde_json::Value;

use crate::connect::{connect_local, peer};
use crate::error::{CliError, exit};
use crate::output::Output;

/// The checks, in the order they run. A failing check skips the rest.
const CHECKS: [&str; 8] = [
    "record", "address", "port", "identity", "protocol", "paired", "service", "latency",
];
/// How long the TCP probe waits for one address.
const PORT_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the secure connection and its handshake may take, end to end.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// The checks of this computer's own service; `network` joins them only when the service listens on the network.
const LOCAL_CHECKS: [&str; 5] = ["endpoint", "connect", "protocol", "service", "latency"];
const LOCAL_CHECKS_WITH_NETWORK: [&str; 6] = [
    "endpoint", "connect", "protocol", "service", "latency", "network",
];
/// The name of a local report. It is not a host name: `local` is reserved for this computer.
const LOCAL_NAME: &str = "local";

/// One check's result: `pass`, `fail`, or `skipped` when an earlier check failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub state: &'static str,
    pub detail: String,
    /// What to do about a failing or skipped check.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

/// Every check of one host, in order, and whether all of them passed.
#[derive(Debug, Serialize)]
pub struct SelfTestReport {
    pub name: String,
    pub passed: bool,
    pub checks: Vec<Check>,
    /// The command that reads the host's service log, when a check failed on a host installed over SSH.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logs: Option<String>,
}

/// The command that prints the service log of an SSH-installed host, through the target and `ssh` options its install used.
fn journal_command(install: &SshInstall) -> String {
    let mut words = vec!["ssh".to_owned()];
    words.extend(install.ssh_args.iter().cloned());
    words.push(install.ssh_target.clone());
    words.push(format!(
        "journalctl --user -u {UNIT_NAME} -n 100 --no-pager"
    ));
    words.join(" ")
}

/// Runs the self-test of host `name`, or of this computer's own service without a name, and prints its report; a failing
/// run fails the command with the report in its data.
pub async fn execute(
    profile: &Profile,
    name: Option<&str>,
    output: Output,
) -> Result<(), CliError> {
    let hosts = Hosts::of(profile);
    let mut report = match name {
        Some(name) => run(&hosts, name).await,
        None => run_local(profile).await,
    };
    if let (false, Some(name)) = (report.passed, name) {
        report.logs = hosts
            .ssh_install(name)
            .ok()
            .flatten()
            .map(|install| journal_command(&install));
    }
    let mut lines: Vec<String> = report.checks.iter().map(line).collect();
    if let Some(command) = &report.logs {
        lines.push(format!("logs: {command}"));
    }
    if report.passed {
        output.success(&report, &lines);
        return Ok(());
    }
    output.text_lines(&lines);
    let failed = report
        .checks
        .iter()
        .filter(|check| check.state == "fail")
        .count();
    let target = name.map_or_else(|| "this computer".to_owned(), |name| format!("host {name}"));
    let mut error = CliError::new(
        "host.self_test_failed",
        format!(
            "{failed} of {} checks failed for {target}",
            report.checks.len()
        ),
        exit::ERROR,
    );
    error.data = Some(Box::new(
        serde_json::to_value(&report).unwrap_or(Value::Null),
    ));
    Err(error)
}

async fn run(hosts: &Hosts, name: &str) -> SelfTestReport {
    let mut checks = Vec::new();
    let record = match read_record(hosts, name) {
        Ok((record, pass)) => {
            checks.push(pass);
            record
        }
        Err(fail) => return finish(name, &CHECKS, vec![fail]),
    };
    let addresses = match resolve(&record.addresses).await {
        Ok(pass) => {
            checks.push(pass.0);
            pass.1
        }
        Err(fail) => {
            checks.push(fail);
            return finish(name, &CHECKS, checks);
        }
    };
    match probe_port(addresses).await {
        Ok(pass) => checks.push(pass),
        Err(fail) => {
            checks.push(fail);
            return finish(name, &CHECKS, checks);
        }
    }
    let client = match connect(hosts, name, &record, &mut checks).await {
        Some(client) => client,
        None => return finish(name, &CHECKS, checks),
    };
    session_checks(&client, &mut checks).await;
    finish(name, &CHECKS, checks)
}

/// This computer's own service: its endpoint, a local connection that never starts the service, the handshake, the
/// service's health and latency, and its network listeners when it has any.
async fn run_local(profile: &Profile) -> SelfTestReport {
    let mut checks = Vec::new();
    if let Err(error) = local_socket_name(profile) {
        checks.push(fail(
            "endpoint",
            format!("the local endpoint cannot be named: {error}"),
            "check the profile folder: `massorca profile secure`".to_owned(),
        ));
        return finish_local(checks);
    }
    checks.push(pass(
        "endpoint",
        format!("profile {} has its local endpoint", profile.name()),
    ));
    let client = match connect_local(profile, false).await {
        Ok(client) => {
            checks.push(pass(
                "connect",
                "accepts a connection on its local endpoint".to_owned(),
            ));
            client
        }
        Err(error) => {
            checks.push(fail(
                "connect",
                error.message,
                "start the service of this profile (`massorca service run`), then run the self-test again".to_owned(),
            ));
            return finish_local(checks);
        }
    };
    checks.push(protocol_pass(client.welcome()));
    service_checks(&client, &mut checks).await;
    network_check(&client, &mut checks).await;
    finish_local(checks)
}

fn finish_local(checks: Vec<Check>) -> SelfTestReport {
    let names: &[&'static str] = if checks.iter().any(|check| check.name == "network") {
        &LOCAL_CHECKS_WITH_NETWORK
    } else {
        &LOCAL_CHECKS
    };
    finish(LOCAL_NAME, names, checks)
}

/// The network listeners of the service, shown only when there are some: a service that listens on no network address
/// serves this computer alone.
async fn network_check(client: &Client, checks: &mut Vec<Check>) {
    let status: Result<NetworkStatus, ClientError> =
        client.call(devices::NETWORK_STATUS, &()).await;
    let status = match status {
        Ok(status) if status.listeners.is_empty() => return,
        Ok(status) => status,
        Err(error) => {
            checks.push(fail(
                "network",
                format!("the network status did not answer: {error}"),
                "restart the service of this profile and run the self-test again".to_owned(),
            ));
            return;
        }
    };
    let listening = status
        .listeners
        .iter()
        .all(|listener| listener.state == "listening");
    let shown: Vec<String> = status.listeners.iter().map(listener_text).collect();
    if listening {
        checks.push(pass("network", shown.join(", ")));
    } else {
        checks.push(fail(
            "network",
            shown.join(", "),
            "start the service on an address this computer has: `massorca service run --listen <address>`".to_owned(),
        ));
    }
}

fn listener_text(listener: &ListenerInfo) -> String {
    let address = listener.bound.as_deref().unwrap_or(&listener.requested);
    match &listener.error {
        Some(error) => format!("{address} {}: {error}", listener.state),
        None => format!("{address} {}", listener.state),
    }
}

/// The host record, with its pin read: the record check.
fn read_record(hosts: &Hosts, name: &str) -> Result<(HostRecord, Check), Check> {
    let record = hosts.get(name).map_err(|error| {
        fail(
            "record",
            error.to_string(),
            format!("pair this computer with the host first: `massorca pair accept <offer> --name {name}`"),
        )
    })?;
    if record.addresses.is_empty() {
        return Err(fail(
            "record",
            format!("host {name} lists no address"),
            format!(
                "pair with the host again so its record holds an address: `massorca host remove {name} --keep-device`"
            ),
        ));
    }
    Pin::from_str(&record.pin).map_err(|error| {
        fail(
            "record",
            format!("the stored pin is not valid: {error}"),
            format!("pair with the host again: `massorca host remove {name} --keep-device`"),
        )
    })?;
    let detail = format!("{name} is paired at {}", record.addresses.join(", "));
    Ok((record, pass("record", detail)))
}

/// Resolves every address of the record: the address check. Returns the socket addresses to probe, in order.
async fn resolve(addresses: &[String]) -> Result<(Check, Vec<SocketAddr>), Check> {
    let owned = addresses.to_vec();
    let joined = tokio::task::spawn_blocking(move || {
        let mut resolved = Vec::new();
        let mut errors = Vec::new();
        for address in &owned {
            match address.to_socket_addrs() {
                Ok(found) => resolved.extend(found),
                Err(error) => errors.push(format!("{address}: {error}")),
            }
        }
        (resolved, errors)
    })
    .await;
    let (resolved, errors) = joined.unwrap_or_default();
    if resolved.is_empty() {
        return Err(fail(
            "address",
            format!("no address resolves: {}", errors.join("; ")),
            "check the host name or address in `massorca host show <name>`; for a Tailscale name, check that Tailscale is connected".to_owned(),
        ));
    }
    let shown: Vec<String> = resolved.iter().map(SocketAddr::to_string).collect();
    Ok((
        pass("address", format!("resolves to {}", shown.join(", "))),
        resolved,
    ))
}

/// Opens a TCP connection to the first address that accepts one: the port check.
async fn probe_port(addresses: Vec<SocketAddr>) -> Result<Check, Check> {
    let shown: Vec<String> = addresses.iter().map(SocketAddr::to_string).collect();
    let reached = tokio::task::spawn_blocking(move || {
        let mut last = String::from("no address to try");
        for address in addresses {
            match TcpStream::connect_timeout(&address, PORT_TIMEOUT) {
                Ok(_) => return Ok(address),
                Err(error) => last = format!("{address}: {error}"),
            }
        }
        Err(last)
    })
    .await
    .unwrap_or_else(|error| Err(error.to_string()));
    match reached {
        Ok(address) => Ok(pass("port", format!("{address} accepts connections"))),
        Err(last) => Err(fail(
            "port",
            format!("nothing accepts connections at {}: {last}", shown.join(", ")),
            "start or restart the service on the host so it listens on that address: `massorca service run --listen <address>` (on an SSH host, restart its systemd unit)".to_owned(),
        )),
    }
}

/// Connects to the host as this device: the identity check, with the handshake's outcome deciding the protocol and
/// pairing checks. Returns the client when the connection is open.
async fn connect(
    hosts: &Hosts,
    name: &str,
    record: &HostRecord,
    checks: &mut Vec<Check>,
) -> Option<Client> {
    let me = peer();
    let attempt = tokio::time::timeout(CONNECT_TIMEOUT, Client::connect_host(hosts, name, &me));
    let outcome = match attempt.await {
        Ok(outcome) => outcome,
        Err(_) => {
            checks.push(fail(
                "identity",
                format!("no answer within {} seconds", CONNECT_TIMEOUT.as_secs()),
                "check that the service on the host is running and reachable, then run the self-test again".to_owned(),
            ));
            return None;
        }
    };
    match outcome {
        Ok(client) => {
            checks.push(identity_pass(record));
            Some(client)
        }
        Err(ClientError::ServerIdentityMismatch(mismatch)) => {
            let actual = mismatch
                .actual
                .clone()
                .unwrap_or_else(|| "a certificate that could not be read".to_owned());
            checks.push(fail(
                "identity",
                format!(
                    "the server key is not the one paired with: paired {}, server presents {actual}; not retried",
                    mismatch.expected
                ),
                mismatch.message(),
            ));
            None
        }
        Err(ClientError::Handshake { error, .. }) => {
            checks.push(identity_pass(record));
            if error.code == codes::PROTOCOL_UNSUPPORTED_VERSION
                || error.code == codes::PROTOCOL_HANDSHAKE
            {
                checks.push(fail(
                    "protocol",
                    format!("refused: {} ({})", error.message, error.code),
                    "update massorca on this computer or on the host so both speak a common protocol version".to_owned(),
                ));
            } else {
                checks.push(pass(
                    "protocol",
                    "the server accepted the protocol".to_owned(),
                ));
                checks.push(fail(
                    "paired",
                    format!("the server refused this device: {} ({})", error.message, error.code),
                    format!("pair this computer again: `massorca host remove {name} --keep-device`, then accept a new offer"),
                ));
            }
            None
        }
        Err(ClientError::HostUnreachable(reason)) => {
            checks.push(fail(
                "identity",
                format!("could not complete the secure connection: {reason}"),
                "check that the service on the host is running and reachable, then run the self-test again".to_owned(),
            ));
            None
        }
        Err(error) => {
            checks.push(fail(
                "identity",
                format!("the connection failed: {error}"),
                "run the self-test again; if it repeats, read the service log on the host"
                    .to_owned(),
            ));
            None
        }
    }
}

/// The checks that need an open connection: protocol, pairing, the service's own health and the round trip.
async fn session_checks(client: &Client, checks: &mut Vec<Check>) {
    checks.push(protocol_pass(client.welcome()));
    let me: Result<WhoAmI, ClientError> = client.call(devices::WHOAMI, &()).await;
    match me {
        Ok(me) => match me.device {
            Some(device) => checks.push(pass(
                "paired",
                format!(
                    "this device is {} with scopes {}",
                    device.name,
                    me.scopes.join(", ")
                ),
            )),
            None => {
                checks.push(fail(
                    "paired",
                    "the server does not list this device".to_owned(),
                    "pair this computer again with a new offer".to_owned(),
                ));
                return;
            }
        },
        Err(error) => {
            checks.push(fail(
                "paired",
                format!("the server refused the request: {error}"),
                "pair this computer again with a new offer".to_owned(),
            ));
            return;
        }
    }
    service_checks(client, checks).await;
}

/// `service.info` and one round trip of that call: the service's own health and its latency.
async fn service_checks(client: &Client, checks: &mut Vec<Check>) {
    let sent = Instant::now();
    let info: Result<ServiceInfo, ClientError> = client.call(service::INFO, &()).await;
    let elapsed = sent.elapsed();
    match info {
        Ok(info) => {
            checks.push(pass(
                "service",
                format!(
                    "version {}, uptime {} s",
                    info.version,
                    uptime_seconds(info.started_at_ms)
                ),
            ));
            checks.push(pass("latency", format!("{} ms", elapsed.as_millis())));
        }
        Err(error) => {
            checks.push(fail(
                "service",
                format!("service.info failed: {error}"),
                "restart the service on the host and run the self-test again".to_owned(),
            ));
        }
    }
}

fn identity_pass(record: &HostRecord) -> Check {
    pass(
        "identity",
        format!("the server key matches the paired pin {}", record.pin),
    )
}

fn protocol_pass(welcome: &Welcome) -> Check {
    pass(
        "protocol",
        format!(
            "protocol {}, server {}",
            welcome.protocol, welcome.server.version
        ),
    )
}

fn uptime_seconds(started_at_ms: i64) -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
        });
    u64::try_from(now.saturating_sub(started_at_ms).max(0) / 1000).unwrap_or(0)
}

/// Pads the report with a skipped check for every step that did not run, and sets `passed`.
fn finish(name: &str, names: &[&'static str], mut ran: Vec<Check>) -> SelfTestReport {
    let failed_check = ran
        .iter()
        .find(|check| check.state == "fail")
        .map(|check| check.name);
    let mut checks = Vec::with_capacity(names.len());
    for &expected in names {
        if let Some(index) = ran.iter().position(|check| check.name == expected) {
            checks.push(ran.remove(index));
            continue;
        }
        let reason = failed_check.map_or_else(
            || "not run".to_owned(),
            |failed| format!("not run because the {failed} check failed"),
        );
        checks.push(Check {
            name: expected,
            state: "skipped",
            detail: reason,
            fix: failed_check.map(|failed| format!("resolve the {failed} check above first")),
        });
    }
    let passed = checks.iter().all(|check| check.state == "pass");
    SelfTestReport {
        name: name.to_owned(),
        passed,
        checks,
        logs: None,
    }
}

fn pass(name: &'static str, detail: String) -> Check {
    Check {
        name,
        state: "pass",
        detail,
        fix: None,
    }
}

fn fail(name: &'static str, detail: String, fix: String) -> Check {
    Check {
        name,
        state: "fail",
        detail,
        fix: Some(fix),
    }
}

/// One check as a text line: its name, its state, and the detail, then the fix when it did not pass.
fn line(check: &Check) -> String {
    let base = format!("{}: {} ({})", check.name, check.state, check.detail);
    match &check.fix {
        Some(fix) => format!("{base}. Fix: {fix}"),
        None => base,
    }
}
