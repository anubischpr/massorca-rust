//! `massorca-fake-update`: a stand-in for a new `massorca` binary in the self-update tests (decision 0132). An example,
//! never shipped.
//!
//! - `--version` prints the text after the last `MASSORCA-FAKE-VERSION:` trailer the test appended to this file's bytes,
//!   or `massorca 9.9.9 (protocol <this build's range>)` without one.
//! - `restart <binary>` writes `<binary>.restarted`, standing in for the supervisor's restart.
//! - Anything else exits 2.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use massorca_protocol::methods::self_update::version_text;
use massorca_protocol::{MIN_PROTOCOL_VERSION, PROTOCOL_VERSION, ProtocolRange};

/// Only the last bytes are searched, so the marker pieces in this program's own data never match.
const TRAILER_WINDOW: usize = 512;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["--version"] => {
            let mut stdout = std::io::stdout();
            match writeln!(stdout, "{}", version_line()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::from(1),
            }
        }
        ["restart", binary] => {
            let mut marker = PathBuf::from(binary).into_os_string();
            marker.push(".restarted");
            match std::fs::write(&marker, b"restarted") {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::from(1),
            }
        }
        _ => ExitCode::from(2),
    }
}

fn version_line() -> String {
    let marker = format!("{}-{}", "MASSORCA-FAKE", "VERSION:");
    let own = std::env::current_exe()
        .and_then(std::fs::read)
        .unwrap_or_default();
    let tail = &own[own.len().saturating_sub(TRAILER_WINDOW)..];
    let tail = String::from_utf8_lossy(tail);
    match tail.rfind(&marker) {
        Some(at) => tail[at + marker.len()..].trim().to_owned(),
        None => format!(
            "massorca {}",
            version_text(
                "9.9.9",
                ProtocolRange {
                    min: MIN_PROTOCOL_VERSION,
                    max: PROTOCOL_VERSION
                }
            )
        ),
    }
}
