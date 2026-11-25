//! The terminal view driven from Rust as the app drives it (decision 0080, task A5): attach, batched output, input, resize
//! and close against a real in-process server. A relay between the app and the server drops the app's connection on
//! purpose while the terminal keeps running on the host.

mod common;

use std::collections::BTreeMap;
use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{app, offer, owner, start, stop, temp_home};
use massorca_client::{AttachUpdate, Client};
use massorca_client_ffi::{FfiError, Host, LinkState, MassOrca, TerminalEvent, TerminalView};
use massorca_protocol::methods::projects::{self, Project, ProjectAddParams, ProjectKind};
use massorca_protocol::methods::terminals::{
    AttachMode, TerminalAttachParams, TerminalCreateParams, TerminalState,
};
use massorca_protocol::methods::workspaces::{self, Workspace, WorkspaceCreateParams};
use massorca_protocol::pairing::Offer;
use massorca_server::RunningService;
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;
use tokio::task::{AbortHandle, JoinHandle};
use tokio::time::timeout;

/// How long one read may take before the test calls it stuck.
const WAIT: Duration = Duration::from_secs(60);
/// The cursor position query a shell waits on, and the answer a terminal emulator gives.
const CURSOR_QUERY: &[u8] = b"\x1b[6n";
const CURSOR_ANSWER: &[u8] = b"\x1b[1;1R";

/// Waits for `future` on the test's runtime, at most `WAIT`.
fn within<T>(test: &Runtime, future: impl Future<Output = T>) -> T {
    test.block_on(async { timeout(WAIT, future).await.expect("an answer in time") })
}

fn utf8(path: &Path) -> String {
    path.to_str().expect("test paths are UTF-8").to_owned()
}

/// The shell's line end, as typed.
fn newline() -> &'static [u8] {
    if cfg!(windows) { b"\r" } else { b"\n" }
}

/// `command` as typed: its text and the line end.
fn typed(command: &str) -> Vec<u8> {
    let mut bytes = command.as_bytes().to_vec();
    bytes.extend_from_slice(newline());
    bytes
}

/// A command that prints `left` and `right` joined, without the joined word in the typed text.
fn joined_echo(left: &str, right: &str) -> String {
    if cfg!(windows) {
        format!("echo ('{left}'+'{right}')")
    } else {
        format!("echo {left}''{right}")
    }
}

/// A command that prints the lines `row 1` to `row <rows>`.
fn numbered_rows(rows: u32) -> String {
    if cfg!(windows) {
        format!("1..{rows} | ForEach-Object {{ 'row ' + $_ }}")
    } else {
        format!("i=1; while [ $i -le {rows} ]; do echo \"row $i\"; i=$((i+1)); done")
    }
}

/// A command that prints `lines` long lines and then `done-marker`.
fn flood(lines: u32) -> String {
    if cfg!(windows) {
        format!(
            "1..{lines} | ForEach-Object {{ 'line ' + $_ + ' ' + ('x' * 40) }}; echo ('done'+'-marker')"
        )
    } else {
        format!(
            "i=1; while [ $i -le {lines} ]; do echo \"line $i xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\"; i=$((i+1)); done; echo done''-marker"
        )
    }
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle.as_bytes())
}

