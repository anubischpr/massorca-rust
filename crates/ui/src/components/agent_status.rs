//! AgentStatus: the one status glyph for an agent or session, with six states and one shape each.

use crate::components::agent_icon::NameTooltip;
use crate::components::progress::{arc_points, spin_angle_now, stroke_polyline};
use crate::theme::ActiveTheme;
use crate::tokens::{
    Color, DURATION_TOOLTIP_DELAY, Palette, SIZE_ICON_MD, SIZE_ICON_SM, SIZE_ICON_XS,
};
use gpui::{
    App, AppContext, Bounds, Hsla, InteractiveElement, IntoElement, ParentElement, PathBuilder,
    Pixels, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, canvas,
    div, point, px,
};

/// Edge of the grid every glyph is drawn on.
const GRID: f32 = 24.0;

/// Centre of the grid.
const CENTRE: (f32, f32) = (GRID / 2.0, GRID / 2.0);

/// Centreline radius of the circles; keeps a 3-unit stroke inside the grid.
const RING_RADIUS: f32 = 10.0;

/// Sweep of the running arc: three quarters of a turn.
const RUNNING_SWEEP: f32 = 270.0;

/// Dashes of the unreachable ring: twelve, each 18 degrees with a gap of 12.
const DASHES: usize = 12;
const DASH_SWEEP: f32 = 18.0;

/// Radius of the dots in the question mark and the exclamation mark.
const DOT_RADIUS: f32 = 1.5;

/// The six states an agent or session can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AgentState {
    /// Working: the arc turns.
    Running,
    /// Needs the user: a filled dot with a question mark, on a tinted row.
    Waiting,
    /// Between tasks: a hollow ring.
    Idle,
    /// Finished: a ring with a check mark.
    Done,
    /// Failed: a triangle with an exclamation mark, on a tinted row.
    Error,
    /// The host cannot be reached: the last known state is dimmed by the caller.
    Unreachable,
}

impl AgentState {
    /// Every state, in the order the spec lists them.
    pub const ALL: [AgentState; 6] = [
        AgentState::Running,
        AgentState::Waiting,
        AgentState::Idle,
        AgentState::Done,
        AgentState::Error,
        AgentState::Unreachable,
    ];

    /// The state word: the accessible name and the tooltip (design-system 8.4 copy).
    pub fn label(self) -> &'static str {
        match self {
            AgentState::Running => "Running",
            AgentState::Waiting => "Waiting for you",
            AgentState::Idle => "Idle",
            AgentState::Done => "Done",
            AgentState::Error => "Error",
            AgentState::Unreachable => "Unreachable",
        }
    }

    /// The glyph colour, from the matching `agent.*` token.
    pub fn color(self, palette: &Palette) -> Color {
        match self {
            AgentState::Running => palette.agent.running,
            AgentState::Waiting => palette.agent.waiting,
            AgentState::Idle => palette.agent.idle,
            AgentState::Done => palette.agent.done,
            AgentState::Error => palette.agent.error,
            AgentState::Unreachable => palette.agent.unreachable,
        }
    }

    /// The solid tint of a row that a waiting or failed session marks, if the state has one.
    pub fn row_tint(self, palette: &Palette) -> Option<Color> {
        match self {
            AgentState::Waiting => Some(palette.agent.waiting_bg),
            AgentState::Error => Some(palette.agent.error_bg),
            AgentState::Running | AgentState::Idle | AgentState::Done | AgentState::Unreachable => {
                None
            }
        }
    }

    /// Whether the glyph moves. Only the running arc turns, as functional progress.
    pub fn turns(self) -> bool {
        matches!(self, AgentState::Running)
    }
}

/// The glyph sizes: 12, 16 and 20 px.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AgentStatusSize {
    /// Status bar items and rows (`size.icon.xs`).
    XSmall,
    /// Tabs (`size.icon.sm`), the default.
    #[default]
    Small,
    /// Cards (`size.icon.md`).
    Medium,
}

impl AgentStatusSize {
    /// The edge length in logical pixels.
    pub fn px(self) -> f32 {
        match self {
            AgentStatusSize::XSmall => SIZE_ICON_XS,
            AgentStatusSize::Small => SIZE_ICON_SM,
            AgentStatusSize::Medium => SIZE_ICON_MD,
        }
    }

