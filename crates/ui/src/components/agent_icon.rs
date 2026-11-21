//! The mark of a coding agent: the app's own asset for each agent (A94), never a letter (A16, A18).

use crate::icon::IconName;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    Color, DURATION_TOOLTIP_DELAY, RADIUS_MD, SIZE_ICON_LG, SIZE_ICON_MD, SIZE_ICON_SM,
    SIZE_ICON_XS, SPACE_1, SPACE_2, TYPE_LABEL,
};
use gpui::{
    App, AppContext, Context, InteractiveElement, IntoElement, ObjectFit, ParentElement, Render,
    RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, StyledImage, Window, div,
    img, px, svg,
};

/// Claude's brand colour, fixed by A94 and not themed. No token exists for it yet.
pub const CLAUDE_BRAND: Color = Color::rgba8(0xD9, 0x77, 0x57, 1.0);

/// Name icon size from A17 (a 14 px icon beside an agent name). No size token exists yet.
const NAME_ICON_PX: f32 = 14.0;

/// The app's PNG marks keep a 2 px corner at 14 px (about 14 percent of the size).
const PNG_CORNER_RATIO: f32 = 2.0 / 14.0;

/// A coding agent that has its own mark. `Unknown` is any agent without an asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AgentKind {
    Claude,
    Codex,
    Antigravity,
    GentleShell,
    Gemini,
    OpenCode,
    Cursor,
    Unknown,
}

impl AgentKind {
    /// Every kind, in catalogue order.
    pub const ALL: [AgentKind; 8] = [
        AgentKind::Claude,
        AgentKind::Codex,
        AgentKind::Antigravity,
        AgentKind::GentleShell,
        AgentKind::Gemini,
        AgentKind::OpenCode,
        AgentKind::Cursor,
        AgentKind::Unknown,
    ];

    /// The display name: tooltip, accessible name and copy (A17, glossary A91).
    pub fn label(self) -> &'static str {
        match self {
            AgentKind::Claude => "Claude",
            AgentKind::Codex => "Codex",
            AgentKind::Antigravity => "Antigravity",
            AgentKind::GentleShell => "Gentle Shell",
            AgentKind::Gemini => "Gemini",
            AgentKind::OpenCode => "OpenCode",
            AgentKind::Cursor => "Cursor",
            AgentKind::Unknown => "Agent",
        }
    }

    /// The bundled asset under `crates/ui/assets`, or `None` for the generic mark.
    pub fn asset(self) -> Option<&'static str> {
        match self {
            AgentKind::Claude => Some("agents/claude.svg"),
            AgentKind::Codex => Some("agents/codex.svg"),
            AgentKind::Antigravity => Some("agents/antigravity.png"),
            AgentKind::GentleShell => Some("agents/gentle-shell.png"),
            AgentKind::Gemini => Some("agents/gemini.png"),
            AgentKind::OpenCode => Some("agents/opencode.svg"),
            AgentKind::Cursor => Some("agents/cursor.png"),
            AgentKind::Unknown => None,
        }
    }

    /// How the mark takes its colour.
    pub fn ink(self) -> MarkInk {
        match self {
            AgentKind::Claude => MarkInk::Fixed(CLAUDE_BRAND),
            AgentKind::Codex | AgentKind::OpenCode => MarkInk::Text,
            AgentKind::Antigravity
            | AgentKind::GentleShell
            | AgentKind::Gemini
            | AgentKind::Cursor => MarkInk::Asset,
            AgentKind::Unknown => MarkInk::Text,
        }
    }
}

/// How a mark takes its colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MarkInk {
    /// One fixed brand colour that does not follow the theme (Claude).
    Fixed(Color),
    /// The text colour, so the mark reads in both themes (Codex, OpenCode).
    Text,
    /// The asset's own colours, drawn as an image (the PNG marks).
    Asset,
}

