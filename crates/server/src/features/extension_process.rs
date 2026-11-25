//! The host side of extension installs (decision 0062): finding a program the way `core-agents` does, and running a
//! vendor command as one argv.

use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use massorca_core_agents::SearchEnv;
use massorca_core_agents::resolve::{PathSource, resolve, vendor_search_env};
use massorca_core_extensions::host::{
    CommandRunner, Exit, MAX_LOG_BYTES, ProgramResolver, ProgramSearch,
};

/// How often a running child is checked against its deadline.
const POLL_INTERVAL: Duration = Duration::from_millis(20);
/// How long the output readers get once the child has ended. A grandchild that still holds a pipe cannot hold up the
/// install longer than this.
const OUTPUT_GRACE: Duration = Duration::from_secs(2);
const TRUNCATED_NOTE: &[u8] = b"\n[output truncated]\n";

/// The variables a vendor command gets from the service environment; every other one, such as a token the service
/// was started with, is withheld.
const PASSED_ENV: &[&str] = &[
    "PATH",
    "PATHEXT",
    "HOME",
    "USERPROFILE",
    "APPDATA",
    "LOCALAPPDATA",
    "TEMP",
    "TMP",
    "TMPDIR",
    "SystemRoot",
    "SYSTEMDRIVE",
    "ComSpec",
    "LANG",
    "LC_ALL",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
];

/// Resolves programs and runs commands on the service host. `env` is what a bare name is searched in; `None` means the
/// service's own `PATH` and `PATHEXT`, read at each call. `service_env` is what [`PASSED_ENV`] is copied from; `None`
/// means the service's own environment.
#[derive(Debug, Default)]
pub(crate) struct HostPrograms {
    env: Option<SearchEnv>,
    service_env: Option<HashMap<String, OsString>>,
}

impl HostPrograms {
    #[cfg(test)]
    fn searching(env: SearchEnv) -> Self {
        Self {
            env: Some(env),
            service_env: None,
        }
    }

    #[cfg(test)]
    fn with_service_env(env: HashMap<String, OsString>) -> Self {
        Self {
            env: None,
            service_env: Some(env),
        }
    }

    /// The search path without relative entries (empty, `.`, `bin`), which would resolve against the service's
    /// working folder. Only this host side filters them; core-agents keeps its own rule.
    fn search_env(&self) -> SearchEnv {
        self.search_path().0
    }

    /// The search path of bare names, absolute entries only, and where it came from (decision 0092).
    fn search_path(&self) -> (SearchEnv, PathSource) {
        let (mut env, source) = match &self.env {
            Some(env) => (env.clone(), PathSource::Process),
            None => vendor_search_env(),
        };
        env.path = env.path.as_deref().map(absolute_entries);
        (env, source)
    }

    /// The allowlisted variables of the service environment. `PATH` is the search path the program was resolved on, so
    /// a shim finds what it calls and no relative entry reaches the command.
    fn passed_env(&self, search: &SearchEnv) -> Vec<(&'static str, OsString)> {
        PASSED_ENV
            .iter()
            .filter_map(|name| {
                let value = if *name == "PATH" {
                    search.path.clone()
                } else {
                    match &self.service_env {
                        Some(env) => env.get(*name).cloned(),
                        None => std::env::var_os(name),
                    }
                };
                value.map(|value| (*name, value))
            })
            .collect()
    }
}

/// The absolute entries of a search path, in order.
fn absolute_entries(path: &OsStr) -> OsString {
    std::env::join_paths(std::env::split_paths(path).filter(|entry| entry.is_absolute()))
        .unwrap_or_default()
}

impl ProgramResolver for HostPrograms {
    fn resolve(&self, binary: &str) -> Option<PathBuf> {
        resolve(binary, &self.search_env())
    }

