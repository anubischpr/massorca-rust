//! The placement card above the start composer (D-AG-01): the host, project and branch pills in one raised card whose
//! bottom edge meets the composer. The pills are drawn here; each page attaches what its pill does.

use gpui::prelude::*;
use gpui::{App, Div, FocusHandle, Role, Stateful, Window, div, px};
use massorca_ui::components::tooltip::TooltipTrigger;
use massorca_ui::style::{FocusRing, StateFill, Surface};
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

/// The focus handle of pill `id`, kept across frames as a Tab stop, as the ui crate's buttons keep theirs.
pub(super) fn pill_focus(id: &'static str, window: &mut Window, cx: &mut App) -> FocusHandle {
    window
        .use_keyed_state(id, cx, |_, cx| cx.focus_handle())
        .read(cx)
        .clone()
        .tab_stop(true)
}

/// A placement pill: its icon, its label, an optional second label, and a chevron. It is a Tab stop with a focus ring,
/// and Enter or Space clicks it. An enabled pill takes the canvas hover and pressed fills (D-AG-01); a pill that is not
/// `enabled` takes neither, and its text is disabled. The page attaches a click only where the pill acts.
pub(super) fn pill(
    id: &'static str,
    icon: IconName,
    label: String,
    secondary: Option<String>,
    enabled: bool,
    theme: &Theme,
    focus: FocusHandle,
) -> Stateful<Div> {
    let leading = Icon::new(icon)
        .size(IconSize::Small)
        .color(pill_text(enabled, theme).icon)
        .into_any_element();
    pill_with(id, leading, label, secondary, enabled, theme, focus)
}

/// A pill whose leading element is given, such as a project's own mark.
pub(super) fn pill_with(
    id: &'static str,
    leading: gpui::AnyElement,
    label: String,
    secondary: Option<String>,
    enabled: bool,
    theme: &Theme,
    focus: FocusHandle,
) -> Stateful<Div> {
    let text = pill_text(enabled, theme);
    let name = match &secondary {
        Some(secondary) => format!("{label} {secondary}"),
        None => label.clone(),
    };
    let mut row = div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .when(enabled, |row| row.role(Role::Button))
        .aria_label(name)
        .track_focus(&focus)
        .focus_ring(theme)
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(SPACE_1))
        .h(px(SIZE_CONTROL_DEFAULT))
        .px(px(SPACE_2))
        .rounded(px(RADIUS_MD))
        .whitespace_nowrap()
        .child(leading)
        .child(
            div()
                .text_style(TYPE_LABEL)
                .text_color(text.label)
                .child(label),
        );
    if enabled {
        row = row.state_fills(theme, Surface::Canvas);
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

/// A pill that is off: its reason shows on hover and on keyboard focus, as the ui crate's disabled buttons show theirs.
pub(super) fn off_pill(
    id: &'static str,
    icon: IconName,
    label: String,
    secondary: Option<String>,
    reason: &'static str,
    theme: &Theme,
    focus: FocusHandle,
) -> impl IntoElement {
    TooltipTrigger::new(
        format!("{id}-reason"),
        reason,
        pill(id, icon, label, secondary, false, theme, focus.clone()),
    )
    .focus_handle(focus)
}

/// The host pill: the host the workspace would run on, off with its reason until the service takes a host for a
/// workspace.
pub(super) fn host_pill(
    id: &'static str,
    host: &str,
    theme: &Theme,
    focus: FocusHandle,
) -> impl IntoElement {
    off_pill(
        id,
        IconName::Server,
        host.to_owned(),
        None,
        NEEDS_NEWER_SERVICE,
        theme,
        focus,
    )
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

    #[test]
    fn a_pill_hovers_and_presses_on_the_canvas_state_fills() {
        let theme = Theme::new(Appearance::Dark);
        let fills = Surface::Canvas.fills(&theme);
        assert_eq!(fills.hover, theme.palette.fill.hover_canvas);
        assert_eq!(fills.pressed, theme.palette.fill.pressed_canvas);
        assert_ne!(fills.pressed, fills.hover);
        assert_ne!(fills.hover, theme.palette.fill.hover_raised);
    }
}