fn count(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

/// The output the host holds for `terminal` from `from` to `until`, read by its owner: what the app's view must match.
fn host_output(test: &Runtime, client: &Client, terminal: &str, from: u64, until: u64) -> Vec<u8> {
    test.block_on(async {
        let mut attachment = client
            .terminal_attach(&TerminalAttachParams {
                terminal: terminal.to_owned(),
                since: Some(from),
                cols: None,
                rows: None,
                answers_queries: None,
            })
            .await
            .expect("the owner attaches");
        let mut bytes = Vec::new();
        while from + (bytes.len() as u64) < until {
            let update = timeout(WAIT, attachment.next())
                .await
                .expect("the output in time")
                .expect("the attach stays open")
                .expect("the attach reports no error");
            match update {
                AttachUpdate::Attached { mode, offset, .. } => {
                    assert_eq!(
                        (mode, offset),
                        (AttachMode::Resume, from),
                        "the host resumes there"
                    );
                }
                AttachUpdate::Output {
                    offset,
                    bytes: fresh,
                } => {
                    assert_eq!(
                        offset,
                        from + bytes.len() as u64,
                        "the host's output is contiguous"
                    );
                    bytes.extend_from_slice(&fresh);
                }
                other => panic!("the host sent {other:?} within its own output"),
            }
        }
        bytes.truncate(usize::try_from(until - from).expect("the range fits in memory"));
        bytes
    })
}

/// The `(offset, length)` of each output event, in order.
fn output_runs(events: &[TerminalEvent]) -> Vec<(u64, usize)> {
    events
        .iter()
        .filter_map(|event| match event {
            TerminalEvent::Output { offset, bytes } => Some((*offset, bytes.len())),
            _ => None,
        })
        .collect()
}

fn is_snapshot(event: &TerminalEvent) -> bool {
    matches!(event, TerminalEvent::Snapshot { .. })
}

fn is_lost(event: &TerminalEvent) -> bool {
    matches!(
        event,
        TerminalEvent::Link {
            link: LinkState::Unverifiable { .. }
        }
    )
}

fn is_back(event: &TerminalEvent) -> bool {
    matches!(
        event,
        TerminalEvent::Link {
            link: LinkState::Live
        }
    )
}

/// Everything a test has read from a view.
#[derive(Default)]
struct Seen {
    events: Vec<TerminalEvent>,
    /// The output bytes read so far, in order.
    output: Vec<u8>,
    /// Cursor answers the link refused while it was down; written once it is back.
    unanswered: usize,
    /// The view reported the link lost and not yet back; a write now could wait on the reconnect.
    link_down: bool,
}

impl Seen {
    /// Reads one event. A cursor query in the output gets its answer, as an emulator gives it; the shell waits for it.
    fn read_one(&mut self, test: &Runtime, view: &TerminalView) -> TerminalEvent {
        let event = test
            .block_on(async { timeout(WAIT, view.next()).await })
            .unwrap_or_else(|_| panic!("no event in time; {}", self.recent()))
            .expect("the view reports no error")
            .expect("the view is open");
        if let TerminalEvent::Output { bytes, .. } = &event {
            self.unanswered += count(bytes, CURSOR_QUERY);
            self.output.extend_from_slice(bytes);
        }
        if is_lost(&event) {
            self.link_down = true;
        } else if is_back(&event) {
            self.link_down = false;
        }
        self.answer_cursor_queries(test, view);
        self.events.push(event.clone());
        event
    }

    /// The last events and output read, for a stuck test's message.
    fn recent(&self) -> String {
        let events: Vec<String> = self
            .events
            .iter()
            .rev()
            .take(4)
            .map(|event| match event {
                TerminalEvent::Output { offset, bytes } => {
                    format!("Output@{offset}+{}", bytes.len())
                }
                TerminalEvent::Snapshot { offset, bytes, .. } => {
                    format!("Snapshot@{offset}+{}", bytes.len())
                }
                other => format!("{other:?}"),
            })
            .collect();
        let tail = &self.output[self.output.len().saturating_sub(200)..];
        format!(
            "last events (newest first) {events:?}, link down {}, unanswered {}, output tail {:?}",
            self.link_down,
            self.unanswered,
            String::from_utf8_lossy(tail)
        )
    }

    /// Answers the cursor queries read so far; a refused answer waits for the link to return.
    fn answer_cursor_queries(&mut self, test: &Runtime, view: &TerminalView) {
        while self.unanswered > 0 && !self.link_down {
            match within(test, view.input(CURSOR_ANSWER.to_vec())) {
                Ok(()) => {}
                // A query that comes after the process ended has no one left to answer it.
                Err(FfiError::Remote { code, .. }) if code == "terminal.exited" => {}
                Err(FfiError::Unreachable { .. }) => {
                    self.link_down = true;
                    return;
                }
                Err(error) => panic!("the answer is written: {error:?}"),
            }
            self.unanswered -= 1;
        }
    }

    /// Reads until an event satisfies `done`, and returns that event.
    fn until(
        &mut self,
        test: &Runtime,
        view: &TerminalView,
        done: impl Fn(&TerminalEvent) -> bool,
    ) -> TerminalEvent {
        loop {
            let event = self.read_one(test, view);
            if done(&event) {
                return event;
            }
        }
    }

    /// Reads until the output read so far contains `needle`.
    fn until_output(&mut self, test: &Runtime, view: &TerminalView, needle: &str) {
        let mut checked = self.output.len().saturating_sub(needle.len());
        while !contains(&self.output[checked..], needle) {
            let before = self.output.len();
            self.read_one(test, view);
            checked = before.saturating_sub(needle.len());
        }
    }

    fn link_states(&self) -> Vec<LinkState> {
        self.events
            .iter()
            .filter_map(|event| match event {
                TerminalEvent::Link { link } => Some(link.clone()),
                _ => None,
            })
            .collect()
    }
}

/// A TCP relay between the app and the server. The test can cut every connection through it and refuse new ones for a
/// while, which drops the app's connection while the terminal keeps running.
struct Relay {
    address: SocketAddr,
    open: Arc<AtomicBool>,
    links: Arc<Mutex<Vec<AbortHandle>>>,
    accepting: JoinHandle<()>,
}

impl Relay {
    async fn to(server: &str) -> Self {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("a relay port");
        let address = listener.local_addr().expect("the relay address");
        let open = Arc::new(AtomicBool::new(true));
        let links = Arc::new(Mutex::new(Vec::new()));
        let accepting = tokio::spawn(relay_connections(
            listener,
            server.to_owned(),
            Arc::clone(&open),
            Arc::clone(&links),
        ));
        Self {
            address,
            open,
            links,
            accepting,
        }
    }

    /// Drops every connection through the relay now.
    fn cut(&self) {
        for link in self.links.lock().expect("the links").drain(..) {
            link.abort();
        }
    }

    /// Lets new connections through, or refuses them while `open` is false.
    fn set_open(&self, open: bool) {
        self.open.store(open, Ordering::SeqCst);
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.accepting.abort();
        self.cut();
    }
}

async fn relay_connections(
    listener: TcpListener,
    server: String,
    open: Arc<AtomicBool>,
    links: Arc<Mutex<Vec<AbortHandle>>>,
) {
    while let Ok((mut app_side, _)) = listener.accept().await {
        if !open.load(Ordering::SeqCst) {
            continue;
        }
        let server = server.clone();
        let link = tokio::spawn(async move {
            if let Ok(mut host_side) = TcpStream::connect(server.as_str()).await {
                let _ = copy_bidirectional(&mut app_side, &mut host_side).await;
            }
        });
        links.lock().expect("the links").push(link.abort_handle());
    }
}

/// A terminal in a folder workspace of `home`, made by its owner.
async fn terminal_in_folder(home: &Path) -> String {
    let folder = home.join("folder");
    std::fs::create_dir_all(&folder).expect("a folder");
    let client = owner(home).await;
    let project: Project = client
        .call(
            projects::ADD,
            &ProjectAddParams {
                path: utf8(&folder),
                name: Some("folder".to_owned()),
                kind: Some(ProjectKind::Folder),
                icon: None,
                workspaces_root: Some(utf8(&home.join("ws-folder"))),
            },
        )
        .await
        .expect("the folder project");
    let workspace: Workspace = client
        .call(
            workspaces::CREATE,
            &WorkspaceCreateParams {
                project: project.id,
                name: "folder".to_owned(),
                branch: None,
                base: None,
                existing_branch: false,
                refresh_base: false,
                no_setup: false,
                path: None,
                request_id: None,
            },
        )
        .await
        .expect("the folder workspace");
    client
        .terminal_create(&TerminalCreateParams {
            workspace: workspace.id,
            shell: None,
            cwd: None,
            env: BTreeMap::new(),
            cols: Some(120),
            rows: Some(40),
            title: None,
            request_id: None,
        })
        .await
        .expect("the terminal is created")
        .id
}

/// A server with one terminal, and an app paired with that server through a relay the test controls.
struct Rig {
    running: RunningService,
    relay: Relay,
    // Dropping the app stops the runtime that serves its connection, so the rig keeps it alive.
    _massorca: Arc<MassOrca>,
    host: Arc<Host>,
    terminal: String,
}

impl Rig {
    fn new(test: &Runtime, home: &Path) -> Self {
        let running = test.block_on(start(home, 0));
        let text = test.block_on(offer(home));
        let mut decoded = Offer::decode(&text).expect("the offer decodes");
        let relay = test.block_on(Relay::to(&decoded.addresses[0]));
        decoded.addresses = vec![relay.address.to_string()];
        let massorca = app();
        test.block_on(massorca.pair(decoded.encode().expose().to_owned(), "office".into()))
            .expect("paired");
        let host = test
            .block_on(massorca.connect("office".into()))
            .expect("connected");
        let terminal = test.block_on(terminal_in_folder(home));
        Self {
            running,
            relay,
            _massorca: massorca,
            host,
            terminal,
        }
    }

    fn attach(&self, test: &Runtime) -> Arc<TerminalView> {
        within(
            test,
            self.host.attach_terminal(self.terminal.clone(), 120, 40),
        )
        .expect("the terminal is attached")
    }
}

/// Waits until the owner's own attach shows `needle`, which the owner reads without a relay.
fn wait_for_output(test: &Runtime, client: &Client, terminal: &str, needle: &str) {
    test.block_on(async {
        let mut attachment = client
            .terminal_attach(&TerminalAttachParams {
                terminal: terminal.to_owned(),
                since: None,
                cols: None,
                rows: None,
                answers_queries: None,
            })
            .await
            .expect("the owner attaches");
        let mut seen = Vec::new();
        loop {
            let update = timeout(WAIT, attachment.next())
                .await
                .expect("the output in time")
                .expect("the attach stays open")
                .expect("the attach reports no error");
            let fresh = match update {
                AttachUpdate::Snapshot { bytes, .. } | AttachUpdate::Output { bytes, .. } => bytes,
                AttachUpdate::Exited { .. } => panic!("the terminal exited before {needle:?}"),
                AttachUpdate::Attached { .. } | AttachUpdate::Resized { .. } => continue,
            };
            let from = seen.len().saturating_sub(needle.len());
            seen.extend_from_slice(&fresh);
            if contains(&seen[from..], needle) {
                return;
            }
        }
    });
}

#[test]
fn a_dropped_connection_mid_output_reads_every_byte_once_in_order() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().expect("a runtime");
    let rig = Rig::new(&test, &home);
    let view = rig.attach(&test);
    let mut seen = Seen::default();
    seen.until(&test, &view, is_snapshot);

    within(&test, view.input(typed(&numbered_rows(8000)))).expect("the command is typed");
    seen.until_output(&test, &view, "row 100");
    rig.relay.cut();
    seen.until_output(&test, &view, "row 8000");

    let links = seen.link_states();
    let lost = links
        .iter()
        .position(|link| matches!(link, LinkState::Unverifiable { .. }));
    let back = links.iter().position(|link| *link == LinkState::Live);
    assert!(
        matches!((lost, back), (Some(lost), Some(back)) if lost < back),
        "the loss and the return are both reported: {links:?}"
    );
    assert_eq!(
        seen.events
            .iter()
            .filter(|event| is_snapshot(event))
            .count(),
        1,
        "the host still held what was missed, so no snapshot replaces it"
    );

    let runs = output_runs(&seen.events);
    let from = runs.first().expect("the view showed output").0;
    let mut until = from;
    for (offset, len) in runs {
        assert_eq!(offset, until, "the output continues where it stopped");
        until = offset + len as u64;
    }
    // A reattach applies the size again, which can make the host repeat the rows on screen. The view must show exactly
    // the bytes the host holds for that range, repeats included.
    let client = test.block_on(owner(&home));
    let host = host_output(&test, &client, &rig.terminal, from, until);
    let difference = seen
        .output
        .iter()
        .zip(&host)
        .position(|(view, host)| view != host);
    assert!(
        seen.output == host,
        "the view shows every byte of the host's output once, in order: {} and {} bytes, first difference at {difference:?}",
        seen.output.len(),
        host.len(),
    );
    test.block_on(stop(rig.running));
}

