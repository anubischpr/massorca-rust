//! The Browse button of the folder fields: the picker fills the field, and a folder on another host has none. A fake
//! picker answers in place of the system's dialog, so no dialog or window opens.

use gpui::TestAppContext;

use super::path_field::{PickKind, answering};
use super::service_tests::{click, draw, rig};
use super::settings_tests::settings_rig;
use crate::model::marketplace::HostTarget;
use crate::model::navigation::MarketplacePage;

#[gpui::test]
fn browse_on_add_a_project_fills_the_folder_field_and_the_form_follows(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    let (asked, picker) = answering(Some("/repos/gamma"));
    rig.shell
        .update(cx, |shell, _| shell.set_path_picker(picker));
    rig.shell
        .update_in(cx, |shell, window, cx| shell.open_add_project(window, cx));
    draw(cx);
    click(cx, "button:add-project-browse");
    assert_eq!(
        *asked.borrow(),
        [PickKind::Folder],
        "the folder picker was asked"
    );
    rig.shell.read_with(cx, |shell, cx| {
        let dialog = shell
            .dialogs
            .add_project
            .as_ref()
            .expect("the dialog is open");
        assert_eq!(dialog.path.read(cx).text(), "/repos/gamma");
        assert_eq!(
            dialog.form.path, "/repos/gamma",
            "the form takes the field's text"
        );
    });
}

#[gpui::test]
fn a_cancelled_browse_leaves_the_typed_path_in_the_field(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    let (asked, picker) = answering(None);
    rig.shell
        .update(cx, |shell, _| shell.set_path_picker(picker));
    rig.shell
        .update_in(cx, |shell, window, cx| shell.open_add_project(window, cx));
    draw(cx);
    let field = rig.shell.read_with(cx, |shell, _| {
        shell
            .dialogs
            .add_project
            .as_ref()
            .expect("the dialog is open")
            .path
            .clone()
    });
    field.update_in(cx, |field, window, cx| {
        field.set_text("/typed/path", window, cx)
    });
    click(cx, "button:add-project-browse");
    assert_eq!(asked.borrow().len(), 1, "the picker was asked");
    assert_eq!(
        field.read_with(cx, |field, _| field.text().to_owned()),
        "/typed/path"
    );
}

#[gpui::test]
fn browse_on_the_workspace_folder_fills_the_settings_field(cx: &mut TestAppContext) {
    let (rig, cx) = settings_rig(cx);
    let (asked, picker) = answering(Some("D:/projects/workspaces"));
    rig.shell
        .update(cx, |shell, _| shell.set_path_picker(picker));
    draw(cx);
    click(cx, "button:settings-folder-browse");
    assert_eq!(*asked.borrow(), [PickKind::Folder]);
    assert_eq!(
        rig.shell.read_with(cx, |shell, cx| shell
            .settings_folder
            .read(cx)
            .text()
            .to_owned()),
        "D:/projects/workspaces"
    );
}

#[gpui::test]
fn browse_on_add_extension_fills_the_folder_and_a_folder_on_another_host_has_none(
    cx: &mut TestAppContext,
) {
    let (rig, cx) = rig(cx, &[]);
    let (asked, picker) = answering(Some("C:/Users/me/kit"));
    rig.shell
        .update(cx, |shell, _| shell.set_path_picker(picker));
    rig.shell.update(cx, |shell, cx| {
        shell.open_marketplace(MarketplacePage::Discover, cx)
    });
    click(cx, "button:marketplace-add-extension");
    click(cx, "button:marketplace-add-browse");
    assert_eq!(*asked.borrow(), [PickKind::Folder]);
    let folder = rig.shell.read_with(cx, |shell, cx| {
        shell
            .marketplace_add
            .as_ref()
            .expect("the dialog is open")
            .field()
            .read(cx)
            .text()
            .to_owned()
    });
    assert_eq!(folder, "C:/Users/me/kit");

    rig.shell.update(cx, |shell, _| {
        shell
            .marketplace_pick
            .aim(HostTarget::Paired("vps".to_owned()))
    });
    draw(cx);
    assert!(
        cx.debug_bounds("button:marketplace-add-browse").is_none(),
        "a folder on another host has no Browse"
    );
}
