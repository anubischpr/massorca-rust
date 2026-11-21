//! The Canvas: the Conversation, the regions beside it and the overlay region (D-SH-01, A5).

use super::Shell;
use crate::model::canvas::Canvas;
use crate::model::navigation::Mode;
use crate::model::regions::Region;
use crate::model::shell_state::ShellModel;
use gpui::{Context, Div, IntoElement, MouseButton, div, img, prelude::*, px};
use massorca_ui::theme::box_shadows;
use massorca_ui::tokens::{
    LAYOUT_CONVERSATION_MIN, LAYOUT_CONVERSATION_READING, RADIUS_CANVAS, RADIUS_LG, SIZE_TOOLBAR,
    SPACE_1, SPACE_3, TYPE_BODY_SM, TYPE_HEADING, TYPE_TITLE_SM,
};
use massorca_ui::{ActiveTheme, TextStyled, Theme};

/// The Canvas: the Conversation and the inline regions, with an open overlay on top (D-SH-01).
pub(super) fn canvas(model: &ShellModel, cx: &mut Context<Shell>) -> Div {
    let theme = *cx.theme();
    let overlay = model.overlay().filter(|region| *region != Region::Sidebar);
    let surface = div()
        .debug_selector(|| "canvas".to_owned())
        .relative()
        .flex()
        .flex_row()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .bg(theme.palette.surface.canvas)
        .rounded_tl(px(RADIUS_CANVAS))
        .overflow_hidden()
        .child(conversation(&model.canvas(), &theme))
        .children(
            model
                .inline_regions()
                .into_iter()
                .map(|region| region_column(region, &theme)),
        );
    surface.children(overlay.map(|region| overlay_panel(region, &theme)))
}

fn conversation(canvas: &Canvas, theme: &Theme) -> impl IntoElement {
    let body = match canvas {
        Canvas::Home(Mode::Workspaces) => start_page(theme).into_any_element(),
        Canvas::Home(Mode::BotTeams) => {
            placeholder("Bot Teams", "The team list is not built yet.", theme).into_any_element()
        }
        Canvas::Session(title) => {
            placeholder(title, "The conversation is not built yet.", theme).into_any_element()
        }
        Canvas::Page(page) => {
            placeholder(page.title(), "This page is not built yet.", theme).into_any_element()
        }
    };
    div()
        .flex_1()
        .min_w(px(LAYOUT_CONVERSATION_MIN))
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .child(body)
}

/// The start page with no session open: the rose logo and the question (A40).
fn start_page(theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(SPACE_3))
        .w_full()
        .max_w(px(LAYOUT_CONVERSATION_READING))
        .child(img("brand/massorca-logo.svg").size(px(48.)).flex_none())
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child("What should we work on?"),
        )
}

/// A centred title and a note for a Canvas content that is not built yet.
fn placeholder(title: &str, note: &str, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(SPACE_1))
        .child(
            div()
                .text_style(TYPE_HEADING)
                .text_color(theme.palette.text.primary)
                .child(title.to_owned()),
        )
        .child(
            div()
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.tertiary)
                .child(note.to_owned()),
        )
}

/// A region docked beside the Conversation, at its default width (D-SH-01).
fn region_column(region: Region, theme: &Theme) -> impl IntoElement {
    div()
        .flex_none()
        .w(px(region.default_width()))
        .min_w(px(region.min_width()))
        .h_full()
        .flex()
        .flex_col()
        .border_l_1()
        .border_color(theme.palette.border.subtle)
        .child(region_header(region, theme))
}

/// A region open over the Canvas when the window is too narrow to fit it inline (X1).
fn overlay_panel(region: Region, theme: &Theme) -> impl IntoElement {
    div()
        .absolute()
        .top_0()
        .bottom_0()
        .right_0()
        .w(px(region.default_width()))
        .flex()
        .flex_col()
        .bg(theme.palette.surface.overlay)
        .border_1()
        .border_color(theme.palette.border.default)
        .rounded_l(px(RADIUS_LG))
        .shadow(box_shadows(theme.shadows.e2))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(region_header(region, theme))
}

fn region_header(region: Region, theme: &Theme) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .h(px(SIZE_TOOLBAR))
        .px(px(SPACE_3))
        .text_style(TYPE_TITLE_SM)
        .text_color(theme.palette.text.primary)
        .truncate()
        .child(region.name())
}
