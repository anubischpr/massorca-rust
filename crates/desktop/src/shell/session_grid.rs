//! The grid the session view last drew its terminal at, so a new or resumed session starts its PTY
//! at that size instead of growing once its view attaches: the inbox ConPTY repaints a grown screen
//! anchored at the top, which leaves blank rows below the agent's prompt.

use gpui::App;
use massorca_ui::components::terminal_surface::geometry::{self, CellSize};
use massorca_ui::components::terminal_surface::grid_paint;
use massorca_ui::tokens::SPACE_2;

use super::Shell;
use super::session_view::SESSION_HEADER_HEIGHT;
use crate::model::canvas::Canvas;
use crate::model::start_page::{StartGrids, StartMode};

/// The grid a terminal pane fits when it is `width` x `height` pixels: the terminal surface pads its grid by SPACE_2 on
/// each side (`TerminalSurface::render`), so the pane's own size is not the grid's.
pub(crate) fn pane_grid(width: f32, height: f32, cell: CellSize) -> (u16, u16) {
    let padding = 2.0 * SPACE_2;
    geometry::grid_fit(width - padding, height - padding, cell)
}

/// The grid of a session's terminal pane when the Conversation is `width` x `height` pixels: the header sits above the
/// pane, so it is taken off the height first.
pub(crate) fn session_pane_grid(width: f32, height: f32, cell: CellSize) -> (u16, u16) {
    pane_grid(width, height - SESSION_HEADER_HEIGHT, cell)
}

impl Shell {
    /// The (cols, rows) the session view last applied to its terminal, if it has drawn one in this run.
    pub(crate) fn session_grid(&self) -> Option<(u16, u16)> {
        self.session_grid
    }

    /// The grid a session started in `mode` opens its terminal at (`model::start_page::start_grid`). A Conversation takes
    /// the session view's grid, else the Conversation fitted to the cells below the session header; a new run has no
    /// ended bar, notice or queue dock, so those bands are not subtracted. A Terminal takes the Work panel's grid.
    pub(crate) fn start_grid(&self, mode: StartMode, cx: &App) -> (u16, u16) {
        let fitted = self.conversation_size.get().and_then(|(width, height)| {
            let cell = grid_paint::cell_metrics(cx)?;
            Some(session_pane_grid(width, height, cell))
        });
        crate::model::start_page::start_grid(
            mode,
            StartGrids {
                shown: self.session_grid(),
                fitted,
                measured: self.work_panel_measured_grid(cx),
                applied: self.work_panel_applied,
            },
        )
    }

    /// Whether the service's record already holds `terminal` at this grid, so a pane fitting to it needs no resize request.
    pub(super) fn terminal_has_grid(&self, terminal: &str, cols: u16, rows: u16, cx: &App) -> bool {
        self.store
            .read(cx)
            .state()
            .terminals
            .iter()
            .any(|record| record.id == terminal && (record.cols, record.rows) == (cols, rows))
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
