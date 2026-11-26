//! The notice a session's header shows when other sessions also write in its folder (decision 0169). A scripted
//! handle stands in for the service, as the session view's element tests do.

use super::session_view::shared_folder_notice;
use super::session_view_tests::{kind, s1, session_rig};
use crate::service::fixtures::agent;
use gpui::TestAppContext;

#[test]
fn the_notice_names_each_other_writer_by_its_title_and_an_unknown_one_generically() {
    let mut docs = agent("s2", "w1", 20);
    docs.title = Some("Docs pass".to_owned());
    let mut open = agent("s1", "w1", 10);
    open.shared_folder_with = vec!["s2".to_owned(), "s3".to_owned()];
    let sessions = [open.clone(), docs];
    assert_eq!(
        shared_folder_notice(&open, &sessions, &[]),
        Some("Also writing in this folder: Docs pass, another session.".to_owned())
    );
}

#[test]
fn no_notice_while_no_other_session_writes_in_the_folder() {
    let open = agent("s1", "w1", 10);
    assert_eq!(
        shared_folder_notice(&open, std::slice::from_ref(&open), &[]),
        None
    );
}

#[gpui::test]
fn a_session_that_shares_its_folder_draws_the_notice(cx: &mut TestAppContext) {
    let mut writer = agent("s2", "w1", 20);
    writer.title = Some("Docs pass".to_owned());
    let mut open = s1(false, None);
    open.shared_folder_with = vec!["s2".to_owned()];
    let (_rig, cx) = session_rig(cx, vec![kind(true)], vec![open, writer]);
    assert!(
        cx.debug_bounds("session-shared-folder").is_some(),
        "another writer in the folder is named in the session's header"
    );
}

#[gpui::test]
fn a_session_alone_in_its_folder_draws_no_notice(cx: &mut TestAppContext) {
    let (_rig, cx) = session_rig(cx, vec![kind(true)], vec![s1(false, None)]);
    assert!(
        cx.debug_bounds("session-shared-folder").is_none(),
        "no notice while no other session writes in the folder"
    );
}
