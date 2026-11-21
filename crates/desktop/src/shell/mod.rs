//! The desktop shell: the window frame, its regions, and the command palette (D-SH-01).

mod actions;
mod canvas;
mod title_bar;
mod tooltip;

use crate::data::DataSource;
use crate::model::shell_state::ShellModel;
use actions::CloseOverlay;
use gpui::{App, Context, FocusHandle, IntoElement, Render, WeakEntity, Window, div, prelude::*};
use massorca_ui::components::text_field;
use massorca_ui::{ActiveTheme, TextStyled};

#[cfg(test)]
mod tests;

/// The root view of the window. Views read only its view model (decision 0023).
#[derive(Debug)]
pub struct Shell {
    model: ShellModel,
    /// Holds keyboard focus when no control does, so shell keys reach the root.
    focus: FocusHandle,
}

impl Shell {
    /// Fills the view model from the data source; the window width is read once here.
    pub fn new(data: &impl DataSource, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let width = f32::from(window.viewport_size().width);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            model: ShellModel::new(data.snapshot(), width),
            focus,
        }
    }

    pub fn model(&self) -> &ShellModel {
        &self.model
    }
}

/// Registers the key bindings of the shell and of the widgets it uses.
pub fn bind_keys(cx: &mut App) {
    text_field::bind_keys(cx);
    actions::bind_keys(cx);
}

/// Applies a control's change to the shell and redraws it. A closed window does nothing.
fn change(shell: &WeakEntity<Shell>, cx: &mut App, apply: impl FnOnce(&mut Shell)) {
    let _ = shell.update(cx, |shell, cx| {
        apply(shell);
        cx.notify();
    });
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        self.model
            .set_window_width(f32::from(window.viewport_size().width));
        let shell = cx.entity().downgrade();
        let title_bar = title_bar::title_bar(&self.model, &shell, window.is_maximized(), &theme);
        let canvas = canvas::canvas(&self.model, cx);
        div()
            .size_full()
            .flex()
            .flex_row()
            .bg(theme.palette.surface.frame)
            .text_color(theme.palette.text.primary)
            .text_body()
            .track_focus(&self.focus)
            .on_action(cx.listener(|shell, _: &CloseOverlay, _, cx| {
                shell.model.close_overlay();
                cx.notify();
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(title_bar)
                    .child(div().flex_1().min_h_0().flex().flex_row().child(canvas)),
            )
    }
}