    fn search(&self) -> ProgramSearch {
        let (env, source) = self.search_path();
        let dirs = env.path.as_deref().map_or_else(Vec::new, |path| {
            std::env::split_paths(path)
                .map(|dir| dir.to_string_lossy().into_owned())
                .collect()
        });
        ProgramSearch {
            source: source.name().to_owned(),
            dirs,
        }
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
    /// One run. The log gets the argv line and the exit status (or the timeout) whatever happens, and up to
    /// `max_log_bytes` of output of this run; an earlier run's bytes never count against it.
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
        if program.contains(['/', '\\']) || Path::new(program).is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("`{program}` names a path; a vendor command names a program only"),
            ));
        }
        let file = OpenOptions::new().create(true).append(true).open(log)?;
        let sink = Arc::new(Mutex::new(Sink::new(file, max_log_bytes)));
        sink.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .line(&format!("$ {}", argv.join(" ")));
        let fail = |error: io::Error| {
            sink.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .line(&format!("could not start: {error}"));
            error
        };
        // One search path for both steps: the program is resolved on it and the command gets it as its PATH.
        let search = self.search_env();
        let resolved = resolve(program, &search).ok_or_else(|| {
            fail(io::Error::new(
                io::ErrorKind::NotFound,
                format!("`{program}` was not found on PATH"),
            ))
        })?;
        // A fresh folder per run, so a command never starts in the service's own working folder.
        let work = WorkDir::create().map_err(&fail)?;

        let mut command = massorca_process::command(&resolved);
        command
            .args(args)
            .current_dir(work.path())
            .env_clear()
            .envs(self.passed_env(&search))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let (mut child, tree) = ProcessTree::spawn(&mut command).map_err(&fail)?;
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

        let waited = wait_until(&mut child, tree, Instant::now() + timeout);
        let grace = Instant::now() + OUTPUT_GRACE;
        for reader in readers {
            let _ = reader.recv_timeout(grace.saturating_duration_since(Instant::now()));
        }
        let mut sink = sink.lock().unwrap_or_else(PoisonError::into_inner);
        let status = match &waited {
            Ok(exit) if exit.timed_out => "timed out and was stopped".to_owned(),
            Ok(exit) => format!(
                "exit {}",
                exit.code
                    .map_or_else(|| "none".to_owned(), |code| code.to_string())
            ),
            Err(error) => format!("could not be waited on: {error}"),
        };
        sink.line(&status);
        sink.finish();
        waited
    }
}

/// A fresh folder a vendor command starts in, named for this run and removed when the run ends. It is private to the
/// service's user where the platform allows it (0062, second review).
struct WorkDir(PathBuf);

static WORK_COUNTER: AtomicU64 = AtomicU64::new(0);

impl WorkDir {
    fn create() -> io::Result<Self> {
        for _ in 0..16 {
            let path = std::env::temp_dir().join(format!(
                "massorca-vendor-{}-{}",
                std::process::id(),
                WORK_COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            match create_private_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "no free folder name for a vendor command",
        ))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        // A process that outlived its command may still hold files open; the folder is then left for the OS.
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn create_private_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(path)
    }
    #[cfg(not(unix))]
    {
        fs::create_dir(path)
    }
}

/// Waits for the child, and kills it with everything it started at the deadline.
fn wait_until(child: &mut Child, tree: ProcessTree, deadline: Instant) -> io::Result<Exit> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                tree.release();
                return Ok(Exit {
                    code: status.code(),
                    timed_out: false,
                });
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(POLL_INTERVAL),
            Ok(None) => {
                tree.kill(child);
                return Ok(Exit {
                    code: None,
                    timed_out: true,
                });
            }
            Err(error) => {
                tree.kill(child);
                return Err(error);
            }
        }
    }
}

/// The child and every process it starts: its own process group on Unix, a kill-on-close job on Windows. A process
/// that detaches itself (a new session, a job breakaway) escapes.
struct ProcessTree {
    /// `None` when the job could not be created or assigned; then only the child is killed.
    #[cfg(windows)]
    job: Option<win32job::Job>,
}

impl ProcessTree {
    #[cfg(unix)]
    fn spawn(command: &mut Command) -> io::Result<(Child, Self)> {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
        Ok((command.spawn()?, Self {}))
    }

    /// The child starts suspended inside a kill-on-close job and runs only once it is in the job, so nothing a vendor
    /// installer starts can escape the job (decision 0062, second review).
    #[cfg(windows)]
    fn spawn(command: &mut Command) -> io::Result<(Child, Self)> {
        let (child, job) = massorca_core_remote::windows_job::spawn_in_job(command, 0)?;
        if job.is_none() {
            tracing::warn!(
                "no job object for a vendor command: a timeout ends only the command itself"
            );
        }
        Ok((child, Self { job }))
    }

