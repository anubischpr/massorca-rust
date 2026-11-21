use rusqlite::ErrorCode;
use rusqlite::ffi;

/// Failure of a store operation.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The database was written by a newer build. It is left untouched.
    #[error(
        "database schema version {found} is newer than the version {supported} this build supports"
    )]
    NewerSchema { found: i64, supported: i64 },
    /// A UNIQUE or PRIMARY KEY constraint rejected the write.
    #[error("conflicts with an existing row ({constraint})")]
    Conflict { constraint: String },
    /// Another constraint (foreign key, CHECK, NOT NULL) rejected the write.
    #[error("violates a database constraint ({constraint})")]
    Constraint { constraint: String },
    #[error("database error: {0}")]
    Sqlite(#[source] rusqlite::Error),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        let constraint_kind = match &error {
            rusqlite::Error::SqliteFailure(
                ffi::Error {
                    code: ErrorCode::ConstraintViolation,
                    extended_code,
                },
                message,
            ) => Some((*extended_code, describe_constraint(message.as_deref()))),
            _ => None,
        };
        match constraint_kind {
            Some((
                ffi::SQLITE_CONSTRAINT_UNIQUE | ffi::SQLITE_CONSTRAINT_PRIMARYKEY,
                constraint,
            )) => Self::Conflict { constraint },
            Some((_, constraint)) => Self::Constraint { constraint },
            None => Self::Sqlite(error),
        }
    }
}

fn describe_constraint(message: Option<&str>) -> String {
    let message = message.unwrap_or("constraint");
    message
        .strip_prefix("UNIQUE constraint failed: ")
        .unwrap_or(message)
        .to_owned()
}
