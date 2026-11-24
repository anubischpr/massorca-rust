//! `massorca-fake-desktop`: a stand-in for `massorca-desktop` in the tests of `massorca open`.
//!
//! It writes its arguments as a JSON array to the file `MASSORCA_FAKE_DESKTOP_RECORD` names, then waits until the file
//! `MASSORCA_FAKE_DESKTOP_RELEASE` names exists. It writes `<release>.exited` as it leaves, so a test knows it is gone
//! before it removes the folder both files are in.

use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

/// Where the arguments are recorded; unset means nothing is recorded.
const RECORD_ENV: &str = "MASSORCA_FAKE_DESKTOP_RECORD";
/// The file whose existence lets the stand-in exit; unset means it exits at once.
const RELEASE_ENV: &str = "MASSORCA_FAKE_DESKTOP_RELEASE";
/// The longest it waits for its release, so a test that fails early leaves nothing running for long.
const MAX_WAIT: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(20);

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args_os()
        .skip(1)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    if let Some(record) = env::var_os(RECORD_ENV)
        && record_arguments(Path::new(&record), &arguments).is_err()
    {
        return ExitCode::FAILURE;
    }
    if let Some(release) = env::var_os(RELEASE_ENV) {
        let release = Path::new(&release);
        wait_for(release);
        // Written last, so a test that sees it knows nothing of this process is still running.
        let _ = fs::write(release.with_extension("exited"), b"");
    }
    ExitCode::SUCCESS
}

/// Writes the arguments under a staging name first, so a reader never sees a partly written record.
fn record_arguments(record: &Path, arguments: &[String]) -> io::Result<()> {
    let text = serde_json::to_string(arguments).map_err(io::Error::other)?;
    let staged = record.with_extension("staged");
    fs::write(&staged, text)?;
    fs::rename(staged, record)
}

fn wait_for(release: &Path) {
    let deadline = Instant::now() + MAX_WAIT;
    while !release.exists() && Instant::now() < deadline {
        thread::sleep(POLL);
    }
}
