//! UsageRing: one provider's usage as a ring around the provider's own mark.

use crate::components::agent_icon::{AgentIcon, AgentIconSize, AgentKind, NameTooltip};
use crate::components::progress::{arc_points, ring_in, spin_angle_now, stroke_polyline};
use crate::theme::ActiveTheme;
use crate::tokens::{
    BORDER_WIDTH_THICK, Color, DURATION_TOOLTIP_DELAY, Palette, SIZE_ICON_SM, SIZE_INDICATOR_DOT,
    SIZE_USAGE_RING,
};
use gpui::{
    App, AppContext, Bounds, Div, Hsla, InteractiveElement, IntoElement, ParentElement, Pixels,
    RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, canvas, div, px,
};

/// Usage from which the arc turns to the warning colour (design-system 2.12).
pub const WARN_PERCENT: f32 = 80.0;

/// Usage from which the arc turns to the limit colour (design-system 2.12).
pub const LIMIT_PERCENT: f32 = 95.0;

/// Degrees of arc per percent of usage.
const DEGREES_PER_PERCENT: f32 = 360.0 / 100.0;

/// Length of the loading arc, in degrees.
const LOADING_SWEEP: f32 = 90.0;

/// The provider slot a ring's arc takes, from 1 to 6, assigned by the core in order of first use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsageSlot(u8);

impl UsageSlot {
    /// The slot with this number, or `None` outside 1 to 6.
    pub fn new(index: u8) -> Option<Self> {
        (1..=6).contains(&index).then_some(Self(index))
    }

    /// The slot number, from 1 to 6.
    pub fn index(self) -> u8 {
        self.0
    }

    /// The arc colour of this slot.
    pub fn color(self, palette: &Palette) -> Color {
        let slots = [
            palette.usage.ring_1,
            palette.usage.ring_2,
            palette.usage.ring_3,
            palette.usage.ring_4,
            palette.usage.ring_5,
            palette.usage.ring_6,
        ];
        slots[usize::from(self.0 - 1)]
    }
}

impl Default for UsageSlot {
    fn default() -> Self {
        Self(1)
    }
}

/// How full the arc is, which picks its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum UsageLevel {
    /// Below the warning threshold: the provider's slot colour.
    Normal,
    /// From the warning threshold: the warning colour.
    Warn,
    /// From the limit threshold, and above 100 percent: the limit colour.
    Limit,
}

/// Clamps a percentage to 0 to 100; NaN counts as empty.
pub fn clamp_percent(percent: f32) -> f32 {
    if percent.is_nan() {
        0.0
    } else {
        percent.clamp(0.0, 100.0)
    }
}

/// The whole percent a reading shows, with no decimals.
fn whole_percent(percent: f32) -> u32 {
    clamp_percent(percent).round() as u32
}

/// The level of a percentage.
pub fn usage_level(percent: f32) -> UsageLevel {
    let percent = clamp_percent(percent);
    if percent >= LIMIT_PERCENT {
        UsageLevel::Limit
    } else if percent >= WARN_PERCENT {
        UsageLevel::Warn
    } else {
        UsageLevel::Normal
    }
}

/// The arc colour for a level: the slot colour, or the warning or limit colour.
pub fn arc_color(level: UsageLevel, slot: UsageSlot, palette: &Palette) -> Color {
    match level {
        UsageLevel::Normal => slot.color(palette),
        UsageLevel::Warn => palette.usage.ring_warn,
        UsageLevel::Limit => palette.usage.ring_limit,
    }
}

/// The level a change from `previous` to `next` crosses upward into, which is announced politely.
pub fn crossed_threshold(previous: Option<f32>, next: f32) -> Option<UsageLevel> {
    let before = previous.map_or(UsageLevel::Normal, usage_level);
    let after = usage_level(next);
    (after > before && after != UsageLevel::Normal).then_some(after)
}

