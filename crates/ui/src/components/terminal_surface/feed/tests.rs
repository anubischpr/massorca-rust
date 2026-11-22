use super::*;
use alacritty_terminal::index::Line;

fn attached_feed(cols: u16, rows: u16, offset: u64) -> TerminalFeed {
    let mut feed = TerminalFeed::new(cols, rows);
    feed.apply(FeedEvent::Attached { offset }).expect("attach");
    feed
}

fn output(feed: &mut TerminalFeed, offset: u64, bytes: &[u8]) -> Result<FeedOutcome, FeedError> {
    feed.apply(FeedEvent::Output { offset, bytes })
}

fn screen_row(feed: &TerminalFeed, row: usize) -> String {
    let grid = feed.term.grid();
    let text: String = (0..feed.columns())
        .map(|column| grid[Line(row as i32)][Column(column)].c)
        .collect();
    text.trim_end().to_owned()
}

#[test]
fn attach_makes_output_continue_from_its_offset() {
    let mut feed = attached_feed(20, 4, 5);
    assert_eq!(output(&mut feed, 5, b"a"), Ok(FeedOutcome::Applied));
    assert_eq!(feed.next_offset(), Some(6));
}

#[test]
fn output_before_an_attach_is_an_error() {
    let mut feed = TerminalFeed::new(20, 4);
    assert_eq!(output(&mut feed, 0, b"a"), Err(FeedError::NotAttached));
    assert_eq!(feed.next_offset(), None);
}

#[test]
fn output_wholly_before_the_next_offset_is_skipped() {
    let mut feed = attached_feed(20, 4, 0);
    assert_eq!(output(&mut feed, 0, b"abc"), Ok(FeedOutcome::Applied));
    assert_eq!(output(&mut feed, 0, b"abc"), Ok(FeedOutcome::Skipped));
    assert_eq!(output(&mut feed, 1, b"bc"), Ok(FeedOutcome::Skipped));
    assert_eq!(screen_row(&feed, 0), "abc");
    assert_eq!(feed.next_offset(), Some(3));
}

#[test]
fn output_straddling_the_next_offset_applies_only_its_new_tail() {
    let mut feed = attached_feed(20, 4, 0);
    output(&mut feed, 0, b"ab").expect("first chunk");
    assert_eq!(output(&mut feed, 1, b"bcd"), Ok(FeedOutcome::Applied));
    assert_eq!(screen_row(&feed, 0), "abcd");
    assert_eq!(feed.next_offset(), Some(4));
}

#[test]
fn output_past_the_next_offset_is_a_gap_and_applies_nothing() {
    let mut feed = attached_feed(20, 4, 0);
    output(&mut feed, 0, b"ab").expect("first chunk");
    assert_eq!(
        output(&mut feed, 5, b"xyz"),
        Ok(FeedOutcome::Gap {
            expected: 2,
            got: 5
        })
    );
    assert_eq!(screen_row(&feed, 0), "ab");
    assert_eq!(feed.next_offset(), Some(2));
}

#[test]
fn a_snapshot_resets_the_screen_and_feeds_exactly_its_length() {
    let mut feed = attached_feed(20, 4, 0);
    output(&mut feed, 0, b"old text").expect("old output");
    feed.apply(FeedEvent::Snapshot {
        offset: 10,
        cols: 20,
        rows: 4,
        len: 6,
    })
    .expect("snapshot");
    assert_eq!(
        feed.next_offset(),
        Some(8),
        "not moved until the bytes arrive"
    );
    feed.apply(FeedEvent::SnapshotBytes(b"new"))
        .expect("first part");
    assert_eq!(feed.next_offset(), Some(8), "still waiting for the rest");
    feed.apply(FeedEvent::SnapshotBytes(b"\r\nx"))
        .expect("second part");
    assert_eq!(feed.next_offset(), Some(10));
    assert_eq!(screen_row(&feed, 0), "new");
    assert_eq!(screen_row(&feed, 1), "x");
    assert_eq!(output(&mut feed, 10, b"y"), Ok(FeedOutcome::Applied));
    assert_eq!(screen_row(&feed, 1), "xy");
}

#[test]
fn a_zero_length_snapshot_completes_at_once() {
    let mut feed = attached_feed(20, 4, 0);
    feed.apply(FeedEvent::Snapshot {
        offset: 7,
        cols: 20,
        rows: 4,
        len: 0,
    })
    .expect("snapshot");
    assert_eq!(feed.next_offset(), Some(7));
}

#[test]
fn a_snapshot_sizes_the_emulator() {
    let mut feed = attached_feed(20, 4, 0);
    feed.apply(FeedEvent::Snapshot {
        offset: 0,
        cols: 33,
        rows: 9,
        len: 0,
    })
    .expect("snapshot");
    assert_eq!((feed.columns(), feed.lines()), (33, 9));
}

#[test]
fn snapshot_bytes_beyond_the_announced_length_are_an_error() {
    let mut feed = attached_feed(20, 4, 0);
    feed.apply(FeedEvent::Snapshot {
        offset: 0,
        cols: 20,
        rows: 4,
        len: 2,
    })
    .expect("snapshot");
    assert_eq!(
        feed.apply(FeedEvent::SnapshotBytes(b"abc")),
        Err(FeedError::SnapshotOverflow {
            remaining: 2,
            got: 3
        })
    );
}

