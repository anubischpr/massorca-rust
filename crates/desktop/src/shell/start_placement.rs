//! The placement card above the start composer (D-AG-01): the host, project and branch pills in one raised card whose
//! bottom edge meets the composer. The pills are drawn here; each page attaches what its pill does.

use gpui::prelude::*;
use gpui::{Div, Stateful, div, px};
use massorca_ui::components::tooltip::{Tooltip, tooltip_view};
use massorca_ui::style::Surface;
use massorca_ui::tokens::{
    RADIUS_LG, RADIUS_MD, SIZE_CONTROL_DEFAULT, SPACE_1, SPACE_2, SPACE_3, TYPE_LABEL,
};
use massorca_ui::{Icon, IconName, IconSize, TextStyled, Theme};

use crate::model::home_start::NEEDS_NEWER_SERVICE;

/// A placement pill: its icon, its label, an optional second label, and a chevron. A pill that is not `enabled` takes
/// no hover fill; the page attaches a click only where the pill acts.
pub(super) fn pill(
    id: &'static str,
    icon: IconName,
    label: String,
    secondary: Option<String>,
    enabled: bool,
    theme: &Theme,
) -> Stateful<Div> {
    let hover = Surface::Raised.fills(theme).hover;
    let mut row = div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(SPACE_1))
        .h(px(SIZE_CONTROL_DEFAULT))
        .px(px(SPACE_2))
        .rounded(px(RADIUS_MD))
        .whitespace_nowrap()
        .child(
            Icon::new(icon)
                .size(IconSize::Small)
                .color(theme.palette.text.secondary),
        )
        .child(
            div()
                .text_style(TYPE_LABEL)
                .text_color(theme.palette.text.primary)
                .child(label),
        );
    if enabled {
        row = row.hover(move |style| style.bg(hover));
    }
    if let Some(secondary) = secondary {
        row = row.child(
            div()
                .text_style(TYPE_LABEL)
                .text_color(theme.palette.text.secondary)
                .child(secondary),
        );
    }
    row.child(
        Icon::new(IconName::ChevronDown)
            .size(IconSize::XSmall)
            .color(theme.palette.text.tertiary),
    )
}

/// The host pill: the host the workspace would run on, off with its reason until the service takes a host for a
/// workspace.
pub(super) fn host_pill(id: &'static str, host: &str, theme: &Theme) -> Stateful<Div> {
    pill(id, IconName::Server, host.to_owned(), None, false, theme)
        .tooltip(tooltip_view(Tooltip::new(NEEDS_NEWER_SERVICE.to_owned())))
}

/// The card the three pills sit in: raised, bordered on its top, sides and top corners but not its bottom, so it
/// reads as one piece with the composer under it (D-AG-01).
pub(super) fn placement_card(
    host: impl IntoElement,
    project: impl IntoElement,
    branch: impl IntoElement,
    theme: &Theme,
) -> impl IntoElement {
    div()
        .debug_selector(|| "start-placement".to_owned())
        .flex()
        .flex_row()
        .items_center()
        .gap(px(SPACE_1))
        .mx(px(SPACE_3))
        .px(px(SPACE_1))
        .py(px(SPACE_1))
        .rounded_t(px(RADIUS_LG))
        .border_1()
        .border_b_0()
        .border_color(theme.palette.border.default)
        .bg(theme.palette.surface.raised)
        .child(host)
        .child(project)
        .child(branch)
}
