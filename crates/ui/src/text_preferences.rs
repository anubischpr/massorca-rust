//! The text preferences of the Settings store (SET-01): the text size, the interface and monospace families, and the
//! terminal's family and scrollback. The shell sets them on connect and on every change; the text styles and the
//! terminal surfaces read them as they draw. Each thread keeps its own copy, so the UI thread and parallel tests never
//! share one.

use std::cell::RefCell;

use gpui::SharedString;

use crate::assets::{FONT_MONO, FONT_SANS};

/// The text size range the settings allow, in percent of the design size.
pub const SIZE_PERCENT_MIN: u16 = 90;
pub const SIZE_PERCENT_MAX: u16 = 130;
/// The scrollback range the settings allow, in rows.
pub const SCROLLBACK_MIN: usize = 100;
pub const SCROLLBACK_MAX: usize = 100_000;

/// The smallest text the design allows on desktop, in pixels; a scaled style never goes below it.
const MIN_TEXT_PX: f32 = 11.0;
/// The scrollback a terminal keeps until a setting says otherwise (the fidelity report's default).
const DEFAULT_SCROLLBACK_LINES: usize = 1000;

/// What the text is drawn with.
#[derive(Clone, Debug, PartialEq)]
pub struct TextPreferences {
    /// Text size in percent of the design size; 100 is the design.
    pub size_percent: u16,
    pub interface_family: SharedString,
    pub monospace_family: SharedString,
    pub terminal_family: SharedString,
    /// Rows of scrollback each terminal keeps.
    pub scrollback_lines: usize,
}

impl Default for TextPreferences {
    fn default() -> Self {
        Self {
            size_percent: 100,
            interface_family: FONT_SANS.into(),
            monospace_family: FONT_MONO.into(),
            terminal_family: FONT_MONO.into(),
            scrollback_lines: DEFAULT_SCROLLBACK_LINES,
        }
    }
}

thread_local! {
    static CURRENT: RefCell<TextPreferences> = RefCell::new(TextPreferences::default());
}

/// The preferences the calling thread draws with.
pub fn current() -> TextPreferences {
    CURRENT.with(|current| current.borrow().clone())
}

/// Replaces the preferences the calling thread draws with; the next draw uses them.
pub fn set(preferences: TextPreferences) {
    CURRENT.with(|current| *current.borrow_mut() = preferences);
}

pub fn interface_family() -> SharedString {
    CURRENT.with(|current| current.borrow().interface_family.clone())
}

pub fn monospace_family() -> SharedString {
    CURRENT.with(|current| current.borrow().monospace_family.clone())
}

pub fn terminal_family() -> SharedString {
    CURRENT.with(|current| current.borrow().terminal_family.clone())
}

pub fn scrollback_lines() -> usize {
    CURRENT.with(|current| current.borrow().scrollback_lines)
}

/// A design size in pixels at the current text size. A style below the minimum is never raised to it, and no style
/// shrinks under it.
pub fn scale_size(size: f32) -> f32 {
    let percent = CURRENT.with(|current| current.borrow().size_percent);
    if percent == 100 {
        return size;
    }
    (size * f32::from(percent) / 100.0).max(MIN_TEXT_PX.min(size))
}

/// A design line height in pixels at the current text size.
pub fn scale_line_height(height: f32) -> f32 {
    let percent = CURRENT.with(|current| current.borrow().size_percent);
    height * f32::from(percent) / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f32, expected: f32) -> bool {
        (actual - expected).abs() < 1e-4
    }

    #[test]
    fn the_design_sizes_hold_until_a_size_is_set() {
        set(TextPreferences::default());
        assert_eq!(scale_size(13.0), 13.0);
        assert_eq!(scale_line_height(20.0), 20.0);
    }

    #[test]
    fn a_size_percent_scales_each_style_and_no_style_drops_under_the_minimum() {
        set(TextPreferences {
            size_percent: 110,
            ..TextPreferences::default()
        });
        assert!(close(scale_size(13.0), 14.3));
        assert!(close(scale_line_height(20.0), 22.0));

        set(TextPreferences {
            size_percent: 90,
            ..TextPreferences::default()
        });
        assert!(close(scale_size(12.0), 11.0), "10.8 rises to the minimum");
        assert!(
            close(scale_size(10.0), 10.0),
            "a style under the minimum keeps its size"
        );
        set(TextPreferences::default());
    }

    #[test]
    fn the_families_and_scrollback_are_the_ones_set() {
        set(TextPreferences {
            interface_family: "Arial".into(),
            scrollback_lines: 250,
            ..TextPreferences::default()
        });
        assert_eq!(interface_family().as_ref(), "Arial");
        assert_eq!(monospace_family().as_ref(), FONT_MONO);
        assert_eq!(scrollback_lines(), 250);
        set(TextPreferences::default());
    }

    #[test]
    fn a_preference_set_on_one_thread_is_not_seen_on_another() {
        set(TextPreferences {
            size_percent: 120,
            ..TextPreferences::default()
        });
        let other = std::thread::spawn(|| current().size_percent)
            .join()
            .expect("the thread ends");
        assert_eq!(other, 100);
        assert_eq!(current().size_percent, 120);
        set(TextPreferences::default());
    }
}
