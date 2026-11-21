//! Progress: a bar that is determinate or indeterminate, and a spinner for short or unknown waits.

use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    BORDER_WIDTH_THICK, Color, DURATION_SPINNER, Palette, SIZE_PROGRESS_HEIGHT, SPACE_1, SPACE_2,
    TYPE_BODY, TYPE_CAPTION,
};
use gpui::{
    Animation, AnimationExt, App, Bounds, Context, ElementId, Hsla, InteractiveElement,
    IntoElement, ParentElement, PathBuilder, Pixels, RenderOnce, Role, SharedString,
    StatefulInteractiveElement, Styled, Task, Window, canvas, div, point, px, relative,
};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Wait before a spinner shows, so a quick action never flashes one (design-system 4.8).
pub const SPINNER_REVEAL_DELAY: Duration = Duration::from_millis(200);

/// Arc length of a spinner, in degrees.
const SPINNER_SWEEP_DEGREES: f32 = 270.0;

/// Length of the travelling segment of an indeterminate bar, as a share of the track.
const SWEEP_FRACTION: f32 = 0.3;

/// Fill of an indeterminate bar that cannot travel (reduced motion), as a static half.
const STILL_FRACTION: f32 = 0.5;

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

/// Whether a bar knows its total.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProgressValue {
    /// A known fraction of the whole, from 0 to 1.
    Determinate(f32),
    /// A task of unknown length.
    Indeterminate,
}

/// What a bar shows besides its value.
#[derive(Clone, Debug, PartialEq)]
pub enum ProgressState {
    /// Working: the fraction, or the travel of an indeterminate bar.
    Active,
    /// Stopped by the user: the fill stays and the word says Paused.
    Paused,
    /// Finished: the fill is full and the word says Done.
    Complete,
    /// Failed: the fill stops at the last value, in the danger colour.
    Failed,
    /// The host dropped: the bar stops and shows its last value with the age.
    Disconnected { age: SharedString },
}

/// Clamps a fraction to 0 to 1; NaN counts as empty.
pub fn clamp_fraction(fraction: f32) -> f32 {
    if fraction.is_nan() {
        0.0
    } else {
        fraction.clamp(0.0, 1.0)
    }
}

/// The whole percent beside a bar, with no decimals.
pub fn percent_text(fraction: f32) -> String {
    format!("{}%", (clamp_fraction(fraction) * 100.0).round() as u32)
}

/// How a bar's fill is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FillShape {
    /// A static fill covering this share of the track.
    Still(f32),
    /// A segment that travels across the track.
    Sweep,
}

/// The fill of a bar in `state`. Under reduced motion an indeterminate bar stays a half-filled
/// track.
pub fn fill_shape(value: ProgressValue, state: &ProgressState, reduced_motion: bool) -> FillShape {
    match (state, value) {
        (ProgressState::Complete, _) => FillShape::Still(1.0),
        (ProgressState::Active, ProgressValue::Indeterminate) if !reduced_motion => {
            FillShape::Sweep
        }
        (_, ProgressValue::Indeterminate) => FillShape::Still(STILL_FRACTION),
        (_, ProgressValue::Determinate(fraction)) => FillShape::Still(clamp_fraction(fraction)),
    }
}

/// The word beside a bar, if its state has one (Progress.md states and copy).
pub fn status_text(
    value: ProgressValue,
    state: &ProgressState,
    reduced_motion: bool,
) -> Option<String> {
    let known = match value {
        ProgressValue::Determinate(fraction) => Some(percent_text(fraction)),
        ProgressValue::Indeterminate => None,
    };
    match state {
        ProgressState::Active => match known {
            Some(percent) => Some(percent),
            None if reduced_motion => Some("Working".to_owned()),
            None => None,
        },
        ProgressState::Paused => Some("Paused".to_owned()),
        ProgressState::Complete => Some("Done".to_owned()),
        ProgressState::Failed => Some(match known {
            Some(percent) => format!("Failed at {percent}"),
            None => "Failed".to_owned(),
        }),
        ProgressState::Disconnected { age } => Some(match known {
            Some(percent) => format!("Connection lost. Last known {percent}, {age}"),
            None => format!("Connection lost. Last known {age}"),
        }),
    }
}

