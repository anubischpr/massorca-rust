//! `massorca pair offer|accept`: a pairing offer for another computer, and pairing this one with a server (decision 0041).

use std::io::{self, Read};

use massorca_client::{Client, Hosts};
use massorca_profile::Profile;
use massorca_protocol::methods::devices::{self, PairingCreateParams, PairingCreated};
use massorca_protocol::pairing::Offer;

use crate::args::{PairAcceptArgs, PairOfferArgs};
use crate::connect::{connect, peer};
use crate::error::CliError;
use crate::output::Output;
use crate::target::Target;
use crate::time_text;

/// The device name when this computer's host name cannot be read.
const FALLBACK_DEVICE_NAME: &str = "massorca";

/// Makes a pairing offer on the service `target` names, and prints it once.
pub async fn offer(
    target: &Target,
    autostart: bool,
    args: &PairOfferArgs,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(target, autostart).await?;
    let params = PairingCreateParams {
        ttl_secs: args.ttl,
        admin: args.admin,
    };
    let created: PairingCreated = client.call(devices::PAIRING_CREATE, &params).await?;
    let lines = offer_lines(&created);
    output.success(&created, &lines);
    Ok(())
}

/// The offer alone on its first line, then the guidance that goes with it.
fn offer_lines(created: &PairingCreated) -> Vec<String> {
    vec![
        created.offer.expose().to_owned(),
        format!("Expires {}.", time_text::utc_minute(created.expires_at_ms)),
        format!("Scopes: {}.", created.scopes.join(", ")),
        format!("Addresses: {}.", created.addresses.join(", ")),
        "It works once. Treat it like a password.".to_owned(),
    ]
}

/// Pairs this computer with the server of an offer, and stores the server as a host of `profile`.
pub async fn accept(
    profile: &Profile,
    args: &PairAcceptArgs,
    output: Output,
) -> Result<(), CliError> {
    let text = offer_text(&args.offer)?;
    let offer = Offer::decode(&text)?;
    let name = args.name.clone().unwrap_or_else(|| offer.server.clone());
    let device_name = args.device_name.clone().unwrap_or_else(default_device_name);
    let (client, record) =
        Client::pair(&Hosts::of(profile), &offer, &name, &device_name, &peer()).await?;
    drop(client);
    let lines = record_lines(&record);
    output.success(&record, &lines);
    Ok(())
}

/// The offer as typed, or read from stdin for `-`, which keeps it out of shell history.
fn offer_text(argument: &str) -> Result<String, CliError> {
    if argument != "-" {
        return Ok(argument.to_owned());
    }
    let mut text = String::new();
    io::stdin().read_to_string(&mut text)?;
    Ok(text)
}

/// This computer's host name, for the name it has on a server: COMPUTERNAME, HOSTNAME, then /etc/hostname.
pub fn default_device_name() -> String {
    let candidates = ["COMPUTERNAME", "HOSTNAME"]
        .into_iter()
        .filter_map(|key| std::env::var(key).ok())
        .chain(std::fs::read_to_string("/etc/hostname").ok());
    candidates
        .map(|name| name.trim().to_owned())
        .find(|name| !name.is_empty())
        .unwrap_or_else(|| FALLBACK_DEVICE_NAME.to_owned())
}

/// What `pair accept` shows of the stored host.
fn record_lines(record: &massorca_client::HostRecord) -> Vec<String> {
    let mut lines = vec![
        format!("host: {}", record.name),
        format!("server: {}", record.server),
    ];
    lines.extend(
        record
            .addresses
            .iter()
            .map(|address| format!("address: {address}")),
    );
    lines.push(format!("pin: {}", record.pin));
    lines.push(format!(
        "device: {} ({})",
        record.device_name, record.device_id
    ));
    lines.push(format!("scopes: {}", record.scopes.join(", ")));
    lines.push(format!(
        "paired: {}",
        time_text::utc_minute(record.paired_at_ms)
    ));
    lines
}
