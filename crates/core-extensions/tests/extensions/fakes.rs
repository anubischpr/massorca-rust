//! A fake host: programs that resolve, and a runner that records each argv. A package-manager command that installs or
//! removes a built-in's CLI (the commands the built-in catalogue declares) installs or removes that CLI in the fake, so
//! a test can see the CLI resolve afterwards. No real program runs.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use massorca_core_extensions::host::{CommandRunner, Exit, ProgramResolver, ProgramSearch};

#[derive(Debug, Default)]
pub struct FakeHost {
    programs: Mutex<HashMap<String, PathBuf>>,
    runs: Mutex<Vec<Vec<String>>>,
    /// Exit codes by the argv's second item (`install`, `uninstall`, `--version`); missing means 0.
    exits: Mutex<HashMap<String, i32>>,
    /// Whether a successful install of a built-in's CLI makes it resolve.
    installs_binary: bool,
    /// Where the fake searches for a bare program name, as the host reports it.
    search: ProgramSearch,
}

impl FakeHost {
    pub fn new() -> Self {
        Self {
            installs_binary: true,
            ..Self::default()
        }
    }

    pub fn with_program(self, name: &str) -> Self {
        self.add_program(name);
        self
    }

    /// The search path the fake reports: its source (`process` or `registry`) and its directories, in order.
    pub fn searching(mut self, source: &str, dirs: &[&str]) -> Self {
        self.search = ProgramSearch {
            source: source.to_owned(),
            dirs: dirs.iter().map(|dir| (*dir).to_owned()).collect(),
        };
        self
    }

    /// The installer succeeds but the program still does not resolve.
    pub fn without_binary(mut self) -> Self {
        self.installs_binary = false;
        self
    }

    pub fn exit_for(self, verb: &str, code: i32) -> Self {
        self.exits
            .lock()
            .expect("exits")
            .insert(verb.to_owned(), code);
        self
    }

    pub fn runs(&self) -> Vec<Vec<String>> {
        self.runs.lock().expect("runs").clone()
    }

    pub fn add_program(&self, name: &str) {
        self.programs.lock().expect("programs").insert(
            name.to_owned(),
            PathBuf::from(format!("C:/Tools/{name}.exe")),
        );
    }

    pub fn remove_program(&self, name: &str) {
        self.programs.lock().expect("programs").remove(name);
    }
}

impl ProgramResolver for FakeHost {
    fn resolve(&self, binary: &str) -> Option<PathBuf> {
        self.programs.lock().expect("programs").get(binary).cloned()
    }

    fn search(&self) -> ProgramSearch {
        self.search.clone()
    }
}

/// The CLI a built-in's install or uninstall command names: the program itself when it is that CLI's own binary (decision
/// 0164), else the CLI of the package it names.
pub fn cli_named_by(argv: &[String]) -> Option<&'static str> {
    if argv
        .first()
        .is_some_and(|program| program == "gentle-shell")
    {
        return Some("gentle-shell");
    }
    const PACKAGES: [(&str, &str); 9] = [
        ("@openai/codex", "codex"),
        ("@anthropic-ai/claude-code", "claude"),
        ("Anthropic.ClaudeCode", "claude"),
        ("claude-code", "claude"),
        ("antigravity.google", "agy"),
        ("gentle-pi", "gentle-shell"),
        ("Git.Git", "git"),
        ("GitHub.cli", "gh"),
        ("OpenJS.NodeJS.LTS", "node"),
    ];
    PACKAGES
        .iter()
        .find(|(package, _)| argv.iter().any(|part| part.contains(package)))
        .map(|(_, cli)| *cli)
}

impl CommandRunner for FakeHost {
    fn run(&self, argv: &[String], log: &Path, _timeout: Duration) -> io::Result<Exit> {
        self.runs.lock().expect("runs").push(argv.to_vec());
        let verb = argv.get(1).cloned().unwrap_or_default();
        let code = self
            .exits
            .lock()
            .expect("exits")
            .get(&verb)
            .copied()
            .unwrap_or(0);
        let mut file = OpenOptions::new().create(true).append(true).open(log)?;
        writeln!(file, "ran {} -> {code}", argv.join(" "))?;
        if code == 0
            && self.installs_binary
            && let Some(cli) = cli_named_by(argv)
        {
            if argv
                .iter()
                .any(|part| part == "uninstall" || part == "self-uninstall")
            {
                self.remove_program(cli);
            } else if argv
                .iter()
                .any(|part| part == "install" || part.contains("install."))
            {
                self.add_program(cli);
            }
        }
        Ok(Exit {
            code: Some(code),
            timed_out: false,
        })
    }
}

/// Holds the `install` run of the built-in CLI `cli` until the test releases it, and reports when it starts. Every
/// other run goes straight to the fake host.
pub struct Gate {
    pub host: Arc<FakeHost>,
    cli: &'static str,
    started: Mutex<Sender<()>>,
    release: Mutex<Receiver<()>>,
}

impl Gate {
    pub fn new(host: Arc<FakeHost>, cli: &'static str) -> (Self, Receiver<()>, Sender<()>) {
        let (started_tx, started_rx) = channel();
        let (release_tx, release_rx) = channel();
        let gate = Self {
            host,
            cli,
            started: Mutex::new(started_tx),
            release: Mutex::new(release_rx),
        };
        (gate, started_rx, release_tx)
    }
}

impl CommandRunner for Gate {
    fn run(&self, argv: &[String], log: &Path, timeout: Duration) -> io::Result<Exit> {
        if cli_named_by(argv) == Some(self.cli) && argv.iter().any(|part| part == "install") {
            let _ = self.started.lock().expect("started").send(());
            let _ = self.release.lock().expect("release").recv();
        }
        self.host.run(argv, log, timeout)
    }
}
