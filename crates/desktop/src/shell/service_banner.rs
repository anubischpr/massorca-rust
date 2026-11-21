//! The notice above the Canvas while the service is not connected (X-CN-01): connecting,
//! reconnecting with its attempt, or unreachable, each with "Retry now" where it helps.

use super::Shell;
use crate::model::shell::{ServiceNotice, ServiceTone};
use gpui::{IntoElement, WeakEntity, div, prelude::*, px};
use massorca_ui::components::banner::{Banner, BannerVariant};
use massorca_ui::components::button::{Button, ButtonVariant};
use massorca_ui::tokens::{SPACE_2, SPACE_3};

pub(super) fn service_banner(
    notice: &ServiceNotice,
    shell: &WeakEntity<Shell>,
) -> impl IntoElement {
    let variant = match notice.tone {
        ServiceTone::Ok => BannerVariant::Success,
        ServiceTone::Busy => BannerVariant::Info,
        ServiceTone::Warning => BannerVariant::Warning,
        ServiceTone::Unreachable => BannerVariant::Unreachable,
    };
    let mut banner = Banner::new("service-banner", variant, notice.message.clone()).loading(
        matches!(notice.tone, ServiceTone::Busy | ServiceTone::Warning),
    );
    if let Some(detail) = &notice.detail {
        banner = banner.detail(detail.clone());
    }
    if notice.retry {
        let shell = shell.clone();
        banner = banner.action(
            Button::new("service-retry-now", "Retry now")
                .variant(ButtonVariant::Secondary)
                .on_click(move |_, cx| {
                    let _ = shell.update(cx, |shell, _| shell.retry_service_now());
                }),
        );
    }
    div()
        .debug_selector(|| "service-banner".to_owned())
        .flex_none()
        .px(px(SPACE_3))
        .pt(px(SPACE_2))
        .child(banner)
}
