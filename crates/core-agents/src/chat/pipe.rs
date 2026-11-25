//! A chat run's process (decision 0101): the vendor CLI with piped standard streams, as the root of its own process tree
//! (decision 0059's `process_tree`), so a stop ends everything it started.
//!
//! Four threads serve it. The reader hands each standard output line to the run's callback, in order, once the run is
//! released (the session row and journal exist by then). The error reader keeps the newest bytes of standard error. The
//! writer owns standard input and writes what it is handed, in order, so no caller ever blocks on a program that does not
//! read. The waiter reaps the program, kills what is left of its tree (a child still holding the output would otherwise
//! keep the run open), lets the readers finish the output within a bound, and reports the exit once.

use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{ChildStdin, ExitStatus, Stdio};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use massorca_core_store::{RunExit, RunExitReason, now_ms};

use super::{MAX_LINE_BYTES, STDERR_TAIL_BYTES};
use crate::process_tree;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What a chat run starts as.
#[derive(Debug, Clone)]
pub(crate) struct PipeSpec {
    pub(crate) program: PathBuf,
    pub(crate) args: Vec<String>,
    pub(crate) cwd: PathBuf,
    /// Set on top of the service's environment; values are never logged.
    pub(crate) env: Vec<(String, massorca_core_terminal::EnvValue)>,
    /// Inherited variables the run must not see.
    pub(crate) remove_env: Vec<String>,
}

/// One standard output line, or a line too long to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Line {
    Text(String),
    TooLong,
}

/// Whether the reader may hand lines on: held until the run is released, dropped when the start failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gate {
    Held,
    Open,
    Discard,
}

/// A live chat process.
pub(crate) struct PipeRun {
    /// Hands bytes to the writer thread; `None` once standard input is closed. Sending never blocks.
    stdin: Mutex<Option<Sender<Vec<u8>>>>,
    tree: Mutex<process_tree::Tree>,
    stderr_tail: Arc<Mutex<VecDeque<u8>>>,
    gate: Arc<(Mutex<Gate>, Condvar)>,
    /// Set once a stop asked for the run's end, so the exit says `killed` (or `service_stopped`).
    kill_reason: Mutex<Option<RunExitReason>>,
    exited: Arc<(Mutex<bool>, Condvar)>,
}

impl std::fmt::Debug for PipeRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PipeRun").finish_non_exhaustive()
    }
}

/// Starts `spec`. `on_line` receives each standard output line once the run is [released](PipeRun::release); `on_exit`
/// receives the run's end once, after the last line.
pub(crate) fn spawn(
    spec: &PipeSpec,
    mut on_line: impl FnMut(Line) + Send + 'static,
    on_exit: impl FnOnce(RunExit) + Send + 'static,
) -> io::Result<Arc<PipeRun>> {
    let mut command = massorca_process::command(&spec.program);
    command
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for name in &spec.remove_env {
        command.env_remove(name);
    }
    for (name, value) in &spec.env {
        command.env(name, value.as_str());
    }
    let (mut child, tree) = process_tree::spawn(&mut command)?;
    let (Some(stdin), Some(stdout), Some(stderr)) =
        (child.stdin.take(), child.stdout.take(), child.stderr.take())
    else {
        let mut tree = tree;
        tree.stop();
        let _ = child.wait();
        return Err(io::Error::other(
            "the chat run's standard streams were not piped",
        ));
    };
    let (to_stdin, writes) = mpsc::channel::<Vec<u8>>();
    let run = Arc::new(PipeRun {
        stdin: Mutex::new(Some(to_stdin)),
        tree: Mutex::new(tree),
        stderr_tail: Arc::new(Mutex::new(VecDeque::new())),
        gate: Arc::new((Mutex::new(Gate::Held), Condvar::new())),
        kill_reason: Mutex::new(None),
        exited: Arc::new((Mutex::new(false), Condvar::new())),
    });

    let gate = Arc::clone(&run.gate);
    let reader = thread::Builder::new()
        .name("chat-reader".to_owned())
        .spawn(move || {
            if !wait_for_gate(&gate) {
                return;
            }
            read_lines(BufReader::new(stdout), &mut on_line);
        });
    let tail = Arc::clone(&run.stderr_tail);
    let errors = thread::Builder::new()
        .name("chat-stderr".to_owned())
        .spawn(move || keep_tail(stderr, &tail, STDERR_TAIL_BYTES));
    let writer = thread::Builder::new()
        .name("chat-writer".to_owned())
        .spawn(move || write_all_of(stdin, &writes));
    let (reader, errors) = match (reader, errors, writer) {
        (Ok(reader), Ok(errors), Ok(_)) => (reader, errors),
        (reader, errors, writer) => {
            run.discard();
            lock(&run.tree).stop();
            let _ = child.wait();
            drop((reader, errors, writer));
            return Err(io::Error::other(
                "the chat run's reader threads did not start",
            ));
        }
    };
    let waiting = Arc::clone(&run);
    let waiter = thread::Builder::new()
        .name("chat-waiter".to_owned())
        .spawn(move || {
            let status = child.wait();
            // The tree goes with its root: nothing the run started outlives it, or holds its output open.
            lock(&waiting.tree).stop();
            // Its end is reported only once the run was released or discarded, so its lines reach the journal first.
            wait_for_gate(&waiting.gate);
            // Every line the program wrote is handed on before its end is reported; a reader still held by a process
            // outside the tree is left behind after the bound.
            join_within(reader, READER_DRAIN);
            join_within(errors, READER_DRAIN);
            let reason = lock(&waiting.kill_reason).unwrap_or(RunExitReason::Exited);
            on_exit(exit_of(status.ok(), reason));
            let (exited, changed) = &*waiting.exited;
            *lock(exited) = true;
            changed.notify_all();
        });
    if let Err(error) = waiter {
        run.discard();
        lock(&run.tree).stop();
        return Err(error);
    }
    Ok(run)
}

