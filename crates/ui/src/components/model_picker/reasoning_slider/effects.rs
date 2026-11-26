//! The reasoning slider's effects: the particles and the shimmer band that rise inside the track at the
//! top level, the rule that decides when a frame is requested, and the bounded move of the thumb and
//! the fill to a new stop. Nothing loops below the top level, and nothing loops while the popover is
//! closed.

#[cfg(test)]
use std::cell::Cell;
use std::f32::consts::TAU;
use std::sync::OnceLock;
use std::time::Instant;

use gpui::{Bounds, Corners, Pixels, Window, fill, hsla, point, px, size};

use crate::tokens::{
    DURATION_MODERATE, DURATION_SLIDER_MOVE, DURATION_SLIDER_SHIMMER, DURATION_SLOW,
};

/// The shimmer band's peak opacity: 55 percent white at its middle.
const SHIMMER_ALPHA: f32 = 0.55;

thread_local! {
    /// Frames the picker has asked for, counted in tests so they can check that a closed picker asks for none.
    #[cfg(test)]
    pub static FRAME_REQUESTS: Cell<usize> = const { Cell::new(0) };
}

/// How many particles rise inside the fill at the top level.
pub const PARTICLE_COUNT: usize = 46;

/// The height of the shimmer band that climbs the fill, in px.
pub const SHIMMER_BAND: f32 = 46.0;

/// The particles' horizontal span inside the 28 px track, in px.
const PARTICLE_LANE: (f32, f32) = (3.0, 25.0);

/// One particle inside the fill.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    /// Its place across the fill, in px from the fill's left edge.
    pub x: f32,
    /// Its starting height as a fraction of the fill's height.
    pub start: f32,
    /// Its radius, in px.
    pub radius: f32,
    /// How fast it rises, in px per second.
    pub rise: f32,
    /// How fast it twinkles, in radians per second.
    pub twinkle: f32,
    /// Where its twinkle starts, in radians.
    pub phase: f32,
}

/// The particles, the same on every build: a fixed-seed generator spreads them over the ranges the
/// spec gives (radius 0.7 to 2 px, rise 4 to 14 px a second, a twinkle cycle of 2.6 to 7.9 s).
pub fn particles() -> Vec<Particle> {
    let mut seed: u32 = 0x9E37_79B9;
    let mut unit = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed >> 8) as f32 / (1u32 << 24) as f32
    };
    let (left, right) = PARTICLE_LANE;
    (0..PARTICLE_COUNT)
        .map(|_| Particle {
            x: left + (right - left) * unit(),
            start: unit(),
            radius: 0.7 + 1.3 * unit(),
            rise: 4.0 + 10.0 * unit(),
            twinkle: 0.8 + 1.6 * unit(),
            phase: TAU * unit(),
        })
        .collect()
}

/// The particle's height in px from the bottom of a fill `fill` px tall, `elapsed` seconds in. It rises
/// and re-enters at the bottom when it leaves the top.
pub fn particle_offset(particle: &Particle, fill: f32, elapsed: f32) -> f32 {
    (particle.start * fill + particle.rise * elapsed).rem_euclid(fill.max(f32::EPSILON))
}

/// The particle's opacity: it twinkles between 35 and 100 percent.
pub fn particle_alpha(particle: &Particle, elapsed: f32) -> f32 {
    0.35 + 0.65 * (0.5 + 0.5 * (particle.twinkle * elapsed + particle.phase).sin())
}

/// The shimmer's period, `duration.slider-shimmer`, in seconds.
pub fn shimmer_period() -> f32 {
    DURATION_SLIDER_SHIMMER.as_secs_f32()
}

/// The band's bottom edge, in px from the bottom of a fill `fill` px tall. The band starts below the
/// fill and leaves above it once per period.
pub fn shimmer_offset(elapsed: f32, fill: f32) -> f32 {
    let progress = (elapsed / shimmer_period()).rem_euclid(1.0);
    progress * (fill + SHIMMER_BAND) - SHIMMER_BAND
}

