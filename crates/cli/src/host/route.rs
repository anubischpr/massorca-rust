//! `massorca host route`: how a paired host is dialed, through ssh or directly (decision 0133).

use massorca_client::{HostRecord, Hosts, SshRoute};
use massorca_core_remote::ssh::stdio_forward_arguments;
use massorca_profile::Profile;
use serde::Serialize;

use crate::args::HostRouteArgs;
use crate::error::CliError;
use crate::output::Output;

/// What `host route` saved: the route, or `null` for a host dialed directly.
#[derive(Debug, Serialize)]
pub struct RouteReport {
    pub host: String,
    pub route: Option<SshRoute>,
}

/// `host route`: saves the route given, or clears it with `--direct`. Makes no connection.
pub fn execute(profile: &Profile, args: &HostRouteArgs, output: Output) -> Result<(), CliError> {
    let hosts = Hosts::of(profile);
    let record = hosts.get(&args.name)?;
    let route = match &args.ssh {
        Some(target) => {
            let route = SshRoute {
                ssh_target: target.clone(),
                ssh_args: args.ssh_arg.clone(),
                forward: args.forward.clone(),
            };
            check(&route, &record)?;
            Some(route)
        }
        None => None,
    };
    hosts.set_route(&args.name, route.as_ref())?;
    let lines = vec![route_line(&args.name, route.as_ref(), &record)];
    output.success(
        &RouteReport {
            host: args.name.clone(),
            route,
        },
        &lines,
    );
    Ok(())
}

/// Refuses a route ssh could misread before it is saved, with the same checks the dial makes.
pub fn check(route: &SshRoute, record: &HostRecord) -> Result<(), CliError> {
    let forward = forward_of(route, record).unwrap_or_default();
    stdio_forward_arguments(&route.ssh_target, &route.ssh_args, forward)?;
    Ok(())
}

/// One line on how `host` is dialed.
pub fn route_line(host: &str, route: Option<&SshRoute>, record: &HostRecord) -> String {
    match route {
        Some(route) => format!(
            "host {host} is dialed through `{}`, forwarding to {}",
            route.describe(),
            forward_of(route, record).unwrap_or("its first address")
        ),
        None => format!("host {host} is dialed directly"),
    }
}

fn forward_of<'a>(route: &'a SshRoute, record: &'a HostRecord) -> Option<&'a str> {
    route
        .forward
        .as_deref()
        .or_else(|| record.addresses.first().map(String::as_str))
}
