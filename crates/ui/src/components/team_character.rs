//! TeamCharacter: a team's shape in its accent, with two eyes (A57, A59). The team pill draws it
//! beside the team name, as bt-board does.

use crate::components::agent_status::fill_polygon;
use crate::theme::ActiveTheme;
use crate::tokens::SIZE_ICON_SM;
use gpui::{
    App, Bounds, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels, RenderOnce, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, canvas, div, px,
};

/// Edge of the grid the character is drawn on. The outlines and the eyes use its units.
const GRID: f32 = 24.0;

/// The eyes' centres on the grid, as the Desarrollo triangle of bt-board draws them.
const EYES: [(f32, f32); 2] = [(9.5, 14.5), (14.5, 14.5)];

/// The eyes' radius on the grid.
const EYE_RADIUS: f32 = 1.3;

/// The shape of a team's character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamShape {
    Triangle,
    Square,
    Circle,
    Diamond,
    Hexagon,
    Star,
}

impl TeamShape {
    /// The outline on the grid. The triangle is the path of bt-board, `M12 3 22 20H2Z`.
    pub fn outline(self) -> Vec<(f32, f32)> {
        match self {
            TeamShape::Triangle => vec![(12.0, 3.0), (22.0, 20.0), (2.0, 20.0)],
            TeamShape::Square => vec![(3.0, 3.0), (21.0, 3.0), (21.0, 21.0), (3.0, 21.0)],
            TeamShape::Circle => circle(12.0, 12.0, 9.0, 24),
            TeamShape::Diamond => vec![(12.0, 2.0), (22.0, 12.0), (12.0, 22.0), (2.0, 12.0)],
            TeamShape::Hexagon => circle(12.0, 12.0, 10.0, 6),
            TeamShape::Star => star(12.0, 12.5, 10.0, 4.5, 5),
        }
    }
}

/// A team's character at a size. It names the team for assistive technology.
#[derive(Debug, IntoElement)]
pub struct TeamCharacterMark {
    shape: TeamShape,
    name: SharedString,
    size: f32,
}

impl TeamCharacterMark {
    pub fn new(shape: TeamShape, name: impl Into<SharedString>) -> Self {
        Self {
            shape,
            name: name.into(),
            size: SIZE_ICON_SM,
        }
    }

    /// The mark's edge in pixels; it is square.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for TeamCharacterMark {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = cx.theme().palette;
        let accent: Hsla = palette.accent.r#default.into();
        let eye: Hsla = palette.accent.on.into();
        let outline = self.shape.outline();
        div()
            .id(SharedString::from(format!("team-character-{}", self.name)))
            .flex_none()
            .size(px(self.size))
            .role(Role::Image)
            .aria_label(self.name)
            .debug_selector(|| "team-character".to_owned())
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        paint_mark(window, bounds, &outline, accent, eye);
                    },
                )
                .size_full(),
            )
    }
}

/// Paints the outline in the accent and the eyes in `eye`, scaling the grid to the bounds.
fn paint_mark(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    outline: &[(f32, f32)],
    accent: Hsla,
    eye: Hsla,
) {
    let (left, top) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    let scale = f32::from(bounds.size.width) / GRID;
    let map = |points: &[(f32, f32)]| -> Vec<(f32, f32)> {
        points
            .iter()
            .map(|&(x, y)| (left + x * scale, top + y * scale))
            .collect()
    };
    fill_polygon(window, &map(outline), accent);
    for (x, y) in EYES {
        fill_polygon(window, &map(&circle(x, y, EYE_RADIUS, 8)), eye);
    }
}

/// Points around a circle, starting at the top and going clockwise.
fn circle(cx: f32, cy: f32, radius: f32, sides: usize) -> Vec<(f32, f32)> {
    (0..sides)
        .map(|index| {
            let angle = std::f32::consts::TAU * index as f32 / sides as f32;
            (cx + radius * angle.sin(), cy - radius * angle.cos())
        })
        .collect()
}

/// A star with `points` points, the first pointing up; its inner corners sit on `inner`.
fn star(cx: f32, cy: f32, outer: f32, inner: f32, points: usize) -> Vec<(f32, f32)> {
    let corners = points * 2;
    (0..corners)
        .map(|index| {
            let radius = if index % 2 == 0 { outer } else { inner };
            let angle = std::f32::consts::TAU * index as f32 / corners as f32;
            (cx + radius * angle.sin(), cy - radius * angle.cos())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHAPES: [TeamShape; 6] = [
        TeamShape::Triangle,
        TeamShape::Square,
        TeamShape::Circle,
        TeamShape::Diamond,
        TeamShape::Hexagon,
        TeamShape::Star,
    ];

    /// Even-odd ray casting: whether the point lies inside the closed outline.
    fn inside(outline: &[(f32, f32)], (x, y): (f32, f32)) -> bool {
        let mut inside = false;
        let mut previous = outline.len() - 1;
        for current in 0..outline.len() {
            let (xi, yi) = outline[current];
            let (xj, yj) = outline[previous];
            if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                inside = !inside;
            }
            previous = current;
        }
        inside
    }

    #[test]
    fn the_triangle_is_the_outline_of_the_artboard() {
        assert_eq!(
            TeamShape::Triangle.outline(),
            vec![(12.0, 3.0), (22.0, 20.0), (2.0, 20.0)]
        );
    }

    #[test]
    fn every_outline_fits_the_grid() {
        for shape in SHAPES {
            for (x, y) in shape.outline() {
                assert!(
                    (0.0..=GRID).contains(&x) && (0.0..=GRID).contains(&y),
                    "{shape:?} has a point at ({x}, {y}) outside the grid"
                );
            }
        }
    }

    #[test]
    fn both_eyes_sit_inside_every_shape() {
        for shape in SHAPES {
            let outline = shape.outline();
            for eye in EYES {
                assert!(
                    inside(&outline, eye),
                    "{shape:?} has an eye at {eye:?} outside its outline"
                );
            }
        }
    }
}
