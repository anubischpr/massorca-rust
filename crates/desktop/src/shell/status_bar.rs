//! The Status bar under the Canvas column: agent status, memory, terminals, ports and hosts
//! (D-SH-05). Values come from the status view model; the glyphs are the shell's own.

use super::Shell;
use super::tooltip::tooltip;
use crate::model::shell::{ServiceStatus, ServiceTone, StatusBarModel};
use crate::model::status_items::{StatusItem, StatusItemVisibility};
use gpui::{
    AnyElement, Div, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, Role, SharedString,
    Stateful, WeakEntity, div, point, prelude::*, px,
};
use massorca_ui::components::agent_status::{
    AgentState as GlyphState, AgentStatus, AgentStatusSize,
};
use massorca_ui::style::Surface;
use massorca_ui::tokens::{
    Color, DURATION_TOOLTIP_DELAY, RADIUS_FULL, RADIUS_SM, SIZE_INDICATOR_DOT, SIZE_STATUSBAR,
    SPACE_1, SPACE_3, TYPE_CAPTION,
};
use massorca_ui::{Icon, IconName, IconSize, TextStyled, Theme};

/// The Status bar: 24 px on the Canvas surface with caption text. Agent status sits at the left;
/// memory, terminals, ports and hosts at the right (StatusBar spec). Items share one hover and
/// press treatment and show their full value in a tooltip (StatusItem spec). Right click, or the
/// menu key, opens the visibility menu; the items it hides are left out (StatusBar spec).
pub(super) fn status_bar(
    status: &StatusBarModel,
    service: Option<&ServiceStatus>,
    visible: &StatusItemVisibility,
    shell: &WeakEntity<Shell>,
    theme: &Theme,
) -> Stateful<Div> {
    let secondary = theme.palette.text.secondary;
    let open_by_mouse = shell.clone();
    let open_by_key = shell.clone();
    let mut bar = div()
        .id("status-bar")
        .role(Role::Status)
        .aria_label("Status bar")
        .on_mouse_down(
            MouseButton::Right,
            move |event: &MouseDownEvent, window, cx| {
                let _ = open_by_mouse.update(cx, |shell, cx| {
                    shell.open_status_menu(event.position, window, cx);
                });
            },
        )
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            let key = event.keystroke.key.as_str();
            if key == "menu" || (key == "f10" && event.keystroke.modifiers.shift) {
                // The keyboard has no pointer to anchor the menu to: it opens at the bar's left.
                let at = point(
                    px(SPACE_3),
                    window.viewport_size().height - px(SIZE_STATUSBAR),
                );
                let _ = open_by_key.update(cx, |shell, cx| {
                    shell.open_status_menu(at, window, cx);
                });
                cx.stop_propagation();
            }
        })
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(SPACE_3))
        .w_full()
        .h(px(SIZE_STATUSBAR))
        .px(px(SPACE_3))
        .bg(theme.palette.surface.canvas)
        .text_style(TYPE_CAPTION);
    if visible.is_shown(StatusItem::Agents) {
        bar = bar.child(item(
            "status-agents",
            Some(agents_glyph(status)),
            status.agents_text(),
            status.agents_tooltip(),
            secondary,
            theme,
        ));
    }
    bar = bar.child(div().flex_1());
    if visible.is_shown(StatusItem::Memory) {
        bar = bar.child(item(
            "status-memory",
            None,
            status.memory.clone(),
            format!("Memory {}", status.memory),
            secondary,
            theme,
        ));
    }
    if visible.is_shown(StatusItem::Terminals) {
        bar = bar.child(item(
            "status-terminals",
            Some(glyph(IconName::SquareTerminal, theme)),
            status.terminals_text(),
            status.terminals_text(),
            secondary,
            theme,
        ));
    }
    if visible.is_shown(StatusItem::Ports) {
        bar = bar.child(item(
            "status-ports",
            Some(glyph(IconName::Plug, theme)),
            status.ports_text(),
            status.ports_text(),
            secondary,
            theme,
        ));
    }
    if visible.is_shown(StatusItem::Hosts) {
        bar = bar.child(hosts_item(status, service, theme));
    }
    bar
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

/// The hosts item. While the service link is not up it names that state instead of a host count
/// (D-SH-05, "Service not running"). Its dot is solid when every host is reached and a ring when
/// one is not.
fn hosts_item(
    status: &StatusBarModel,
    service: Option<&ServiceStatus>,
    theme: &Theme,
) -> AnyElement {
    let link = service.filter(|link| link.tone != ServiceTone::Ok);
    let unreachable = link.is_some() || status.hosts_unreachable > 0;
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
    let (text, tip) = match link {
        Some(link) => (link.text.clone(), link.tooltip.clone()),
        None => (status.hosts_text(), status.hosts_tooltip()),
    };
    item(
        "status-hosts",
        Some(dot.into_any_element()),
        text,
        tip,
        color,
        theme,
    )
    .into_any_element()
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