/// Corner radius of a PNG mark drawn at `px` pixels.
pub fn png_corner_radius(px: f32) -> f32 {
    px * PNG_CORNER_RATIO
}

/// The sizes a mark is drawn at: 12, 14 (A17), 16 (default), 20 and 24 px.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum AgentIconSize {
    /// Dense chips and the status bar (`size.icon.xs`).
    XSmall,
    /// Beside an agent name in a title or label (A17).
    Name,
    /// Rows, tables and chips (`size.icon.sm`), the default.
    #[default]
    Small,
    /// Android rows (`size.icon.md`).
    Medium,
    /// Model picker rail (`size.icon.lg`).
    Large,
}

impl AgentIconSize {
    /// The edge length in logical pixels.
    pub fn px(self) -> f32 {
        match self {
            AgentIconSize::XSmall => SIZE_ICON_XS,
            AgentIconSize::Name => NAME_ICON_PX,
            AgentIconSize::Small => SIZE_ICON_SM,
            AgentIconSize::Medium => SIZE_ICON_MD,
            AgentIconSize::Large => SIZE_ICON_LG,
        }
    }
}

/// An agent's own mark. Its name is the tooltip and the accessible label.
#[derive(Debug, IntoElement)]
pub struct AgentIcon {
    kind: AgentKind,
    size: AgentIconSize,
}

impl AgentIcon {
    pub fn new(kind: AgentKind) -> Self {
        Self {
            kind,
            size: AgentIconSize::default(),
        }
    }

    pub fn size(mut self, size: AgentIconSize) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for AgentIcon {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.theme().palette;
        let edge = self.size.px();
        let mark = match (self.kind.asset(), self.kind.ink()) {
            (None, _) => svg()
                .path(IconName::Bot.path())
                .size(px(edge))
                .flex_none()
                .text_color(p.text.secondary)
                .into_any_element(),
            (Some(path), MarkInk::Asset) => img(path)
                .size(px(edge))
                .flex_none()
                .rounded(px(png_corner_radius(edge)))
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
            (Some(path), MarkInk::Fixed(color)) => svg()
                .path(path)
                .size(px(edge))
                .flex_none()
                .text_color(color)
                .into_any_element(),
            (Some(path), MarkInk::Text) => svg()
                .path(path)
                .size(px(edge))
                .flex_none()
                .text_color(p.text.primary)
                .into_any_element(),
        };
        let name = SharedString::from(self.kind.label());
        let tooltip_name = name.clone();
        div()
            .id(SharedString::from(format!(
                "agent-icon-{}",
                self.kind.label()
            )))
            .flex_none()
            .size(px(edge))
            .role(Role::Image)
            .aria_label(name)
            .tooltip_show_delay(DURATION_TOOLTIP_DELAY)
            .tooltip(move |_, cx| cx.new(|_| NameTooltip(tooltip_name.clone())).into())
            .child(mark)
    }
}

/// A name on hover, in the overlay style of tooltips; shared by marks and status glyphs.
#[derive(Debug)]
pub(crate) struct NameTooltip(pub(crate) SharedString);

