//! The hosted-service commands (SKL-35): `marketplace services list`, `connect` and `disconnect`. The token is not
//! an argument: the CLI reads it from stdin or a named environment variable.

use serde_json::{Value, json};

use super::CommandDescriptor;

pub(super) fn hosted_service_commands() -> Vec<CommandDescriptor> {
    let service = json!({ "enum": massorca_protocol::methods::hosted_services::SERVICE_IDS });
    vec![
        command(
            "marketplace.services.list",
            "List the hosted services and their connection state on this host",
            "marketplace.service.status",
            json!({ "type": "object", "properties": { "service": service } }),
        ),
        command(
            "marketplace.services.connect",
            "Connect a hosted service with a token",
            "marketplace.service.connect",
            json!({
                "type": "object",
                "required": ["service", "token"],
                "properties": {
                    "service": service,
                    "token": { "type": "string", "writeOnly": true },
                    "base_url": { "type": "string" },
                    "email": { "type": "string" }
                }
            }),
        ),
        command(
            "marketplace.services.disconnect",
            "Disconnect a hosted service",
            "marketplace.service.disconnect",
            json!({ "type": "object", "required": ["service"], "properties": { "service": service } }),
        ),
    ]
}

fn command(id: &str, title: &str, method: &str, args_schema: Value) -> CommandDescriptor {
    CommandDescriptor {
        id: id.to_owned(),
        title: title.to_owned(),
        method: method.to_owned(),
        args_schema,
    }
}
