//! The confirm before the main window closes (PLAT-10). A close asks first while a session runs or waits for the
//! person, because only the window goes: the service keeps every session running. Closing sends no call to the
//! service. `massorca agent list` shows what keeps running.

use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Styled, WeakEntity, Window, div, px,
};
use massorca_ui::components::dialog::{Dialog, DialogVariant};
use massorca_ui::tokens::{SPACE_2, TYPE_BODY_SM};
use massorca_ui::{ActiveTheme, TextStyled};

use super::Shell;
use crate::model::close_confirm::{CloseRisk, close_check};

/// Says what happens to the sessions when the window goes.
const KEEP_RUNNING: &str = "The service keeps running; sessions continue.";

/// The confirm on screen, if a close is waiting for it.
pub(in crate::shell) fn dialog(
    shell: &Shell,
    weak: &WeakEntity<Shell>,
    cx: &App,
) -> Option<AnyElement> {
    let risk = shell.close_confirm?;
    let theme = *cx.theme();
    let body = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .text_style(TYPE_BODY_SM)
        .text_color(theme.palette.text.secondary)
        .child(sessions_line(risk.running_sessions))
        .child(KEEP_RUNNING);
    let (close, cancel) = (weak.clone(), weak.clone());
    Some(
        Dialog::new("close-window", "Close window?")
            .open(true)
            .variant(DialogVariant::Destructive)
            .child(body)
            .action("Close window")
            .cancel_label("Cancel")
            .on_action(move |window, cx| {
                let _ = close.update(cx, |shell, cx| shell.confirm_close(window, cx));
            })
            .on_dismiss(move |_, cx| {
                let _ = cancel.update(cx, |shell, cx| shell.cancel_close(cx));
            })
            .into_any_element(),
    )
}

/// What the confirm says first: how many sessions are running or waiting for the person.
pub(super) fn sessions_line(count: usize) -> String {
    match count {
        1 => "1 session is running or waiting for you.".to_owned(),
        _ => format!("{count} sessions are running or waiting for you."),
    }
}

impl Shell {
    /// The title bar's close: closes now when nothing is at stake, otherwise opens the confirm.
    pub(super) fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.window_may_close(window, cx) {
            window.remove_window();
        }
    }

    /// Whether the window may close for a close the window itself asked for, such as the OS close. With nothing
    /// at stake the secret a dialog holds is cleared and the close goes ahead; otherwise the confirm opens.
    pub(super) fn window_may_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match self.close_risk(cx) {
            Some(risk) => {
                self.close_confirm = Some(risk);
                cx.notify();
                false
            }
            None => {
                self.close_secret_dialog(window, cx);
                true
            }
        }
    }

    /// "Close window": clears the secret a dialog holds, then removes the window. Removing it does not ask again.
    fn confirm_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_confirm = None;
        self.close_secret_dialog(window, cx);
        window.remove_window();
    }

    fn cancel_close(&mut self, cx: &mut Context<Self>) {
        self.close_confirm = None;
        cx.notify();
    }

    fn close_risk(&self, cx: &App) -> Option<CloseRisk> {
        close_check(&self.store.read(cx).state().agents)
    }
}
