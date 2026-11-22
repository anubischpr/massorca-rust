//! A fake host: programs that resolve, and a runner that records each argv and "installs" a program on demand.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use massorca_core_extensions::host::{CommandRunner, Exit, ProgramResolver};

#[derive(Debug, Default)]
pub struct FakeHost {
    programs: Mutex<HashMap<String, PathBuf>>,
    runs: Mutex<Vec<Vec<String>>>,
    /// Exit codes by the argv's second item (`install`, `uninstall`, `--version`); missing means 0.
    exits: Mutex<HashMap<String, i32>>,
    /// Whether a successful `npm install -g <binary>` makes `<binary>` resolve.
    installs_binary: bool,
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
}

impl ProgramResolver for FakeHost {
    fn resolve(&self, binary: &str) -> Option<PathBuf> {
        self.programs.lock().expect("programs").get(binary).cloned()
    }
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
            && verb == "install"
            && self.installs_binary
            && argv[0] == "npm"
            && let Some(binary) = argv.last()
        {
            self.add_program(binary);
        }
        Ok(Exit {
            code: Some(code),
            timed_out: false,
        })
    }
}

/// Holds every `install` run of a CLI named `slow...` until the test releases it, and reports when each one
/// starts. Everything else runs on the fake host.
pub struct Gate {
    pub host: Arc<FakeHost>,
    started: Mutex<Sender<()>>,
    release: Mutex<Receiver<()>>,
}

impl Gate {
    pub fn new(host: Arc<FakeHost>) -> (Self, Receiver<()>, Sender<()>) {
        let (started_tx, started_rx) = channel();
        let (release_tx, release_rx) = channel();
        let gate = Self {
            host,
            started: Mutex::new(started_tx),
            release: Mutex::new(release_rx),
        };
        (gate, started_rx, release_tx)
    }
}

impl CommandRunner for Gate {
    fn run(&self, argv: &[String], log: &Path, timeout: Duration) -> io::Result<Exit> {
        if argv.get(1).map(String::as_str) == Some("install")
            && argv.last().is_some_and(|name| name.starts_with("slow"))
        {
            let _ = self.started.lock().expect("started").send(());
            let _ = self.release.lock().expect("release").recv();
        }
        self.host.run(argv, log, timeout)
    }
}