#[test]
#[cfg_attr(
    windows,
    ignore = "quarantined 2026-11-25: stalls on windows-latest; network session is fixing the cause; remove with that fix"
)]
fn a_gap_the_host_no_longer_holds_arrives_as_a_snapshot() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().expect("a runtime");
    let rig = Rig::new(&test, &home);
    let view = rig.attach(&test);
    let mut seen = Seen::default();
    seen.until(&test, &view, is_snapshot);

    rig.relay.set_open(false);
    rig.relay.cut();
    seen.until(&test, &view, is_lost);

    // The app is away while the terminal writes more than the host's output buffer holds (1 MiB).
    let client = test.block_on(owner(&home));
    within(
        &test,
        client.terminal_input(&rig.terminal, &typed(&flood(30_000))),
    )
    .expect("the flood is typed");
    wait_for_output(&test, &client, &rig.terminal, "done-marker");

    rig.relay.set_open(true);
    seen.until(&test, &view, is_back);
    let snapshot = seen.until(&test, &view, is_snapshot);
    let TerminalEvent::Snapshot { bytes, .. } = snapshot else {
        panic!("the snapshot is the event read");
    };
    assert!(
        contains(&bytes, "done-marker"),
        "the snapshot shows the screen the app missed"
    );

    within(&test, view.input(typed(&joined_echo("ma", "rker2")))).expect("the marker is typed");
    seen.until_output(&test, &view, "marker2");
    test.block_on(stop(rig.running));
}

