//! Element tests of an image file's diff: the before and after images draw side by side, and a side over the size cap
//! is named in a sentence instead. The state is set directly, and no service answers, so no window opens.

use gpui::{TestAppContext, VisualTestContext};
use massorca_protocol::methods::git::ChangeKind;
use massorca_protocol::methods::git::diff::DiffTarget;

use super::service_tests::{Rig, draw, rig};
use super::source_control::OpenFile;
use crate::model::image_preview::{ImagePreview, ImageSide};
use crate::service::StoreUpdate;
use crate::service::fixtures::workspace;

/// The signature of a PNG: enough bytes for a side to hold an image.
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

/// Opens the diff of `pic.png` with `preview` as its images in a workspace, and draws the page. The Canvas takes a
/// diff over only while a workspace page is on screen.
fn open_image(rig: &Rig, cx: &mut VisualTestContext, preview: ImagePreview) {
    rig.updates
        .send(StoreUpdate::WorkspaceChanged(workspace(
            "w1", "p1", "feature", 1,
        )))
        .expect("the store listens");
    cx.run_until_parked();
    rig.shell.update(cx, |shell, cx| {
        shell.model.select_workspace(Some("w1".to_owned()));
        cx.notify();
    });
    // The first frame brings the source control in line with the selection, which drops any earlier open file.
    draw(cx);
    rig.shell.update(cx, |shell, cx| {
        shell.source_control.open = Some(OpenFile {
            path: "pic.png".to_owned(),
            target: DiffTarget::Worktree,
            original_path: None,
            kind: ChangeKind::Modified,
        });
        shell.source_control.images = Some(preview);
        cx.notify();
    });
    draw(cx);
}

#[gpui::test]
fn an_image_file_draws_its_before_and_after_side_by_side(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_image(
        &rig,
        cx,
        ImagePreview {
            mime: "image/png",
            before: Some(ImageSide::Loaded {
                bytes: PNG.to_vec(),
            }),
            after: Some(ImageSide::Loaded {
                bytes: PNG.to_vec(),
            }),
        },
    );
    let canvas = rig.shell.read_with(cx, |shell, _| shell.model.canvas());
    assert!(
        matches!(canvas, crate::model::canvas::Canvas::Workspace(_)),
        "the canvas shows the workspace, got {canvas:?}"
    );
    assert!(
        cx.debug_bounds("button:diff-back").is_some(),
        "the diff page is drawn"
    );
    assert!(
        cx.debug_bounds("image-preview-before").is_some(),
        "the before side is drawn"
    );
    assert!(
        cx.debug_bounds("image-preview-after").is_some(),
        "the after side is drawn"
    );
    assert!(
        cx.debug_bounds("image-preview-after-note").is_none(),
        "no sentence where an image draws"
    );
}

#[gpui::test]
fn a_side_over_the_cap_is_named_in_a_sentence_instead_of_an_image(cx: &mut TestAppContext) {
    let (rig, cx) = rig(cx, &[("p1", "alpha")]);
    open_image(
        &rig,
        cx,
        ImagePreview {
            mime: "image/png",
            before: Some(ImageSide::Loaded {
                bytes: PNG.to_vec(),
            }),
            after: Some(ImageSide::TooLarge { size: 5 << 20 }),
        },
    );
    assert!(
        cx.debug_bounds("image-preview-after-note").is_some(),
        "the size sentence shows where the image would be"
    );
    assert!(
        cx.debug_bounds("image-preview-before-note").is_none(),
        "the side that loaded has no sentence"
    );
}
