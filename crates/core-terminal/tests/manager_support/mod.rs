//! Helpers shared by the `TerminalManager` tests: real shells, every wait bounded.

#![allow(
    dead_code,
    reason = "each test binary uses a different part of this module"
)]

use std::collections::BTreeMap;
use std::ops::Deref;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::shell::EnvSnapshot;
use massorca_core_terminal::{
    AttachItem, AttachMode, Attachment, CreateSpec, ManagerConfig, ShellChoice, TerminalInfo,
    TerminalManager,
};

pub const TIMEOUT: Duration = Duration::from_secs(10);

/// Wide enough that ConPTY does not wrap the lines the tests read.
pub const SIZE: PtySize = PtySize {
    cols: 200,
    rows: 50,
};

/// A manager that kills its terminals when the test ends, failed assertions included.
pub struct Managed(pub TerminalManager);

impl Managed {
    pub fn new() -> Self {
        Self::with(|_| {})
    }

    pub fn with(change: impl FnOnce(&mut ManagerConfig)) -> Self {
        let mut config = ManagerConfig::new(EnvSnapshot::current(), "0.0.0-test".to_owned());
        change(&mut config);
        Self(TerminalManager::new(config))
    }
}

impl Deref for Managed {
    type Target = TerminalManager;

    fn deref(&self) -> &TerminalManager {
        &self.0
    }
}

impl Drop for Managed {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}

/// The platform's interactive shell: `cmd.exe` on Windows, `sh` elsewhere.
pub fn shell() -> ShellChoice {
    if cfg!(windows) {
        ShellChoice {
            program: "cmd.exe".to_owned(),
            args: vec!["/d".to_owned()],
        }
    } else {
        ShellChoice {
            program: "sh".to_owned(),
            args: Vec::new(),
        }
    }
}

/// A shell that runs `script` and exits: `cmd /d /c` or `sh -c`.
pub fn script_shell(script: &str) -> ShellChoice {
    let flag = if cfg!(windows) { "/c" } else { "-c" };
    let mut choice = shell();
    choice.args.push(flag.to_owned());
    choice.args.push(script.to_owned());
    choice
}

pub fn spec(workspace: &str, cwd: &Path) -> CreateSpec {
    CreateSpec {
        workspace_id: workspace.to_owned(),
        cwd: cwd.to_path_buf(),
        shell: Some(shell()),
        env: BTreeMap::new(),
        size: SIZE,
        title: None,
        request_id: None,
    }
}

/// A line typed into the shell, with the key that ends it.
pub fn line(text: &str) -> Vec<u8> {
    let end = if cfg!(windows) { "\r" } else { "\n" };
    format!("{text}{end}").into_bytes()
}

/// A command whose output differs from its echo: the shell expands part of it.
/// Returns the typed line and the text only the output contains.
pub fn marker_command(tag: &str) -> (Vec<u8>, String) {
    if cfg!(windows) {
        (
            line(&format!("echo {tag}-%OS%")),
            format!("{tag}-Windows_NT"),
        )
    } else {
        (line(&format!("echo {tag}-$((6*7))")), format!("{tag}-42"))
    }
}

/// Polls `done` until it holds, or panics after `TIMEOUT`.
pub fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + TIMEOUT;
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(20));
    }
}

/// Removes escape sequences and control characters, incrementally across chunks.
#[derive(Default)]
pub struct Visible {
    pub text: Vec<u8>,
    state: StripState,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum StripState {
    #[default]
    Text,
    Escape,
    Csi,
    Osc,
    OscEscape,
}

impl Visible {
    pub fn feed(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.state = match (self.state, byte) {
                (StripState::Text, 0x1b) => StripState::Escape,
                (StripState::Text, byte) => {
                    if byte >= 0x20 || byte == b'\n' {
                        self.text.push(byte);
                    }
                    StripState::Text
                }
                (StripState::Escape, b'[') => StripState::Csi,
                (StripState::Escape, b']') => StripState::Osc,
                (StripState::Escape, _) => StripState::Text,
                (StripState::Csi, b'@'..=b'~') => StripState::Text,
                (StripState::Csi, _) => StripState::Csi,
                (StripState::Osc, 0x07) => StripState::Text,
                (StripState::Osc, 0x1b) => StripState::OscEscape,
                (StripState::Osc, _) => StripState::Osc,
                (StripState::OscEscape, _) => StripState::Text,
            };
        }
    }

    pub fn contains(&self, needle: &str) -> bool {
        contains(&self.text, needle.as_bytes())
    }

    /// The last few hundred characters, for failure messages.
    pub fn tail(&self) -> String {
        let text = self.as_string();
        let start = text.len().saturating_sub(400);
        text.get(start..).unwrap_or(&text).to_owned()
    }

    pub fn as_string(&self) -> String {
        String::from_utf8_lossy(&self.text).into_owned()
    }
}

pub fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

pub fn visible(bytes: &[u8]) -> Visible {
    let mut visible = Visible::default();
    visible.feed(bytes);
    visible
}

const CURSOR_QUERY: &[u8] = b"\x1b[6n";

/// An attachment read by a test: checks that output offsets are contiguous and answers
/// ConPTY's cursor query as a real client does.
pub struct Watch {
    pub attachment: Attachment,
    manager: TerminalManager,
    id: String,
    pub mode: AttachMode,
    pub info: TerminalInfo,
    /// Offset of the first byte of `output`.
    pub base: u64,
    pub snapshot: Vec<u8>,
    pub snapshot_size: (u16, u16),
    pub snapshots: usize,
    pub output: Vec<u8>,
    pub output_items: usize,
    pub visible: Visible,
    pub resized: Vec<(u16, u16)>,
    pub exited: Option<TerminalInfo>,
    pub ended: bool,
    /// The first output after a resume is history the service already answered.
    history_pending: bool,
    queries_scanned: usize,
}

