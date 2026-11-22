//! The host side of extension installs (decision 0062): finding a program the way `core-agents` does, and running a
//! vendor command as one argv.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use massorca_core_agents::SearchEnv;
use massorca_core_agents::resolve::resolve;
use massorca_core_extensions::host::{CommandRunner, Exit, MAX_LOG_BYTES, ProgramResolver};
use massorca_core_remote::hide_window;

/// How often a running child is checked against its deadline.
const POLL_INTERVAL: Duration = Duration::from_millis(20);
/// How long the output readers get once the child has ended. A grandchild that still holds a pipe cannot hold up the
/// install longer than this.
const OUTPUT_GRACE: Duration = Duration::from_secs(2);
const TRUNCATED_NOTE: &[u8] = b"\n[output truncated]\n";

/// Resolves programs and runs commands on the service host. `env` is what a bare name is searched in; `None` means the
/// service's own `PATH` and `PATHEXT`, read at each call.
#[derive(Debug, Default)]
pub(crate) struct HostPrograms {
    env: Option<SearchEnv>,
}

impl HostPrograms {
    #[cfg(test)]
    fn searching(env: SearchEnv) -> Self {
        Self { env: Some(env) }
    }

    fn search_env(&self) -> SearchEnv {
        self.env.clone().unwrap_or_else(SearchEnv::current)
    }
}

impl ProgramResolver for HostPrograms {
    fn resolve(&self, binary: &str) -> Option<PathBuf> {
        resolve(binary, &self.search_env())
    }
}

impl CommandRunner for HostPrograms {
    /// Runs `argv` with no shell and no string joining. `argv[0]` is resolved to an absolute path first, so `npm`
    /// finds `npm.cmd` on Windows, where std quotes a batch file's arguments safely. Nothing is elevated. stdin is
    /// null and the output is appended to `log`, `MAX_LOG_BYTES` in all.
    fn run(&self, argv: &[String], log: &Path, timeout: Duration) -> io::Result<Exit> {
        self.run_capped(argv, log, timeout, MAX_LOG_BYTES)
    }
}

impl HostPrograms {
    fn run_capped(
        &self,
        argv: &[String],
        log: &Path,
        timeout: Duration,
        max_log_bytes: u64,
    ) -> io::Result<Exit> {
        let (program, args) = argv
            .split_first()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "the command is empty"))?;
        let resolved = resolve(program, &self.search_env()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("`{program}` was not found on PATH"),
            )
        })?;
        let file = OpenOptions::new().create(true).append(true).open(log)?;
        let used = file.metadata()?.len();
        let sink = Arc::new(Mutex::new(Sink::new(
            file,
            max_log_bytes.saturating_sub(used),
        )));
        sink.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .write(format!("$ {}\n", argv.join(" ")).as_bytes());

        let mut command = Command::new(&resolved);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        hide_window(&mut command);
        let mut child = command.spawn()?;
        let readers: Vec<Receiver<()>> = [
            child
                .stdout
                .take()
                .map(|pipe| copy_to_sink(pipe, Arc::clone(&sink))),
            child
                .stderr
                .take()
                .map(|pipe| copy_to_sink(pipe, Arc::clone(&sink))),
        ]
        .into_iter()
        .flatten()
        .collect();

        let exit = wait_until(&mut child, Instant::now() + timeout)?;
        let grace = Instant::now() + OUTPUT_GRACE;
        for reader in readers {
            let _ = reader.recv_timeout(grace.saturating_duration_since(Instant::now()));
        }
        sink.lock().unwrap_or_else(PoisonError::into_inner).finish();
        Ok(exit)
    }
}

/// Waits for the child, and kills it at the deadline.
fn wait_until(child: &mut Child, deadline: Instant) -> io::Result<Exit> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return Ok(Exit {
                    code: status.code(),
                    timed_out: false,
                });
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Ok(Exit {
                    code: None,
                    timed_out: true,
                });
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        }
    }
}

/// Copies a pipe into the shared log from its own thread; the receiver hears when the pipe closes.
fn copy_to_sink(mut pipe: impl Read + Send + 'static, sink: Arc<Mutex<Sink>>) -> Receiver<()> {
    let (done, finished) = mpsc::channel();
    thread::spawn(move || {
        let mut chunk = [0_u8; 8192];
        while let Ok(read) = pipe.read(&mut chunk) {
            if read == 0 {
                break;
            }
            sink.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .write(&chunk[..read]);
        }
        let _ = done.send(());
    });
    finished
}

/// The log file with a byte budget. Output past the budget is dropped, and one note says so.
struct Sink {
    file: File,
    remaining: u64,
    /// Whether the budget left room for the note when it was set.
    room_for_note: bool,
    truncated: bool,
}

impl Sink {
    fn new(file: File, budget: u64) -> Self {
        Self {
            file,
            // The note must fit inside the cap too.
            remaining: budget.saturating_sub(TRUNCATED_NOTE.len() as u64),
            room_for_note: budget >= TRUNCATED_NOTE.len() as u64,
            truncated: false,
        }
    }

    /// Appends as much of `bytes` as the budget allows. A write error only loses log text, never the install.
    fn write(&mut self, bytes: &[u8]) {
        let take =
            usize::try_from(self.remaining).map_or(bytes.len(), |left| left.min(bytes.len()));
        if take < bytes.len() {
            self.truncated = true;
        }
        if take > 0 && self.file.write_all(&bytes[..take]).is_ok() {
            self.remaining -= take as u64;
        }
    }

