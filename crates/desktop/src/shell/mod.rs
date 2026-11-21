//! The desktop shell: the window frame, its regions, and the command palette (D-SH-01).

use crate::data::DataSource;
use crate::model::shell_state::ShellModel;
use gpui::{App, Context, IntoElement, Render, Window, div, prelude::*};
use massorca_ui::components::text_field;
use massorca_ui::tokens::TYPE_HEADING;
use massorca_ui::{ActiveTheme, TextStyled};

#[cfg(test)]
mod tests;

/// The root view of the window. Views read only its view model (decision 0023).
#[derive(Debug)]
pub struct Shell {
    model: ShellModel,
}

impl Shell {
    /// Fills the view model from the data source; the window width is read once here.
    pub fn new(data: &impl DataSource, window: &Window, _cx: &mut Context<Self>) -> Self {
        let width = f32::from(window.viewport_size().width);
        Self {
            model: ShellModel::new(data.snapshot(), width),
        }
    }

    pub fn model(&self) -> &ShellModel {
        &self.model
    }
}

/// Registers the key bindings of the shell and of the widgets it uses.
pub fn bind_keys(cx: &mut App) {
    text_field::bind_keys(cx);
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        self.model
            .set_window_width(f32::from(window.viewport_size().width));
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(theme.palette.surface.frame)
            .text_color(theme.palette.text.primary)
            .text_body()
            .child(
                div()
                    .text_style(TYPE_HEADING)
                    .child("What should we work on?"),
            )
    }
}