    /// The stroke weight in grid units: 3 at 12 px, 2.25 at 16 px and 2.1 at 20 px.
    fn grid_stroke(self) -> f32 {
        match self {
            AgentStatusSize::XSmall => 3.0,
            AgentStatusSize::Small => 2.25,
            AgentStatusSize::Medium => 2.1,
        }
    }

    /// The stroke width in pixels: the grid weight scaled to the glyph size.
    pub fn stroke_px(self) -> f32 {
        self.grid_stroke() * self.px() / GRID
    }
}

/// Which colour a glyph part is drawn in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ink {
    /// The state colour.
    State,
    /// The surface colour, cut out of a filled dot (the question mark).
    Knockout,
}

/// One painted part of a glyph, in grid units.
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    /// A polyline stroked at the glyph's stroke width.
    Stroke {
        points: Vec<(f32, f32)>,
        closed: bool,
        ink: Ink,
    },
    /// A filled polygon.
    Fill { points: Vec<(f32, f32)>, ink: Ink },
}

fn stroke(points: Vec<(f32, f32)>, closed: bool, ink: Ink) -> Part {
    Part::Stroke {
        points,
        closed,
        ink,
    }
}

fn dot(centre: (f32, f32), ink: Ink) -> Part {
    Part::Fill {
        points: arc_points(centre, DOT_RADIUS, 0.0, 360.0),
        ink,
    }
}

/// The parts of a state's glyph. `angle` turns the running arc, in degrees from 12 o'clock.
pub fn glyph_parts(state: AgentState, angle: f32) -> Vec<Part> {
    let ring = || arc_points(CENTRE, RING_RADIUS, 0.0, 360.0);
    match state {
        AgentState::Running => vec![stroke(
            arc_points(CENTRE, RING_RADIUS, angle, RUNNING_SWEEP),
            false,
            Ink::State,
        )],
        AgentState::Waiting => {
            // The hook of the question mark, then its stem down to the dot.
            let mut question = arc_points((12.0, 9.0), 3.0, 270.0, 199.0);
            question.push((12.0, 13.0));
            vec![
                Part::Fill {
                    points: ring(),
                    ink: Ink::State,
                },
                stroke(question, false, Ink::Knockout),
                dot((12.0, 17.0), Ink::Knockout),
            ]
        }
        AgentState::Idle => vec![stroke(ring(), true, Ink::State)],
        AgentState::Done => vec![
            stroke(ring(), true, Ink::State),
            stroke(
                vec![(9.0, 12.0), (11.0, 14.0), (15.0, 10.0)],
                false,
                Ink::State,
            ),
        ],
        AgentState::Error => vec![
            stroke(
                vec![(12.0, 3.0), (21.5, 20.5), (2.5, 20.5)],
                true,
                Ink::State,
            ),
            stroke(vec![(12.0, 9.0), (12.0, 13.5)], false, Ink::State),
            dot((12.0, 17.0), Ink::State),
        ],
        AgentState::Unreachable => {
            let mut parts: Vec<Part> = (0..DASHES)
                .map(|index| {
                    let start = index as f32 * (360.0 / DASHES as f32);
                    stroke(
                        arc_points(CENTRE, RING_RADIUS, start, DASH_SWEEP),
                        false,
                        Ink::State,
                    )
                })
                .collect();
            parts.push(stroke(vec![(4.9, 4.9), (19.1, 19.1)], false, Ink::State));
            parts
        }
    }
}

/// The two colours a glyph paints with.
#[derive(Clone, Copy, Debug)]
struct Colours {
    state: Hsla,
    knockout: Hsla,
}

impl Colours {
    fn of(self, ink: Ink) -> Hsla {
        match ink {
            Ink::State => self.state,
            Ink::Knockout => self.knockout,
        }
    }
}