/// Attaches and reads up to the first byte of live output (the snapshot, if any).
pub async fn watch(manager: &TerminalManager, id: &str, since: Option<u64>) -> Watch {
    let attachment = manager.attach(id, since).expect("attach");
    let mut watch = Watch {
        attachment,
        manager: manager.clone(),
        id: id.to_owned(),
        mode: AttachMode::Snapshot,
        info: manager.get(id).expect("the terminal exists"),
        base: 0,
        snapshot: Vec::new(),
        snapshot_size: (0, 0),
        snapshots: 0,
        output: Vec::new(),
        output_items: 0,
        visible: Visible::default(),
        resized: Vec::new(),
        exited: None,
        ended: false,
        history_pending: false,
        queries_scanned: 0,
    };
    match watch.step().await {
        Some(AttachItem::Attached { mode, .. }) => {
            if mode == AttachMode::Snapshot {
                match watch.step().await {
                    Some(AttachItem::Snapshot { .. }) => {}
                    other => panic!("expected a snapshot after attached, got {other:?}"),
                }
            }
        }
        other => panic!("expected attached first, got {other:?}"),
    }
    watch
}

impl Watch {
    /// Offset of the next byte this watch expects.
    pub fn offset(&self) -> u64 {
        self.base + self.output.len() as u64
    }

    /// The next item, applied to this watch's state. Panics after `TIMEOUT`.
    pub async fn step(&mut self) -> Option<AttachItem> {
        let item = tokio::time::timeout(TIMEOUT, self.attachment.next())
            .await
            .expect("timed out waiting for an attach item");
        self.apply(&item);
        item
    }

    fn apply(&mut self, item: &Option<AttachItem>) {
        match item {
            None => self.ended = true,
            Some(AttachItem::Attached { info, mode, offset }) => {
                self.info = info.clone();
                self.mode = *mode;
                self.base = *offset;
                self.history_pending = *mode == AttachMode::Resume;
            }
            Some(AttachItem::Snapshot {
                offset,
                cols,
                rows,
                bytes,
            }) => {
                self.snapshots += 1;
                self.base = *offset;
                self.output.clear();
                self.queries_scanned = 0;
                self.snapshot = bytes.clone();
                self.snapshot_size = (*cols, *rows);
                self.visible = visible(bytes);
            }
            Some(AttachItem::Output { offset, bytes }) => {
                assert_eq!(*offset, self.offset(), "output must be contiguous");
                self.output.extend_from_slice(bytes);
                self.output_items += 1;
                self.visible.feed(bytes);
                if std::mem::take(&mut self.history_pending) {
                    self.queries_scanned = self.output.len();
                } else {
                    self.answer_cursor_queries();
                }
            }
            Some(AttachItem::Resized { cols, rows }) => self.resized.push((*cols, *rows)),
            Some(AttachItem::Exited { info }) => self.exited = Some(info.clone()),
        }
        assert_eq!(self.attachment.offset(), self.offset(), "attachment offset");
    }

    fn answer_cursor_queries(&mut self) {
        let from = self.queries_scanned.saturating_sub(CURSOR_QUERY.len() - 1);
        let queries = self.output[from..]
            .windows(CURSOR_QUERY.len())
            .filter(|window| *window == CURSOR_QUERY)
            .count();
        self.queries_scanned = self.output.len();
        for _ in 0..queries {
            let _ = self.manager.input(&self.id, b"\x1b[1;1R");
        }
    }

    /// Reads until the visible text contains `needle`.
    pub async fn wait_for_text(&mut self, needle: &str) {
        let deadline = Instant::now() + TIMEOUT;
        // Only text added since the last look can complete the needle.
        let mut searched: usize = 0;
        loop {
            let from = searched.saturating_sub(needle.len());
            if contains(&self.visible.text[from..], needle.as_bytes()) {
                return;
            }
            searched = self.visible.text.len();
            assert!(
                Instant::now() < deadline && !self.ended,
                "timed out waiting for {needle:?} after {} bytes, ending {:?}",
                self.output.len(),
                self.visible.tail()
            );
            if self
                .step()
                .await
                .is_some_and(|item| matches!(item, AttachItem::Snapshot { .. }))
            {
                searched = 0;
            }
        }
    }

    /// Reads until the screen shows something other than blanks: the shell has started.
    pub async fn wait_for_output(&mut self) {
        let deadline = Instant::now() + TIMEOUT;
        while self.visible.text.iter().all(u8::is_ascii_whitespace) {
            assert!(
                Instant::now() < deadline && !self.ended,
                "the shell printed nothing"
            );
            self.step().await;
        }
    }

    pub async fn wait_for_offset(&mut self, offset: u64) {
        while self.offset() < offset {
            assert!(!self.ended, "the attachment ended before offset {offset}");
            self.step().await;
        }
    }

    /// Reads until the terminal's `exited` item and the end of the attachment.
    pub async fn wait_for_exit(&mut self) -> TerminalInfo {
        while !self.ended {
            self.step().await;
        }
        self.exited.clone().expect("exited before the end")
    }

    /// Reads until no item arrives for `quiet`.
    pub async fn settle(&mut self, quiet: Duration) {
        let deadline = Instant::now() + TIMEOUT;
        while let Ok(item) = tokio::time::timeout(quiet, self.attachment.next()).await {
            assert!(Instant::now() < deadline, "the terminal never went quiet");
            self.apply(&item);
            if item.is_none() {
                return;
            }
        }
    }
}
