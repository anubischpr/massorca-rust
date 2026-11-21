//! The Title bar: Sidebar toggle, navigation, title, region toggles and window controls (D-SH-03).

use super::tooltip::tooltip;
use super::{Shell, change};
use crate::model::regions::Region;
use crate::model::shell_state::ShellModel;
use crate::model::title_bar::{RegionToggle, TitleBarModel, title_bar as title_bar_model};
use gpui::{
    Div, IntoElement, MouseButton, MouseDownEvent, WeakEntity, Window, WindowControlArea, div,
    prelude::*, px, svg,
};
use massorca_ui::components::button::{Button, ButtonSize, ButtonVariant};
use massorca_ui::components::icon_button::IconButton;
use massorca_ui::tokens::{
    DURATION_TOOLTIP_DELAY, SIZE_ICON_SM, SIZE_TITLEBAR_CONTROL_WIDTH, SIZE_TITLEBAR_MACOS,
    SIZE_TITLEBAR_WINDOWS, SPACE_1, SPACE_2, SPACE_HALF, TYPE_TITLE_SM,
};
use massorca_ui::{IconName, TextStyled, Theme};

/// The Title bar row, above the Sidebar and the Canvas column (D-SH-03).
pub(super) fn title_bar(
    model: &ShellModel,
    shell: &WeakEntity<Shell>,
    maximized: bool,
    theme: &Theme,
) -> Div {
    let bar = title_bar_model(model);
    let mut row = div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(SPACE_1))
        .h(px(bar_height()))
        .w_full()
        .pl(px(SPACE_1 + SPACE_HALF))
        .bg(theme.palette.surface.frame)
        .child(sidebar_toggle(&bar, shell.clone()))
        .child(history(&bar, shell))
        .child(title(&bar, theme))
        .children(
            bar.region_toggles
                .iter()
                .map(|toggle| region_toggle(*toggle, shell.clone())),
        );
    if !cfg!(target_os = "macos") {
        row = row.child(window_controls(maximized, theme));
    }
    row
}

fn bar_height() -> f32 {
    if cfg!(target_os = "macos") {
        SIZE_TITLEBAR_MACOS
    } else {
        SIZE_TITLEBAR_WINDOWS
    }
}

fn sidebar_toggle(bar: &TitleBarModel, shell: WeakEntity<Shell>) -> impl IntoElement {
    let button = IconButton::new("sidebar-toggle", IconName::PanelLeft, bar.sidebar_label)
        .size(ButtonSize::Default)
        .disabled(!bar.sidebar_enabled)
        .on_click(move |_, cx| change(&shell, cx, |shell| shell.model.toggle_sidebar()));
    with_tooltip(button, bar.sidebar_label)
}

/// Back and Forward; a full page shows one labelled Back instead (D-SH-03).
fn history(bar: &TitleBarModel, shell: &WeakEntity<Shell>) -> Div {
    let row = div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap(px(SPACE_1));
    if bar.full_page.is_some() {
        let back = shell.clone();
        let page_back = Button::new("back-page", "Back")
            .variant(ButtonVariant::Ghost)
            .size(ButtonSize::Default)
            .leading_icon(IconName::ArrowLeft)
            .on_click(move |_, cx| change(&back, cx, |shell| shell.model.go_back()));
        row.child(page_back)
    } else {
        let back = shell.clone();
        let forward = shell.clone();
        let back_button = IconButton::new("back", IconName::ArrowLeft, "Back")
            .size(ButtonSize::Default)
            .disabled(!bar.can_go_back)
            .on_click(move |_, cx| change(&back, cx, |shell| shell.model.go_back()));
        let forward_button = IconButton::new("forward", IconName::ArrowRight, "Forward")
            .size(ButtonSize::Default)
            .disabled(!bar.can_go_forward)
            .on_click(move |_, cx| change(&forward, cx, |shell| shell.model.go_forward()));
        row.child(with_tooltip(back_button, "Back"))
            .child(with_tooltip(forward_button, "Forward"))
    }
}