    fn finish(&mut self) {
        if self.truncated && self.room_for_note {
            let _ = self.file.write_all(TRUNCATED_NOTE);
        }
        self.truncated = false;
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    const LONG: Duration = Duration::from_secs(60);

    fn strings(argv: &[&str]) -> Vec<String> {
        argv.iter().map(|part| (*part).to_owned()).collect()
    }

    /// A command that prints `text` and exits with `code`, built for the host shell.
    fn say(text: &str, code: i32) -> Vec<String> {
        if cfg!(windows) {
            strings(&["cmd", "/D", "/C", &format!("echo {text}& exit {code}")])
        } else {
            strings(&["sh", "-c", &format!("echo {text}; exit {code}")])
        }
    }

    fn host() -> HostPrograms {
        HostPrograms::default()
    }

    #[test]
    fn output_is_appended_and_the_exit_code_reported() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("install.log");

        let first = host().run(&say("one", 0), &log, LONG).expect("it runs");
        let second = host().run(&say("two", 3), &log, LONG).expect("it runs");

        assert!(first.success());
        assert_eq!(second.code, Some(3));
        let text = fs::read_to_string(&log).expect("the log");
        let one = text.find("one").expect("first run is logged");
        let two = text.find("two").expect("second run is logged");
        assert!(one < two, "the second run is appended: {text:?}");
        assert!(
            text.contains("$ "),
            "each run starts with its argv: {text:?}"
        );
    }

    #[test]
    fn a_missing_program_and_an_empty_command_are_errors() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");

        let missing = host().run(&strings(&["massorca-no-such-program"]), &log, LONG);
        let empty = host().run(&[], &log, LONG);

        assert_eq!(
            missing.expect_err("no program").kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(
            empty.expect_err("no argv").kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn a_relative_path_never_resolves() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");

        let error = host()
            .run(&strings(&["./planted"]), &log, LONG)
            .expect_err("a relative program is refused");

        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn output_past_the_cap_is_dropped_with_a_note() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");
        let noisy = if cfg!(windows) {
            strings(&["cmd", "/D", "/C", &format!("echo {}", "x".repeat(300))])
        } else {
            strings(&["sh", "-c", &format!("echo {}", "x".repeat(300))])
        };

        let exit = host().run_capped(&noisy, &log, LONG, 100).expect("it runs");

        assert!(exit.success());
        let text = fs::read(&log).expect("the log");
        assert!(text.len() <= 100, "the log has {} bytes", text.len());
        assert!(text.ends_with(TRUNCATED_NOTE));
    }

    #[test]
    fn a_log_already_at_its_cap_gets_nothing_more() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");
        fs::write(&log, [b'a'; 64]).expect("a full log");

        host()
            .run_capped(&say("late", 0), &log, LONG, 64)
            .expect("it runs");

        assert_eq!(fs::read(&log).expect("the log").len(), 64);
    }

    #[test]
    fn a_child_past_its_timeout_is_killed_and_reported() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");
        let sleeper = if cfg!(windows) {
            strings(&["cmd", "/D", "/C", "ping -n 30 127.0.0.1 >NUL"])
        } else {
            strings(&["sh", "-c", "sleep 30"])
        };
        let started = Instant::now();

        let exit = host()
            .run(&sleeper, &log, Duration::from_millis(300))
            .expect("it runs");

        assert!(exit.timed_out && !exit.success());
        assert!(started.elapsed() < Duration::from_secs(20), "it was killed");
    }

    #[cfg(unix)]
    #[test]
    fn arguments_reach_the_program_as_given() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");
        let argv = strings(&[
            "sh",
            "-c",
            "printf '[%s][%s]' \"$1\" \"$2\"",
            "sh",
            "a b",
            "$HOME;x",
        ]);

        host().run(&argv, &log, LONG).expect("it runs");

        let text = fs::read_to_string(&log).expect("the log");
        assert!(text.contains("[a b][$HOME;x]"), "{text:?}");
    }

    #[cfg(windows)]
    #[test]
    fn a_batch_file_gets_its_arguments_unexpanded() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");
        let script = dir.path().join("vendor.cmd");
        fs::write(&script, "@echo off\r\necho got [%1]\r\n").expect("a batch file");
        let argv = vec![script.to_string_lossy().into_owned(), "a&b".to_owned()];

        let exit = host().run(&argv, &log, LONG).expect("it runs");

        assert!(exit.success());
        let text = fs::read_to_string(&log).expect("the log");
        assert!(
            text.contains("got [\"a&b\"]") || text.contains("got [a&b]"),
            "{text:?}"
        );
    }

    #[test]
    fn a_bare_name_resolves_in_the_search_path_only() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let name = if cfg!(windows) {
            "vendorcli.exe"
        } else {
            "vendorcli"
        };
        fs::write(dir.path().join(name), "").expect("a program file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(dir.path().join(name), fs::Permissions::from_mode(0o755))
                .expect("executable");
        }
        let found = HostPrograms::searching(SearchEnv {
            path: Some(dir.path().as_os_str().to_owned()),
            pathext: None,
        });
        let empty = HostPrograms::searching(SearchEnv::default());

        let resolved = found.resolve("vendorcli").expect("it resolves");
        assert!(
            resolved
                .to_string_lossy()
                .eq_ignore_ascii_case(&dir.path().join(name).to_string_lossy()),
            "{resolved:?}"
        );
        assert_eq!(empty.resolve("vendorcli"), None);
    }
}
