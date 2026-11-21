//! The Status bar under the Canvas column: agent status, memory, terminals, ports and hosts
//! (D-SH-05). Values come from the status view model; the glyphs are the shell's own.

use super::tooltip::tooltip;
use crate::model::shell::{ServiceStatus, ServiceTone, StatusBarModel};
use gpui::{AnyElement, Div, IntoElement, SharedString, div, prelude::*, px};
use massorca_ui::components::agent_status::{
    AgentState as GlyphState, AgentStatus, AgentStatusSize,
};
use massorca_ui::style::Surface;
use massorca_ui::tokens::{
    Color, DURATION_TOOLTIP_DELAY, RADIUS_FULL, RADIUS_SM, SIZE_INDICATOR_DOT, SIZE_STATUSBAR,
    SPACE_1, SPACE_3, TYPE_CAPTION,
};
use massorca_ui::{Icon, IconName, IconSize, TextStyled, Theme};

/// The Status bar: 24 px on the Canvas surface with caption text. Items share one hover and
/// press treatment and show their full value in a tooltip (StatusItem spec).
pub(super) fn status_bar(
    status: &StatusBarModel,
    service: Option<&ServiceStatus>,
    theme: &Theme,
) -> Div {
    let secondary = theme.palette.text.secondary;
    div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(SPACE_3))
        .w_full()
        .h(px(SIZE_STATUSBAR))
        .px(px(SPACE_3))
        .bg(theme.palette.surface.canvas)
        .text_style(TYPE_CAPTION)
        .child(item(
            "status-agents",
            Some(agents_glyph(status)),
            status.agents_text(),
            status.agents_tooltip(),
            secondary,
            theme,
        ))
        .child(item(
            "status-memory",
            None,
            format!("Memory {}", status.memory),
            format!("Memory {}", status.memory),
            secondary,
            theme,
        ))
        .child(item(
            "status-terminals",
            Some(glyph(IconName::SquareTerminal, theme)),
            status.terminals_text(),
            status.terminals_text(),
            secondary,
            theme,
        ))
        .child(item(
            "status-ports",
            Some(glyph(IconName::Plug, theme)),
            status.ports_text(),
            status.ports_text(),
            secondary,
            theme,
        ))
        .child(div().flex_1())
        .when_some(service, |bar, service| {
            bar.child(service_item(service, theme))
        })
        .child(hosts_item(status, theme))
}

/// The service item: a dot in the colour of the link's state and a short word. The tooltip gives
/// the server, the retry time or the reason.
fn service_item(service: &ServiceStatus, theme: &Theme) -> impl IntoElement {
    let dot = div()
        .flex_none()
        .size(px(SIZE_INDICATOR_DOT))
        .rounded(px(RADIUS_FULL));
    let status = &theme.palette.status;
    let dot = match service.tone {
        ServiceTone::Ok => dot.bg(status.success_solid),
        ServiceTone::Busy => dot.border_1().border_color(theme.palette.border.strong),
        ServiceTone::Warning => dot.bg(status.warning_solid),
        ServiceTone::Unreachable => dot.bg(status.danger_solid),
    };
    item(
        "status-service",
        Some(dot.into_any_element()),
        service.text.clone(),
        service.tooltip.clone(),
        theme.palette.text.secondary,
        theme,
    )
}

/// One status item: an optional glyph and a caption. Its hover fill sits on the Canvas.
fn item(
    key: &'static str,
    glyph: Option<AnyElement>,
    text: impl Into<SharedString>,
    tip: impl Into<SharedString>,
    color: Color,
    theme: &Theme,
) -> impl IntoElement {
    let fills = Surface::Canvas.fills(theme);
    let primary = theme.palette.text.primary;
    let mut row = div()
        .id(key)
        .debug_selector(move || key.to_owned())
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(SPACE_1))
        .h(px(SIZE_STATUSBAR))
        .px(px(SPACE_1))
        .rounded(px(RADIUS_SM))
        .text_color(color)
        .hover(move |style| style.bg(fills.hover).text_color(primary))
        .active(move |style| style.bg(fills.pressed))
        .tooltip_show_delay(DURATION_TOOLTIP_DELAY)
        .tooltip(tooltip(tip));
    if let Some(glyph) = glyph {
        row = row.child(glyph);
    }
    let text: SharedString = text.into();
    row.child(div().min_w_0().truncate().child(text))
}

/// The hosts item: a dot that is solid when every host is reached and a ring when one is not,
/// and the count dimmed when a host is unreachable (D-SH-05).
fn hosts_item(status: &StatusBarModel, theme: &Theme) -> impl IntoElement {
    let unreachable = status.hosts_unreachable > 0;
    let dot = div()
        .flex_none()
        .size(px(SIZE_INDICATOR_DOT))
        .rounded(px(RADIUS_FULL));
    let dot = if unreachable {
        dot.border_1().border_color(theme.palette.border.strong)
    } else {
        dot.bg(theme.palette.status.success_solid)
    };
    let color = if unreachable {
        theme.palette.text.tertiary
    } else {
        theme.palette.text.secondary
    };
    item(
        "status-hosts",
        Some(dot.into_any_element()),
        status.hosts_text(),
        status.hosts_tooltip(),
        color,
        theme,
    )
}

fn glyph(name: IconName, theme: &Theme) -> AnyElement {
    Icon::new(name)
        .size(IconSize::XSmall)
        .color(theme.palette.text.secondary)
        .into_any_element()
}

/// The agents glyph shows the most urgent state: waiting for the user, then running, then idle.
fn agents_glyph(status: &StatusBarModel) -> AnyElement {
    let state = if status.waiting > 0 {
        GlyphState::Waiting
    } else if status.running > 0 {
        GlyphState::Running
    } else {
        GlyphState::Idle
    };
    AgentStatus::new(state)
        .size(AgentStatusSize::XSmall)
        .into_any_element()
}