/// The announcement for a threshold level (UsageRing.md copy).
pub fn announcement(level: UsageLevel) -> Option<String> {
    match level {
        UsageLevel::Normal => None,
        UsageLevel::Warn => Some(format!("Usage at {} percent", WARN_PERCENT as u32)),
        UsageLevel::Limit => Some(format!("Usage at {} percent", LIMIT_PERCENT as u32)),
    }
}

/// What the ring knows about a provider's usage right now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UsageReading {
    /// A reported percentage. Above 100 it draws a full arc in the limit colour.
    Percent(f32),
    /// Asked for and not back yet: a short arc turns.
    Loading,
    /// The provider reports no usage: track only.
    NotReported,
    /// The usage cannot be had: track only, and the mark is unavailable.
    Unavailable,
    /// The reading failed: track only, with a danger dot. The last value stays in the name.
    Failed { last: Option<f32> },
}

/// The accessible name and tooltip (UsageRing.md copy), such as "Claude usage 62 percent,
/// resets in 2 days".
pub fn usage_name(
    agent: &str,
    reading: UsageReading,
    resets_in: Option<&str>,
    last_known: Option<&str>,
) -> String {
    match reading {
        UsageReading::Percent(percent) => {
            let mut name = format!("{agent} usage {} percent", whole_percent(percent));
            if let Some(resets) = resets_in {
                name = format!("{name}, resets in {resets}");
            }
            if let Some(age) = last_known {
                name = format!("{name}, last known {age}");
            }
            name
        }
        UsageReading::Loading => format!("{agent} usage loading"),
        UsageReading::NotReported => "Usage not reported".to_owned(),
        UsageReading::Unavailable => "Usage unavailable".to_owned(),
        UsageReading::Failed { last: Some(last) } => format!(
            "Usage could not be read, last value {} percent",
            whole_percent(last)
        ),
        UsageReading::Failed { last: None } => "Usage could not be read".to_owned(),
    }
}

/// The arc of a ring: where it starts and how far it sweeps, in degrees from 12 o'clock, and
/// its colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UsageArc {
    pub start: f32,
    pub sweep: f32,
    pub color: Color,
}

/// The arc a reading draws, if any. A dimmed ring (its host is unreachable) draws its arc in the
/// tertiary text colour. `spin_start` is where the loading arc starts.
pub fn arc_for(
    reading: UsageReading,
    dimmed: bool,
    slot: UsageSlot,
    palette: &Palette,
    spin_start: f32,
) -> Option<UsageArc> {
    match reading {
        UsageReading::Percent(percent) => {
            let sweep = clamp_percent(percent) * DEGREES_PER_PERCENT;
            if sweep <= 0.0 {
                return None;
            }
            let color = if dimmed {
                palette.text.tertiary
            } else {
                arc_color(usage_level(percent), slot, palette)
            };
            Some(UsageArc {
                start: 0.0,
                sweep,
                color,
            })
        }
        UsageReading::Loading => Some(UsageArc {
            start: spin_start,
            sweep: LOADING_SWEEP,
            color: palette.text.tertiary,
        }),
        UsageReading::NotReported | UsageReading::Unavailable | UsageReading::Failed { .. } => None,
    }
}

/// The dot on the mark's top-right corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CornerDot {
    /// The usage is past the warning threshold.
    Warn,
    /// The usage is past the limit threshold.
    Limit,
    /// The reading failed.
    Danger,
}

/// The corner dot a reading shows. A dimmed ring shows none, because its value is stale.
pub fn corner_dot(reading: UsageReading, dimmed: bool) -> Option<CornerDot> {
    if dimmed {
        return None;
    }
    match reading {
        UsageReading::Percent(percent) => match usage_level(percent) {
            UsageLevel::Normal => None,
            UsageLevel::Warn => Some(CornerDot::Warn),
            UsageLevel::Limit => Some(CornerDot::Limit),
        },
        UsageReading::Failed { .. } => Some(CornerDot::Danger),
        UsageReading::Loading | UsageReading::NotReported | UsageReading::Unavailable => None,
    }
}