#[test]
fn output_during_a_snapshot_is_an_error() {
    let mut feed = attached_feed(20, 4, 0);
    feed.apply(FeedEvent::Snapshot {
        offset: 0,
        cols: 20,
        rows: 4,
        len: 4,
    })
    .expect("snapshot");
    feed.apply(FeedEvent::SnapshotBytes(b"ab")).expect("part");
    assert_eq!(
        output(&mut feed, 0, b"x"),
        Err(FeedError::SnapshotIncomplete { missing: 2 })
    );
}

#[test]
fn snapshot_bytes_without_a_snapshot_are_an_error() {
    let mut feed = attached_feed(20, 4, 0);
    assert_eq!(
        feed.apply(FeedEvent::SnapshotBytes(b"a")),
        Err(FeedError::UnexpectedSnapshot)
    );
}

#[test]
fn a_new_attach_discards_an_unfinished_snapshot() {
    let mut feed = attached_feed(20, 4, 0);
    feed.apply(FeedEvent::Snapshot {
        offset: 0,
        cols: 20,
        rows: 4,
        len: 10,
    })
    .expect("snapshot");
    feed.apply(FeedEvent::Attached { offset: 3 })
        .expect("attach");
    assert_eq!(output(&mut feed, 3, b"x"), Ok(FeedOutcome::Applied));
}

#[test]
fn a_gap_is_recovered_by_a_snapshot_at_the_gap_offset() {
    let mut feed = attached_feed(20, 4, 0);
    output(&mut feed, 0, b"ab").expect("first chunk");
    assert!(matches!(
        output(&mut feed, 5, b"xyz"),
        Ok(FeedOutcome::Gap { .. })
    ));
    feed.apply(FeedEvent::Attached { offset: 5 })
        .expect("reattach");
    feed.apply(FeedEvent::Snapshot {
        offset: 5,
        cols: 20,
        rows: 4,
        len: 3,
    })
    .expect("snapshot");
    feed.apply(FeedEvent::SnapshotBytes(b"xyz")).expect("bytes");
    assert_eq!(output(&mut feed, 5, b"!"), Ok(FeedOutcome::Applied));
    assert_eq!(screen_row(&feed, 0), "xyz!");
}

#[test]
fn a_resize_changes_the_grid_and_keeps_the_offset() {
    let mut feed = attached_feed(20, 4, 0);
    output(&mut feed, 0, b"ab").expect("output");
    feed.apply(FeedEvent::Resized { cols: 10, rows: 3 })
        .expect("resize");
    assert_eq!((feed.columns(), feed.lines()), (10, 3));
    assert_eq!(feed.next_offset(), Some(2));
}

#[test]
fn a_resize_is_clamped_to_the_service_limits() {
    let mut feed = attached_feed(20, 4, 0);
    feed.apply(FeedEvent::Resized { cols: 0, rows: 0 })
        .expect("tiny");
    assert_eq!((feed.columns(), feed.lines()), (2, 1));
    feed.apply(FeedEvent::Resized {
        cols: u16::MAX,
        rows: u16::MAX,
    })
    .expect("huge");
    assert_eq!((feed.columns(), feed.lines()), (1000, 500));
}

#[test]
fn an_exit_records_its_code() {
    let mut feed = attached_feed(20, 4, 0);
    assert_eq!(feed.exit(), None);
    feed.apply(FeedEvent::Exited { code: Some(1) })
        .expect("exit");
    assert_eq!(feed.exit(), Some(Exit { code: Some(1) }));
}

#[test]
fn the_program_sets_the_title_with_osc_two() {
    let mut feed = attached_feed(20, 4, 0);
    output(&mut feed, 0, b"\x1b]2;zsh\x07").expect("title");
    assert_eq!(feed.title().as_deref(), Some("zsh"));
}

#[test]
fn a_cursor_report_request_is_queued_as_a_reply_once() {
    let mut feed = attached_feed(20, 4, 0);
    output(&mut feed, 0, b"\x1b[6n").expect("request");
    assert_eq!(feed.take_replies(), b"\x1b[1;1R".to_vec());
    assert!(feed.take_replies().is_empty());
}

#[test]
fn queued_replies_are_capped_so_a_chatty_program_cannot_grow_memory() {
    let mut feed = attached_feed(20, 4, 0);
    let requests = b"\x1b[6n".repeat(2000);
    output(&mut feed, 0, &requests).expect("requests");
    assert!(feed.take_replies().len() <= 4096);
}

#[test]
fn a_selection_copies_the_cells_between_its_ends() {
    let mut feed = attached_feed(20, 4, 0);
    output(&mut feed, 0, b"hello").expect("output");
    feed.begin_selection(
        SelectionType::Simple,
        ViewPoint { row: 0, column: 1 },
        Side::Left,
    );
    feed.extend_selection(ViewPoint { row: 0, column: 3 }, Side::Right);
    assert_eq!(feed.selected_text().as_deref(), Some("ell"));
    feed.clear_selection();
    assert_eq!(feed.selected_text(), None);
}