#[test]
fn input_reaches_the_terminal_and_a_resize_reaches_the_host() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().expect("a runtime");
    let rig = Rig::new(&test, &home);
    let view = rig.attach(&test);
    let mut seen = Seen::default();
    seen.until(&test, &view, is_snapshot);

    within(&test, view.input(typed(&joined_echo("ma", "rker")))).expect("the input is written");
    seen.until_output(&test, &view, "marker");

    within(&test, view.resize(100, 30)).expect("the resize is accepted");
    let resized = seen.until(&test, &view, |event| {
        matches!(event, TerminalEvent::Resized { .. })
    });
    assert_eq!(
        resized,
        TerminalEvent::Resized {
            cols: 100,
            rows: 30
        }
    );
    let client = test.block_on(owner(&home));
    let shown = test
        .block_on(client.terminal_show(&rig.terminal))
        .expect("the terminal is shown");
    assert_eq!((shown.cols, shown.rows), (100, 30));
    test.block_on(stop(rig.running));
}

#[test]
fn close_detaches_and_leaves_the_terminal_running() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().expect("a runtime");
    let rig = Rig::new(&test, &home);
    let view = rig.attach(&test);

    view.close();
    assert_eq!(within(&test, view.next()).expect("no error"), None);
    let refused =
        within(&test, view.input(b"x".to_vec())).expect_err("a closed view takes no input");
    assert!(matches!(refused, FfiError::Internal { .. }), "{refused:?}");

    let client = test.block_on(owner(&home));
    let shown = test
        .block_on(client.terminal_show(&rig.terminal))
        .expect("the terminal is shown");
    assert_eq!(shown.state, TerminalState::Live);
    test.block_on(stop(rig.running));
}

