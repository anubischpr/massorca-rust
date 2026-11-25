//! The grid the session view last drew its terminal at, so a new or resumed session starts its PTY
//! at that size instead of growing once its view attaches: the inbox ConPTY repaints a grown screen
//! anchored at the top, which leaves blank rows below the agent's prompt.

use gpui::App;

use super::Shell;
use crate::model::canvas::Canvas;

impl Shell {
    /// The (cols, rows) the session view last applied to its terminal, if it has drawn one in this run.
    pub(crate) fn session_grid(&self) -> Option<(u16, u16)> {
        self.session_grid
    }

    /// Remembers `terminal`'s new grid when it is the terminal the session view shows.
    pub(super) fn note_terminal_grid(&mut self, terminal: &str, cols: u16, rows: u16, cx: &App) {
        let shown = matches!(self.model.canvas(), Canvas::Session(_))
            .then(|| self.model.snapshot().active_session.clone())
            .flatten()
            .and_then(|id| {
                let state = self.store.read(cx).state();
                state
                    .agents
                    .iter()
                    .find(|session| session.id == id)
                    .and_then(|session| session.terminal_id.clone())
            });
        if shown.as_deref() == Some(terminal) {
            self.session_grid = Some((cols, rows));
        }
    }
}
