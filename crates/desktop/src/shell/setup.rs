//! The setup checklist (D-OB-12): a full page with the steps the core can show, a progress bar and
//! the count of steps done. Opened from Settings, General and account; setup is optional.

use gpui::prelude::*;
use gpui::{AnyElement, App, div, px};
use massorca_ui::components::list_row::{ListRow, RowLeading};
use massorca_ui::components::progress::ProgressBar;
use massorca_ui::tokens::{LAYOUT_PAGE_SETTINGS, SPACE_2, SPACE_3, SPACE_6, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, IconName, TextStyled, Theme};

use crate::model::setup::{SetupRow, Step, done_count};

/// The checklist page: the progress, one row per step, and where to find it again.
pub(super) fn setup_page(rows: &[SetupRow], cx: &mut App) -> AnyElement {
    let theme = *cx.theme();
    let p = &theme.palette;
    let done = done_count(rows);
    let total = rows.len();
    let fraction = done as f32 / total.max(1) as f32;

    let mut list = div().flex().flex_col().gap(px(SPACE_2));
    for (index, row) in rows.iter().enumerate() {
        list = list.child(step_row(index, row, &theme));
    }

    div()
        .id("setup-page")
        .size_full()
        .overflow_y_scroll()
        .bg(p.surface.canvas)
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .w_full()
                .max_w(px(LAYOUT_PAGE_SETTINGS))
                .p(px(SPACE_6))
                .flex()
                .flex_col()
                .gap(px(SPACE_6))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(SPACE_3))
                        .child(ProgressBar::new("Setup progress").determinate(fraction))
                        .child(
                            div()
                                .text_body()
                                .text_color(p.text.secondary)
                                .child(format!("{done} of {total} done")),
                        ),
                )
                .child(list)
                .child(
                    div()
                        .text_style(TYPE_BODY_SM)
                        .text_color(p.text.tertiary)
                        .child(
                            "Setup is optional. Find it again in Settings, General and account.",
                        ),
                ),
        )
        .into_any_element()
}

/// One step: its glyph and word for the state, the step's name and what it says.
fn step_row(index: usize, row: &SetupRow, theme: &Theme) -> impl IntoElement {
    let (glyph, word) = match row.step {
        Step::Done => (IconName::CircleCheck, "Done"),
        Step::NotStarted => (IconName::Circle, "Not started"),
        Step::Warning => (IconName::TriangleAlert, "Needs attention"),
        Step::NotAvailable => (IconName::CircleDashed, "Not available yet"),
    };
    let color = match row.step {
        Step::Done => theme.palette.status.success_fg,
        Step::Warning => theme.palette.status.warning_fg,
        Step::NotStarted | Step::NotAvailable => theme.palette.text.tertiary,
    };
    ListRow::new(format!("setup-step-{index}"), row.title)
        .secondary(row.detail.clone())
        .leading(RowLeading::Icon(glyph))
        .trailing(div().text_style(TYPE_BODY_SM).text_color(color).child(word))
        .lines(massorca_ui::components::list_row::RowLines::Two)
}
