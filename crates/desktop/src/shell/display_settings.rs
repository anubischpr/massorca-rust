//! The display settings in the shell (SET-01): the Status bar's shown items and the text preferences the window draws
//! with. They follow what the settings store holds, on connect and on every change.

use gpui::Context;
use massorca_ui::text_preferences;

use super::Shell;
use crate::model::display_settings::DisplaySettings;

impl Shell {
    /// Applies `settings`. The Status bar takes its shown items only when those change, so this window's own visibility
    /// choice survives a change to another setting; the text preferences reach every window at once.
    pub(super) fn follow_display_settings(
        &mut self,
        settings: DisplaySettings,
        cx: &mut Context<Self>,
    ) {
        if settings.status_items != self.applied_status_items {
            self.model.set_status_items_shown(&settings.status_items);
            self.applied_status_items = settings.status_items;
        }
        if settings.text != text_preferences::current() {
            text_preferences::set(settings.text);
            cx.refresh_windows();
        }
    }
}
