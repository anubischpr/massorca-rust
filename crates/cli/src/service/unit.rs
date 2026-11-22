//! `massorca service unit`: the systemd user unit that runs the service on a Linux server (decision 0045).

use std::path::Path;

use massorca_core_remote::bootstrap::{UnitSpec, systemd_unit};
use massorca_server::ListenSpec;
use serde::Serialize;

use crate::args::UnitArgs;
use crate::error::CliError;
use crate::output::Output;

/// The listen address when none is given: this node's Tailscale address (decision 0043).
const DEFAULT_LISTEN: &str = "tailscale";
/// The binary the unit runs, as systemd expands it on the host.
const DEFAULT_EXE: &str = "%h/.massorca/bin/massorca";
/// The MASSORCA_HOME the unit gives the service, as systemd expands it on the host.
const DEFAULT_HOME: &str = "%h/.massorca/home";

/// The unit text as a result document.
#[derive(Debug, Serialize)]
pub struct UnitDocument {
    pub unit: String,
}

/// Prints the unit for `args`. The global `--home` is the unit's MASSORCA_HOME, else `%h/.massorca/home`.
pub fn print(args: &UnitArgs, home: Option<&Path>, output: Output) -> Result<(), CliError> {
    let listen = listen_addresses(&args.listen)?;
    let home = home.map_or_else(
        || DEFAULT_HOME.to_owned(),
        |path| path.display().to_string(),
    );
    let unit = systemd_unit(&UnitSpec {
        exe: args.exe.as_deref().unwrap_or(DEFAULT_EXE),
        home: &home,
        listen: &listen,
    });
    let lines: Vec<String> = unit.lines().map(str::to_owned).collect();
    output.success(&UnitDocument { unit }, &lines);
    Ok(())
}

/// The listen addresses given, or the default. Each must parse as `service run --listen` takes it.
pub fn listen_addresses(values: &[String]) -> Result<Vec<String>, CliError> {
    let listen = if values.is_empty() {
        vec![DEFAULT_LISTEN.to_owned()]
    } else {
        values.to_vec()
    };
    for address in &listen {
        ListenSpec::parse(address, false)?;
    }
    Ok(listen)
}