/// The band under reduced motion: resting at the top of the fill.
pub fn held_shimmer_offset(fill: f32) -> f32 {
    fill - SHIMMER_BAND
}

/// Whether the top effects request frames: only while the popover is open, the top level is selected
/// and motion is not reduced.
pub fn effect_frames(open: bool, top: bool, reduced: bool) -> bool {
    open && top && !reduced
}

/// The bounded move of the thumb and the fill, `duration.slider-move`, in seconds.
pub fn move_seconds() -> f32 {
    DURATION_SLIDER_MOVE.as_secs_f32()
}

/// Whether the move to a new stop is still running `since` seconds after the change. Reduced motion
/// has no move.
pub fn move_running(since: f32, reduced: bool) -> bool {
    !reduced && since < move_seconds()
}

/// How far the move to a new stop has got, from 0 at the old stop to 1 at the new one, eased with
/// `ease.standard`. Reduced motion jumps straight to the new stop.
pub fn move_progress(since: f32, reduced: bool) -> f32 {
    if reduced {
        return 1.0;
    }
    ease_standard((since / move_seconds()).clamp(0.0, 1.0))
}

/// `ease.standard`, cubic-bezier(0.2, 0, 0, 1), at progress `t` in 0 to 1.
pub fn ease_standard(t: f32) -> f32 {
    cubic_bezier(0.2, 0.0, 0.0, 1.0, t)
}

/// The output of a cubic bezier with control points (`x1`, `y1`) and (`x2`, `y2`) at input `x`.
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let curve = |a: f32, b: f32, s: f32| {
        3.0 * (1.0 - s) * (1.0 - s) * s * a + 3.0 * (1.0 - s) * s * s * b + s * s * s
    };
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for _ in 0..24 {
        let mid = (low + high) / 2.0;
        if curve(x1, x2, mid) < x {
            low = mid;
        } else {
            high = mid;
        }
    }
    curve(y1, y2, (low + high) / 2.0)
}

static EPOCH: OnceLock<Instant> = OnceLock::new();

/// Seconds since the effects first read the clock. Reduced motion reads it as 0, so the top level holds
/// the same frame.
pub fn clock_seconds(reduced: bool) -> f32 {
    if reduced {
        return 0.0;
    }
    EPOCH.get_or_init(Instant::now).elapsed().as_secs_f32()
}

/// Asks the window for the next frame. The picker re-renders then, and the top effects advance.
pub fn request_frame(window: &mut Window) {
    #[cfg(test)]
    FRAME_REQUESTS.with(|count| count.set(count.get() + 1));
    window.request_animation_frame();
}

/// The halo's peak alpha at the bar's edge: the artboard's radial gradient is 30 percent strong there, and
/// the halo sits at 55 percent opacity.
pub const HALO_PEAK: f32 = 0.165;

/// The halo's rings: one per px it reaches past the bar.
pub const HALO_RINGS: usize = 6;

/// The halo's rings as (px past the bar, alpha). Stacked, they fall from `peak` at the bar's edge to
/// nothing `HALO_RINGS` px out, as the artboard's radial gradient does.
pub fn halo_rings(peak: f32) -> [(f32, f32); HALO_RINGS] {
    let target = |distance: f32| peak * (1.0 - distance / HALO_RINGS as f32).max(0.0);
    let mut rings = [(0.0, 0.0); HALO_RINGS];
    let mut clear_outside = 1.0;
    for index in (0..HALO_RINGS).rev() {
        let clear_inside = 1.0 - target(index as f32 + 0.5);
        rings[index] = (index as f32 + 1.0, 1.0 - clear_inside / clear_outside);
        clear_outside = clear_inside;
    }
    rings
}

