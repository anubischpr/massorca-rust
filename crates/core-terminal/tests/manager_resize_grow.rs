//! A terminal made taller after its screen filled: output written after the resize lands right below
//! the output before it.
//!
//! Known Windows behaviour, not a failure: the inbox ConPTY repaints its whole screen on a grow, its
//! rows anchored at the top, so the new rows stay blank below the prompt. A Unix PTY keeps no screen,
//! so the emulator pulls scrollback into the new rows and the prompt stays on the last row.

mod emulator_support;
mod manager_support;

use emulator_support::Client;
use manager_support::{Managed, SIZE, line, marker_command, spec, watch};
use massorca_core_terminal::pty::PtySize;

/// Rows added by the resize.
const GROWTH: u16 = 20;

/// Prints more numbered lines than the terminal has rows, so the first ones scroll away.
fn fill_command(lines: u16) -> Vec<u8> {
    if cfg!(windows) {
        line(&format!("for /L %i in (1,1,{lines}) do @echo filler-%i"))
    } else {
        line(&format!(
            "i=1; while [ $i -le {lines} ]; do echo filler-$i; i=$((i+1)); done"
        ))
    }
}

#[tokio::test]
async fn output_after_a_taller_resize_lands_right_below_the_output_before_it() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let manager = Managed::new();
    let info = manager.create(spec("ws", dir.path())).expect("create");
    let mut live = watch(&manager, &info.id, None).await;

    manager
        .input(&info.id, &fill_command(SIZE.rows * 2))
        .expect("input");
    let (command, before) = marker_command("before");
    manager.input(&info.id, &command).expect("input");
    live.wait_for_text(&before).await;

    let taller = PtySize {
        cols: SIZE.cols,
        rows: SIZE.rows + GROWTH,
    };
    manager.resize(&info.id, taller).expect("resize");
    while live.resized.is_empty() {
        live.step().await;
    }
    // `Resized` is queued ahead of the output drawn at the new size (0034).
    let at_resize = live.output.len();

    let (command, after) = marker_command("after");
    manager.input(&info.id, &command).expect("input");
    live.wait_for_text(&after).await;

    let mut client = Client::new(SIZE.cols, SIZE.rows);
    client.advance(&live.snapshot);
    client.advance(&live.output[..at_resize]);
    client.resize(taller.cols, taller.rows);
    client.advance(&live.output[at_resize..]);

    let rows = client.rows();
    let (Some(above), Some(below)) = (client.row_of(&before), client.row_of(&after)) else {
        panic!("both markers are on the screen: {rows:?}");
    };
    assert!(
        above < below,
        "{after} on row {below} is below {before} on row {above}: {rows:?}"
    );
    if cfg!(not(windows)) {
        let last = rows.iter().rposition(|row| !row.is_empty());
        assert_eq!(
            last,
            Some(rows.len() - 1),
            "the prompt stays on the last row after the grow: {rows:?}"
        );
    }
}
