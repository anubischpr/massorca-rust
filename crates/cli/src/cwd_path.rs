//! Paths the user types are made absolute against the CLI's working directory before they reach the service,
//! which resolves no relative path (ADR 0008).

use std::path::Path;

use crate::target::Target;

use crate::error::{CliError, exit};

/// A selector as the service should see it: a path-like selector (one with a separator, or one
/// that starts with a dot) becomes absolute against this directory, so the service never resolves
/// it against its own. Ids and names pass through unchanged.
pub fn selector(text: &str) -> Result<String, CliError> {
    if text.contains(['/', '\\']) || text.starts_with('.') {
        absolute(Path::new(text))
    } else {
        Ok(text.to_owned())
    }
}

/// `path` as absolute text. A relative path is joined to the current directory.
pub fn absolute(path: &Path) -> Result<String, CliError> {
    let resolved = std::path::absolute(path).map_err(|error| {
        CliError::new(
            "usage.invalid_path",
            format!("cannot resolve {}: {error}", path.display()),
            exit::USAGE,
        )
    })?;
    resolved.into_os_string().into_string().map_err(|_| {
        CliError::new(
            "usage.invalid_path",
            format!("{} is not valid UTF-8", path.display()),
            exit::USAGE,
        )
    })
}

/// How the paths of one command are passed on: made absolute here for the local service, or sent as typed to a
/// remote host, whose file system this computer cannot resolve against (decision 0046).
#[derive(Debug, Clone, Copy)]
pub struct PathRules {
    remote: bool,
}

impl PathRules {
    pub fn for_target(target: &Target) -> Self {
        Self {
            remote: !target.is_local(),
        }
    }

    pub fn selector(self, text: &str) -> Result<String, CliError> {
        if self.remote {
            Ok(text.to_owned())
        } else {
            selector(text)
        }
    }

    /// A remote path must already be absolute on the host; the service refuses a relative one.
    pub fn path(self, path: &Path) -> Result<String, CliError> {
        if !self.remote {
            return absolute(path);
        }
        path.to_str().map(str::to_owned).ok_or_else(|| {
            CliError::new(
                "usage.invalid_path",
                format!("{} is not valid UTF-8", path.display()),
                exit::USAGE,
            )
        })
    }
}
