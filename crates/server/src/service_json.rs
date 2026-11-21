//! `service.json`: what the service is, for people and diagnostics. Clients connect by endpoint name and never read it.

use std::fs;
use std::io;

use massorca_profile::Profile;
use massorca_protocol::{MIN_PROTOCOL_VERSION, PROTOCOL_VERSION};
use serde_json::json;

/// Writes the file once the endpoint is bound. It stays in place after shutdown.
pub(crate) fn write(profile: &Profile, version: &str, started_at_ms: i64) -> io::Result<()> {
    let info = json!({
        "pid": std::process::id(),
        "version": version,
        "protocol": { "min": MIN_PROTOCOL_VERSION, "max": PROTOCOL_VERSION },
        "profile": profile.name(),
        "started_at_ms": started_at_ms,
    });
    let bytes = serde_json::to_vec_pretty(&info).map_err(io::Error::other)?;
    fs::write(profile.info_path(), bytes)
}