pub(super) fn fill_polygon(window: &mut Window, points: &[(f32, f32)], color: Hsla) {
    let mut points = points.iter().map(|&(x, y)| point(px(x), px(y)));
    let Some(first) = points.next() else {
        return;
    };
    let mut path = PathBuilder::fill();
    path.move_to(first);
    for next in points {
        path.line_to(next);
    }
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

/// Paints the parts into `bounds`, scaling grid units to the box.
fn paint_glyph(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    parts: &[Part],
    colours: Colours,
    stroke_px: f32,
) {
    let (left, top) = (f32::from(bounds.origin.x), f32::from(bounds.origin.y));
    let scale = f32::from(bounds.size.width) / GRID;
    let map = |points: &[(f32, f32)]| -> Vec<(f32, f32)> {
        points
            .iter()
            .map(|&(x, y)| (left + x * scale, top + y * scale))
            .collect()
    };
    for part in parts {
        match part {
            Part::Stroke {
                points,
                closed,
                ink,
            } => stroke_polyline(window, &map(points), stroke_px, colours.of(*ink), *closed),
            Part::Fill { points, ink } => fill_polygon(window, &map(points), colours.of(*ink)),
        }
    }
}

/// The status glyph of an agent or session. It is not a stop: the containing row takes focus,
/// and the state word is its accessible name and tooltip.
#[derive(Debug, IntoElement)]
pub struct AgentStatus {
    state: AgentState,
    size: AgentStatusSize,
}

impl AgentStatus {
    pub fn new(state: AgentState) -> Self {
        Self {
            state,
            size: AgentStatusSize::default(),
        }
    }

    pub fn size(mut self, size: AgentStatusSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for AgentStatus {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = cx.theme().palette;
        let turns = self.state.turns();
        if turns {
            window.request_animation_frame();
        }
        let angle = if turns { spin_angle_now() } else { 0.0 };
        let parts = glyph_parts(self.state, angle);
        let colours = Colours {
            state: self.state.color(&palette).into(),
            knockout: palette.surface.canvas.into(),
        };
        let stroke_px = self.size.stroke_px();
        let label = SharedString::from(self.state.label());
        let tooltip = label.clone();
        div()
            .id(SharedString::from(format!(
                "agent-status-{}",
                self.state.label()
            )))
            .flex_none()
            .size(px(self.size.px()))
            .role(Role::Image)
            .aria_label(label)
            .tooltip_show_delay(DURATION_TOOLTIP_DELAY)
            .tooltip(move |_, cx| cx.new(|_| NameTooltip(tooltip.clone())).into())
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        paint_glyph(window, bounds, &parts, colours, stroke_px);
                    },
                )
                .size_full(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_point(actual: (f32, f32), expected: (f32, f32)) {
        assert!(
            (actual.0 - expected.0).abs() < 1e-3 && (actual.1 - expected.1).abs() < 1e-3,
            "expected {expected:?}, got {actual:?}"
        );
    }

    fn only_stroke(parts: Vec<Part>) -> (Vec<(f32, f32)>, bool) {
        match parts.as_slice() {
            [Part::Stroke { points, closed, .. }] => (points.clone(), *closed),
            other => panic!("expected one stroke, got {other:?}"),
        }
    }

    #[test]
    fn each_state_has_its_state_word() {
        let words: Vec<_> = AgentState::ALL.iter().map(|s| s.label()).collect();
        assert_eq!(
            words,
            vec![
                "Running",
                "Waiting for you",
                "Idle",
                "Done",
                "Error",
                "Unreachable"
            ]
        );
    }

    #[test]
    fn each_state_takes_its_agent_token() {
        for palette in [Palette::DARK, Palette::LIGHT] {
            assert_eq!(AgentState::Running.color(&palette), palette.agent.running);
            assert_eq!(AgentState::Waiting.color(&palette), palette.agent.waiting);
            assert_eq!(AgentState::Idle.color(&palette), palette.agent.idle);
            assert_eq!(AgentState::Done.color(&palette), palette.agent.done);
            assert_eq!(AgentState::Error.color(&palette), palette.agent.error);
            assert_eq!(
                AgentState::Unreachable.color(&palette),
                palette.agent.unreachable
            );
        }
    }

    #[test]
    fn waiting_and_error_tint_their_rows_and_nothing_else_does() {
        let palette = Palette::LIGHT;
        assert_eq!(
            AgentState::Waiting.row_tint(&palette),
            Some(palette.agent.waiting_bg)
        );
        assert_eq!(
            AgentState::Error.row_tint(&palette),
            Some(palette.agent.error_bg)
        );
        for state in [
            AgentState::Running,
            AgentState::Idle,
            AgentState::Done,
            AgentState::Unreachable,
        ] {
            assert_eq!(state.row_tint(&palette), None, "{state:?}");
        }
    }

    #[test]
    fn only_the_running_arc_turns() {
        assert!(AgentState::Running.turns());
        for state in [
            AgentState::Waiting,
            AgentState::Idle,
            AgentState::Done,
            AgentState::Error,
            AgentState::Unreachable,
        ] {
            assert!(!state.turns(), "{state:?}");
        }
    }

    #[test]
    fn sizes_and_stroke_weights_follow_the_grid() {
        assert_eq!(AgentStatusSize::XSmall.px(), 12.0);
        assert_eq!(AgentStatusSize::Small.px(), 16.0);
        assert_eq!(AgentStatusSize::Medium.px(), 20.0);
        assert!((AgentStatusSize::XSmall.stroke_px() - 1.5).abs() < 1e-6);
        assert!((AgentStatusSize::Small.stroke_px() - 1.5).abs() < 1e-6);
        assert!((AgentStatusSize::Medium.stroke_px() - 1.75).abs() < 1e-6);
        assert_eq!(AgentStatusSize::default(), AgentStatusSize::Small);
    }

    #[test]
    fn the_running_arc_is_three_quarters_and_turns_with_the_angle() {
        let (points, closed) = only_stroke(glyph_parts(AgentState::Running, 0.0));
        assert!(!closed);
        assert_point(points[0], (12.0, 2.0));
        assert_point(*points.last().expect("an arc has points"), (2.0, 12.0));
        let (turned, _) = only_stroke(glyph_parts(AgentState::Running, 90.0));
        assert_point(turned[0], (22.0, 12.0));
    }

    #[test]
    fn idle_is_a_closed_ring() {
        let (points, closed) = only_stroke(glyph_parts(AgentState::Idle, 0.0));
        assert!(closed);
        for point in points {
            let radius = ((point.0 - 12.0).powi(2) + (point.1 - 12.0).powi(2)).sqrt();
            assert!((radius - RING_RADIUS).abs() < 1e-3, "{point:?}");
        }
    }

    #[test]
    fn done_is_a_ring_with_a_check_mark() {
        let parts = glyph_parts(AgentState::Done, 0.0);
        assert_eq!(parts.len(), 2);
        let Part::Stroke { points, .. } = &parts[1] else {
            panic!("the check mark is a stroke");
        };
        assert_eq!(points, &vec![(9.0, 12.0), (11.0, 14.0), (15.0, 10.0)]);
    }

    #[test]
    fn waiting_is_a_filled_dot_with_a_knocked_out_question_mark() {
        let parts = glyph_parts(AgentState::Waiting, 0.0);
        assert!(matches!(
            parts[0],
            Part::Fill {
                ink: Ink::State,
                ..
            }
        ));
        assert!(matches!(
            parts[1],
            Part::Stroke {
                ink: Ink::Knockout,
                ..
            }
        ));
        assert!(matches!(
            parts[2],
            Part::Fill {
                ink: Ink::Knockout,
                ..
            }
        ));
    }

    #[test]
    fn error_is_a_triangle_with_an_exclamation_mark() {
        let parts = glyph_parts(AgentState::Error, 0.0);
        let (triangle, closed) = only_stroke(parts[..1].to_vec());
        assert!(closed);
        assert_eq!(triangle.len(), 3);
        assert!(matches!(parts[1], Part::Stroke { closed: false, .. }));
        assert!(matches!(parts[2], Part::Fill { .. }));
    }

    #[test]
    fn unreachable_is_twelve_dashes_and_a_slash() {
        let parts = glyph_parts(AgentState::Unreachable, 0.0);
        assert_eq!(parts.len(), DASHES + 1);
        for dash in &parts[..DASHES] {
            let (points, closed) = only_stroke(vec![dash.clone()]);
            assert!(!closed);
            assert!(points.len() >= 2);
        }
        let (slash, _) = only_stroke(parts[DASHES..].to_vec());
        assert_eq!(slash.len(), 2);
    }

    #[test]
    fn every_glyph_stays_inside_the_grid() {
        for state in AgentState::ALL {
            for part in glyph_parts(state, 45.0) {
                let points = match part {
                    Part::Stroke { points, .. } | Part::Fill { points, .. } => points,
                };
                for (x, y) in points {
                    assert!((0.0..=GRID).contains(&x), "{state:?} x={x}");
                    assert!((0.0..=GRID).contains(&y), "{state:?} y={y}");
                }
            }
        }
    }
}

#[cfg(test)]
mod a11y_tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{TestAppContext, accesskit::Role};

    #[gpui::test]
    fn an_agent_status_is_an_image_named_by_its_state_word(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(AgentStatus::new(AgentState::Running).render(window, cx))
        })
        .expect("reported");
        assert_eq!(node.role(), Role::Image);
        assert_eq!(node.label(), Some(AgentState::Running.label()));
    }
}
