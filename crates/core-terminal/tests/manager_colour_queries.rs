//! Colour queries (OSC 10, 11 and 12) against a real shell: the service answers from its
//! palette when no emulator is attached, and leaves them to an attached emulator.
//! Unix only: the script reads the reply from its terminal with `dd`.
#![cfg(unix)]

mod manager_support;

use std::time::{Duration, Instant};

use manager_support::{Managed, TIMEOUT, contains, line, script_shell, spec};
use massorca_core_terminal::{AttachItem, Attachment, QueryAnswers, TerminalTheme};

/// Asks `query` once told to go, reads a reply of `len` bytes, and prints which colour it got.
fn colour_shell(query: &str, len: usize) -> massorca_core_terminal::ShellChoice {
    script_shell(&format!(
        "read go; stty -icanon -echo; printf '{query}'; \
         reply=$(dd bs=1 count={len} 2>/dev/null); \
         case \"$reply\" in \
           *rgb:1010/1010/1212*) echo dark-bg-$((6*7));; \
           *rgb:1f1f/1f1f/2626*) echo light-fg-$((6*7));; \
           *rgb:0000/0000/0000*) echo emulator-$((6*7));; \
           *) echo other-$((6*7));; \
         esac; sleep 60"
    ))
}

async fn read_until_seen(attachment: &mut Attachment, needle: &[u8]) {
    let mut raw = Vec::new();
    let deadline = Instant::now() + TIMEOUT;
    while !contains(&raw, needle) {
        assert!(Instant::now() < deadline, "no {needle:?} in {raw:?}");
        match tokio::time::timeout(TIMEOUT, attachment.next()).await {
            Ok(Some(AttachItem::Output { bytes, .. })) => raw.extend_from_slice(&bytes),
            Ok(Some(_)) => {}
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[tokio::test]
async fn the_service_answers_a_background_query_ended_by_bel() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let manager = Managed::new();
    let mut create = spec("ws", dir.path());
    create.shell = Some(colour_shell("\\033]11;?\\007", 24));
    let info = manager.create(create).expect("create");
    // A log tail: it reads but answers nothing.
    let mut log = manager
        .attach(&info.id, Some(0), QueryAnswers::Service)
        .expect("attach");
    manager.input(&info.id, &line("go")).expect("input");
    read_until_seen(&mut log, b"dark-bg-42").await;
}

#[tokio::test]
async fn the_service_answers_a_foreground_query_ended_by_st_from_the_light_palette() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let manager = Managed::new();
    manager.set_theme(TerminalTheme::Light);
    let mut create = spec("ws", dir.path());
    create.shell = Some(colour_shell("\\033]10;?\\033\\\\", 25));
    let info = manager.create(create).expect("create");
    let mut log = manager
        .attach(&info.id, Some(0), QueryAnswers::Service)
        .expect("attach");
    manager.input(&info.id, &line("go")).expect("input");
    read_until_seen(&mut log, b"light-fg-42").await;
}

#[tokio::test]
async fn an_attached_emulator_answers_a_colour_query_alone() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let manager = Managed::new();
    let mut create = spec("ws", dir.path());
    create.shell = Some(colour_shell("\\033]11;?\\007", 24));
    let info = manager.create(create).expect("create");
    let mut emulator = manager
        .attach(&info.id, Some(0), QueryAnswers::Client)
        .expect("attach");
    manager.input(&info.id, &line("go")).expect("input");
    read_until_seen(&mut emulator, b"\x1b]11;?\x07").await;
    // Had the service answered too, the shell would have read its colour, not the emulator's.
    tokio::time::sleep(Duration::from_millis(300)).await;
    manager
        .input(&info.id, b"\x1b]11;rgb:0000/0000/0000\x07")
        .expect("input");
    read_until_seen(&mut emulator, b"emulator-42").await;
}
