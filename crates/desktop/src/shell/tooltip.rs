//! One-line tooltips for the shell's controls, drawn on the overlay surface (Tooltip spec).

use gpui::{AnyView, App, Context, IntoElement, Render, SharedString, Window, div, prelude::*, px};
use massorca_ui::theme::box_shadows;
use massorca_ui::tokens::{LAYOUT_TOOLTIP_MAX_WIDTH, RADIUS_LG, SPACE_1, SPACE_2, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, TextStyled};

/// The view a tooltip shows: its text on the overlay surface.
#[derive(Debug)]
pub(super) struct ShellTooltip {
    text: SharedString,
}

impl Render for ShellTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        div()
            .max_w(px(LAYOUT_TOOLTIP_MAX_WIDTH))
            .px(px(SPACE_2))
            .py(px(SPACE_1))
            .rounded(px(RADIUS_LG))
            .bg(theme.palette.surface.overlay)
            .border_1()
            .border_color(theme.palette.border.default)
            .shadow(box_shadows(theme.shadows.e2))
            .text_style(TYPE_BODY_SM)
            .text_color(theme.palette.text.primary)
            .child(self.text.clone())
    }
}

/// The tooltip builder for `Div::tooltip`, showing `text`.
pub(super) fn tooltip(
    text: impl Into<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| ShellTooltip { text: text.clone() }).into()
}
