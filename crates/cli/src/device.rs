//! `massorca device list|revoke|forget|whoami`: the devices paired with the service (decision 0041).

use massorca_protocol::methods::devices::{
    self, DeviceForgetParams, DeviceInfo, DeviceList, DeviceRevokeParams, WhoAmI,
};

use crate::args::DeviceCommand;
use crate::connect::connect;
use crate::error::CliError;
use crate::output::Output;
use crate::target::Target;
use crate::time_text;
use crate::workspace_text;

/// Runs one `device` subcommand against the service `target` names.
pub async fn execute(
    target: &Target,
    autostart: bool,
    command: DeviceCommand,
    output: Output,
) -> Result<(), CliError> {
    let client = connect(target, autostart).await?;
    match command {
        DeviceCommand::List => {
            let list: DeviceList = client.call(devices::LIST, &()).await?;
            let lines = if list.devices.is_empty() {
                vec!["no devices".to_owned()]
            } else {
                device_table(&list.devices)
            };
            output.success(&list, &lines);
        }
        DeviceCommand::Revoke { device } => {
            let params = DeviceRevokeParams { device };
            let revoked: DeviceInfo = client.call(devices::REVOKE, &params).await?;
            let line = format!("revoked {} ({})", revoked.name, revoked.id);
            output.success(&revoked, &[line]);
        }
        DeviceCommand::Forget { device } => {
            let params = DeviceForgetParams { device };
            let forgotten: DeviceInfo = client.call(devices::FORGET, &params).await?;
            let line = format!("forgot {} ({})", forgotten.name, forgotten.id);
            output.success(&forgotten, &[line]);
        }
        DeviceCommand::Whoami => {
            let me: WhoAmI = client.call(devices::WHOAMI, &()).await?;
            output.success(&me, &whoami_lines(&me));
        }
    }
    Ok(())
}

/// The devices as a table; `*` marks the device this command runs as.
fn device_table(devices: &[DeviceInfo]) -> Vec<String> {
    let rows = devices
        .iter()
        .map(|device| {
            vec![
                if device.current { "*" } else { "" }.to_owned(),
                device.id.clone(),
                device.name.clone(),
                device.scopes.join(","),
                time_text::utc_minute(device.paired_at_ms),
                device
                    .last_seen_ms
                    .map_or_else(|| "never".to_owned(), time_text::utc_minute),
                if device.revoked_at_ms.is_some() {
                    "revoked"
                } else {
                    "active"
                }
                .to_owned(),
            ]
        })
        .collect();
    workspace_text::table(
        &["", "ID", "NAME", "SCOPES", "PAIRED", "LAST SEEN", "STATE"],
        rows,
    )
}

fn whoami_lines(me: &WhoAmI) -> Vec<String> {
    let mut lines = vec![
        format!("principal: {}", me.principal),
        format!("scopes: {}", me.scopes.join(", ")),
    ];
    if let Some(device) = &me.device {
        lines.push(format!("device: {} ({})", device.name, device.id));
    }
    lines
}
