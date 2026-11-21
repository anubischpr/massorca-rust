//! Embedded assets: fonts, Lucide icons, agent icons and the logo (0024).

use gpui::{App, AssetSource, SharedString};
use std::borrow::Cow;

include!(concat!(env!("OUT_DIR"), "/assets_table.rs"));

/// The interface font family name (Geist).
pub const FONT_SANS: &str = "Geist";
/// The mono font family name (JetBrains Mono).
pub const FONT_MONO: &str = "JetBrains Mono";

/// Serves every file under `crates/ui/assets` from the binary.
#[derive(Clone, Copy, Debug, Default)]
pub struct Assets;

impl Assets {
    pub fn get(path: &str) -> Option<&'static [u8]> {
        ASSETS
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, bytes)| *bytes)
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path).map(Cow::Borrowed))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(ASSETS
            .iter()
            .filter(|(p, _)| p.starts_with(path))
            .map(|(p, _)| SharedString::from(*p))
            .collect())
    }
}

/// Registers the bundled Geist and JetBrains Mono files with the text system.
pub fn load_fonts(cx: &App) -> anyhow::Result<()> {
    let fonts = ASSETS
        .iter()
        .filter(|(p, _)| p.starts_with("fonts/") && p.ends_with(".ttf"))
        .map(|(_, bytes)| Cow::Borrowed(*bytes))
        .collect();
    cx.text_system().add_fonts(fonts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundles_the_static_font_files_of_the_design_system() {
        for f in [
            "fonts/geist/Geist-Regular.ttf",
            "fonts/geist/Geist-Medium.ttf",
            "fonts/geist/Geist-SemiBold.ttf",
            "fonts/jetbrains-mono/JetBrainsMono-Regular.ttf",
            "fonts/jetbrains-mono/JetBrainsMono-Italic.ttf",
            "fonts/jetbrains-mono/JetBrainsMono-Bold.ttf",
            "fonts/jetbrains-mono/JetBrainsMono-BoldItalic.ttf",
        ] {
            assert!(Assets::get(f).is_some_and(|b| b.len() > 10_000), "{f}");
        }
    }

    #[test]
    fn list_filters_by_prefix() {
        let agents = Assets.list("agents/").unwrap();
        assert!(
            agents
                .iter()
                .any(|p| p.as_ref() == "agents/gentle-shell.png")
        );
        assert!(agents.iter().all(|p| p.starts_with("agents/")));
    }
}
