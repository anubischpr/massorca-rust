//! `massorca-fake-vendor`: a stand-in for a vendor CLI and its installer, for the extension tests (decision 0062).
//!
//! A test copies it under the name of the CLI it fakes, so `<name> --version` works, and points a package's
//! install and uninstall commands at it. It never touches anything outside the folders its arguments name.
//!
//! - `--version` prints a version line and exits 0.
//! - `install <dir> <name>` copies this program to `<dir>/<name>`, the way a vendor installer puts its CLI on `PATH`.
//! - `install-nothing` exits 0 and installs nothing, so the verify step fails.
//! - `uninstall <dir> <name>` removes `<dir>/<name>`.
//! - `record <file> <text>` appends a line, so a test can see that a command ran and with which arguments.
//! - `fail [code]` writes to stderr and exits with `code` (7 by default).

use std::env;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["--version"] => {
            say("fake-vendor 1.0");
            ExitCode::SUCCESS
        }
        ["install", dir, name] => install(dir, name),
        ["install-nothing"] => ExitCode::SUCCESS,
        ["uninstall", dir, name] => finish(fs::remove_file(target(dir, name))),
        ["record", file, text] => finish(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(file)
                .and_then(|mut file| writeln!(file, "{text}")),
        ),
        ["fail"] => fail(7),
        ["fail", code] => fail(code.parse().unwrap_or(7)),
        _ => {
            complain(&format!("fake-vendor: unknown arguments {words:?}"));
            ExitCode::from(2)
        }
    }
}

/// `<dir>/<name>`, with the platform's executable suffix.
fn target(dir: &str, name: &str) -> PathBuf {
    PathBuf::from(dir).join(format!("{name}{}", env::consts::EXE_SUFFIX))
}

fn install(dir: &str, name: &str) -> ExitCode {
    let copied = env::current_exe().and_then(|exe| fs::copy(exe, target(dir, name)));
    say(&format!("fake-vendor: installed {name} into {dir}"));
    finish(copied.map(|_| ()))
}

fn fail(code: u8) -> ExitCode {
    complain("fake-vendor: failing on purpose");
    ExitCode::from(code)
}

fn finish(result: io::Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            complain(&format!("fake-vendor: {error}"));
            ExitCode::FAILURE
        }
    }
}

/// Writes a line to stdout. A closed pipe is not worth failing over in a test double.
fn say(text: &str) {
    let _ = writeln!(io::stdout(), "{text}");
}

/// Writes a line to stderr.
fn complain(text: &str) {
    let _ = writeln!(io::stderr(), "{text}");
}
