//! The brand colours of the reasoning slider: the fill runs between an agent's two listing colours,
//! and the level name takes whichever of the two reads better on the picker's surface.

use crate::tokens::Color;

/// An agent's listing colours (`listing.banner.colors`), first and second. One colour serves both ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AgentBrand {
    pub first: Color,
    pub second: Option<Color>,
}

/// The two colours of the fill: `first` at the bottom, `second` at the top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrandColours {
    pub first: Color,
    pub second: Color,
}

/// The fill colours of `brand`, or the accent (`default` to `fg`) when the agent lists none.
pub fn brand_colours(brand: Option<&AgentBrand>, accent: (Color, Color)) -> BrandColours {
    match brand {
        Some(brand) => BrandColours {
            first: brand.first,
            second: brand.second.unwrap_or(brand.first),
        },
        None => BrandColours {
            first: accent.0,
            second: accent.1,
        },
    }
}

/// The colour of the level name: whichever brand colour contrasts more with `surface`.
pub fn name_colour(brand: BrandColours, surface: Color) -> Color {
    if brand.first.contrast(surface) >= brand.second.contrast(surface) {
        brand.first
    } else {
        brand.second
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
