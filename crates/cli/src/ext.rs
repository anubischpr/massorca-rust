//! `massorca ext ...`: reserved for extensions, which arrive in milestone M6.

use crate::error::{CliError, exit};

/// Every extension command fails the same way until milestone M6, without touching the service.
pub fn unavailable() -> CliError {
    CliError::new(
        "extension.not_implemented",
        "extensions arrive in milestone M6",
        exit::ERROR,
    )
}
