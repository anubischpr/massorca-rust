//! The reasoning slider's effects: the particles and the shimmer band that rise inside the fill at the
//! top level, the rule that decides when a frame is requested, and the bounded move of the thumb and
//! the fill to a new stop. Nothing loops below the top level, and nothing loops while the popover is
//! closed.

#[cfg(test)]
use std::cell::Cell;
use std::f32::consts::TAU;
use std::sync::OnceLock;
use std::time::Instant;

use gpui::{
    Bounds, Corners, Pixels, Window, fill, hsla, linear_color_stop, linear_gradient, point, px, size,
};

use crate::tokens::{DURATION_SLIDER_MOVE, DURATION_SLIDER_SHIMMER};

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

/// Paints the top level's particles and shimmer band into the fill `bounds`. Under reduced motion the clock is
/// frozen at 0 and the band rests at the top of the fill, so the frame holds still.
pub fn paint_top(bounds: Bounds<Pixels>, elapsed: f32, reduced: bool, window: &mut Window) {
    let height = f32::from(bounds.size.height);
    let width = f32::from(bounds.size.width);
    let origin = bounds.origin;
    let band_bottom = if reduced {
        held_shimmer_offset(height)
    } else {
        shimmer_offset(elapsed, height)
    };
    let band_top = height - band_bottom - SHIMMER_BAND;
    let half = SHIMMER_BAND / 2.0;
    let white = hsla(0.0, 0.0, 1.0, SHIMMER_ALPHA);
    let clear = hsla(0.0, 0.0, 1.0, 0.0);
    let lower = Bounds::new(
        point(origin.x, origin.y + px(band_top + half)),
        size(px(width), px(half)),
    );
    let upper = Bounds::new(
        point(origin.x, origin.y + px(band_top)),
        size(px(width), px(half)),
    );
    window.paint_quad(fill(
        lower,
        linear_gradient(0., linear_color_stop(clear, 0.), linear_color_stop(white, 1.)),
    ));
    window.paint_quad(fill(
        upper,
        linear_gradient(0., linear_color_stop(white, 0.), linear_color_stop(clear, 1.)),
    ));
    for particle in particles() {
        let centre_y = height - particle_offset(&particle, height, elapsed);
        let radius = particle.radius;
        let at = Bounds::new(
            point(origin.x + px(particle.x - radius), origin.y + px(centre_y - radius)),
            size(px(2.0 * radius), px(2.0 * radius)),
        );
        let alpha = particle_alpha(&particle, elapsed);
        window.paint_quad(
            fill(at, hsla(0.0, 0.0, 1.0, alpha)).corner_radii(Corners::all(px(radius))),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_top_level_has_46_particles_in_the_spec_ranges() {
        let particles = particles();
        assert_eq!(particles.len(), PARTICLE_COUNT);
        for particle in &particles {
            assert!((0.7..=2.0).contains(&particle.radius), "radius {}", particle.radius);
            assert!((4.0..=14.0).contains(&particle.rise), "rise {}", particle.rise);
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
        assert_eq!(shimmer_offset(0.0, 200.0), -SHIMMER_BAND, "it starts below the fill");
        assert_eq!(shimmer_offset(1.6, 200.0), -SHIMMER_BAND, "one period later it starts again");
        assert!((shimmer_offset(0.8, 200.0) - 77.0).abs() < 0.01, "halfway up the fill");
        assert!(shimmer_offset(1.5, 200.0) > 200.0 - SHIMMER_BAND, "it nearly leaves the top");
    }

    #[test]
    fn a_held_shimmer_rests_at_the_top_of_the_fill() {
        assert_eq!(held_shimmer_offset(200.0), 154.0, "its top edge is the fill's top edge");
    }

    #[test]
    fn frames_are_requested_only_while_open_on_the_top_level_and_not_reduced() {
        assert!(effect_frames(true, true, false));
        assert!(!effect_frames(false, true, false), "closed: no frames");
        assert!(!effect_frames(true, false, false), "below the top: no frames");
        assert!(!effect_frames(true, true, true), "reduced motion holds still");
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
        assert!(move_progress(move_seconds() / 2.0, false) > 0.5, "ease.standard starts fast");
        assert_eq!(move_progress(0.0, true), 1.0, "reduced motion jumps to the new stop");
    }
}