/// How long the waiter lets the readers finish the output once the program and its tree are gone.
const READER_DRAIN: Duration = Duration::from_secs(5);

/// Waits for `thread` to finish, at most `bound`; a thread still running then is left to end on its own.
fn join_within(thread: thread::JoinHandle<()>, bound: Duration) {
    let deadline = Instant::now() + bound;
    while !thread.is_finished() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    if thread.is_finished() {
        let _ = thread.join();
    }
}

/// Writes each message handed on to `stdin`, in order, until the senders are gone or a write fails.
fn write_all_of(mut stdin: ChildStdin, writes: &mpsc::Receiver<Vec<u8>>) {
    for bytes in writes {
        if stdin
            .write_all(&bytes)
            .and_then(|()| stdin.flush())
            .is_err()
        {
            return;
        }
    }
}

/// Blocks until the gate opens; false when the run's lines are to be dropped.
fn wait_for_gate(gate: &(Mutex<Gate>, Condvar)) -> bool {
    let (state, changed) = gate;
    let mut state = lock(state);
    while *state == Gate::Held {
        state = changed.wait(state).unwrap_or_else(PoisonError::into_inner);
    }
    *state == Gate::Open
}

/// Hands each line of `reader` to `on_line`, without its line ending. A line longer than [`MAX_LINE_BYTES`] is skipped
/// and handed on as [`Line::TooLong`]; invalid UTF-8 is replaced.
pub(crate) fn read_lines(mut reader: impl BufRead, on_line: &mut impl FnMut(Line)) {
    let mut line = Vec::new();
    let mut too_long = false;
    loop {
        let (consumed, ended) = {
            let Ok(buffer) = reader.fill_buf() else {
                return;
            };
            if buffer.is_empty() {
                // The end of the output: a last line without a line ending still counts.
                if too_long {
                    on_line(Line::TooLong);
                } else if !line.is_empty() {
                    on_line(Line::Text(text_of(&line)));
                }
                return;
            }
            match buffer.iter().position(|byte| *byte == b'\n') {
                Some(end) => {
                    if !too_long {
                        line.extend_from_slice(&buffer[..end]);
                    }
                    (end + 1, true)
                }
                None => {
                    if !too_long {
                        line.extend_from_slice(buffer);
                    }
                    (buffer.len(), false)
                }
            }
        };
        reader.consume(consumed);
        if line.len() > MAX_LINE_BYTES {
            too_long = true;
            line = Vec::new();
        }
        if ended {
            if too_long {
                on_line(Line::TooLong);
            } else {
                on_line(Line::Text(text_of(&line)));
            }
            line.clear();
            too_long = false;
        }
    }
}

fn text_of(bytes: &[u8]) -> String {
    let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    String::from_utf8_lossy(bytes).into_owned()
}

/// Keeps the newest `max` bytes of `reader` in `tail`.
pub(crate) fn keep_tail(mut reader: impl Read, tail: &Mutex<VecDeque<u8>>, max: usize) {
    let mut buffer = [0_u8; 4096];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(read) => {
                let mut tail = lock(tail);
                tail.extend(&buffer[..read]);
                let excess = tail.len().saturating_sub(max);
                tail.drain(..excess);
            }
        }
    }
}

fn exit_of(status: Option<ExitStatus>, reason: RunExitReason) -> RunExit {
    let (code, signal) = match status {
        Some(status) => (status.code().map(i64::from), signal_of(status)),
        None => (None, None),
    };
    RunExit {
        code,
        signal,
        reason,
        at: now_ms(),
    }
}

