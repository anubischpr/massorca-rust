//! TerminalSurface: one terminal drawn as a grid of text cells, fed from its attach
//! stream (docs/design/components/TerminalSurface.md; docs/architecture/terminals.md).

pub mod colors;
pub mod feed;
pub mod geometry;
pub mod grid_paint;
pub mod keys;

mod view;

pub use view::{InputHandler, ResizeHandler, RestartHandler, TerminalSurface};
