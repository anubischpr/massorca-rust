//! Attaching to terminals: snapshot, exact resume, several clients and slow clients (0034).
//! Every wait is bounded, so a failure reports instead of hanging the suite.

mod manager_support;

use std::time::Instant;

use manager_support::{
    Managed, SIZE, TIMEOUT, Watch, big_file, marker_command, print_file_then_marker, script_shell,
    spec, visible, wait_until, watch,
};
use massorca_core_terminal::attach::{MAX_ATTACHMENTS, QUEUE_BYTES, RESNAPSHOT_INTERVAL};
use massorca_core_terminal::pty::PtySize;
use massorca_core_terminal::ring::RING_CAPACITY;
use massorca_core_terminal::{AttachItem, AttachMode, TerminalError, TerminalState};

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
    // Everything up to the marker, by offset; output still arriving is beyond `until`.
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

    // Compared over the offsets both have read, which hold the marker.
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
    let attached_at = Instant::now();
    let stalled = manager.attach(&info.id, None).expect("attach");
    let mut stalled = Watch::from_attachment(&manager, &info.id, stalled);
    let start = reader.offset();
    let file = big_file(dir.path(), QUEUE_BYTES * 2);
    let (command, expected) = print_file_then_marker(&file, "stalled");

    manager.input(&info.id, &command).expect("input");
    let mut peak = 0;
    reader
        .wait_for_text_sampling(&expected, || {
            peak = peak.max(stalled.attachment.queued_bytes());
        })
        .await;
    let flooded = reader.offset() - start;
    assert!(
        flooded > QUEUE_BYTES as u64,
        "only {flooded} bytes: the queue never filled"
    );
    assert!(peak > 0, "the stalled client never queued anything");
    assert!(peak <= QUEUE_BYTES, "{peak} bytes queued");
    let seen = reader.offset();

    // The snapshot is taken when the client reads, after everything the reader has seen.
    assert!(matches!(
        stalled.step().await,
        Some(AttachItem::Attached { .. })
    ));
    let Some(AttachItem::Snapshot { offset, bytes, .. }) = stalled.step().await else {
        panic!("a lagging attachment resumes with a snapshot");
    };
    assert!(offset >= seen, "snapshot at {offset}, reader saw {seen}");
    // The attach's own snapshot counts: the next one waits out the interval.
    assert!(attached_at.elapsed() >= RESNAPSHOT_INTERVAL);
    assert!(visible(&bytes).contains(&expected));

    // Output continues from the snapshot's offset with nothing lost.
    let (command, expected) = marker_command("caught-up");
    manager.input(&info.id, &command).expect("input");
    stalled.wait_for_text(&expected).await;
    reader.wait_for_offset(stalled.offset()).await;
    let from = (offset - reader.base) as usize;
    let to = (stalled.offset() - reader.base) as usize;
    assert_eq!(stalled.output[..], reader.output[from..to]);
}

#[tokio::test]
async fn an_attach_beyond_the_limit_is_refused_until_one_detaches() {
    let dir = temp_dir();
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let mut attachments: Vec<_> = (0..MAX_ATTACHMENTS)
        .map(|_| manager.attach(&info.id, None).expect("attach"))
        .collect();

    let refused = manager.attach(&info.id, None);

    assert!(
        matches!(refused, Err(TerminalError::TooManyAttachments)),
        "{refused:?}"
    );
    attachments.pop();
    manager.attach(&info.id, None).expect("a freed slot");
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

#[tokio::test]
async fn an_attachment_receives_all_output_before_exited() {
    let dir = temp_dir();
    let manager = Managed::new();
    let mut create = spec("ws", dir.path());
    let script = if cfg!(windows) {
        "for /l %i in (1,1,3000) do @echo line-%i"
    } else {
        "i=1; while [ $i -le 3000 ]; do echo line-$i; i=$((i+1)); done"
    };
    create.shell = Some(script_shell(script));
    let info = manager.create(create).expect("create");
    let mut first = watch(&manager, &info.id, None).await;

    let exited = first.wait_for_exit().await;

    assert_eq!(exited.state, TerminalState::Exited);
    assert!(
        first.visible.contains("line-3000"),
        "the last line must arrive before exited, ending {:?}",
        first.visible.tail()
    );
    // Nothing reaches the terminal after `Exited`: resuming from there finds no output.
    let mut resumed = watch(&manager, &info.id, Some(first.offset())).await;
    resumed.wait_for_exit().await;
    assert_eq!(resumed.mode, AttachMode::Resume);
    assert_eq!(
        resumed.offset(),
        first.offset(),
        "output after exited: {:?}",
        String::from_utf8_lossy(&resumed.output)
    );
}
