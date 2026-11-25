//! The brand colours of the reasoning slider's fill and of the level name.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::Color;

    fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color::rgba8(r, g, b, 1.0)
    }

    fn accent() -> (Color, Color) {
        (rgb(116, 97, 216), rgb(184, 168, 248))
    }

    #[test]
    fn two_listed_colours_run_from_the_bottom_of_the_fill_to_the_top() {
        let brand = AgentBrand {
            first: rgb(217, 119, 87),
            second: Some(rgb(181, 83, 47)),
        };
        let fill = brand_colours(Some(&brand), accent());
        assert_eq!(fill.first, rgb(217, 119, 87));
        assert_eq!(fill.second, rgb(181, 83, 47));
    }

    #[test]
    fn one_listed_colour_serves_both_ends() {
        let brand = AgentBrand {
            first: rgb(255, 46, 138),
            second: None,
        };
        let fill = brand_colours(Some(&brand), accent());
        assert_eq!((fill.first, fill.second), (rgb(255, 46, 138), rgb(255, 46, 138)));
    }

    #[test]
    fn an_agent_without_listed_colours_fills_with_the_accent() {
        let fill = brand_colours(None, accent());
        assert_eq!((fill.first, fill.second), accent());
    }

    #[test]
    fn the_level_name_takes_the_brand_colour_that_contrasts_more_with_the_surface() {
        let surface = rgb(37, 37, 40);
        let light = rgb(244, 244, 247);
        let deep = rgb(181, 83, 47);
        let fill = BrandColours {
            first: deep,
            second: light,
        };
        assert_eq!(name_colour(fill, surface), light);
        let fill = BrandColours {
            first: light,
            second: deep,
        };
        assert_eq!(name_colour(fill, rgb(255, 255, 255)), deep);
    }
}
