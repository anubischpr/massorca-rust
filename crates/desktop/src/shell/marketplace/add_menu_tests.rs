//! Element tests of the Add extension split button: its More ways to add menu, the entries that work, and the zip entry,
//! which reviews a picked file under its file: source. A scripted service answers each call, and a fake picker answers
//! in place of the system's dialog.

use super::super::path_field::{PickKind, answering};
use super::super::service_tests::{click, draw, rig};
use super::extension_tests::review;
use crate::model::navigation::{MarketplacePage, Page};
use crate::service::{Command, ExtensionCommand};
use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::extensions::{SourceKind, SourceRef};

/// Whether an element with this debug selector is drawn, after a fresh draw.
fn shown(cx: &mut VisualTestContext, selector: &'static str) -> bool {
    draw(cx);
    cx.debug_bounds(selector).is_some()
}

/// Opens the Marketplace from the rail and presses the chevron of the Add extension split button.
fn open_more_ways_to_add(cx: &mut VisualTestContext) {
    click(cx, "rail-marketplace");
    click(cx, "button:marketplace-add-more");
}

#[gpui::test]
fn the_more_ways_to_add_menu_lists_the_three_entries_that_work(cx: &mut TestAppContext) {
    let (_rig, cx) = rig(cx, &[]);
    open_more_ways_to_add(cx);
    assert!(shown(cx, "menu-item-0"), "From the Marketplace is listed");
    assert!(shown(cx, "menu-item-1"), "From a folder is listed");
    assert!(shown(cx, "menu-item-2"), "From a file (.zip) is listed");
    assert!(
        !shown(cx, "menu-item-3"),
        "no entry without a service path: Creator mode, Sources and From a Git URL are left out"
    );
}

#[gpui::test]
fn from_a_folder_opens_the_add_extension_dialog(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[]);
    open_more_ways_to_add(cx);
    click(cx, "menu-item-1");
    assert!(
        rig.shell
            .read_with(cx, |shell, _| shell.marketplace_add.is_some()),
        "the folder entry opens the existing Add extension dialog"
    );
}

#[gpui::test]
fn from_the_marketplace_opens_discover(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[]);
    click(cx, "rail-marketplace");
    click(cx, "marketplace-nav-installed");
    click(cx, "button:marketplace-add-more");
    click(cx, "menu-item-0");
    assert_eq!(
        rig.shell
            .read_with(cx, |shell, _| shell.model().navigation().page),
        Page::Marketplace(MarketplacePage::Discover)
    );
}

#[gpui::test]
fn from_a_file_zip_reviews_the_picked_file_under_its_file_source(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[]);
    let (asked, picker) = answering(Some("/packages/notes.zip"));
    rig.shell
        .update(cx, |shell, _| shell.set_path_picker(picker));
    open_more_ways_to_add(cx);
    click(cx, "menu-item-2");
    cx.run_until_parked();
    assert_eq!(
        *asked.borrow(),
        [PickKind::File],
        "the file picker was asked"
    );
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Inspect { source, .. })) => {
            assert_eq!(source, "file:/packages/notes.zip");
        }
        _ => panic!("the picked file is reviewed with the service"),
    }
}

#[gpui::test]
fn a_zip_install_sends_the_hash_its_review_read(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[]);
    let (_asked, picker) = answering(Some("/packages/notes.zip"));
    rig.shell
        .update(cx, |shell, _| shell.set_path_picker(picker));
    open_more_ways_to_add(cx);
    click(cx, "menu-item-2");
    cx.run_until_parked();
    let reply = match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Inspect { reply, .. })) => reply,
        _ => panic!("the zip is reviewed with the service"),
    };
    let mut answer = review(false);
    answer.source = SourceRef {
        kind: SourceKind::File,
        reference: "/packages/notes.zip".to_owned(),
    };
    let _ = reply.send(Ok(answer));
    cx.run_until_parked();
    draw(cx);
    click(cx, "dialog-action");
    match rig.script.next() {
        Some(Command::Extension(ExtensionCommand::Install { params, .. })) => {
            assert_eq!(params.source, "file:/packages/notes.zip");
            assert_eq!(
                params.sha256.as_deref(),
                Some("0123abcd"),
                "an archive installs against the hash its review read"
            );
        }
        _ => panic!("the press sends the install"),
    }
}

#[gpui::test]
fn the_folder_dialog_draws_no_disabled_git_url_control(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[]);
    rig.shell
        .update(cx, |shell, cx| shell.open_add_extension(cx));
    draw(cx);
    assert!(
        shown(cx, "marketplace-add-field"),
        "the folder field is drawn"
    );
    assert!(
        !shown(cx, "button:marketplace-add-git-url"),
        "no disabled control: a Git URL has no service path yet (SKL-18)"
    );
}