impl Render for NameTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.theme().palette;
        div()
            .px(px(SPACE_2))
            .py(px(SPACE_1))
            .rounded(px(RADIUS_MD))
            .bg(p.surface.overlay)
            .border_1()
            .border_color(p.border.default)
            .text_style(TYPE_LABEL)
            .text_color(p.text.primary)
            .child(self.0.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::Assets;

    #[test]
    fn every_mapped_asset_is_bundled() {
        for kind in AgentKind::ALL {
            if let Some(path) = kind.asset() {
                assert!(Assets::get(path).is_some(), "{kind:?} -> {path}");
            }
        }
    }

    #[test]
    fn each_agent_maps_to_the_app_asset() {
        assert_eq!(AgentKind::Claude.asset(), Some("agents/claude.svg"));
        assert_eq!(AgentKind::Codex.asset(), Some("agents/codex.svg"));
        assert_eq!(
            AgentKind::Antigravity.asset(),
            Some("agents/antigravity.png")
        );
        assert_eq!(
            AgentKind::GentleShell.asset(),
            Some("agents/gentle-shell.png")
        );
        assert_eq!(AgentKind::Gemini.asset(), Some("agents/gemini.png"));
        assert_eq!(AgentKind::OpenCode.asset(), Some("agents/opencode.svg"));
        assert_eq!(AgentKind::Cursor.asset(), Some("agents/cursor.png"));
    }

    #[test]
    fn unknown_agent_uses_the_generic_bot_mark() {
        assert_eq!(AgentKind::Unknown.asset(), None);
        assert!(Assets::get(IconName::Bot.path()).is_some());
    }

    #[test]
    fn claude_is_the_fixed_brand_colour() {
        let MarkInk::Fixed(color) = AgentKind::Claude.ink() else {
            panic!("Claude must use a fixed colour");
        };
        assert_eq!(color, CLAUDE_BRAND);
        assert_eq!((color.r * 255.0).round() as u8, 0xD9);
        assert_eq!((color.g * 255.0).round() as u8, 0x77);
        assert_eq!((color.b * 255.0).round() as u8, 0x57);
    }

    #[test]
    fn codex_and_opencode_follow_the_text_colour() {
        assert_eq!(AgentKind::Codex.ink(), MarkInk::Text);
        assert_eq!(AgentKind::OpenCode.ink(), MarkInk::Text);
    }

    #[test]
    fn png_marks_keep_their_own_colours() {
        for kind in [
            AgentKind::Antigravity,
            AgentKind::GentleShell,
            AgentKind::Gemini,
            AgentKind::Cursor,
        ] {
            assert_eq!(kind.ink(), MarkInk::Asset, "{kind:?}");
        }
    }

    #[test]
    fn gentle_shell_is_the_rose_and_never_pi() {
        assert_eq!(AgentKind::GentleShell.label(), "Gentle Shell");
        assert_eq!(
            AgentKind::GentleShell.asset(),
            Some("agents/gentle-shell.png")
        );
        for kind in AgentKind::ALL {
            assert_ne!(kind.label(), "Pi");
        }
    }

    #[test]
    fn labels_are_the_display_names() {
        let labels: Vec<_> = AgentKind::ALL.iter().map(|k| k.label()).collect();
        assert_eq!(
            labels,
            vec![
                "Claude",
                "Codex",
                "Antigravity",
                "Gentle Shell",
                "Gemini",
                "OpenCode",
                "Cursor",
                "Agent",
            ]
        );
    }

    #[test]
    fn sizes_follow_the_design_scale() {
        let sizes = [
            AgentIconSize::XSmall,
            AgentIconSize::Name,
            AgentIconSize::Small,
            AgentIconSize::Medium,
            AgentIconSize::Large,
        ]
        .map(AgentIconSize::px);
        assert_eq!(sizes, [12.0, 14.0, 16.0, 20.0, 24.0]);
        assert_eq!(AgentIconSize::default(), AgentIconSize::Small);
    }

    #[test]
    fn png_corner_matches_the_app_at_fourteen_pixels() {
        assert!((png_corner_radius(14.0) - 2.0).abs() < 1e-6);
        assert!((png_corner_radius(28.0) - 4.0).abs() < 1e-6);
    }
}

#[cfg(test)]
mod a11y_tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{TestAppContext, accesskit::Role};

    #[gpui::test]
    fn an_agent_icon_is_an_image_named_by_its_agent(cx: &mut TestAppContext) {
        let node = root(cx, |window, cx| {
            reported(AgentIcon::new(AgentKind::Claude).render(window, cx))
        })
        .expect("reported");
        assert_eq!(node.role(), Role::Image);
        assert_eq!(node.label(), Some(AgentKind::Claude.label()));
    }
}