    #[cfg(not(any(unix, windows)))]
    fn spawn(command: &mut Command) -> io::Result<(Child, Self)> {
        Ok((command.spawn()?, Self {}))
    }

    /// Kills the tree, then reaps the child. The group is signalled before the child is reaped, so its id cannot have
    /// been reused.
    fn kill(self, child: &mut Child) {
        #[cfg(unix)]
        {
            // The child leads its own group (`process_group(0)`), so its id is the group id.
            if let Some(group) = i32::try_from(child.id())
                .ok()
                .and_then(rustix::process::Pid::from_raw)
                && let Err(error) =
                    rustix::process::kill_process_group(group, rustix::process::Signal::KILL)
                && error != rustix::io::Errno::SRCH
            {
                tracing::warn!(error = ?error, "a vendor command's process group could not be killed");
            }
        }
        // Closing a kill-on-close job ends every process in it.
        #[cfg(windows)]
        drop(self.job);
        #[cfg(not(windows))]
        let _ = self;
        let _ = child.kill();
        let _ = child.wait();
    }

    /// After a normal exit, leaves anything the child left running alone, as a plain spawn would.
    fn release(self) {
        #[cfg(windows)]
        if let Some(job) = self.job {
            let _ = job.set_extended_limit_info(win32job::ExtendedLimitInfo::new().clear_limits());
        }
        #[cfg(not(windows))]
        let _ = self;
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

/// The log file with a byte budget. Output past the budget is dropped, and one note says so. `finish` closes the file,
/// so a reader still draining a pipe some leftover process holds writes nothing more.
struct Sink {
    file: Option<File>,
    remaining: u64,
    /// Whether the budget left room for the note when it was set.
    room_for_note: bool,
    truncated: bool,
}

impl Sink {
    fn new(file: File, budget: u64) -> Self {
        Self {
            file: Some(file),
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
        if take > 0
            && let Some(file) = self.file.as_mut()
            && file.write_all(&bytes[..take]).is_ok()
        {
            self.remaining -= take as u64;
        }
    }

    /// Writes a line the budget never drops: the argv and the exit status of every run are always logged.
    fn line(&mut self, text: &str) {
        if let Some(file) = self.file.as_mut() {
            let _ = writeln!(file, "{text}");
        }
    }

    fn finish(&mut self) {
        if let Some(mut file) = self.file.take()
            && self.truncated
            && self.room_for_note
        {
            let _ = file.write_all(TRUNCATED_NOTE);
        }
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

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
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
        // The first line is the argv, always logged; the budget applies to the output after it.
        let output = text
            .split(|byte| *byte == b'\n')
            .skip(1)
            .collect::<Vec<_>>()
            .concat();
        let kept = output.iter().filter(|byte| **byte == b'x').count();
        assert!(kept <= 100, "{kept} output bytes kept of a budget of 100");
        assert!(
            text.windows(TRUNCATED_NOTE.len())
                .any(|window| window == TRUNCATED_NOTE)
        );
    }

    #[test]
    fn each_run_gets_its_own_budget_of_output() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");
        fs::write(&log, [b'a'; 64]).expect("an earlier run's log");

        host()
            .run_capped(&say("late", 0), &log, LONG, 64)
            .expect("it runs");

        let text = fs::read_to_string(&log).expect("the log");
        assert!(text.contains("late"), "{text:?}");
    }

    #[test]
    fn the_argv_and_exit_status_are_logged_even_when_the_output_budget_is_spent() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");

        host()
            .run_capped(&say(&"y".repeat(300), 3), &log, LONG, 0)
            .expect("it runs");

        let text = fs::read_to_string(&log).expect("the log");
        assert!(text.starts_with("$ "), "{text:?}");
        assert!(text.contains("exit 3"), "{text:?}");
    }

    #[test]
    fn a_vendor_command_starts_in_a_fresh_folder_removed_after_the_run() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");
        let pwd = if cfg!(windows) {
            strings(&["cmd", "/D", "/C", "cd"])
        } else {
            strings(&["sh", "-c", "pwd"])
        };

        host().run(&pwd, &log, LONG).expect("it runs");

        let text = fs::read_to_string(&log).expect("the log");
        let started_in = text
            .lines()
            .map(str::trim)
            .find(|line| line.contains("massorca-vendor-"))
            .expect("the folder the command printed");
        assert_ne!(
            Path::new(started_in),
            std::env::current_dir().expect("the service folder")
        );
        assert!(!Path::new(started_in).exists(), "the folder is removed");
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
        let argv = vec!["vendor.cmd".to_owned(), "a&b".to_owned()];
        let found = HostPrograms {
            env: Some(SearchEnv {
                path: dir
                    .path()
                    .parent()
                    .map(|_| dir.path().as_os_str().to_owned()),
                pathext: None,
            }),
            service_env: None,
        };

        let exit = found.run(&argv, &log, LONG).expect("it runs");

        assert!(exit.success());
        let text = fs::read_to_string(&log).expect("the log");
        assert!(
            text.contains("got [\"a&b\"]") || text.contains("got [a&b]"),
            "{text:?}"
        );
    }

