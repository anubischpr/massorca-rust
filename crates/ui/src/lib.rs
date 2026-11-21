//! The MassOrca design system for GPUI (decisions 0021 to 0025).

pub mod assets;
pub mod style;
pub mod theme;
pub mod tokens;

pub use assets::{Assets, load_fonts};
pub use theme::{ActiveTheme, TextStyled, Theme};
pub use tokens::Appearance;

/// Installs the design system into an app: assets must already be set on the
/// `Application`; this loads the fonts and sets the theme.
pub fn init(appearance: Appearance, cx: &mut gpui::App) -> anyhow::Result<()> {
    load_fonts(cx)?;
    Theme::set(appearance, cx);
    Ok(())
}