/// How far an effect that began `since` seconds ago has faded in over `seconds`. Reduced motion shows
/// it at once.
pub fn fade_progress(since: f32, seconds: f32, reduced: bool) -> f32 {
    if reduced {
        return 1.0;
    }
    (since / seconds).clamp(0.0, 1.0)
}

/// The top effects' fade-in, `duration.moderate`, in seconds.
pub fn effects_fade_seconds() -> f32 {
    DURATION_MODERATE.as_secs_f32()
}

/// The halo's fade-in, `duration.slow`, in seconds.
pub fn halo_fade_seconds() -> f32 {
    DURATION_SLOW.as_secs_f32()
}

/// A track's outline: a rectangle whose two ends are half circles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pill {
    pub width: f32,
    pub height: f32,
}

impl Pill {
    /// The radius of the two ends, half the width.
    pub fn radius(&self) -> f32 {
        self.width.min(self.height) / 2.0
    }

    /// The outline's left and right edges in the row `y` px down from its top.
    pub fn span_at(&self, y: f32) -> (f32, f32) {
        let radius = self.radius();
        let depth = if y < radius {
            radius - y
        } else if y > self.height - radius {
            y - (self.height - radius)
        } else {
            return (0.0, self.width);
        };
        let half = (radius * radius - depth * depth).max(0.0).sqrt();
        (self.width / 2.0 - half, self.width / 2.0 + half)
    }

    /// The centre of a disc of radius `r` at (`x`, `y`), moved the least it takes to lie wholly inside.
    pub fn inset_centre(&self, x: f32, y: f32, r: f32) -> (f32, f32) {
        let radius = self.radius();
        let cap = if y < radius {
            radius
        } else if y > self.height - radius {
            self.height - radius
        } else {
            return (x.max(r).min(self.width - r), y);
        };
        let (dx, dy) = (x - self.width / 2.0, y - cap);
        let limit = (radius - r).max(0.0);
        let distance = (dx * dx + dy * dy).sqrt();
        if distance <= limit {
            (x, y)
        } else {
            (
                self.width / 2.0 + dx * limit / distance,
                cap + dy * limit / distance,
            )
        }
    }
}

/// One device pixel row of the shimmer band, from `top` to `bottom` px down the track, between `left`
/// and `right`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stripe {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
    pub alpha: f32,
}

/// The band's rows at `scale` device pixels per px. Each row is one device pixel tall and spans only the
/// outline at its height, so the band never paints past the rounded ends.
pub fn shimmer_stripes(pill: Pill, elapsed: f32, reduced: bool, scale: f32) -> Vec<Stripe> {
    let band_bottom = if reduced {
        held_shimmer_offset(pill.height)
    } else {
        shimmer_offset(elapsed, pill.height)
    };
    let band_top = pill.height - band_bottom - SHIMMER_BAND;
    let first = (band_top.max(0.0) * scale).floor() as i32;
    let last = ((band_top + SHIMMER_BAND).min(pill.height) * scale).ceil() as i32;
    (first..last)
        .filter_map(|row| {
            let top = row as f32 / scale;
            let bottom = ((row + 1) as f32 / scale).min(pill.height);
            let centre = (top + bottom) / 2.0;
            let alpha = band_alpha(centre, band_top);
            if alpha <= 0.0 || bottom <= top {
                return None;
            }
            let (left, right) = pill.span_at(centre);
            Some(Stripe {
                top,
                bottom,
                left,
                right,
                alpha,
            })
        })
        .collect()
}

/// The band's alpha at `y`: nothing at its top and bottom edges, `SHIMMER_ALPHA` at its middle.
fn band_alpha(y: f32, band_top: f32) -> f32 {
    let t = (y - band_top) / (SHIMMER_BAND / 2.0);
    if (0.0..=2.0).contains(&t) {
        SHIMMER_ALPHA * (1.0 - (t - 1.0).abs())
    } else {
        0.0
    }
}

