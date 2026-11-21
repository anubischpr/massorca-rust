//! Splitter: the draggable divider between two side-by-side regions (docs/design/components/Splitter.md).
//! The width rules are plain Rust: a drag, the Left and Right keys (8 px, 24 px with Shift), Home,
//! End and a double click each reduce to one [`SplitterModel`] call that returns the region's new
//! [`Extent`]. The element that draws the divider builds on these rules.

use crate::tokens::{LAYOUT_CONVERSATION_MIN, SPACE_2, SPACE_6};

/// Distance below a region's minimum at which dragging collapses it (design-system 4.8).
pub const SPLITTER_SNAP_PX: f32 = 40.0;
/// Edge travel of one Left or Right press, and with Shift (Splitter.md: space.2 and space.6).
pub const KEY_STEP_PX: f32 = SPACE_2;
pub const KEY_STEP_SHIFT_PX: f32 = SPACE_6;

/// Which region the splitter resizes: the one left of it (Sidebar) or right of it (Work panel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegionSide {
    Left,
    Right,
}

impl RegionSide {
    /// How far the region grows when the edge moves one pixel to the right.
    fn growth(self) -> f32 {
        match self {
            Self::Left => 1.0,
            Self::Right => -1.0,
        }
    }
}

/// A region is open at a width, or collapsed: hidden, with its splitter at the edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Extent {
    Open(f32),
    Collapsed,
}

/// The width rules of one region: its default, its minimum and maximum (layout tokens), and the
/// room the Conversation minimum leaves it (see [`room_beside_conversation`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitterLimits {
    pub default: f32,
    pub min: f32,
    pub max: f32,
    pub room: f32,
}

impl SplitterLimits {
    /// The widest the region may open: its maximum or the room left, never below its minimum.
    pub fn ceiling(self) -> f32 {
        self.max.min(self.room).max(self.min)
    }

    fn clamp(self, width: f32) -> f32 {
        width.clamp(self.min, self.ceiling())
    }
}

/// The widest a region may be while the Conversation keeps its minimum: the canvas width less
/// the other regions beside the Conversation and the Conversation's own minimum.
pub fn room_beside_conversation(canvas_width: f32, other_regions: f32) -> f32 {
    canvas_width - other_regions - LAYOUT_CONVERSATION_MIN
}

/// The state of one splitter: the region it resizes, its width rules and its extent now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitterModel {
    pub side: RegionSide,
    pub limits: SplitterLimits,
    pub extent: Extent,
}

impl SplitterModel {
    pub fn new(side: RegionSide, limits: SplitterLimits, extent: Extent) -> Self {
        Self {
            side,
            limits,
            extent,
        }
    }

    /// The extent after a pointer drag that travelled `travel` px from a press made at `start`.
    /// An open region stops at its minimum and collapses once the pointer passes it by
    /// [`SPLITTER_SNAP_PX`]; a collapsed one reopens when the pointer reaches the minimum.
    pub fn dragged(self, start: Extent, travel: f32) -> Extent {
        let from = match start {
            Extent::Open(width) => width,
            Extent::Collapsed => 0.0,
        };
        let target = from + self.side.growth() * travel;
        match start {
            Extent::Open(_) if target < self.limits.min - SPLITTER_SNAP_PX => Extent::Collapsed,
            Extent::Collapsed if target < self.limits.min => Extent::Collapsed,
            _ => Extent::Open(self.limits.clamp(target)),
        }
    }

    /// The extent after the edge moved `dx` px with a key. A key never collapses a region.
    pub fn nudged(self, dx: f32) -> Extent {
        match self.extent {
            Extent::Open(width) => self.open_at(width + self.side.growth() * dx),
            Extent::Collapsed => Extent::Collapsed,
        }
    }

    /// Home: the minimum width. A collapsed region stays collapsed.
    pub fn home(self) -> Extent {
        self.open_at(self.limits.min)
    }

    /// End: the widest allowed width. A collapsed region stays collapsed.
    pub fn end(self) -> Extent {
        self.open_at(self.limits.ceiling())
    }

    /// A double click: the default width, reopening a collapsed region.
    pub fn reset(self) -> Extent {
        Extent::Open(self.limits.clamp(self.limits.default))
    }

    /// The width a screen reader is told; 0 while collapsed.
    pub fn width(self) -> f32 {
        match self.extent {
            Extent::Open(width) => width,
            Extent::Collapsed => 0.0,
        }
    }

