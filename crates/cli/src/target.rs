//! Which service a command talks to: the local service of a profile, or a paired host (decision 0046).

use massorca_profile::Profile;

use crate::error::{CliError, exit};

/// The `--host` value that means the local service.
const LOCAL: &str = "local";

/// The profile a command runs in, and the paired host it addresses. No host means the local service.
#[derive(Debug, Clone)]
pub struct Target {
    pub profile: Profile,
    pub host: Option<String>,
}

impl Target {
    /// `--host local` and an empty `--host` both address the local service.
    pub fn new(profile: Profile, host: Option<String>) -> Self {
        Self {
            profile,
            host: host.filter(|name| is_remote(name.as_str())),
        }
    }

    /// Whether the command addresses this computer's own service.
    pub fn is_local(&self) -> bool {
        self.host.is_none()
    }
}

fn is_remote(name: &str) -> bool {
    !name.is_empty() && name != LOCAL
}

/// Refuses `--host` for a command that works on this computer only.
pub fn reject_host(host: Option<&str>, command: &str) -> Result<(), CliError> {
    match host {
        Some(name) if is_remote(name) => Err(CliError::new(
            "cli.host_not_supported",
            format!("{command} works on this computer only; remove --host"),
            exit::USAGE,
        )),
        _ => Ok(()),
    }
}
