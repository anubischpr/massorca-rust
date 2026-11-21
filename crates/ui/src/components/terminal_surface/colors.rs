//! Terminal colours as design tokens: the sixteen ANSI slots from the palette, the
//! xterm 256-colour cube and grey ramp beyond them, and truecolour as given.

use crate::tokens::{Color, TerminalColors};
use alacritty_terminal::term::color::Colors as Overrides;
use alacritty_terminal::vte::ansi::{Color as ProgramColor, NamedColor};

/// The colour of ANSI slot `index` (0 to 15) from the palette.
pub fn ansi(palette: &TerminalColors, index: u8) -> Color {
    let slots = [
        palette.ansi_black,
        palette.ansi_red,
        palette.ansi_green,
        palette.ansi_yellow,
        palette.ansi_blue,
        palette.ansi_magenta,
        palette.ansi_cyan,
        palette.ansi_white,
        palette.ansi_bright_black,
        palette.ansi_bright_red,
        palette.ansi_bright_green,
        palette.ansi_bright_yellow,
        palette.ansi_bright_blue,
        palette.ansi_bright_magenta,
        palette.ansi_bright_cyan,
        palette.ansi_bright_white,
    ];
    slots[usize::from(index).min(slots.len() - 1)]
}

/// Colour `index` of xterm's 256-colour table from 16 up: the 6 x 6 x 6 cube, then the
/// grey ramp. Indices below 16 are the palette slots, which `ansi` draws instead.
pub fn indexed(index: u8) -> Color {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    match index {
        232.. => {
            let grey = 8 + 10 * (index - 232);
            Color::rgba8(grey, grey, grey, 1.0)
        }
        _ => {
            let cube = usize::from(index.saturating_sub(16));
            let (red, green, blue) = (cube / 36, (cube / 6) % 6, cube % 6);
            Color::rgba8(LEVELS[red], LEVELS[green], LEVELS[blue], 1.0)
        }
    }
}

/// Blends `from` towards `to` by `amount` (0 keeps `from`, 1 gives `to`).
pub fn mix(from: Color, to: Color, amount: f32) -> Color {
    let blend = |a: f32, b: f32| a + (b - a) * amount;
    Color {
        r: blend(from.r, to.r),
        g: blend(from.g, to.g),
        b: blend(from.b, to.b),
        a: from.a,
    }
}

/// The colour a program's cell colour is drawn with. A palette override the program
/// set (OSC 4 and the like) wins over the default for the same slot.
pub fn resolve(palette: &TerminalColors, color: ProgramColor, overrides: &Overrides) -> Color {
    match color {
        ProgramColor::Spec(rgb) => Color::rgba8(rgb.r, rgb.g, rgb.b, 1.0),
        ProgramColor::Indexed(index) => match overrides[usize::from(index)] {
            Some(rgb) => Color::rgba8(rgb.r, rgb.g, rgb.b, 1.0),
            None if index < 16 => ansi(palette, index),
            None => indexed(index),
        },
        ProgramColor::Named(named) => match overrides[named] {
            Some(rgb) => Color::rgba8(rgb.r, rgb.g, rgb.b, 1.0),
            None => named_default(palette, named),
        },
    }
}

/// The palette colour of a named slot: the ANSI 16, the default foreground, background
/// and cursor, and the dim variants, which are the ANSI slots mixed towards the background.
fn named_default(palette: &TerminalColors, named: NamedColor) -> Color {
    let code = named as usize;
    match code {
        0..=15 => ansi(palette, code as u8),
        259..=266 => mix(ansi(palette, (code - 259) as u8), palette.bg, 0.5),
        267 => palette.fg,
        268 => mix(palette.fg, palette.bg, 0.5),
        256 => palette.fg,
        257 => palette.bg,
        258 => palette.cursor,
        _ => palette.fg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::Palette;
    use alacritty_terminal::vte::ansi::Rgb;

    fn assert_close(actual: Color, expected: Color) {
        for (a, e) in [
            (actual.r, expected.r),
            (actual.g, expected.g),
            (actual.b, expected.b),
            (actual.a, expected.a),
        ] {
            assert!((a - e).abs() < 1e-5, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn ansi_slots_come_from_the_palette() {
        let palette = Palette::DARK.terminal;
        assert_eq!(ansi(&palette, 1), palette.ansi_red);
        assert_eq!(ansi(&palette, 8), palette.ansi_bright_black);
        assert_eq!(ansi(&palette, 15), palette.ansi_bright_white);
    }

    #[test]
    fn the_cube_and_grey_ramp_follow_xterm() {
        assert_eq!(indexed(16), Color::rgba8(0, 0, 0, 1.0));
        assert_eq!(indexed(196), Color::rgba8(255, 0, 0, 1.0));
        assert_eq!(indexed(231), Color::rgba8(255, 255, 255, 1.0));
        assert_eq!(indexed(232), Color::rgba8(8, 8, 8, 1.0));
        assert_eq!(indexed(255), Color::rgba8(238, 238, 238, 1.0));
    }

    #[test]
    fn named_slots_resolve_to_the_theme_foreground_background_and_cursor() {
        let palette = Palette::DARK.terminal;
        let none = Overrides::default();
        let resolved = |named| resolve(&palette, ProgramColor::Named(named), &none);
        assert_eq!(resolved(NamedColor::Foreground), palette.fg);
        assert_eq!(resolved(NamedColor::Background), palette.bg);
        assert_eq!(resolved(NamedColor::Cursor), palette.cursor);
        assert_eq!(resolved(NamedColor::Green), palette.ansi_green);
    }

    #[test]
    fn truecolour_is_drawn_as_given() {
        let palette = Palette::DARK.terminal;
        let none = Overrides::default();
        let rgb = Rgb { r: 1, g: 2, b: 3 };
        assert_eq!(
            resolve(&palette, ProgramColor::Spec(rgb), &none),
            Color::rgba8(1, 2, 3, 1.0)
        );
    }

    #[test]
    fn a_program_palette_override_wins_over_the_default() {
        let palette = Palette::DARK.terminal;
        let mut overrides = Overrides::default();
        overrides[1] = Some(Rgb { r: 9, g: 8, b: 7 });
        let expected = Color::rgba8(9, 8, 7, 1.0);
        assert_eq!(
            resolve(&palette, ProgramColor::Named(NamedColor::Red), &overrides),
            expected
        );
        assert_eq!(
            resolve(&palette, ProgramColor::Indexed(1), &overrides),
            expected
        );
    }

    #[test]
    fn mix_blends_towards_the_other_colour() {
        let white = Color::rgba8(255, 255, 255, 1.0);
        let black = Color::rgba8(0, 0, 0, 1.0);
        let half = Color {
            r: 0.5,
            g: 0.5,
            b: 0.5,
            a: 1.0,
        };
        assert_close(mix(white, black, 0.5), half);
        assert_eq!(mix(white, black, 0.0), white);
    }
}
