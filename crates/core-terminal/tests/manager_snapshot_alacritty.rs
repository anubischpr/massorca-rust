//! A reattach from a snapshot leaves a second emulator where the shell is: output typed after the
//! reattach lands after the output before it, as the real stream places it.

mod emulator_support;
mod manager_support;

use emulator_support::{Client, vt100_model};
use manager_support::{Managed, SIZE, marker_command, spec, watch};
use massorca_core_terminal::AttachMode;

#[tokio::test]
async fn output_after_a_snapshot_reattach_lands_after_the_output_before_it() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let mut live = watch(&manager, &info.id, None).await;
    let mut outputs = Vec::new();
    for tag in ["one", "two", "three"] {
        let (command, expected) = marker_command(tag);
        manager.input(&info.id, &command).expect("input");
        live.wait_for_text(&expected).await;
        outputs.push(expected);
    }
    let previous = outputs.last().cloned().expect("three outputs");

    let mut attached = watch(&manager, &info.id, None).await;
    assert_eq!(attached.mode, AttachMode::Snapshot);
    // The snapshot's cursor is the cursor the stream left: vt100 reads the same bytes to the same place.
    while live.offset() < attached.base {
        live.step().await;
    }
    let upto = usize::try_from(attached.base - live.base).expect("the stream fits in memory");
    let stream = [live.snapshot.as_slice(), &live.output[..upto]].concat();
    let (cols, rows) = (SIZE.cols, SIZE.rows);
    assert_eq!(
        vt100_model(cols, rows, &attached.snapshot)
            .screen()
            .cursor_position(),
        vt100_model(cols, rows, &stream).screen().cursor_position(),
        "the snapshot's cursor is the stream's cursor"
    );

    let mut client = Client::new(cols, rows);
    client.advance(&attached.snapshot);
    let (command, expected) = marker_command("after");
    manager.input(&info.id, &command).expect("input");
    attached.wait_for_text(&expected).await;
    client.advance(&attached.output);
    let before = client.row_of(&previous);
    let after = client.row_of(&expected);
    assert!(
        matches!((before, after), (Some(before), Some(after)) if after > before),
        "{expected} on row {after:?}, not below {previous} on row {before:?}; the screen: {:?}",
        client.rows()
    );
}
