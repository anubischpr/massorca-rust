//! The pairing offer as a QR code in the terminal (SRV-06): half-block characters, two module rows per line.

use qrcode::{Color, QrCode};

/// Light modules around the code. Four is the standard; two keeps a long offer within 80 columns and still scans.
const QUIET: usize = 2;

/// The QR code of `text`, one string per terminal line, or `None` when the text is too long for a QR code. Light modules
/// are drawn as blocks, so on a dark terminal the code reads dark on light like a printed one.
pub fn lines(text: &str) -> Option<Vec<String>> {
    let code = QrCode::new(text.as_bytes()).ok()?;
    let width = code.width();
    let colors = code.to_colors();
    let size = width + 2 * QUIET;
    let light = |x: usize, y: usize| -> bool {
        if x < QUIET || y < QUIET || x >= width + QUIET || y >= width + QUIET {
            return true;
        }
        colors
            .get((y - QUIET) * width + (x - QUIET))
            .is_none_or(|color| *color == Color::Light)
    };
    let mut out = Vec::with_capacity(size.div_ceil(2));
    for y in (0..size).step_by(2) {
        let line = (0..size)
            .map(|x| match (light(x, y), y + 1 < size && light(x, y + 1)) {
                (true, true) => '\u{2588}',
                (true, false) => '\u{2580}',
                (false, true) => '\u{2584}',
                (false, false) => ' ',
            })
            .collect();
        out.push(line);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_is_square_with_a_quiet_border() {
        let lines = lines("mop1.placeholder-offer-text").expect("a code");
        let columns = lines[0].chars().count();
        assert!(lines.iter().all(|line| line.chars().count() == columns));
        assert_eq!(lines.len(), columns.div_ceil(2));
        assert!(
            lines[0].chars().all(|c| c == '\u{2588}'),
            "the top border is light"
        );
    }

    #[test]
    fn text_too_long_for_a_code_gives_none() {
        assert!(lines(&"x".repeat(8000)).is_none());
    }
}
