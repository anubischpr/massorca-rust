//! The Add a project dialog's icon: the grid chooses a bundled icon, Reset leaves the icon to the service, and an upload
//! is kept as the icon or refused with its reason under the grid. A fake picker answers in place of the system's dialog.

use gpui::TestAppContext;
use massorca_protocol::methods::projects::{ProjectIcon, ProjectKind};

use super::path_field::{PickKind, answering};
use super::service_tests::{click, inspection, rig};
use crate::service::Command;

/// A PNG's signature padded to `len` bytes: the icon checks read the format and the size only.
fn png(len: usize) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.resize(len, 0);
    bytes
}

fn lucide(name: &str) -> ProjectIcon {
    ProjectIcon::Lucide {
        name: name.to_owned(),
    }
}

#[gpui::test]
fn a_grid_icon_chosen_in_add_a_project_is_the_form_icon(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.shell
        .update_in(cx, |shell, window, cx| shell.open_add_project(window, cx));
    click(cx, "project-icon-button");
    click(cx, "project-icon-cell-star");
    rig.shell.read_with(cx, |shell, _| {
        let form = &shell
            .dialogs
            .add_project
            .as_ref()
            .expect("the dialog is open")
            .form;
        assert_eq!(form.icon, Some(lucide("star")));
        assert!(!form.icon_open, "the grid closes once an icon is chosen");
    });
}

#[gpui::test]
fn reset_in_add_a_project_leaves_the_icon_to_the_service(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.shell
        .update_in(cx, |shell, window, cx| shell.open_add_project(window, cx));
    click(cx, "project-icon-button");
    click(cx, "project-icon-cell-star");
    click(cx, "project-icon-button");
    click(cx, "project-icon-reset");
    rig.shell.read_with(cx, |shell, _| {
        let form = &shell
            .dialogs
            .add_project
            .as_ref()
            .expect("the dialog is open")
            .form;
        assert_eq!(form.icon, None);
    });
}

#[gpui::test]
fn the_icon_chosen_in_add_a_project_travels_with_the_add(cx: &mut TestAppContext) {
    let (mut rig, cx) = rig(cx, &[("p1", "alpha")]);
    let (_, picker) = answering(Some("/repos/gamma"));
    rig.shell
        .update(cx, |shell, _| shell.set_path_picker(picker));
    rig.shell
        .update_in(cx, |shell, window, cx| shell.open_add_project(window, cx));
    click(cx, "button:add-project-browse");
    click(cx, "project-icon-button");
    click(cx, "project-icon-cell-star");
    rig.shell
        .update(cx, |shell, cx| shell.submit_add_project(cx));
    cx.run_until_parked();
    // The dialog asks the service what the path is first; a repository goes on to the add.
    let params = loop {
        match rig.script.next() {
            Some(Command::InspectProject { reply, .. }) => {
                let _ = reply.send(Ok(inspection(ProjectKind::Git, "/repos/gamma", None)));
                cx.run_until_parked();
            }
            Some(Command::AddProject { params, .. }) => break params,
            Some(_) => {}
            None => panic!("a project.add call was expected"),
        }
    };
    assert_eq!(params.path, "/repos/gamma");
    assert_eq!(params.icon, Some(lucide("star")));
}

#[gpui::test]
fn an_uploaded_png_becomes_the_add_a_project_icon(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let image = folder.path().join("badge.png");
    std::fs::write(&image, png(64)).expect("the image file");
    let (asked, picker) = answering(image.to_str());
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.shell
        .update(cx, |shell, _| shell.set_path_picker(picker));
    rig.shell
        .update_in(cx, |shell, window, cx| shell.open_add_project(window, cx));
    click(cx, "project-icon-button");
    click(cx, "project-icon-upload");
    assert_eq!(*asked.borrow(), [PickKind::File]);
    rig.shell.read_with(cx, |shell, _| {
        let form = &shell
            .dialogs
            .add_project
            .as_ref()
            .expect("the dialog is open")
            .form;
        assert!(
            matches!(form.icon, Some(ProjectIcon::Image { .. })),
            "the PNG is the icon"
        );
        assert!(!form.icon_open, "the grid closes once the icon is read");
    });
}

#[gpui::test]
fn an_upload_that_is_not_a_png_keeps_the_grid_open_with_its_reason(cx: &mut TestAppContext) {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let notes = folder.path().join("notes.txt");
    std::fs::write(&notes, b"not an image").expect("the text file");
    let (_, picker) = answering(notes.to_str());
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    rig.shell
        .update(cx, |shell, _| shell.set_path_picker(picker));
    rig.shell
        .update_in(cx, |shell, window, cx| shell.open_add_project(window, cx));
    click(cx, "project-icon-button");
    click(cx, "project-icon-upload");
    rig.shell.read_with(cx, |shell, _| {
        let form = &shell
            .dialogs
            .add_project
            .as_ref()
            .expect("the dialog is open")
            .form;
        assert_eq!(form.icon, None, "a refused file is not the icon");
        assert!(form.icon_open, "the grid stays open for another choice");
        assert!(
            form.icon_refusal
                .as_deref()
                .is_some_and(|reason| reason.starts_with("The icon was not set:")),
            "the refusal says why: {:?}",
            form.icon_refusal
        );
    });
}