#[test]
fn a_killed_terminal_reports_how_it_ended_and_the_view_ends() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().expect("a runtime");
    let rig = Rig::new(&test, &home);
    let view = rig.attach(&test);
    let mut seen = Seen::default();
    seen.until(&test, &view, is_snapshot);

    let client = test.block_on(owner(&home));
    test.block_on(client.terminal_kill(&rig.terminal, true))
        .expect("the terminal is killed");
    let exited = seen.until(&test, &view, |event| {
        matches!(event, TerminalEvent::Exited { .. })
    });
    assert!(
        matches!(&exited, TerminalEvent::Exited { reason, .. } if reason == "killed"),
        "{exited:?}"
    );
    assert_eq!(within(&test, view.next()).expect("no error"), None);
    test.block_on(stop(rig.running));
}

#[test]
fn an_attach_to_a_missing_terminal_is_refused_by_the_host() {
    let (_temp, home) = temp_home();
    let test = Runtime::new().expect("a runtime");
    let rig = Rig::new(&test, &home);

    let refused = within(
        &test,
        rig.host
            .attach_terminal("no-such-terminal".to_owned(), 80, 24),
    )
    .expect_err("the host refuses the attach");
    assert!(matches!(refused, FfiError::Remote { .. }), "{refused:?}");
    test.block_on(stop(rig.running));
}
