//! The reasoning slider's stops: where each level sits on the bar, which stop a pointer or a key
//! selects, and what a screen reader hears. Offsets run up from the bottom of the bar, in px.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_and_last_stops_sit_14_px_inside_the_bar_ends() {
        assert_eq!(stop_offset(0, 2, 200.0), 14.0);
        assert_eq!(stop_offset(1, 2, 200.0), 186.0);
    }

    #[test]
    fn the_stops_between_the_ends_are_evenly_spaced() {
        let offsets: Vec<f32> = (0..5).map(|index| stop_offset(index, 5, 220.0)).collect();
        assert_eq!(offsets, [14.0, 62.0, 110.0, 158.0, 206.0]);
    }

    #[test]
    fn the_fill_reaches_14_px_past_the_selected_stop() {
        assert_eq!(fill_height(0, 2, 200.0), 28.0);
        assert_eq!(fill_height(1, 2, 200.0), 200.0, "the top fill reaches the bar's top edge");
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
            assert_eq!(nearest_stop(stop_offset(index, 5, 220.0), 220.0, 5), index);
        }
    }

    #[test]
    fn keys_raise_lower_and_jump_to_the_ends() {
        assert_eq!(key_target("up", 1, 5), Some(2));
        assert_eq!(key_target("right", 1, 5), Some(2));
        assert_eq!(key_target("down", 2, 5), Some(1));
        assert_eq!(key_target("left", 2, 5), Some(1));
        assert_eq!(key_target("up", 4, 5), Some(4), "the top level stays put");
        assert_eq!(key_target("down", 0, 5), Some(0), "the bottom level stays put");
        assert_eq!(key_target("home", 3, 5), Some(0));
        assert_eq!(key_target("end", 1, 5), Some(4));
        assert_eq!(key_target("pageup", 1, 5), Some(4));
        assert_eq!(key_target("pagedown", 3, 5), Some(0));
        assert_eq!(key_target("enter", 1, 5), None);
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
