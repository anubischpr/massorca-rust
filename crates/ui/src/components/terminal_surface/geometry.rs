//! Grid geometry: how many cells fit a pane, and which cell a pixel falls in.
//!
//! Pure arithmetic on pixels, so the surface's resize and pointer mapping are
//! testable without a window.

use alacritty_terminal::term::{MIN_COLUMNS, MIN_SCREEN_LINES};

use super::feed::{MAX_COLUMNS, MAX_LINES, ViewPoint};

/// The pixel size of one cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellSize {
    pub width: f32,
    pub height: f32,
}

/// A cell under a pointer: its place in the view, and whether the pointer is on its right half.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellHit {
    pub point: ViewPoint,
    pub right_half: bool,
}

/// The columns and lines that fit `width` x `height` pixels, within the service's
/// limits (decision 0033: 2 to 1000 columns, 1 to 500 lines).
pub fn grid_fit(width: f32, height: f32, cell: CellSize) -> (u16, u16) {
    let columns = (width / cell.width)
        .floor()
        .clamp(MIN_COLUMNS as f32, MAX_COLUMNS as f32);
    let lines = (height / cell.height)
        .floor()
        .clamp(MIN_SCREEN_LINES as f32, MAX_LINES as f32);
    (columns as u16, lines as u16)
}

/// The cell under the pixel `(x, y)` measured from the grid's top-left corner. Points
/// outside the grid clamp to its nearest cell.
pub fn cell_at(x: f32, y: f32, cell: CellSize, columns: usize, lines: usize) -> CellHit {
    let column = (x / cell.width)
        .floor()
        .clamp(0.0, columns.saturating_sub(1) as f32) as usize;
    let row = (y / cell.height)
        .floor()
        .clamp(0.0, lines.saturating_sub(1) as f32) as usize;
    CellHit {
        point: ViewPoint { row, column },
        right_half: x - column as f32 * cell.width >= cell.width / 2.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CELL: CellSize = CellSize {
        width: 8.0,
        height: 16.0,
    };

    #[test]
    fn the_grid_holds_as_many_whole_cells_as_fit() {
        assert_eq!(grid_fit(800.0, 320.0, CELL), (100, 20));
        assert_eq!(grid_fit(815.9, 335.9, CELL), (101, 20));
    }

    #[test]
    fn the_grid_is_clamped_to_the_service_limits() {
        assert_eq!(grid_fit(5.0, 5.0, CELL), (2, 1));
        assert_eq!(grid_fit(1.0e9, 1.0e9, CELL), (1000, 500));
        assert_eq!(grid_fit(0.0, -4.0, CELL), (2, 1));
    }

    #[test]
    fn a_pixel_maps_to_its_cell_and_half() {
        let left = cell_at(3.0, 2.0, CELL, 10, 5);
        assert_eq!(left.point, ViewPoint { row: 0, column: 0 });
        assert!(!left.right_half);
        let right = cell_at(12.0, 17.0, CELL, 10, 5);
        assert_eq!(right.point, ViewPoint { row: 1, column: 1 });
        assert!(right.right_half);
    }

    #[test]
    fn a_pixel_outside_the_grid_clamps_to_its_edge_cell() {
        let before = cell_at(-10.0, -10.0, CELL, 10, 5);
        assert_eq!(before.point, ViewPoint { row: 0, column: 0 });
        let after = cell_at(900.0, 900.0, CELL, 10, 5);
        assert_eq!(after.point, ViewPoint { row: 4, column: 9 });
    }
}
