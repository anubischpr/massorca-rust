//! Skeleton: grey blocks in the shape of content that is loading, so the layout holds while data arrives.

use crate::components::progress::RevealGate;
use crate::theme::ActiveTheme;
use crate::tokens::{
    Color, Palette, RADIUS_FULL, RADIUS_LG, RADIUS_MD, RADIUS_XS, SIZE_AVATAR_MD, SIZE_AVATAR_SM,
    SIZE_CONTROL_DEFAULT, SIZE_SKELETON_LINE, SPACE_1, SPACE_2, SPACE_4,
};
use gpui::{
    App, Div, ElementId, IntoElement, ParentElement, RenderOnce, Styled, Window, div, px, relative,
};
use std::time::Duration;

/// Wait before a skeleton shows, so a fast load never flashes one (design-system 4.8).
pub const SKELETON_REVEAL_DELAY: Duration = Duration::from_millis(100);

/// Width of a text line, as a share of its container.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineWidth {
    /// 100 percent.
    Full,
    /// 80 percent.
    Most,
    /// 60 percent.
    Partial,
}

impl LineWidth {
    /// The share of the container the line covers.
    pub fn fraction(self) -> f32 {
        match self {
            LineWidth::Full => 1.0,
            LineWidth::Most => 0.8,
            LineWidth::Partial => 0.6,
        }
    }
}

/// The shape a skeleton block takes (Skeleton.md variants).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkeletonShape {
    /// One text line, `size.skeleton.line` tall.
    Line(LineWidth),
    /// A list row: a leading circle and two lines, one control tall.
    Row,
    /// A raised card with a title line and three lines.
    Card,
    /// A circle the size of a medium avatar.
    Avatar,
    /// A rectangle that fills its container: a chart, editor or image area.
    Block,
}

impl SkeletonShape {
    /// The corner radius the shape stands for.
    pub fn corner_radius(self) -> f32 {
        match self {
            SkeletonShape::Line(_) => RADIUS_XS,
            SkeletonShape::Row | SkeletonShape::Block => RADIUS_MD,
            SkeletonShape::Card => RADIUS_LG,
            SkeletonShape::Avatar => RADIUS_FULL,
        }
    }
}

/// A static placeholder in the shape of loading content. It is never the final state: callers
/// replace it with content, an empty state or an error state. It does not animate.
#[derive(Debug, IntoElement)]
pub struct Skeleton {
    shape: SkeletonShape,
    delay: Option<ElementId>,
}

impl Skeleton {
    pub fn new(shape: SkeletonShape) -> Self {
        Self { shape, delay: None }
    }

    /// A text line of the given width.
    pub fn line(width: LineWidth) -> Self {
        Self::new(SkeletonShape::Line(width))
    }

    /// Shows only after `SKELETON_REVEAL_DELAY`; `id` keeps the timer across frames. Until then the
    /// shape keeps its space, so the layout does not move.
    pub fn delayed(mut self, id: impl Into<ElementId>) -> Self {
        self.delay = Some(id.into());
        self
    }
}

impl RenderOnce for Skeleton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let shown = match self.delay {
            None => true,
            Some(id) => {
                let gate = window
                    .use_keyed_state(id, cx, |_, cx| RevealGate::new(SKELETON_REVEAL_DELAY, cx));
                gate.read(cx).revealed()
            }
        };
        let palette = cx.theme().palette;
        let fill = shown.then_some(palette.fill.skeleton);
        match self.shape {
            SkeletonShape::Line(width) => line(width, fill),
            SkeletonShape::Row => row(fill),
            SkeletonShape::Card => card(fill, shown, &palette),
            SkeletonShape::Avatar => block(div().size(px(SIZE_AVATAR_MD)).rounded_full(), fill),
            SkeletonShape::Block => block(div().size_full().rounded(px(RADIUS_MD)), fill),
        }
    }
}

/// Paints the block's fill only when it is shown; an unshown block keeps its size.
fn block(shape: Div, fill: Option<Color>) -> Div {
    match fill {
        Some(color) => shape.bg(color),
        None => shape,
    }
}

fn line(width: LineWidth, fill: Option<Color>) -> Div {
    block(
        div()
            .h(px(SIZE_SKELETON_LINE))
            .w(relative(width.fraction()))
            .rounded(px(SkeletonShape::Line(width).corner_radius())),
        fill,
    )
}

fn row(fill: Option<Color>) -> Div {
    div()
        .h(px(SIZE_CONTROL_DEFAULT))
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .child(block(div().size(px(SIZE_AVATAR_SM)).rounded_full(), fill))
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(SPACE_1))
                .child(line(LineWidth::Full, fill))
                .child(line(LineWidth::Partial, fill)),
        )
}

fn card(fill: Option<Color>, shown: bool, palette: &Palette) -> Div {
    let frame = div()
        .flex()
        .flex_col()
        .gap(px(SPACE_2))
        .p(px(SPACE_4))
        .rounded(px(SkeletonShape::Card.corner_radius()));
    let frame = if shown {
        frame
            .bg(palette.surface.raised)
            .border_1()
            .border_color(palette.border.r#default)
    } else {
        frame
    };
    frame
        .child(line(LineWidth::Most, fill))
        .child(line(LineWidth::Full, fill))
        .child(line(LineWidth::Most, fill))
        .child(line(LineWidth::Partial, fill))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_widths_are_one_hundred_eighty_and_sixty_percent() {
        let widths =
            [LineWidth::Full, LineWidth::Most, LineWidth::Partial].map(LineWidth::fraction);
        assert_eq!(widths, [1.0, 0.8, 0.6]);
    }

    #[test]
    fn corner_radii_follow_the_shape() {
        assert_eq!(
            SkeletonShape::Line(LineWidth::Full).corner_radius(),
            RADIUS_XS
        );
        assert_eq!(SkeletonShape::Row.corner_radius(), RADIUS_MD);
        assert_eq!(SkeletonShape::Block.corner_radius(), RADIUS_MD);
        assert_eq!(SkeletonShape::Card.corner_radius(), RADIUS_LG);
        assert_eq!(SkeletonShape::Avatar.corner_radius(), RADIUS_FULL);
    }

    #[test]
    fn a_row_is_one_control_tall_and_its_two_lines_fill_it() {
        assert_eq!(2.0 * SIZE_SKELETON_LINE + SPACE_1, SIZE_CONTROL_DEFAULT);
    }

    #[test]
    fn a_skeleton_waits_one_hundred_milliseconds() {
        assert_eq!(SKELETON_REVEAL_DELAY, Duration::from_millis(100));
    }
}

#[cfg(test)]
mod a11y_tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::TestAppContext;

    #[gpui::test]
    fn skeleton_blocks_stay_out_of_the_tree(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(Skeleton::new(SkeletonShape::Block).render(window, cx))
        });
        assert!(node.is_none());
    }
}
