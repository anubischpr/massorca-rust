//! The service ids a routine call needs, as the store numbered them, and the sentence for a routine that is gone.

use super::super::Shell;
use crate::model::teams::{BotId, RoutineId};
use gpui::App;

/// The note a routine call gets when the routine is no longer in the store.
pub(super) const ROUTINE_GONE: &str = "That routine is gone. Nothing was changed.";

impl Shell {
    /// The service's id for a bot of this shell, as the store numbered it.
    pub(super) fn bot_key(&self, bot: BotId, cx: &App) -> Option<String> {
        self.service_key_of(bot.0, cx)
    }

    /// The service's id for a routine of this shell, as the store numbered it.
    pub(super) fn routine_key(&self, routine: RoutineId, cx: &App) -> Option<String> {
        self.service_key_of(routine.0, cx)
    }

    fn service_key_of(&self, number: u32, cx: &App) -> Option<String> {
        self.store
            .read(cx)
            .state()
            .team_ids
            .key(number)
            .map(str::to_owned)
    }
}
