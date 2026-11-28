//! The diff of a file opened from Source control, drawn in the Canvas in place of the workspace's start
//! page (D-DF-01). The Work panel is not on main yet, so the file takes the Canvas (A7).

use std::rc::Rc;

use gpui::{AnyElement, App, IntoElement, WeakEntity, div, prelude::*, px};
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::components::diff_view::{DiffAction, DiffModel, DiffStatus, DiffView, FileDiff};
use massorca_ui::tokens::{SPACE_2, SPACE_3, TYPE_BODY_STRONG};
use massorca_ui::{ActiveTheme, IconName, TextStyled};

use super::Shell;
use super::diff_image_preview::image_preview_panel;
use super::source_control::{DiffLoad, OpenFile};
use crate::model::source_control_diff::file_status;

/// The open file's diff, with a way back to the workspace's start page.
pub(super) fn diff_page(shell: &Shell, handle: &WeakEntity<Shell>, cx: &App) -> AnyElement {
    let theme = *cx.theme();
    let Some(open) = shell.source_control.open.clone() else {
        return div().into_any_element();
    };
    let connected = shell.store.read(cx).state().connection.is_connected();
    let back = {
        let handle = handle.clone();
        Button::new("diff-back", "Back")
            .variant(ButtonVariant::Ghost)
            .leading_icon(IconName::ArrowLeft)
            .on_click(move |_, cx| {
                let _ = handle.update(cx, |shell, cx| shell.close_file(cx));
            })
    };
    let header = div()
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .px(px(SPACE_3))
        .py(px(SPACE_2))
        .child(back)
        .child(
            div()
                .min_w_0()
                .truncate()
                .text_style(TYPE_BODY_STRONG)
                .text_color(theme.palette.text.primary)
                .child(open.path.clone()),
        );
    let (model, status) = match &shell.source_control.diff {
        Some(DiffLoad::Ready { model, status, .. }) => (Rc::clone(model), status.clone()),
        Some(DiffLoad::Failed(message)) => (
            Rc::new(empty_model(&open)),
            DiffStatus::Failed(message.clone().into()),
        ),
        Some(DiffLoad::Loading) | None => (Rc::new(empty_model(&open)), DiffStatus::Loading),
    };
    let retry = {
        let handle = handle.clone();
        let file = open.clone();
        move |action: DiffAction, _: &mut gpui::Window, cx: &mut App| {
            if action == DiffAction::Retry {
                let file = file.clone();
                let _ = handle.update(cx, |shell, cx| shell.open_file(file, cx));
            }
        }
    };
    // A binary change's note gives way to its image previews, which say what the file holds (GIT-06).
    let binary = matches!(status, DiffStatus::Binary { .. });
    let view = DiffView::new("source-control-diff", model)
        .status(status)
        .disconnected(!connected)
        .on_action(retry);
    let mut page = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .h_full()
        .child(header);
    if matches!(
        shell.source_control.diff,
        Some(DiffLoad::Ready {
            truncated: true,
            ..
        })
    ) {
        page = page.child(Banner::new(
            "diff-truncated",
            BannerVariant::Warning,
            "This diff is cut at the size limit.",
        ));
    }
    let images = shell.source_control.images.as_ref();
    if let Some(preview) = images {
        page = page.child(image_preview_panel(preview, cx));
    }
    if binary && images.is_some() {
        return page.into_any_element();
    }
    page.child(view).into_any_element()
}

/// The diff of a file that has no lines to show yet, so the view has a model while it waits.
fn empty_model(open: &OpenFile) -> DiffModel {
    DiffModel::new(FileDiff {
        path: open.path.clone().into(),
        status: file_status(open.kind),
        hunks: Vec::new(),
        hidden_after: Vec::new(),
    })
}