/// The fill colour: the accent, the danger colour when failed, tertiary text once the host drops.
fn fill_color(state: &ProgressState, palette: &Palette) -> Color {
    match state {
        ProgressState::Failed => palette.status.danger_solid,
        ProgressState::Disconnected { .. } => palette.text.tertiary,
        _ => palette.accent.r#default,
    }
}

/// The colour of the status word beside the bar.
fn status_color(state: &ProgressState, palette: &Palette) -> Color {
    match state {
        ProgressState::Failed => palette.status.danger_fg,
        ProgressState::Disconnected { .. } => palette.text.tertiary,
        _ => palette.text.secondary,
    }
}

/// A bar for work with a name. The name is the thing in progress, as text.
#[derive(Debug, IntoElement)]
pub struct ProgressBar {
    name: SharedString,
    value: ProgressValue,
    state: ProgressState,
}

impl ProgressBar {
    /// An active bar at 0 percent.
    pub fn new(name: impl Into<SharedString>) -> Self {
        Self {
            name: name.into(),
            value: ProgressValue::Determinate(0.0),
            state: ProgressState::Active,
        }
    }

    /// Shows a known fraction of the whole, from 0 to 1.
    pub fn determinate(mut self, fraction: f32) -> Self {
        self.value = ProgressValue::Determinate(fraction);
        self
    }

    /// Shows a task of unknown length.
    pub fn indeterminate(mut self) -> Self {
        self.value = ProgressValue::Indeterminate;
        self
    }

    pub fn state(mut self, state: ProgressState) -> Self {
        self.state = state;
        self
    }
}

