//! Command output: every command writes to stdout and stderr through here, except `terminal attach`, which streams raw terminal bytes.
//!
//! Results go to stdout as text, or as one `{"ok":true,"result":...}` document with `--json`.
//! Errors go to stderr as text, or as one `{"ok":false,"error":...}` document on stdout with `--json`.

use std::io::{self, Write};
use std::process::ExitCode;

use serde::Serialize;
use serde_json::{Value, json};

use crate::error::{CliError, exit};

/// How this run reports: text, or one JSON document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Output {
    json: bool,
}

impl Output {
    pub fn new(json: bool) -> Self {
        Self { json }
    }

    /// Prints a success: `lines` in text mode, the result document in JSON mode.
    pub fn success<T: Serialize>(self, result: &T, lines: &[String]) {
        if self.json {
            write_stdout(&json!({ "ok": true, "result": result }).to_string());
        } else {
            for line in lines {
                write_stdout(line);
            }
        }
    }

    /// Prints a failure: `error: <message> (<code>)` on stderr, or the error document in JSON mode.
    pub fn failure(self, error: &CliError) {
        if self.json {
            write_stdout(&error_document(error).to_string());
        } else {
            write_stderr(&format!("error: {error}"));
        }
    }
}

/// Reports a command line clap rejected, or prints its help or version.
pub fn usage(error: clap::Error, json: bool) -> ExitCode {
    if json && error.use_stderr() {
        let failure = CliError::new(
            "usage.invalid_arguments",
            usage_message(&error),
            exit::USAGE,
        );
        write_stdout(&error_document(&failure).to_string());
        return ExitCode::from(exit::USAGE);
    }
    let _ = error.print();
    ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(exit::USAGE))
}

/// The first line of clap's message, without its `error:` prefix.
fn usage_message(error: &clap::Error) -> String {
    let text = error.to_string();
    text.lines()
        .next()
        .unwrap_or_default()
        .trim_start_matches("error:")
        .trim()
        .to_owned()
}

fn error_document(error: &CliError) -> Value {
    json!({
        "ok": false,
        "error": { "code": error.code, "message": error.message, "data": error.data }
    })
}

fn write_stdout(text: &str) {
    // A closed pipe must not panic the command; its work is already done.
    let _ = writeln!(io::stdout().lock(), "{text}");
}

fn write_stderr(text: &str) {
    let _ = writeln!(io::stderr().lock(), "{text}");
}