/// The session title, or the page title on a full page. The empty part of the bar drags the
/// window, and a double click zooms it.
fn title(bar: &TitleBarModel, theme: &Theme) -> impl IntoElement {
    let color = if bar.title_is_placeholder {
        theme.palette.text.tertiary
    } else {
        theme.palette.text.primary
    };
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .items_center()
        .px(px(SPACE_2))
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_down(
            MouseButton::Left,
            |event: &MouseDownEvent, window: &mut Window, _| {
                if event.click_count == 2 {
                    window.zoom_window();
                } else {
                    window.start_window_move();
                }
            },
        )
        .child(
            div()
                .id("title-text")
                .min_w_0()
                .truncate()
                .text_style(TYPE_TITLE_SM)
                .text_color(color)
                .child(bar.title.clone())
                .tooltip(tooltip(bar.title.clone())),
        )
}

fn region_toggle(toggle: RegionToggle, shell: WeakEntity<Shell>) -> impl IntoElement {
    let (id, icon) = toggle_icon(toggle.region);
    let region = toggle.region;
    let name = region.name();
    let button = IconButton::new(id, icon, name)
        .size(ButtonSize::Default)
        .toggle(toggle.on)
        .on_click(move |_, cx| {
            change(&shell, cx, move |shell| shell.model.toggle_region(region));
        });
    with_tooltip(button, name)
}

fn toggle_icon(region: Region) -> (&'static str, IconName) {
    match region {
        Region::SessionSummary => ("toggle-session-summary", IconName::ListChecks),
        Region::WorkPanel => ("toggle-work-panel", IconName::PanelBottom),
        Region::ProjectSidebar => ("toggle-project-sidebar", IconName::PanelRight),
        Region::Sidebar => ("toggle-sidebar", IconName::PanelLeft),
    }
}

/// Minimise, maximise and close, drawn by the app with the native hit areas (D-SH-03).
fn window_controls(maximized: bool, theme: &Theme) -> impl IntoElement {
    let (maximise_label, maximise_icon) = if maximized {
        ("Restore", IconName::Copy)
    } else {
        ("Maximise", IconName::Square)
    };
    div()
        .flex()
        .flex_row()
        .flex_none()
        .h(px(bar_height()))
        .child(window_control(
            "window-minimise",
            "Minimise",
            IconName::Minus,
            WindowControlArea::Min,
            false,
            theme,
            |window| window.minimize_window(),
        ))
        .child(window_control(
            "window-maximise",
            maximise_label,
            maximise_icon,
            WindowControlArea::Max,
            false,
            theme,
            |window| window.zoom_window(),
        ))
        .child(window_control(
            "window-close",
            "Close",
            IconName::X,
            WindowControlArea::Close,
            true,
            theme,
            |window| window.remove_window(),
        ))
}

/// One 46 by 36 window control. Close turns danger on hover; the others take the frame hover fill.
fn window_control(
    id: &'static str,
    label: &'static str,
    icon: IconName,
    area: WindowControlArea,
    danger: bool,
    theme: &Theme,
    activate: impl Fn(&mut Window) + 'static,
) -> impl IntoElement {
    let (hover_fill, pressed_fill, hover_icon) = if danger {
        (
            theme.palette.status.danger_solid,
            theme.palette.status.danger_pressed,
            theme.palette.status.danger_on,
        )
    } else {
        (
            theme.palette.fill.hover_frame,
            theme.palette.fill.pressed_frame,
            theme.palette.text.primary,
        )
    };
    let control = div()
        .id(id)
        .group(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(px(SIZE_TITLEBAR_CONTROL_WIDTH))
        .h_full()
        .window_control_area(area)
        .hover(move |style| style.bg(hover_fill))
        .active(move |style| style.bg(pressed_fill))
        .on_click(move |_, window, _| activate(window))
        .child(
            svg()
                .path(icon.path())
                .size(px(SIZE_ICON_SM))
                .flex_none()
                .text_color(theme.palette.text.secondary)
                .group_hover(id, move |style| style.text_color(hover_icon)),
        );
    with_tooltip(control, label)
}

/// Wraps a control in a hover area that shows its tooltip after the standard delay.
fn with_tooltip(control: impl IntoElement, label: &'static str) -> impl IntoElement {
    div()
        .id(label)
        .flex_none()
        .tooltip_show_delay(DURATION_TOOLTIP_DELAY)
        .tooltip(tooltip(label))
        .child(control)
}
