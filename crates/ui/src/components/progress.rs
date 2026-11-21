//! Progress: spinners for short or unknown waits, and the arc and reveal helpers that indicators share.

use crate::icon::IconSize;
use crate::theme::ActiveTheme;
use crate::tokens::{BORDER_WIDTH_THICK, Color, DURATION_SPINNER};
use gpui::{
    App, Bounds, Context, ElementId, Hsla, IntoElement, ParentElement, PathBuilder, Pixels,
    RenderOnce, Styled, Task, Window, canvas, div, point, px,
};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Wait before a spinner shows, so a quick action never flashes one (design-system 4.8).
pub const SPINNER_REVEAL_DELAY: Duration = Duration::from_millis(200);

/// Arc length of a spinner, in degrees.
const SPINNER_SWEEP_DEGREES: f32 = 270.0;

/// The clock every spinner turns on, so spinners on one screen stay in step.
static SPIN_EPOCH: OnceLock<Instant> = OnceLock::new();

/// Degrees (0 to 360, clockwise from 12 o'clock) that the shared spinner clock has turned.
pub fn spin_angle_now() -> f32 {
    spin_angle(SPIN_EPOCH.get_or_init(Instant::now).elapsed())
}

/// The turn after `elapsed`: one revolution per `duration.spinner`, at a linear pace.
pub fn spin_angle(elapsed: Duration) -> f32 {
    (elapsed.as_secs_f32() / DURATION_SPINNER.as_secs_f32()).fract() * 360.0
}

/// Points along a circle, clockwise from 12 o'clock; `start` and `sweep` are in degrees.
pub(crate) fn arc_points(
    centre: (f32, f32),
    radius: f32,
    start: f32,
    sweep: f32,
) -> Vec<(f32, f32)> {
    let steps = ((sweep.abs() / 4.0).ceil() as usize).max(2);
    (0..=steps)
        .map(|step| {
            let angle = (start + sweep * step as f32 / steps as f32).to_radians();
            (
                centre.0 + radius * angle.sin(),
                centre.1 - radius * angle.cos(),
            )
        })
        .collect()
}

/// Centre and radius of the circle whose stroke of `width` fits inside `bounds`.
pub(crate) fn ring_in(bounds: Bounds<Pixels>, width: f32) -> ((f32, f32), f32) {
    let left = f32::from(bounds.origin.x);
    let top = f32::from(bounds.origin.y);
    let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
    ((left + w / 2.0, top + h / 2.0), (w.min(h) - width) / 2.0)
}

/// Strokes a polyline in window coordinates; a path GPUI cannot build is skipped.
pub(crate) fn stroke_polyline(
    window: &mut Window,
    points: &[(f32, f32)],
    width: f32,
    color: Hsla,
    closed: bool,
) {
    let mut points = points.iter().map(|&(x, y)| point(px(x), px(y)));
    let Some(first) = points.next() else {
        return;
    };
    let mut path = PathBuilder::stroke(px(width));
    path.move_to(first);
    for next in points {
        path.line_to(next);
    }
    if closed {
        path.close();
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

/// Becomes revealed once `delay` has passed, so a wait shows late and never flashes.
pub struct RevealGate {
    revealed: bool,
    _timer: Task<()>,
}

impl RevealGate {
    pub fn new(delay: Duration, cx: &mut Context<Self>) -> Self {
        let timer = cx.spawn(async move |gate, cx| {
            cx.background_executor().timer(delay).await;
            gate.update(cx, |gate, cx| {
                gate.revealed = true;
                cx.notify();
            })
            .ok();
        });
        Self {
            revealed: false,
            _timer: timer,
        }
    }

    pub fn revealed(&self) -> bool {
        self.revealed
    }
}

impl std::fmt::Debug for RevealGate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RevealGate")
            .field("revealed", &self.revealed)
            .finish()
    }
}

/// A short or unknown wait: an arc that turns over `duration.spinner`. It keeps turning under
/// reduced motion because it is functional progress (handoff 2.6).
#[derive(Debug, IntoElement)]
pub struct Spinner {
    size: IconSize,
    color: Option<Color>,
    sweep: f32,
    delay: Option<ElementId>,
}

