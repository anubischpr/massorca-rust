//! Attaching to terminals: snapshot, exact resume, several clients and slow clients (0034).
//! Every wait is bounded, so a failure reports instead of hanging the suite.

mod manager_support;

use std::time::Duration;

use manager_support::{
    Managed, SIZE, TIMEOUT, big_file, marker_command, print_file_then_marker, script_shell, spec,
    visible, wait_until, watch,
};
use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::ring::RING_CAPACITY;
use massorca_core_terminal::{AttachItem, AttachMode, TerminalState};

const QUIET: Duration = Duration::from_millis(300);
/// The attachment queue limit of 0034.
const QUEUE_CHUNKS: usize = 256;

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("a temporary directory")
}

#[tokio::test]
async fn the_first_attach_is_a_snapshot_of_the_screen() {
    let dir = temp_dir();
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let mut first = watch(&manager, &info.id, None).await;
    let (command, expected) = marker_command("snap");
    manager.input(&info.id, &command).expect("input");
    first.wait_for_text(&expected).await;

    let second = watch(&manager, &info.id, None).await;

    assert_eq!(second.mode, AttachMode::Snapshot);
    assert_eq!(second.snapshot_size, (SIZE.cols, SIZE.rows));
    assert!(
        second.visible.contains(&expected),
        "{:?}",
        second.visible.as_string()
    );
    assert!(second.base >= first.base);
}

#[tokio::test]
async fn reattaching_with_since_resumes_with_exactly_the_missed_bytes() {
    let dir = temp_dir();
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let mut truth = watch(&manager, &info.id, None).await;
    let mut early = watch(&manager, &info.id, None).await;
    let (command, expected) = marker_command("before");
    manager.input(&info.id, &command).expect("input");
    early.wait_for_text(&expected).await;
    let since = early.offset();
    drop(early);

    let (command, expected) = marker_command("between");
    manager.input(&info.id, &command).expect("input");
    truth.wait_for_text(&expected).await;
    truth.settle(QUIET).await;
    let until = truth.offset();

    let mut resumed = watch(&manager, &info.id, Some(since)).await;
    assert_eq!(resumed.mode, AttachMode::Resume);
    assert_eq!(resumed.base, since);
    resumed.wait_for_offset(until).await;

    let missed = (until - since) as usize;
    let from = (since - truth.base) as usize;
    assert_eq!(
        resumed.output[..missed],
        truth.output[from..from + missed],
        "resume must hold exactly the bytes produced while detached"
    );
    assert!(visible(&resumed.output[..missed]).contains(&expected));
}

#[tokio::test]
async fn since_older_than_the_ring_gets_a_snapshot() {
    let dir = temp_dir();
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let mut truth = watch(&manager, &info.id, None).await;
    let file = big_file(dir.path(), RING_CAPACITY * 3 / 2);
    let (command, expected) = print_file_then_marker(&file, "flooded");

    manager.input(&info.id, &command).expect("input");
    truth.wait_for_text(&expected).await;
    assert!(
        truth.offset() > RING_CAPACITY as u64,
        "only {} bytes of output",
        truth.offset()
    );

    let late = watch(&manager, &info.id, Some(0)).await;
    assert_eq!(late.mode, AttachMode::Snapshot);
    assert!(late.base > 0);
    let beyond = watch(&manager, &info.id, Some(u64::MAX)).await;
    assert_eq!(beyond.mode, AttachMode::Snapshot);
}

#[tokio::test]
async fn two_attachments_receive_the_same_output() {
    let dir = temp_dir();
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let mut first = watch(&manager, &info.id, None).await;
    let mut second = watch(&manager, &info.id, None).await;

    let (command, expected) = marker_command("both");
    manager.input(&info.id, &command).expect("input");
    first.wait_for_text(&expected).await;
    second.wait_for_text(&expected).await;
    first.settle(QUIET).await;
    second.settle(QUIET).await;

    let start = first.base.max(second.base);
    let end = first.offset().min(second.offset());
    assert!(end > start);
    let slice = |watch: &manager_support::Watch| {
        watch.output[(start - watch.base) as usize..(end - watch.base) as usize].to_vec()
    };
    assert_eq!(slice(&first), slice(&second));
    assert!(visible(&slice(&first)).contains(&expected));
}

#[tokio::test]
async fn a_resize_from_one_client_reaches_the_other() {
    let dir = temp_dir();
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let mut first = watch(&manager, &info.id, None).await;
    let mut second = watch(&manager, &info.id, None).await;

    manager
        .resize(&info.id, PtySize { cols: 90, rows: 25 })
        .expect("resize");

    for watch in [&mut first, &mut second] {
        while watch.resized.is_empty() {
            watch.step().await;
        }
        assert_eq!(watch.resized, vec![(90, 25)]);
    }
}

#[tokio::test]
async fn a_client_that_never_reads_gets_a_snapshot_and_never_blocks_the_shell() {
    let dir = temp_dir();
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let mut reader = watch(&manager, &info.id, None).await;
    reader.wait_for_output().await;
    let mut stalled = manager.attach(&info.id, None).expect("attach");
    let items_before = reader.output_items;
    let file = big_file(dir.path(), RING_CAPACITY * 3 / 2);
    let (command, expected) = print_file_then_marker(&file, "stalled");

    manager.input(&info.id, &command).expect("input");
    reader.wait_for_text(&expected).await;
    assert!(
        reader.output_items - items_before > QUEUE_CHUNKS,
        "only {} chunks: the queue never filled",
        reader.output_items - items_before
    );
    reader.settle(QUIET).await;

    let mut first_content = None;
    while first_content.is_none() {
        let item = tokio::time::timeout(TIMEOUT, stalled.next())
            .await
            .expect("an item")
            .expect("not ended");
        match item {
            AttachItem::Attached { .. } => {}
            other => first_content = Some(other),
        }
    }
    let Some(AttachItem::Snapshot { offset, bytes, .. }) = first_content else {
        panic!("a lagging attachment resumes with a snapshot: {first_content:?}");
    };
    assert_eq!(offset, reader.offset());
    assert_eq!(stalled.offset(), offset);
    assert!(visible(&bytes).contains(&expected));
}

#[tokio::test]
async fn attaching_to_an_exited_terminal_shows_its_last_screen() {
    let dir = temp_dir();
    let manager = Managed::new();
    let mut create = spec("ws", dir.path());
    let (script, expected) = if cfg!(windows) {
        ("echo bye-%OS%", "bye-Windows_NT")
    } else {
        ("echo bye-$((6*7))", "bye-42")
    };
    create.shell = Some(script_shell(script));
    let info = manager.create(create).expect("create");
    wait_until("the terminal to exit", || {
        manager
            .get(&info.id)
            .is_ok_and(|info| info.state == TerminalState::Exited)
    });

    let mut attachment = manager.attach(&info.id, None).expect("attach");
    let mut next = async || {
        tokio::time::timeout(TIMEOUT, attachment.next())
            .await
            .expect("an item")
    };

    assert!(matches!(
        next().await,
        Some(AttachItem::Attached {
            mode: AttachMode::Snapshot,
            ..
        })
    ));
    let Some(AttachItem::Snapshot { bytes, .. }) = next().await else {
        panic!("a snapshot follows attached");
    };
    assert!(
        visible(&bytes).contains(expected),
        "{:?}",
        visible(&bytes).as_string()
    );
    let Some(AttachItem::Exited { info: exited }) = next().await else {
        panic!("exited follows the snapshot");
    };
    assert_eq!(exited.state, TerminalState::Exited);
    assert!(next().await.is_none());
}