#[cfg(unix)]
fn signal_of(status: ExitStatus) -> Option<String> {
    use std::os::unix::process::ExitStatusExt;
    status.signal().map(|signal| match signal {
        libc::SIGKILL => "SIGKILL".to_owned(),
        libc::SIGTERM => "SIGTERM".to_owned(),
        libc::SIGINT => "SIGINT".to_owned(),
        libc::SIGHUP => "SIGHUP".to_owned(),
        other => format!("signal {other}"),
    })
}

#[cfg(not(unix))]
fn signal_of(_status: ExitStatus) -> Option<String> {
    None
}

impl PipeRun {
    /// Lets the reader hand lines on.
    pub(crate) fn release(&self) {
        self.set_gate(Gate::Open);
    }

    /// Drops every line the run writes: its start failed.
    pub(crate) fn discard(&self) {
        self.set_gate(Gate::Discard);
    }

    fn set_gate(&self, to: Gate) {
        let (state, changed) = &*self.gate;
        let mut state = lock(state);
        if *state == Gate::Held {
            *state = to;
        }
        changed.notify_all();
    }

    /// Hands `bytes` to the run's writer, which writes messages whole and in order. Never blocks on the program: a
    /// message it does not read waits in the writer. Fails only once standard input is closed.
    pub(crate) fn write(&self, bytes: &[u8]) -> io::Result<()> {
        let closed = || io::Error::new(io::ErrorKind::BrokenPipe, "the chat run's input is closed");
        let stdin = lock(&self.stdin);
        let Some(stdin) = stdin.as_ref() else {
            return Err(closed());
        };
        stdin.send(bytes.to_vec()).map_err(|_| closed())
    }

    /// The newest bytes of the run's standard error, as text.
    pub(crate) fn stderr_tail(&self) -> String {
        let tail = lock(&self.stderr_tail);
        let (front, back) = tail.as_slices();
        let mut bytes = Vec::with_capacity(tail.len());
        bytes.extend_from_slice(front);
        bytes.extend_from_slice(back);
        String::from_utf8_lossy(&bytes).into_owned()
    }

    /// Whether the run's end was reported.
    pub(crate) fn has_exited(&self) -> bool {
        *lock(&self.exited.0)
    }

    /// Blocks until the run's end is reported or `deadline` passes; true when it was.
    fn wait_until(&self, deadline: Instant) -> bool {
        let (exited, changed) = &*self.exited;
        let mut exited = lock(exited);
        while !*exited {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            exited = changed
                .wait_timeout(exited, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        true
    }

    /// Ends the run, recording `reason`: unless `force`, its standard input is closed and it has `grace` to end on its
    /// own; then its whole tree is killed. Returns once the end is reported, or after a short margin.
    pub(crate) fn stop(&self, force: bool, reason: RunExitReason, grace: Duration) -> bool {
        if self.has_exited() {
            return true;
        }
        lock(&self.kill_reason).get_or_insert(reason);
        // Lines written while the stop runs are still read, so a run that was never released is released now.
        self.release();
        if !force {
            lock(&self.stdin).take();
            if self.wait_until(Instant::now() + grace) {
                return true;
            }
        }
        lock(&self.tree).stop();
        lock(&self.stdin).take();
        self.wait_until(Instant::now() + KILL_EXIT_MARGIN)
    }
}

/// How long a stop waits for the reported end after killing the tree.
const KILL_EXIT_MARGIN: Duration = Duration::from_secs(5);

#[cfg(test)]
mod tests {
    use super::*;

    fn lines_of(input: &[u8]) -> Vec<Line> {
        let mut lines = Vec::new();
        read_lines(BufReader::with_capacity(4, input), &mut |line| {
            lines.push(line);
        });
        lines
    }

    #[test]
    fn lines_are_split_on_line_feeds_across_buffer_ends() {
        assert_eq!(
            lines_of(b"{\"a\":1}\r\nsecond line\n\nlast"),
            [
                Line::Text("{\"a\":1}".to_owned()),
                Line::Text("second line".to_owned()),
                Line::Text(String::new()),
                Line::Text("last".to_owned()),
            ]
        );
    }

    #[test]
    fn a_line_past_the_limit_is_skipped_and_reading_goes_on() {
        let mut input = vec![b'x'; MAX_LINE_BYTES + 10];
        input.extend_from_slice(b"\nnext\n");
        let mut lines = Vec::new();
        read_lines(BufReader::new(input.as_slice()), &mut |line| {
            lines.push(line);
        });
        assert_eq!(lines, [Line::TooLong, Line::Text("next".to_owned())]);
    }

    #[test]
    fn the_error_tail_keeps_the_newest_bytes() {
        let tail = Mutex::new(VecDeque::new());
        keep_tail(&b"0123456789"[..], &tail, 4);
        assert_eq!(lock(&tail).iter().copied().collect::<Vec<_>>(), b"6789");
    }
}