    fn open_at(self, width: f32) -> Extent {
        match self.extent {
            Extent::Open(_) => Extent::Open(self.limits.clamp(width)),
            Extent::Collapsed => Extent::Collapsed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits(room: f32) -> SplitterLimits {
        SplitterLimits {
            default: 256.0,
            min: 200.0,
            max: 400.0,
            room,
        }
    }

    fn model(side: RegionSide, width: f32) -> SplitterModel {
        SplitterModel::new(side, limits(f32::INFINITY), Extent::Open(width))
    }

    #[test]
    fn a_left_region_grows_with_the_pointer_and_a_right_one_shrinks() {
        let left = model(RegionSide::Left, 300.0);
        assert_eq!(left.dragged(Extent::Open(300.0), 50.0), Extent::Open(350.0));
        let right = model(RegionSide::Right, 300.0);
        assert_eq!(
            right.dragged(Extent::Open(300.0), 50.0),
            Extent::Open(250.0)
        );
    }

    #[test]
    fn an_open_region_stops_at_its_minimum_then_collapses_past_the_snap_distance() {
        let left = model(RegionSide::Left, 300.0);
        assert_eq!(
            left.dragged(Extent::Open(300.0), -90.0),
            Extent::Open(210.0)
        );
        assert_eq!(
            left.dragged(Extent::Open(300.0), -100.0),
            Extent::Open(200.0)
        );
        assert_eq!(
            left.dragged(Extent::Open(300.0), -139.0),
            Extent::Open(200.0),
            "inside the snap distance it holds at the minimum"
        );
        assert_eq!(
            left.dragged(Extent::Open(300.0), -141.0),
            Extent::Collapsed,
            "past the minimum less 40 px it collapses"
        );
    }

    #[test]
    fn a_collapsed_region_reopens_at_the_minimum_when_the_pointer_comes_back() {
        let left = model(RegionSide::Left, 300.0);
        assert_eq!(left.dragged(Extent::Collapsed, 150.0), Extent::Collapsed);
        assert_eq!(left.dragged(Extent::Collapsed, 200.0), Extent::Open(200.0));
    }

    #[test]
    fn the_room_beside_the_conversation_caps_the_width() {
        assert_eq!(room_beside_conversation(1000.0, 400.0), 240.0);
        let capped = SplitterModel::new(
            RegionSide::Left,
            SplitterLimits {
                room: room_beside_conversation(1000.0, 400.0),
                ..limits(0.0)
            },
            Extent::Open(300.0),
        );
        assert_eq!(capped.limits.ceiling(), 240.0);
        assert_eq!(
            capped.dragged(Extent::Open(300.0), 100.0),
            Extent::Open(240.0)
        );
    }

    #[test]
    fn the_ceiling_never_falls_below_the_minimum() {
        assert_eq!(limits(50.0).ceiling(), 200.0);
    }

    #[test]
    fn keys_move_by_their_step_and_never_collapse_the_region() {
        let left = model(RegionSide::Left, 300.0);
        assert_eq!(left.nudged(KEY_STEP_PX), Extent::Open(308.0));
        assert_eq!(left.nudged(-1000.0), Extent::Open(200.0));
        let right = model(RegionSide::Right, 300.0);
        assert_eq!(right.nudged(KEY_STEP_SHIFT_PX), Extent::Open(276.0));
        let collapsed =
            SplitterModel::new(RegionSide::Left, limits(f32::INFINITY), Extent::Collapsed);
        assert_eq!(collapsed.nudged(KEY_STEP_PX), Extent::Collapsed);
    }

    #[test]
    fn home_and_end_go_to_the_limits_and_reset_goes_to_the_default() {
        let left = model(RegionSide::Left, 300.0);
        assert_eq!(left.home(), Extent::Open(200.0));
        assert_eq!(left.end(), Extent::Open(400.0));
        assert_eq!(left.reset(), Extent::Open(256.0));
        let collapsed =
            SplitterModel::new(RegionSide::Left, limits(f32::INFINITY), Extent::Collapsed);
        assert_eq!(
            collapsed.reset(),
            Extent::Open(256.0),
            "a double click reopens"
        );
        assert_eq!(collapsed.home(), Extent::Collapsed);
    }

    #[test]
    fn the_accessible_width_is_zero_while_collapsed() {
        assert_eq!(model(RegionSide::Left, 300.0).width(), 300.0);
        let collapsed =
            SplitterModel::new(RegionSide::Left, limits(f32::INFINITY), Extent::Collapsed);
        assert_eq!(collapsed.width(), 0.0);
    }
}