impl RenderOnce for ProgressBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = cx.theme().palette;
        let reduced = cx.reduce_motion();
        let color = fill_color(&self.state, &palette);
        let fill = match fill_shape(self.value, &self.state, reduced) {
            FillShape::Still(fraction) => div()
                .absolute()
                .top_0()
                .bottom_0()
                .left_0()
                .w(relative(fraction))
                .rounded_full()
                .bg(color)
                .into_any_element(),
            FillShape::Sweep => div()
                .absolute()
                .top_0()
                .bottom_0()
                .w(relative(SWEEP_FRACTION))
                .rounded_full()
                .bg(color)
                .with_animation(
                    SharedString::from(format!("progress-sweep-{}", self.name)),
                    Animation::new(DURATION_SPINNER).repeat(),
                    |segment, delta| {
                        segment.left(relative((1.0 + SWEEP_FRACTION) * delta - SWEEP_FRACTION))
                    },
                )
                .into_any_element(),
        };
        let track = div()
            .relative()
            .flex_1()
            .h(px(SIZE_PROGRESS_HEIGHT))
            .rounded_full()
            .overflow_hidden()
            .bg(palette.progress.track)
            .border_1()
            .border_color(palette.border.subtle)
            .child(fill);
        let status = status_text(self.value, &self.state, reduced).map(|text| {
            let color = status_color(&self.state, &palette);
            let mut word = div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(SPACE_1))
                .text_style(TYPE_CAPTION)
                .tabular_figures()
                .text_color(color);
            if matches!(self.state, ProgressState::Failed) {
                word = word.child(
                    Icon::new(IconName::OctagonAlert)
                        .size(IconSize::XSmall)
                        .color(color),
                );
            }
            word.child(text)
        });
        let mut row = div().flex().items_center().gap(px(SPACE_2)).child(track);
        if let Some(status) = status {
            row = row.child(status);
        }
        let mut root = div()
            .id(SharedString::from(format!("progress-{}", self.name)))
            .flex()
            .flex_col()
            .gap(px(SPACE_1))
            .role(Role::ProgressIndicator)
            .aria_label(self.name.clone())
            .child(
                div()
                    .text_style(TYPE_BODY)
                    .text_color(palette.text.primary)
                    .child(self.name),
            )
            .child(row);
        if let ProgressValue::Determinate(fraction) = self.value {
            root = root
                .aria_numeric_value(f64::from(clamp_fraction(fraction)) * 100.0)
                .aria_min_numeric_value(0.0)
                .aria_max_numeric_value(100.0);
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext, TestAppContext};

    #[test]
    fn fractions_are_clamped_and_nan_counts_as_empty() {
        assert_eq!(clamp_fraction(-0.5), 0.0);
        assert_eq!(clamp_fraction(1.5), 1.0);
        assert_eq!(clamp_fraction(f32::NAN), 0.0);
        assert_eq!(clamp_fraction(0.25), 0.25);
    }

    #[test]
    fn percent_has_no_decimals() {
        assert_eq!(percent_text(0.62), "62%");
        assert_eq!(percent_text(0.004), "0%");
        assert_eq!(percent_text(1.0), "100%");
        assert_eq!(percent_text(2.0), "100%");
    }

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

    #[test]
    fn indeterminate_bars_travel_unless_reduced_motion_holds_them() {
        let indeterminate = ProgressValue::Indeterminate;
        assert_eq!(
            fill_shape(indeterminate, &ProgressState::Active, false),
            FillShape::Sweep
        );
        assert_eq!(
            fill_shape(indeterminate, &ProgressState::Active, true),
            FillShape::Still(0.5)
        );
        assert_eq!(
            status_text(indeterminate, &ProgressState::Active, false),
            None
        );
        assert_eq!(
            status_text(indeterminate, &ProgressState::Active, true).as_deref(),
            Some("Working")
        );
    }

    #[test]
    fn determinate_fill_follows_the_value_except_when_complete() {
        let value = ProgressValue::Determinate(0.62);
        assert_eq!(
            fill_shape(value, &ProgressState::Active, false),
            FillShape::Still(0.62)
        );
        assert_eq!(
            fill_shape(value, &ProgressState::Failed, false),
            FillShape::Still(0.62)
        );
        assert_eq!(
            fill_shape(
                ProgressValue::Determinate(0.2),
                &ProgressState::Complete,
                false
            ),
            FillShape::Still(1.0)
        );
    }

    #[test]
    fn status_words_follow_the_states() {
        let value = ProgressValue::Determinate(0.62);
        let cases = [
            (ProgressState::Active, "62%"),
            (ProgressState::Paused, "Paused"),
            (ProgressState::Complete, "Done"),
            (ProgressState::Failed, "Failed at 62%"),
            (
                ProgressState::Disconnected {
                    age: "5 min ago".into(),
                },
                "Connection lost. Last known 62%, 5 min ago",
            ),
        ];
        for (state, text) in cases {
            assert_eq!(
                status_text(value, &state, false).as_deref(),
                Some(text),
                "{state:?}"
            );
        }
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

#[cfg(test)]
mod a11y_tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{TestAppContext, accesskit::Role};

    #[gpui::test]
    fn a_determinate_bar_reports_its_percentage_out_of_one_hundred(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(
                ProgressBar::new("Upload")
                    .determinate(0.25)
                    .render(window, cx),
            )
        })
        .expect("reported");
        assert_eq!(node.role(), Role::ProgressIndicator);
        assert_eq!(node.label(), Some("Upload"));
        assert_eq!(node.numeric_value(), Some(25.0));
        assert_eq!(node.min_numeric_value(), Some(0.0));
        assert_eq!(node.max_numeric_value(), Some(100.0));
    }

    #[gpui::test]
    fn an_indeterminate_bar_reports_no_value(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(
                ProgressBar::new("Connecting")
                    .indeterminate()
                    .render(window, cx),
            )
        })
        .expect("reported");
        assert_eq!(node.numeric_value(), None);
    }
}
