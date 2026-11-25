//! The placement card above the start composer (D-AG-01): the host, project and branch pills in one raised card whose
//! bottom edge meets the composer. The pills are drawn here; each page attaches what its pill does.

use gpui::prelude::*;
use gpui::{Div, Stateful, div, px};
use massorca_ui::components::tooltip::{Tooltip, tooltip_view};
use massorca_ui::style::Surface;
use massorca_ui::tokens::{
    Color, RADIUS_LG, RADIUS_MD, SIZE_CONTROL_DEFAULT, SPACE_1, SPACE_2, SPACE_3, TYPE_LABEL,
};
use massorca_ui::{Icon, IconName, IconSize, TextStyled, Theme};

use crate::model::home_start::NEEDS_NEWER_SERVICE;

/// A pill's text colours. A disabled pill takes the disabled text in every part, as the ui crate's disabled controls do.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PillText {
    icon: Color,
    label: Color,
    secondary: Color,
    chevron: Color,
}

fn pill_text(enabled: bool, theme: &Theme) -> PillText {
    let text = &theme.palette.text;
    if enabled {
        PillText {
            icon: text.secondary,
            label: text.primary,
            secondary: text.secondary,
            chevron: text.tertiary,
        }
    } else {
        PillText {
            icon: text.disabled,
            label: text.disabled,
            secondary: text.disabled,
            chevron: text.disabled,
        }
    }
}

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
    let text = pill_text(enabled, theme);
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
        .child(Icon::new(icon).size(IconSize::Small).color(text.icon))
        .child(
            div()
                .text_style(TYPE_LABEL)
                .text_color(text.label)
                .child(label),
        );
    if enabled {
        row = row.hover(move |style| style.bg(hover));
    }
    if let Some(secondary) = secondary {
        row = row.child(
            div()
                .text_style(TYPE_LABEL)
                .text_color(text.secondary)
                .child(secondary),
        );
    }
    row.child(
        Icon::new(IconName::ChevronDown)
            .size(IconSize::XSmall)
            .color(text.chevron),
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

#[cfg(test)]
mod tests {
    use super::*;
    use massorca_ui::Appearance;

    #[test]
    fn a_disabled_pill_takes_the_disabled_text_in_every_part() {
        let theme = Theme::new(Appearance::Dark);
        let text = &theme.palette.text;
        assert_eq!(
            pill_text(false, &theme),
            PillText {
                icon: text.disabled,
                label: text.disabled,
                secondary: text.disabled,
                chevron: text.disabled,
            }
        );
        assert_eq!(pill_text(true, &theme).label, text.primary);
        assert_ne!(pill_text(true, &theme).label, text.disabled);
    }
}