    /// The allowlisted part of the real environment, plus `extra`, as the service environment a run copies from.
    fn service_env(extra: &[(&str, &OsStr)]) -> HashMap<String, OsString> {
        let mut env: HashMap<String, OsString> = PASSED_ENV
            .iter()
            .filter_map(|name| std::env::var_os(name).map(|value| ((*name).to_owned(), value)))
            .collect();
        for (name, value) in extra {
            env.insert((*name).to_owned(), (*value).to_owned());
        }
        env
    }

    #[test]
    fn only_allowlisted_variables_reach_the_command() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");
        let host = HostPrograms::with_service_env(service_env(&[
            ("MASSORCA_TEST_SECRET", OsStr::new("hunter2-token")),
            ("HTTPS_PROXY", OsStr::new("http://proxy.invalid:1")),
        ]));
        let print_env = if cfg!(windows) {
            strings(&["cmd", "/D", "/C", "set"])
        } else {
            strings(&["sh", "-c", "env"])
        };

        host.run(&print_env, &log, LONG).expect("it runs");

        let text = fs::read_to_string(&log).expect("the log");
        assert!(text.contains("proxy.invalid"), "{text}");
        assert!(!text.contains("hunter2-token"), "{text}");
    }

    #[test]
    fn relative_search_path_entries_are_skipped() {
        let absolute = std::env::temp_dir();
        let path = std::env::join_paths([
            PathBuf::from("."),
            PathBuf::from("relative/bin"),
            absolute.clone(),
        ])
        .expect("a search path");

        let kept: Vec<PathBuf> = std::env::split_paths(&absolute_entries(&path)).collect();

        assert_eq!(kept, vec![absolute]);
        assert!(absolute_entries(OsStr::new("")).is_empty());
    }

    const TREE_MARK: &str = "massorca-tree-test";
    const TREE_TEST: &str = "features::extension_process::tests::";

    /// The temporary folder of a tree test run, when this process is one of its children.
    fn tree_dir() -> Option<PathBuf> {
        let dir = PathBuf::from(std::env::var_os(if cfg!(windows) {
            "TEMP"
        } else {
            "TMPDIR"
        })?);
        dir.join(TREE_MARK).exists().then_some(dir)
    }

    /// The test binary, run as `name`. Its path is absolute; a test that runs it through the host names it bare instead.
    fn this_test(name: &str) -> Vec<String> {
        let exe = std::env::current_exe().expect("the test binary");
        vec![
            exe.to_string_lossy().into_owned(),
            "--exact".to_owned(),
            format!("{TREE_TEST}{name}"),
            "--ignored".to_owned(),
            "--nocapture".to_owned(),
        ]
    }

    /// Run only by the tree test, as the installer: it starts a grandchild, then outlives the timeout.
    #[test]
    #[ignore = "run as a child by a_timeout_kills_everything_the_installer_started"]
    fn tree_installer() {
        if tree_dir().is_none() {
            return;
        }
        let argv = this_test("tree_grandchild");
        let mut grandchild = massorca_process::command(&argv[0])
            .args(&argv[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the grandchild starts");
        thread::sleep(Duration::from_secs(60));
        let _ = grandchild.wait();
    }

    /// Run only by `tree_installer`: it marks that it started, and later that it survived.
    #[test]
    #[ignore = "run as a grandchild by a_timeout_kills_everything_the_installer_started"]
    fn tree_grandchild() {
        let Some(dir) = tree_dir() else { return };
        fs::write(dir.join("grandchild-started"), "").expect("mark the start");
        thread::sleep(Duration::from_secs(6));
        fs::write(dir.join("grandchild-survived"), "").expect("mark the survival");
    }

    #[test]
    fn a_timeout_kills_everything_the_installer_started() {
        let dir = tempfile::tempdir().expect("a temp dir");
        fs::write(dir.path().join(TREE_MARK), "").expect("the mark");
        let temp = dir.path().as_os_str();
        let exe = std::env::current_exe().expect("the test binary");
        let mut argv = this_test("tree_installer");
        argv[0] = exe
            .file_name()
            .expect("a file name")
            .to_string_lossy()
            .into_owned();
        // The test binary is named bare and found on the search path of its own folder.
        let host = HostPrograms {
            env: Some(SearchEnv {
                path: exe.parent().map(|folder| folder.as_os_str().to_owned()),
                pathext: None,
            }),
            service_env: Some(service_env(&[
                ("TEMP", temp),
                ("TMP", temp),
                ("TMPDIR", temp),
            ])),
        };
        let exit = host
            .run(&argv, &dir.path().join("log"), Duration::from_secs(3))
            .expect("it runs");

        assert!(exit.timed_out);
        thread::sleep(Duration::from_secs(8));
        assert!(
            dir.path().join("grandchild-started").exists(),
            "the installer started its grandchild before the timeout"
        );
        assert!(
            !dir.path().join("grandchild-survived").exists(),
            "the grandchild outlived the timeout"
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

    /// A vendor bundle installer (Gentle Shell's) puts a generated `.cmd` launcher on PATH, not an npm package.
    #[cfg(windows)]
    #[test]
    fn a_bundle_launcher_on_the_search_path_is_detected() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let bundle = dir.path().join("bundle").join("bin");
        fs::create_dir_all(&bundle).expect("a bundle folder");
        fs::write(
            bundle.join("gentle-shell.cmd"),
            "@echo off
",
        )
        .expect("a launcher");
        let host = HostPrograms::searching(SearchEnv {
            path: Some(bundle.as_os_str().to_owned()),
            pathext: None,
        });

        let resolved = host.resolve("gentle-shell").expect("the launcher resolves");

        assert!(
            resolved
                .to_string_lossy()
                .eq_ignore_ascii_case(&bundle.join("gentle-shell.cmd").to_string_lossy()),
            "{resolved:?}"
        );
    }

    /// Windows' PATHEXT is upper case, so a `.cmd` shim is resolved and run under that spelling; Unix ignores it.
    fn windows_pathext() -> Option<String> {
        Some(".COM;.EXE;.BAT;.CMD".to_owned())
    }

    /// The `.cmd` shim npm writes for a global install is found on a search path with Windows' upper-case PATHEXT.
    #[cfg(windows)]
    #[test]
    fn a_cmd_shim_is_detected_under_windows_pathext() {
        let dir = tempfile::tempdir().expect("a temp dir");
        fs::write(dir.path().join("codex.cmd"), "@echo off\r\n").expect("a shim");
        let host = HostPrograms::searching(SearchEnv {
            path: Some(dir.path().as_os_str().to_owned()),
            pathext: windows_pathext(),
        });

        let resolved = host.resolve("codex").expect("the shim resolves");

        assert!(
            resolved
                .to_string_lossy()
                .eq_ignore_ascii_case(&dir.path().join("codex.cmd").to_string_lossy()),
            "{resolved:?}"
        );
    }

    /// `npm install` through `npm.cmd`: the shim gets its arguments and its exit status is the installer's.
    #[cfg(windows)]
    #[test]
    fn a_cmd_shim_runs_as_a_vendor_command_with_its_arguments() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");
        fs::write(
            dir.path().join("npm.cmd"),
            "@echo off\r\necho npm got [%1] [%2] [%3]\r\n",
        )
        .expect("a shim");
        let host = HostPrograms::searching(SearchEnv {
            path: Some(dir.path().as_os_str().to_owned()),
            pathext: windows_pathext(),
        });

        let exit = host
            .run(
                &strings(&["npm", "install", "-g", "@openai/codex"]),
                &log,
                LONG,
            )
            .expect("the shim runs");

        assert!(exit.success(), "{exit:?}");
        let text = fs::read_to_string(&log).expect("the log");
        assert!(
            text.contains("npm got [install] [-g] [@openai/codex]"),
            "{text}"
        );
    }

    /// A verify command that is a `.cmd` shim runs, and the exit status it returns decides the verify.
    #[cfg(windows)]
    #[test]
    fn a_cmd_shim_verify_reports_its_own_exit_status() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");
        fs::write(
            dir.path().join("gentle-shell.cmd"),
            "@echo off\r\nif \"%1\"==\"--version\" exit /b 0\r\nexit /b 2\r\n",
        )
        .expect("a shim");
        let host = HostPrograms::searching(SearchEnv {
            path: Some(dir.path().as_os_str().to_owned()),
            pathext: windows_pathext(),
        });

        let passed = host
            .run(&strings(&["gentle-shell", "--version"]), &log, LONG)
            .expect("the shim runs");
        let refused = host
            .run(&strings(&["gentle-shell", "--check"]), &log, LONG)
            .expect("the shim runs");

        assert!(passed.success(), "{passed:?}");
        assert_eq!(refused.code, Some(2), "{refused:?}");
    }

    /// A vendor command runs with the search path its program was resolved on, so a shim finds what it calls.
    #[test]
    fn a_vendor_command_runs_with_the_path_its_program_was_resolved_on() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");
        #[cfg(windows)]
        fs::write(
            dir.path().join("printpath.cmd"),
            "@echo off\r\necho PATH=%PATH%\r\n",
        )
        .expect("a program");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let script = dir.path().join("printpath");
            fs::write(&script, "#!/bin/sh\necho \"PATH=$PATH\"\n").expect("a program");
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("executable");
        }
        let host = HostPrograms::searching(SearchEnv {
            path: Some(dir.path().as_os_str().to_owned()),
            pathext: windows_pathext(),
        });

        let exit = host
            .run(&strings(&["printpath"]), &log, LONG)
            .expect("the program runs");

        assert!(exit.success(), "{exit:?}");
        let text = fs::read_to_string(&log).expect("the log");
        assert!(
            text.contains(&dir.path().to_string_lossy().into_owned()),
            "the command got the resolved search path: {text}"
        );
    }

    /// Only the absolute entries of the search path reach the command, and they are the ones its program was resolved
    /// on: the relative `.` and `bin` do not resolve and do not reach the child's PATH.
    #[test]
    fn a_vendor_command_gets_only_the_absolute_search_path_it_resolved_on() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let log = dir.path().join("log");
        #[cfg(windows)]
        fs::write(
            dir.path().join("printpath.cmd"),
            "@echo off\r\necho PATH=%PATH%\r\n",
        )
        .expect("a program");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let script = dir.path().join("printpath");
            fs::write(&script, "#!/bin/sh\necho \"PATH=$PATH\"\n").expect("a program");
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("executable");
        }
        let search = std::env::join_paths([
            PathBuf::from("."),
            PathBuf::from("bin"),
            dir.path().to_path_buf(),
        ])
        .expect("a search path");
        let host = HostPrograms::searching(SearchEnv {
            path: Some(search),
            pathext: if cfg!(windows) {
                Some(".COM;.EXE;.BAT;.CMD".to_owned())
            } else {
                None
            },
        });

        let exit = host
            .run(&strings(&["printpath"]), &log, LONG)
            .expect("the program runs");

        assert!(exit.success(), "{exit:?}");
        let text = fs::read_to_string(&log).expect("the log");
        let path = text
            .lines()
            .find_map(|line| line.strip_prefix("PATH="))
            .expect("the command printed its PATH");
        let entries: Vec<PathBuf> = std::env::split_paths(path).collect();
        assert_eq!(entries, vec![dir.path().to_path_buf()], "{text}");
    }
}