/// Top-left corner of a corner dot, centred on the mark's top-right corner.
fn dot_origin() -> (f32, f32) {
    let mark_offset = (SIZE_USAGE_RING - SIZE_ICON_SM) / 2.0;
    (
        mark_offset + SIZE_ICON_SM - SIZE_INDICATOR_DOT / 2.0,
        mark_offset - SIZE_INDICATOR_DOT / 2.0,
    )
}

fn paint_ring(window: &mut Window, bounds: Bounds<Pixels>, track: Hsla, arc: Option<UsageArc>) {
    let (centre, radius) = ring_in(bounds, BORDER_WIDTH_THICK);
    stroke_polyline(
        window,
        &arc_points(centre, radius, 0.0, 360.0),
        BORDER_WIDTH_THICK,
        track,
        true,
    );
    if let Some(arc) = arc {
        stroke_polyline(
            window,
            &arc_points(centre, radius, arc.start, arc.sweep),
            BORDER_WIDTH_THICK,
            Hsla::from(arc.color),
            arc.sweep >= 360.0,
        );
    }
}

/// A 32 px ring around a provider's mark, showing one provider's usage. It is a read-only meter.
#[derive(Debug, IntoElement)]
pub struct UsageRing {
    agent: AgentKind,
    reading: UsageReading,
    slot: UsageSlot,
    resets_in: Option<SharedString>,
    host_age: Option<SharedString>,
}

impl UsageRing {
    pub fn new(agent: AgentKind, reading: UsageReading) -> Self {
        Self {
            agent,
            reading,
            slot: UsageSlot::default(),
            resets_in: None,
            host_age: None,
        }
    }

    /// Sets the provider slot that picks the arc colour.
    pub fn slot(mut self, slot: UsageSlot) -> Self {
        self.slot = slot;
        self
    }

    /// Sets the reset time shown in the name, such as "2 days".
    pub fn resets_in(mut self, resets_in: impl Into<SharedString>) -> Self {
        self.resets_in = Some(resets_in.into());
        self
    }

    /// Marks the host as unreachable, with the age of the last known value, such as "5 min ago".
    pub fn disconnected(mut self, age: impl Into<SharedString>) -> Self {
        self.host_age = Some(age.into());
        self
    }
}

impl RenderOnce for UsageRing {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = cx.theme().palette;
        let dimmed = self.host_age.is_some();
        let loading = matches!(self.reading, UsageReading::Loading);
        if loading {
            window.request_animation_frame();
        }
        let spin_start = if loading { spin_angle_now() } else { 0.0 };
        let arc = arc_for(self.reading, dimmed, self.slot, &palette, spin_start);
        let track = Hsla::from(palette.usage.ring_track);
        let name = SharedString::from(usage_name(
            self.agent.label(),
            self.reading,
            self.resets_in.as_deref(),
            self.host_age.as_deref(),
        ));
        let value = match self.reading {
            UsageReading::Percent(percent) => Some(f64::from(clamp_percent(percent))),
            _ => None,
        };
        let mark_offset = (SIZE_USAGE_RING - SIZE_ICON_SM) / 2.0;
        let mut ring = div()
            .id(SharedString::from(format!(
                "usage-ring-{}",
                self.agent.label()
            )))
            .relative()
            .flex_none()
            .size(px(SIZE_USAGE_RING))
            .role(Role::ProgressIndicator)
            .aria_label(name.clone())
            .tooltip_show_delay(DURATION_TOOLTIP_DELAY)
            .tooltip(move |_, cx| cx.new(|_| NameTooltip(name.clone())).into())
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, (), window, _| paint_ring(window, bounds, track, arc),
                )
                .absolute()
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .top(px(mark_offset))
                    .left(px(mark_offset))
                    .child(AgentIcon::new(self.agent).size(AgentIconSize::Small)),
            );
        if let Some(value) = value {
            ring = ring.aria_numeric_value(value);
        }
        if let Some(dot) = corner_dot(self.reading, dimmed) {
            ring = ring.child(corner_dot_element(dot, &palette));
        }
        ring
    }
}

