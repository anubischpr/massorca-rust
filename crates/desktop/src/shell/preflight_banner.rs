//! The startup preflight notice above the Canvas: a tool the service reports missing or too old, with what to do about
//! it (PLAT-08). It never blocks the Canvas, and its dismiss hides it for this connection.

use super::{Shell, change};
use crate::model::preflight::PreflightNotice;
use gpui::{IntoElement, WeakEntity, div, prelude::*, px};
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::tokens::{SPACE_2, SPACE_3};

pub(super) fn preflight_banner(
    notice: &PreflightNotice,
    shell: &WeakEntity<Shell>,
) -> impl IntoElement {
    let dismiss = shell.clone();
    let banner = Banner::new(
        "preflight-banner",
        BannerVariant::Warning,
        notice.message.clone(),
    )
    .title(notice.title.clone())
    .on_dismiss(move |_, cx| change(&dismiss, cx, |shell| shell.model.dismiss_preflight()));
    div()
        .debug_selector(|| "preflight-banner".to_owned())
        .flex_none()
        .px(px(SPACE_3))
        .pt(px(SPACE_2))
        .child(banner)
}
