//! The Windows user PATH as one value, read and written through a store that tests replace with a fake.

use crate::error::{CliError, exit};

/// Error code when the user PATH cannot be read; nothing is changed after it.
pub const UNREADABLE: &str = "cli.user_path_unreadable";
/// Error code when Windows refuses to store the user PATH.
pub const NOT_WRITABLE: &str = "cli.user_path_not_writable";

/// The user PATH as stored: its text, and whether Windows expands `%NAME%` references in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathValue {
    pub text: String,
    pub expandable: bool,
}

/// The user PATH. Writing it also tells running programs that the environment changed.
pub trait UserPath: std::fmt::Debug {
    fn read(&self) -> Result<PathValue, CliError>;
    fn write(&self, value: &PathValue) -> Result<(), CliError>;
}

/// The user PATH of a platform that has none: reads and writes both fail, so nothing is changed.
#[derive(Debug)]
pub struct NoUserPath;

impl UserPath for NoUserPath {
    fn read(&self) -> Result<PathValue, CliError> {
        Err(CliError::new(
            UNREADABLE,
            "this platform has no user PATH to read",
            exit::ERROR,
        ))
    }

    fn write(&self, _value: &PathValue) -> Result<(), CliError> {
        Err(CliError::new(
            NOT_WRITABLE,
            "this platform has no user PATH to change",
            exit::ERROR,
        ))
    }
}
