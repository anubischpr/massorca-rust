//! `massorca-fake-vendor`: a stand-in for the package managers and CLIs the built-in agents use (0062), for the extension
//! tests. It is an example, not a shipped binary.
//!
//! The tests copy it under the names of the programs the built-ins' commands start (`npm`, `winget`, `brew`,
//! `powershell`, `bash`) and under each CLI's name.
//!
//! - As a package manager it installs the CLI a built-in's command names into the first folder of its `PATH`, the way a
//!   real install puts the CLI on `PATH`, and removes it for an uninstall. A file `fail-install` in that folder makes the
//!   install fail with exit 7.
//! - As a CLI it answers `--version` with exit 0. A file `fail-verify` in its folder makes that exit 1.
//! - Anything else exits 2.

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

const PACKAGE_MANAGERS: [&str; 5] = ["npm", "winget", "brew", "powershell", "bash"];

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    let name = env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    let folder = first_path_folder();
    if PACKAGE_MANAGERS.contains(&name.as_str()) {
        return package_manager(&folder, &words);
    }
    match words.as_slice() {
        ["--version"] if folder.join("fail-verify").exists() => {
            complain("fake-vendor: the version check fails on purpose");
            ExitCode::from(1)
        }
        ["--version"] => {
            say("fake-vendor 1.0");
            ExitCode::SUCCESS
        }
        _ => {
            complain(&format!("fake-vendor: unknown arguments {words:?}"));
            ExitCode::from(2)
        }
    }
}

/// The CLI a command installs or removes, by the package or installer it names.
fn cli_named_by(words: &[&str]) -> Option<&'static str> {
    const PACKAGES: [(&str, &str); 5] = [
        ("@openai/codex", "codex"),
        ("@anthropic-ai/claude-code", "claude"),
        ("Anthropic.ClaudeCode", "claude"),
        ("claude-code", "claude"),
        ("antigravity.google", "agy"),
    ];
    PACKAGES
        .iter()
        .find(|(package, _)| words.iter().any(|word| word.contains(package)))
        .map(|(_, cli)| *cli)
}

fn package_manager(folder: &PathBuf, words: &[&str]) -> ExitCode {
    let Some(cli) = cli_named_by(words) else {
        complain(&format!("fake-vendor: no CLI is named by {words:?}"));
        return ExitCode::from(2);
    };
    let target = folder.join(format!("{cli}{}", env::consts::EXE_SUFFIX));
    if words.contains(&"uninstall") {
        return finish(fs::remove_file(target));
    }
    if words
        .iter()
        .any(|word| *word == "install" || word.contains("install."))
    {
        if folder.join("fail-install").exists() {
            complain("fake-vendor: the install fails on purpose");
            return ExitCode::from(7);
        }
        let copied = env::current_exe().and_then(|exe| fs::copy(exe, &target));
        say(&format!("fake-vendor: installed {cli}"));
        return finish(copied.map(|_| ()));
    }
    complain(&format!("fake-vendor: unknown arguments {words:?}"));
    ExitCode::from(2)
}

/// The first folder on `PATH`: where the CLIs of a test's service are installed.
fn first_path_folder() -> PathBuf {
    env::var_os("PATH")
        .and_then(|path| env::split_paths(&path).next())
        .unwrap_or_else(|| PathBuf::from("."))
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
