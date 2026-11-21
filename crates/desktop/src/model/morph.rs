//! The mode switch morph: the outgoing and incoming icons cross-fade with a scale and a turn
//! (D-SH-10, handoff 2.6 and 2.7). Each icon's frame is a pure function of the progress.

use massorca_ui::tokens::Easing;

/// The scale of the outgoing and the incoming icon at the ends of the morph.
pub const MORPH_SCALE: f32 = 0.6;
/// The turn of the outgoing (negative) and the incoming (positive) icon, in degrees.
pub const MORPH_ANGLE_DEG: f32 = 30.0;
/// The share of the morph in which both icons are visible.
pub const MORPH_OVERLAP_PERCENT: f32 = 20.0;

/// The share of the morph each icon takes to fade: two spans, less the overlap, cover it all.
const SPAN: f32 = (1.0 + MORPH_OVERLAP_PERCENT / 100.0) / 2.0;

/// One icon of the morph at one moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MorphFrame {
    pub scale: f32,
    pub angle_deg: f32,
    pub alpha: f32,
}

/// The outgoing icon at progress `t` (0 to 1): it shrinks, turns back and fades out.
pub fn outgoing(t: f32) -> MorphFrame {
    let p = (t / SPAN).clamp(0.0, 1.0);
    MorphFrame {
        scale: 1.0 + (MORPH_SCALE - 1.0) * p,
        angle_deg: -MORPH_ANGLE_DEG * p,
        alpha: 1.0 - p,
    }
}

/// The incoming icon at progress `t`: it grows back from the small, turned state and fades in.
pub fn incoming(t: f32) -> MorphFrame {
    let q = ((t - (1.0 - SPAN)) / SPAN).clamp(0.0, 1.0);
    MorphFrame {
        scale: MORPH_SCALE + (1.0 - MORPH_SCALE) * q,
        angle_deg: MORPH_ANGLE_DEG * (1.0 - q),
        alpha: q,
    }
}

/// Maps linear progress through a motion curve (handoff 2.6).
pub fn ease(curve: Easing, t: f32) -> f32 {
    match curve {
        Easing::Linear => t,
        Easing::CubicBezier(x1, y1, x2, y2) => cubic_bezier(x1, y1, x2, y2, t),
    }
}

/// The y of the cubic bezier from (0, 0) to (1, 1) at the x = `x`, found by bisection.
fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for _ in 0..60 {
        let mid = (low + high) / 2.0;
        if bezier(x1, x2, mid) < x {
            low = mid;
        } else {
            high = mid;
        }
    }
    bezier(y1, y2, (low + high) / 2.0)
}

fn bezier(p1: f32, p2: f32, s: f32) -> f32 {
    let inv = 1.0 - s;
    3.0 * inv * inv * s * p1 + 3.0 * inv * s * s * p2 + s * s * s
}

#[cfg(test)]
mod tests {
    use super::*;
    use massorca_ui::tokens::EASE_STANDARD;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    #[test]
    fn the_morph_starts_on_the_outgoing_icon_and_ends_on_the_incoming_one() {
        let out = outgoing(0.0);
        assert!(close(out.scale, 1.0) && close(out.angle_deg, 0.0) && close(out.alpha, 1.0));
        let incoming_start = incoming(0.0);
        assert!(close(incoming_start.alpha, 0.0));
        assert!(close(incoming_start.scale, MORPH_SCALE));
        assert!(close(outgoing(1.0).alpha, 0.0));
        let end = incoming(1.0);
        assert!(close(end.scale, 1.0) && close(end.angle_deg, 0.0) && close(end.alpha, 1.0));
    }

    #[test]
    fn both_icons_are_visible_only_in_the_configured_overlap() {
        assert!(outgoing(0.5).alpha > 0.0 && incoming(0.5).alpha > 0.0);
        assert!(close(incoming(0.3).alpha, 0.0));
        assert!(close(outgoing(0.7).alpha, 0.0));
        assert!(close(2.0 * SPAN - 1.0, MORPH_OVERLAP_PERCENT / 100.0));
    }

    #[test]
    fn standard_easing_keeps_both_ends_and_moves_quickly_at_first() {
        assert!(close(ease(EASE_STANDARD, 0.0), 0.0));
        assert!(close(ease(EASE_STANDARD, 1.0), 1.0));
        assert!(ease(EASE_STANDARD, 0.5) > 0.5);
    }

    #[test]
    fn linear_easing_is_the_identity() {
        assert!(close(ease(Easing::Linear, 0.25), 0.25));
    }
}
