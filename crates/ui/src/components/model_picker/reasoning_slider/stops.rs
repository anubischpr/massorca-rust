//! The reasoning slider's stops: where each level sits on the bar, which level a key or a pointer
//! selects, and what a screen reader hears. Offsets run up from the bottom of the bar, in px.

/// The distance from each end of the bar to its first and last stop, in px.
pub const STOP_INSET: f32 = 14.0;

/// The same stop as a fraction of the stop span, 0 at the bottom stop and 1 at the top one. The
/// stops are positioned with it, so they follow the real height of the bar.
pub fn stop_fraction(index: usize, count: usize) -> f32 {
    index as f32 / count.saturating_sub(1).max(1) as f32
}

/// The stop nearest to `offset` px from the bottom of a bar `height` px tall; a pointer past an end
/// selects that end.
pub fn nearest_stop(offset: f32, height: f32, count: usize) -> usize {
    let Some(last) = count.checked_sub(1).filter(|last| *last > 0) else {
        return 0;
    };
    let span = height - 2.0 * STOP_INSET;
    if span <= 0.0 {
        return 0;
    }
    let step = span / last as f32;
    let index = ((offset - STOP_INSET) / step).round();
    index.clamp(0.0, last as f32) as usize
}

/// A move the keys and the actions make on the slider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slide {
    /// One level up (Up, Right).
    Raise,
    /// One level down (Down, Left).
    Lower,
    /// The bottom level (Home, Page Down).
    First,
    /// The top level (End, Page Up).
    Last,
}

impl Slide {
    /// The level `index` becomes after this move; a move past an end stays at it.
    pub fn apply(self, index: usize, count: usize) -> usize {
        let last = count.saturating_sub(1);
        match self {
            Slide::Raise => (index + 1).min(last),
            Slide::Lower => index.saturating_sub(1),
            Slide::First => 0,
            Slide::Last => last,
        }
    }
}

/// The value a screen reader announces: the level's name and its place, as in "Medium, 2 of 5".
pub fn value_text(label: &str, index: usize, count: usize) -> String {
    format!("{label}, {} of {count}", index + 1)
}

/// A model with fewer than two levels has no slider: it shows the "no reasoning levels" note.
pub fn has_slider(levels: usize) -> bool {
    levels >= 2
}

/// Whether `index` is the top level of a slider, the one that shows the top effects.
pub fn is_top_level(index: usize, count: usize) -> bool {
    has_slider(count) && index + 1 == count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_fractions_run_from_the_bottom_stop_to_the_top_stop() {
        assert_eq!(stop_fraction(0, 5), 0.0);
        assert_eq!(stop_fraction(2, 5), 0.5);
        assert_eq!(stop_fraction(4, 5), 1.0);
        assert_eq!(stop_fraction(0, 1), 0.0);
    }

    #[test]
    fn a_pointer_snaps_to_the_nearest_stop() {
        assert_eq!(nearest_stop(70.0, 220.0, 5), 1);
        assert_eq!(nearest_stop(90.0, 220.0, 5), 2);
        assert_eq!(nearest_stop(-10.0, 220.0, 5), 0, "below the bar snaps to the first stop");
        assert_eq!(nearest_stop(500.0, 220.0, 5), 4, "above the bar snaps to the last stop");
    }

    #[test]
    fn every_stop_selects_itself() {
        for index in 0..5 {
            let offset = STOP_INSET + (220.0 - 2.0 * STOP_INSET) * stop_fraction(index, 5);
            assert_eq!(nearest_stop(offset, 220.0, 5), index);
        }
    }

    #[test]
    fn slides_move_one_level_and_jump_to_the_ends() {
        assert_eq!(Slide::Raise.apply(1, 5), 2);
        assert_eq!(Slide::Raise.apply(4, 5), 4, "the top level stays put");
        assert_eq!(Slide::Lower.apply(2, 5), 1);
        assert_eq!(Slide::Lower.apply(0, 5), 0, "the bottom level stays put");
        assert_eq!(Slide::First.apply(3, 5), 0);
        assert_eq!(Slide::Last.apply(1, 5), 4);
    }

    #[test]
    fn the_value_text_names_the_level_and_its_place() {
        assert_eq!(value_text("Medium", 1, 5), "Medium, 2 of 5");
        assert_eq!(value_text("Max", 1, 2), "Max, 2 of 2");
    }

    #[test]
    fn a_slider_needs_two_or_more_levels() {
        assert!(!has_slider(0));
        assert!(!has_slider(1));
        assert!(has_slider(2));
        assert!(has_slider(5));
    }

    #[test]
    fn only_the_last_level_of_a_slider_is_the_top_level() {
        assert!(!is_top_level(0, 5));
        assert!(is_top_level(4, 5));
        assert!(!is_top_level(0, 1), "a single level has no slider and no top effects");
    }
}