/// The particle's centre at `elapsed`, pulled inside the rounded ends where it would cross them.
pub fn particle_centre(pill: Pill, particle: &Particle, elapsed: f32) -> (f32, f32) {
    let y = pill.height - particle_offset(particle, pill.height, elapsed);
    pill.inset_centre(particle.x, y, particle.radius)
}

/// Paints the top level's shimmer band and particles inside the rounded track `track`. Every row and
/// particle stays inside the outline, so no square corner shows at any frame. Under reduced motion the
/// clock is frozen at 0 and the band rests at the top of the track, so the frame holds still.
pub fn paint_top(
    track: Bounds<Pixels>,
    elapsed: f32,
    reduced: bool,
    fade: f32,
    window: &mut Window,
) {
    let pill = Pill {
        width: f32::from(track.size.width),
        height: f32::from(track.size.height),
    };
    let origin = track.origin;
    for stripe in shimmer_stripes(pill, elapsed, reduced, window.scale_factor()) {
        let at = Bounds::new(
            point(origin.x + px(stripe.left), origin.y + px(stripe.top)),
            size(
                px(stripe.right - stripe.left),
                px(stripe.bottom - stripe.top),
            ),
        );
        window.paint_quad(fill(at, hsla(0.0, 0.0, 1.0, stripe.alpha * fade)));
    }
    for particle in particles() {
        let (x, y) = particle_centre(pill, &particle, elapsed);
        let radius = particle.radius;
        let at = Bounds::new(
            point(origin.x + px(x - radius), origin.y + px(y - radius)),
            size(px(2.0 * radius), px(2.0 * radius)),
        );
        let alpha = particle_alpha(&particle, elapsed) * fade;
        window.paint_quad(
            fill(at, hsla(0.0, 0.0, 1.0, alpha)).corner_radii(Corners::all(px(radius))),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The track's outline at the reasoning column's size: 28 px wide and 220 px tall.
    fn outline() -> Pill {
        Pill {
            width: 28.0,
            height: 220.0,
        }
    }

    fn distance(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
        ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt()
    }

    #[test]
    fn the_track_ends_are_half_circles_as_wide_as_the_track() {
        let pill = outline();
        assert_eq!(pill.radius(), 14.0);
        assert_eq!(pill.span_at(110.0), (0.0, 28.0), "the sides run full width");
        let (left, right) = pill.span_at(0.5);
        assert!(
            left > 0.0 && right < 28.0,
            "the top row sits inside the rounded end"
        );
    }

    #[test]
    fn no_band_row_in_a_rounded_end_reaches_the_track_edge() {
        let pill = outline();
        for scale in [1.0, 1.25, 1.5] {
            for elapsed in [0.0, 0.4, 0.8, 1.2, 1.4316, 1.5999] {
                for stripe in shimmer_stripes(pill, elapsed, false, scale) {
                    let centre = (stripe.top + stripe.bottom) / 2.0;
                    if centre < pill.radius() || centre > pill.height - pill.radius() {
                        assert!(
                            stripe.left > 0.0 && stripe.right < pill.width,
                            "the row at {centre} px spans {}..{} at {scale}x: a square corner",
                            stripe.left,
                            stripe.right
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_held_frame_paints_no_square_corner() {
        let stripes = shimmer_stripes(outline(), 0.0, true, 1.0);
        assert!(
            !stripes.is_empty(),
            "the band rests at the top of the track"
        );
        for stripe in stripes.iter().filter(|stripe| stripe.top < 14.0) {
            assert!(
                stripe.left > 0.0 && stripe.right < 28.0,
                "the held band's top rows are rounded"
            );
        }
    }

    #[test]
    fn the_band_crossing_the_top_end_paints_no_square_corner() {
        let stripes = shimmer_stripes(outline(), 1.4316, false, 1.0);
        assert!(
            stripes.iter().any(|stripe| stripe.top < 14.0),
            "the band reaches the rounded top"
        );
        for stripe in stripes.iter().filter(|stripe| stripe.top < 14.0) {
            assert!(stripe.left > 0.0 && stripe.right < 28.0);
        }
    }

    #[test]
    fn band_rows_are_whole_device_pixels_so_neighbouring_rows_meet_without_seams() {
        for scale in [1.0, 1.25, 1.5] {
            for stripe in shimmer_stripes(outline(), 0.7, false, scale) {
                for edge in [stripe.top * scale, stripe.bottom * scale] {
                    assert!(
                        (edge - edge.round()).abs() < 1e-3,
                        "the edge at {edge} device px is not whole at {scale}x"
                    );
                }
            }
        }
    }

    #[test]
    fn a_particle_at_a_rounded_end_is_pulled_inside_the_track() {
        let pill = outline();
        let radius = pill.radius();
        for step in 0..200 {
            let elapsed = step as f32 * 0.07;
            for particle in particles() {
                let (x, y) = particle_centre(pill, &particle, elapsed);
                let r = particle.radius;
                let inside = if y < radius {
                    distance(x, y, radius, radius) <= radius - r + 1e-3
                } else if y > pill.height - radius {
                    distance(x, y, radius, pill.height - radius) <= radius - r + 1e-3
                } else {
                    x >= r - 1e-3 && x <= pill.width - r + 1e-3
                };
                assert!(
                    inside,
                    "a particle at ({x}, {y}) of radius {r} crosses the rounded end at {elapsed} s"
                );
            }
        }
    }

    #[test]
    fn the_top_level_has_46_particles_in_the_spec_ranges() {
        let particles = particles();
        assert_eq!(particles.len(), PARTICLE_COUNT);
        for particle in &particles {
            assert!(
                (0.7..=2.0).contains(&particle.radius),
                "radius {}",
                particle.radius
            );
            assert!(
                (4.0..=14.0).contains(&particle.rise),
                "rise {}",
                particle.rise
            );
            let cycle = TAU / particle.twinkle;
            assert!((2.6..=7.9).contains(&cycle), "cycle {cycle}");
            assert!((3.0..=25.0).contains(&particle.x), "x {}", particle.x);
        }
    }

    #[test]
    fn the_particles_are_the_same_on_every_build() {
        assert_eq!(particles(), particles());
    }

    #[test]
    fn a_particle_rises_and_re_enters_at_the_bottom() {
        let particle = Particle {
            x: 10.0,
            start: 0.0,
            radius: 1.0,
            rise: 10.0,
            twinkle: 1.0,
            phase: 0.0,
        };
        assert_eq!(particle_offset(&particle, 100.0, 0.0), 0.0);
        assert_eq!(particle_offset(&particle, 100.0, 3.0), 30.0);
        assert_eq!(
            particle_offset(&particle, 100.0, 10.5),
            5.0,
            "it leaves the top and comes back at the bottom"
        );
    }

    #[test]
    fn a_particle_twinkles_between_35_and_100_percent() {
        let particle = Particle {
            x: 10.0,
            start: 0.0,
            radius: 1.0,
            rise: 4.0,
            twinkle: 1.0,
            phase: 0.0,
        };
        let alphas: Vec<f32> = (0..200)
            .map(|step| particle_alpha(&particle, step as f32 * 0.05))
            .collect();
        let low = alphas.iter().copied().fold(f32::MAX, f32::min);
        let high = alphas.iter().copied().fold(f32::MIN, f32::max);
        assert!((low - 0.35).abs() < 0.01, "low {low}");
        assert!((high - 1.0).abs() < 0.01, "high {high}");
    }

    #[test]
    fn the_shimmer_band_climbs_the_fill_once_per_period() {
        assert_eq!(shimmer_period(), 1.6);
        assert_eq!(
            shimmer_offset(0.0, 200.0),
            -SHIMMER_BAND,
            "it starts below the fill"
        );
        assert_eq!(
            shimmer_offset(1.6, 200.0),
            -SHIMMER_BAND,
            "one period later it starts again"
        );
        assert!(
            (shimmer_offset(0.8, 200.0) - 77.0).abs() < 0.01,
            "halfway up the fill"
        );
        assert!(
            shimmer_offset(1.5, 200.0) > 200.0 - SHIMMER_BAND,
            "it nearly leaves the top"
        );
    }

    #[test]
    fn a_held_shimmer_rests_at_the_top_of_the_fill() {
        assert_eq!(
            held_shimmer_offset(200.0),
            154.0,
            "its top edge is the fill's top edge"
        );
    }

    #[test]
    fn frames_are_requested_only_while_open_on_the_top_level_and_not_reduced() {
        assert!(effect_frames(true, true, false));
        assert!(!effect_frames(false, true, false), "closed: no frames");
        assert!(
            !effect_frames(true, false, false),
            "below the top: no frames"
        );
        assert!(
            !effect_frames(true, true, true),
            "reduced motion holds still"
        );
    }

    #[test]
    fn the_move_is_bounded_to_its_220_ms_token() {
        assert!((move_seconds() - 0.22).abs() < 1e-6);
        assert!(move_running(0.1, false));
        assert!(!move_running(0.22, false), "the move ends with its token");
        assert!(!move_running(0.0, true), "reduced motion has no move");
    }

    #[test]
    fn the_move_eases_out_from_the_old_stop_to_the_new_one() {
        assert_eq!(move_progress(0.0, false), 0.0);
        assert!((move_progress(move_seconds(), false) - 1.0).abs() < 1e-3);
        assert!(
            move_progress(move_seconds() / 2.0, false) > 0.5,
            "ease.standard starts fast"
        );
        assert_eq!(
            move_progress(0.0, true),
            1.0,
            "reduced motion jumps to the new stop"
        );
    }

    #[test]
    fn the_effects_fade_in_over_200_ms_and_the_halo_over_300_ms() {
        assert!((effects_fade_seconds() - 0.2).abs() < 1e-6);
        assert!((halo_fade_seconds() - 0.3).abs() < 1e-6);
        assert_eq!(fade_progress(0.0, effects_fade_seconds(), false), 0.0);
        assert!((fade_progress(0.1, effects_fade_seconds(), false) - 0.5).abs() < 1e-6);
        assert!((fade_progress(0.15, halo_fade_seconds(), false) - 0.5).abs() < 1e-6);
        assert_eq!(
            fade_progress(1.0, halo_fade_seconds(), false),
            1.0,
            "it stays shown once faded in"
        );
    }

    #[test]
    fn reduced_motion_shows_the_top_effects_at_once() {
        assert_eq!(fade_progress(0.0, effects_fade_seconds(), true), 1.0);
        assert_eq!(fade_progress(0.0, halo_fade_seconds(), true), 1.0);
    }

    #[test]
    fn the_halo_rings_stack_into_the_radial_falloff_from_the_bar_to_nothing() {
        let rings = halo_rings(HALO_PEAK);
        for band in 0..HALO_RINGS {
            let stacked = 1.0
                - rings[band..]
                    .iter()
                    .map(|(_, alpha)| 1.0 - alpha)
                    .product::<f32>();
            let falloff = HALO_PEAK * (1.0 - (band as f32 + 0.5) / HALO_RINGS as f32);
            assert!(
                (stacked - falloff).abs() < 1e-5,
                "{band} px out: stacked {stacked} against the falloff {falloff}"
            );
        }
        assert_eq!(
            rings[HALO_RINGS - 1].0,
            HALO_RINGS as f32,
            "the outermost ring reaches 6 px out"
        );
        assert!(rings.iter().all(|(_, alpha)| (0.0..=1.0).contains(alpha)));
    }
}
