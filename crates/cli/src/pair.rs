//! `massorca pair offer|cancel|accept`: a pairing offer for another computer, retiring one, and pairing this one with a
//! server (decision 0041).

use std::io::{self, Read};

use massorca_client::{Client, Hosts, default_device_name};
use massorca_profile::Profile;
use massorca_protocol::methods::devices::{
    self, PairingCancelParams, PairingCancelled, PairingCreateParams, PairingCreated,
};
use massorca_protocol::pairing::Offer;

use crate::args::{PairAcceptArgs, PairCancelArgs, PairOfferArgs};
use crate::connect::{connect, peer};
use crate::error::CliError;
use crate::host::records::record_lines;
use crate::output::Output;
use crate::target::Target;
use crate::time_text;

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
        // `admin` asks older services for full access too; they do not read `standard`.
        admin: !args.standard,
        standard: args.standard,
        addresses: args.addresses.clone(),
    };
    let created: PairingCreated = client.call(devices::PAIRING_CREATE, &params).await?;
    let lines = offer_lines(&created);
    output.success(&created, &lines);
    Ok(())
}

/// The offer alone on its first line, then the guidance that goes with it.
fn offer_lines(created: &PairingCreated) -> Vec<String> {
    let mut lines = vec![
        created.offer.expose().to_owned(),
        format!("Expires {}.", time_text::utc_minute(created.expires_at_ms)),
        format!("Scopes: {}.", created.scopes.join(", ")),
        format!("Addresses: {}.", created.addresses.join(", ")),
        "It works once. Treat it like a password.".to_owned(),
    ];
    if let Some(id) = &created.offer_id {
        lines.push(format!(
            "Offer id: {id}. `massorca pair cancel {id}` retires it before it expires."
        ));
    }
    lines
}

/// Retires one pending offer, or every one, on the service `target` names.
pub async fn cancel(
    target: &Target,
    autostart: bool,
    args: &PairCancelArgs,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(target, autostart).await?;
    let params = PairingCancelParams {
        offer_id: args.offer_id.clone(),
        all: args.all,
    };
    let cancelled: PairingCancelled = client.pairing_cancel(&params).await?;
    let line = match cancelled.cancelled {
        1 => "Retired 1 offer. It no longer pairs.".to_owned(),
        count => format!("Retired {count} offers."),
    };
    output.success(&cancelled, &[line]);
    Ok(())
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