fn corner_dot_element(dot: CornerDot, palette: &Palette) -> Div {
    let color = match dot {
        CornerDot::Warn => palette.usage.ring_warn,
        CornerDot::Limit => palette.usage.ring_limit,
        CornerDot::Danger => palette.status.danger_solid,
    };
    let (left, top) = dot_origin();
    div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .size(px(SIZE_INDICATOR_DOT))
        .rounded_full()
        .bg(color)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-3,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn percentages_are_clamped_and_nan_counts_as_empty() {
        assert_eq!(clamp_percent(-1.0), 0.0);
        assert_eq!(clamp_percent(150.0), 100.0);
        assert_eq!(clamp_percent(f32::NAN), 0.0);
        assert_eq!(clamp_percent(62.0), 62.0);
    }

    #[test]
    fn the_warning_and_limit_thresholds_are_inclusive() {
        assert_eq!(usage_level(0.0), UsageLevel::Normal);
        assert_eq!(usage_level(79.9), UsageLevel::Normal);
        assert_eq!(usage_level(80.0), UsageLevel::Warn);
        assert_eq!(usage_level(94.9), UsageLevel::Warn);
        assert_eq!(usage_level(95.0), UsageLevel::Limit);
        assert_eq!(usage_level(120.0), UsageLevel::Limit);
        assert_eq!(usage_level(f32::NAN), UsageLevel::Normal);
    }

    #[test]
    fn slots_run_from_one_to_six() {
        assert_eq!(UsageSlot::new(0), None);
        assert_eq!(UsageSlot::new(7), None);
        for index in 1..=6 {
            assert_eq!(UsageSlot::new(index).map(UsageSlot::index), Some(index));
        }
        assert_eq!(UsageSlot::default().index(), 1);
    }

    #[test]
    fn each_slot_takes_its_provider_colour_and_levels_pick_warn_and_limit() {
        for palette in [Palette::DARK, Palette::LIGHT] {
            let slots = [
                palette.usage.ring_1,
                palette.usage.ring_2,
                palette.usage.ring_3,
                palette.usage.ring_4,
                palette.usage.ring_5,
                palette.usage.ring_6,
            ];
            for (index, expected) in slots.into_iter().enumerate() {
                let slot = UsageSlot::new(index as u8 + 1).expect("slots run from 1 to 6");
                assert_eq!(slot.color(&palette), expected);
            }
            let slot = UsageSlot::new(3).expect("slot 3 exists");
            assert_eq!(
                arc_color(UsageLevel::Normal, slot, &palette),
                palette.usage.ring_3
            );
            assert_eq!(
                arc_color(UsageLevel::Warn, slot, &palette),
                palette.usage.ring_warn
            );
            assert_eq!(
                arc_color(UsageLevel::Limit, slot, &palette),
                palette.usage.ring_limit
            );
        }
    }

    #[test]
    fn a_rise_across_a_threshold_is_announced_once() {
        assert_eq!(crossed_threshold(None, 50.0), None);
        assert_eq!(crossed_threshold(Some(70.0), 85.0), Some(UsageLevel::Warn));
        assert_eq!(crossed_threshold(Some(79.0), 80.0), Some(UsageLevel::Warn));
        assert_eq!(crossed_threshold(Some(85.0), 90.0), None);
        assert_eq!(crossed_threshold(Some(90.0), 96.0), Some(UsageLevel::Limit));
        assert_eq!(crossed_threshold(Some(96.0), 85.0), None);
        assert_eq!(
            announcement(UsageLevel::Warn).as_deref(),
            Some("Usage at 80 percent")
        );
        assert_eq!(
            announcement(UsageLevel::Limit).as_deref(),
            Some("Usage at 95 percent")
        );
        assert_eq!(announcement(UsageLevel::Normal), None);
    }

    #[test]
    fn the_name_carries_the_numbers_and_the_copy_of_each_state() {
        assert_eq!(
            usage_name("Claude", UsageReading::Percent(62.4), Some("2 days"), None),
            "Claude usage 62 percent, resets in 2 days"
        );
        assert_eq!(
            usage_name(
                "Claude",
                UsageReading::Percent(62.0),
                None,
                Some("5 min ago")
            ),
            "Claude usage 62 percent, last known 5 min ago"
        );
        assert_eq!(
            usage_name("Claude", UsageReading::Percent(150.0), None, None),
            "Claude usage 100 percent"
        );
        assert_eq!(
            usage_name("Claude", UsageReading::Loading, None, None),
            "Claude usage loading"
        );
        assert_eq!(
            usage_name("Claude", UsageReading::NotReported, None, None),
            "Usage not reported"
        );
        assert_eq!(
            usage_name("Claude", UsageReading::Unavailable, None, None),
            "Usage unavailable"
        );
        assert_eq!(
            usage_name(
                "Claude",
                UsageReading::Failed { last: Some(62.0) },
                None,
                None
            ),
            "Usage could not be read, last value 62 percent"
        );
        assert_eq!(
            usage_name("Claude", UsageReading::Failed { last: None }, None, None),
            "Usage could not be read"
        );
    }

    #[test]
    fn the_arc_starts_at_twelve_and_spans_the_percentage() {
        let palette = Palette::DARK;
        let slot = UsageSlot::default();
        let half = arc_for(UsageReading::Percent(50.0), false, slot, &palette, 0.0)
            .expect("50 percent draws an arc");
        assert_eq!(half.start, 0.0);
        assert_close(half.sweep, 180.0);
        let full = arc_for(UsageReading::Percent(150.0), false, slot, &palette, 0.0)
            .expect("over 100 percent draws a full arc");
        assert_close(full.sweep, 360.0);
        assert_eq!(full.color, palette.usage.ring_limit);
        assert_eq!(
            arc_for(UsageReading::Percent(0.0), false, slot, &palette, 0.0),
            None
        );
    }

    #[test]
    fn a_dimmed_ring_draws_its_arc_in_tertiary_text() {
        let palette = Palette::LIGHT;
        let arc = arc_for(
            UsageReading::Percent(62.0),
            true,
            UsageSlot::default(),
            &palette,
            0.0,
        )
        .expect("dimmed rings still draw their last value");
        assert_eq!(arc.color, palette.text.tertiary);
    }

    #[test]
    fn loading_draws_a_ninety_degree_arc_from_the_spinner_clock() {
        let palette = Palette::DARK;
        let arc = arc_for(
            UsageReading::Loading,
            false,
            UsageSlot::default(),
            &palette,
            200.0,
        )
        .expect("loading draws an arc");
        assert_eq!(arc.start, 200.0);
        assert_close(arc.sweep, 90.0);
        assert_eq!(arc.color, palette.text.tertiary);
    }

    #[test]
    fn states_without_a_value_draw_only_the_track() {
        let palette = Palette::DARK;
        let slot = UsageSlot::default();
        for reading in [
            UsageReading::NotReported,
            UsageReading::Unavailable,
            UsageReading::Failed { last: Some(62.0) },
        ] {
            assert_eq!(
                arc_for(reading, false, slot, &palette, 0.0),
                None,
                "{reading:?}"
            );
        }
    }

    #[test]
    fn corner_dots_mark_thresholds_and_failures_but_not_stale_values() {
        assert_eq!(corner_dot(UsageReading::Percent(62.0), false), None);
        assert_eq!(
            corner_dot(UsageReading::Percent(85.0), false),
            Some(CornerDot::Warn)
        );
        assert_eq!(
            corner_dot(UsageReading::Percent(97.0), false),
            Some(CornerDot::Limit)
        );
        assert_eq!(
            corner_dot(UsageReading::Failed { last: None }, false),
            Some(CornerDot::Danger)
        );
        assert_eq!(corner_dot(UsageReading::Percent(97.0), true), None);
    }

    #[test]
    fn the_corner_dot_sits_on_the_mark_corner() {
        let (left, top) = dot_origin();
        assert_close(left, 21.0);
        assert_close(top, 5.0);
    }
}