impl Spinner {
    pub fn new() -> Self {
        Self {
            size: IconSize::default(),
            color: None,
            sweep: SPINNER_SWEEP_DEGREES,
            delay: None,
        }
    }

    /// Sets the edge length; desktop spinners are 16 px (`size.icon.sm`).
    pub fn size(mut self, size: IconSize) -> Self {
        self.size = size;
        self
    }

    /// Overrides the accent colour of the arc.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Sets the arc length in degrees (a usage ring's loading arc is 90).
    pub fn sweep(mut self, degrees: f32) -> Self {
        self.sweep = degrees;
        self
    }

    /// Shows only after `SPINNER_REVEAL_DELAY`; `id` keeps the timer across frames.
    pub fn delayed(mut self, id: impl Into<ElementId>) -> Self {
        self.delay = Some(id.into());
        self
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Spinner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let edge = px(self.size.px());
        let shown = match self.delay {
            None => true,
            Some(id) => {
                let gate = window
                    .use_keyed_state(id, cx, |_, cx| RevealGate::new(SPINNER_REVEAL_DELAY, cx));
                gate.read(cx).revealed()
            }
        };
        let frame = div().flex_none().size(edge);
        if !shown {
            // The box keeps its size, so the layout does not move when the arc appears.
            return frame.into_any_element();
        }
        window.request_animation_frame();
        let color: Hsla = self
            .color
            .unwrap_or_else(|| cx.theme().palette.accent.r#default)
            .into();
        let sweep = self.sweep;
        frame
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| {
                        let (centre, radius) = ring_in(bounds, BORDER_WIDTH_THICK);
                        let points = arc_points(centre, radius, spin_angle_now(), sweep);
                        stroke_polyline(window, &points, BORDER_WIDTH_THICK, color, false);
                    },
                )
                .size_full(),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext, TestAppContext};

    #[test]
    fn spinner_turns_once_per_duration_and_wraps() {
        assert_eq!(spin_angle(Duration::ZERO), 0.0);
        assert!((spin_angle(Duration::from_millis(400)) - 180.0).abs() < 1e-3);
        assert!(spin_angle(DURATION_SPINNER).abs() < 1e-3);
        assert!((spin_angle(Duration::from_millis(1200)) - 180.0).abs() < 1e-3);
    }

    #[test]
    fn arcs_start_at_twelve_and_run_clockwise() {
        let quarter = arc_points((0.0, 0.0), 1.0, 0.0, 90.0);
        let (x, y) = quarter[0];
        assert!(
            x.abs() < 1e-5 && (y + 1.0).abs() < 1e-5,
            "12 o'clock: {x},{y}"
        );
        let (x, y) = *quarter.last().expect("an arc has points");
        assert!(
            (x - 1.0).abs() < 1e-5 && y.abs() < 1e-5,
            "3 o'clock: {x},{y}"
        );
        let three_quarters = arc_points((0.0, 0.0), 1.0, 0.0, 270.0);
        let (x, y) = *three_quarters.last().expect("an arc has points");
        assert!(
            (x + 1.0).abs() < 1e-5 && y.abs() < 1e-5,
            "9 o'clock: {x},{y}"
        );
    }

    #[gpui::test]
    fn reveal_gate_turns_on_once_the_delay_has_passed(cx: &mut TestAppContext) {
        let gate = cx.new(|cx| RevealGate::new(SPINNER_REVEAL_DELAY, cx));
        cx.run_until_parked();
        assert!(!gate.read_with(cx, |gate, _| gate.revealed()));
        cx.executor()
            .advance_clock(SPINNER_REVEAL_DELAY - Duration::from_millis(1));
        cx.run_until_parked();
        assert!(!gate.read_with(cx, |gate, _| gate.revealed()));
        cx.executor().advance_clock(Duration::from_millis(1));
        cx.run_until_parked();
        assert!(gate.read_with(cx, |gate, _| gate.revealed()));
    }
}
