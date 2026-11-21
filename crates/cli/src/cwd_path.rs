//! Paths the user types are made absolute against the CLI's working directory before they reach the service,
//! which resolves no relative path (ADR 0008).

use std::path::Path;

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
