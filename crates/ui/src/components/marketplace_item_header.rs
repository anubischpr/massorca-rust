//! Marketplace item header: plate and name with state, the meta line, then host and action.

use crate::components::agent_icon::{AgentIcon, AgentIconSize, AgentKind};
use crate::components::badge::Badge;
use crate::components::button::Button;
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    Palette, RADIUS_MD, SPACE_1, SPACE_2, SPACE_3, SPACE_4, TYPE_BODY_SM, TYPE_HEADING,
    TYPE_MONO_SM,
};
use gpui::{
    AnyElement, App, Div, ElementId, ImageSource, InteractiveElement, IntoElement, ParentElement,
    RenderOnce, Role, SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div, img,
    px,
};

/// The plate behind the item icon, 56 px (teams-extensions-settings.md, 7.2); no token has it yet.
const PLATE_SIZE: f32 = 56.0;
/// Separates the parts of the meta line.
const META_SEPARATOR: &str = " · ";

/// Splits a meta line at its last separator when the last part is a version, such as "1.2.0".
fn split_version(meta: &str) -> (&str, Option<&str>) {
    match meta.rsplit_once(META_SEPARATOR) {
        Some((text, version)) if looks_like_version(version) => (text, Some(version)),
        _ => (meta, None),
    }
}

/// A version starts with a digit, or with a "v" before one.
fn looks_like_version(part: &str) -> bool {
    part.strip_prefix('v')
        .unwrap_or(part)
        .starts_with(|c: char| c.is_ascii_digit())
}

/// The item name as the page heading, described by its state word.
fn name_heading(
    id: impl Into<ElementId>,
    name: SharedString,
    state: Option<SharedString>,
    p: &Palette,
) -> Stateful<Div> {
    let heading = div()
        .id(id)
        .role(Role::Heading)
        .aria_label(name.clone())
        .min_w_0()
        .truncate()
        .text_style(TYPE_HEADING)
        .text_color(p.text.primary)
        .child(name);
    match state {
        Some(word) => heading.aria_description(word),
        None => heading,
    }
}

/// The meta line, with a trailing version set in mono.
fn meta_line(meta: &str, p: &Palette) -> AnyElement {
    let (text, version) = split_version(meta);
    let mut line = div()
        .flex()
        .items_center()
        .min_w_0()
        .text_style(TYPE_BODY_SM)
        .text_color(p.text.secondary)
        .child(div().min_w_0().truncate().child(text.to_owned()));
    if let Some(version) = version {
        line = line.child(div().flex_none().child(META_SEPARATOR)).child(
            div()
                .flex_none()
                .text_style(TYPE_MONO_SM)
                .tabular_figures()
                .text_color(p.text.tertiary)
                .child(version.to_owned()),
        );
    }
    line.into_any_element()
}

/// The 56 px plate: the agent's own mark, else the logo, else the item icon at 24 px.
fn plate(
    icon: IconName,
    logo: Option<ImageSource>,
    agent: Option<AgentKind>,
    p: &Palette,
) -> AnyElement {
    let content = match (agent, logo) {
        (Some(agent), _) => div()
            .debug_selector(|| "marketplace-header-agent".to_owned())
            .child(AgentIcon::new(agent).size(AgentIconSize::Large))
            .into_any_element(),
        (None, Some(logo)) => div()
            .debug_selector(|| "marketplace-header-logo".to_owned())
            .size_full()
            .child(img(logo).size_full())
            .into_any_element(),
        (None, None) => div()
            .debug_selector(|| "marketplace-header-icon".to_owned())
            .child(
                Icon::new(icon)
                    .size(IconSize::Large)
                    .color(p.text.secondary),
            )
            .into_any_element(),
    };
    div()
        .flex_none()
        .size(px(PLATE_SIZE))
        .rounded(px(RADIUS_MD))
        .bg(p.surface.sunken)
        .flex()
        .items_center()
        .justify_center()
        .child(content)
        .into_any_element()
}

/// The item page header: plate and name with state and Copy link, meta line, host row.
#[derive(IntoElement)]
pub struct MarketplaceItemHeader {
    id: ElementId,
    name: SharedString,
    icon: IconName,
    logo: Option<ImageSource>,
    agent: Option<AgentKind>,
    state: Option<SharedString>,
    meta: Option<SharedString>,
    copy_link: Option<Button>,
    host: Option<AnyElement>,
    action: Option<Button>,
}

impl std::fmt::Debug for MarketplaceItemHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MarketplaceItemHeader")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("state", &self.state)
            .field("meta", &self.meta)
            .finish_non_exhaustive()
    }
}

impl MarketplaceItemHeader {
    /// `name` is the page heading; `icon` is drawn in the plate.
    pub fn new(id: impl Into<ElementId>, name: impl Into<SharedString>, icon: IconName) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            icon,
            logo: None,
            agent: None,
            state: None,
            meta: None,
            copy_link: None,
            host: None,
            action: None,
        }
    }

    /// The logo, drawn in the plate in place of the icon.
    pub fn logo(mut self, logo: impl Into<ImageSource>) -> Self {
        self.logo = Some(logo.into());
        self
    }

    /// The agent's own mark, drawn in the plate before any logo or icon.
    pub fn agent(mut self, agent: AgentKind) -> Self {
        self.agent = Some(agent);
        self
    }

    /// The state chip beside the name, for example "Connected".
    pub fn state(mut self, word: impl Into<SharedString>) -> Self {
        self.state = Some(word.into());
        self
    }

    /// The meta line, for example "MassOrca extras · Marketplace · 1.2.0"; a trailing version is set in mono.
    pub fn meta(mut self, text: impl Into<SharedString>) -> Self {
        self.meta = Some(text.into());
        self
    }

    /// The Copy link button, beside the name.
    pub fn copy_link(mut self, button: Button) -> Self {
        self.copy_link = Some(button);
        self
    }

    /// The host selector row, for example a select that reads "This computer, Connected".
    pub fn host(mut self, selector: impl IntoElement) -> Self {
        self.host = Some(selector.into_any_element());
        self
    }

    /// The primary action, beside the host selector.
    pub fn action(mut self, button: Button) -> Self {
        self.action = Some(button);
        self
    }
}

impl RenderOnce for MarketplaceItemHeader {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = &theme.palette;
        let heading = name_heading((self.id.clone(), "name"), self.name, self.state.clone(), p);
        let mut title = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(SPACE_2))
            .min_w_0()
            .child(heading);
        if let Some(word) = self.state {
            title = title.child(Badge::text(word));
        }
        let top = div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(SPACE_2))
            .min_w_0()
            .child(title)
            .children(self.copy_link);
        let mut identity = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(SPACE_1))
            .child(top);
        if let Some(meta) = &self.meta {
            identity = identity.child(meta_line(meta, p));
        }
        let name_row = div()
            .flex()
            .items_start()
            .gap(px(SPACE_4))
            .min_w_0()
            .child(plate(self.icon, self.logo, self.agent, p))
            .child(identity);
        let mut root = div()
            .flex()
            .flex_col()
            .gap(px(SPACE_3))
            .min_w_0()
            .child(name_row);
        if self.host.is_some() || self.action.is_some() {
            let host_row = div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(SPACE_2))
                .min_w_0()
                .children(self.host.map(|host| div().flex_1().min_w_0().child(host)))
                .children(self.action.map(|action| div().flex_none().child(action)));
            root = root.child(host_row);
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::a11y::probe::{reported, root};
    use gpui::{Image, ImageFormat, TestAppContext, accesskit::Role};
    use std::sync::Arc;

    #[test]
    fn marketplace_item_header_meta_sets_a_trailing_version_apart() {
        assert_eq!(
            split_version("MassOrca extras · Marketplace · 1.2.0"),
            ("MassOrca extras · Marketplace", Some("1.2.0"))
        );
        assert_eq!(
            split_version("Marketplace · v2.0"),
            ("Marketplace", Some("v2.0"))
        );
        assert_eq!(
            split_version("Marketplace · Writing"),
            ("Marketplace · Writing", None)
        );
        assert_eq!(split_version("1.2.0"), ("1.2.0", None));
    }

    #[gpui::test]
    fn marketplace_item_header_name_is_the_heading_described_by_its_state(cx: &mut TestAppContext) {
        let node = root(cx, |_, cx| {
            let p = cx.theme().palette;
            reported(name_heading(
                "name",
                "Claude".into(),
                Some("Connected".into()),
                &p,
            ))
        })
        .expect("the heading is reported");
        assert_eq!(node.role(), Role::Heading);
        assert_eq!(node.label(), Some("Claude"));
        assert_eq!(node.description(), Some("Connected"));
    }

    /// A one-pixel SVG, enough for the plate to draw an image.
    const LOGO: &[u8] = b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>";

    #[gpui::test]
    fn a_logo_replaces_the_kind_icon_in_the_plate(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| LogoHeader { logo: true });
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("marketplace-header-logo").is_some(),
            "the logo is drawn"
        );
        assert!(
            cx.debug_bounds("marketplace-header-icon").is_none(),
            "the kind icon makes way for the logo"
        );
    }

    #[gpui::test]
    fn without_a_logo_the_kind_icon_stays_in_the_plate(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| LogoHeader { logo: false });
        cx.run_until_parked();
        assert!(
            cx.debug_bounds("marketplace-header-logo").is_none(),
            "no logo is drawn"
        );
        assert!(
            cx.debug_bounds("marketplace-header-icon").is_some(),
            "the kind icon is drawn"
        );
    }

    #[gpui::test]
    fn an_agent_mark_replaces_the_kind_icon_in_the_plate(cx: &mut TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| AgentHeader);
        cx.run_until_parked();
        assert!(cx.debug_bounds("marketplace-header-agent").is_some());
        assert!(cx.debug_bounds("marketplace-header-icon").is_none());
    }

    struct AgentHeader;

    impl gpui::Render for AgentHeader {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            MarketplaceItemHeader::new("name", "Gentle Shell", IconName::Bot)
                .agent(AgentKind::GentleShell)
        }
    }

    /// Hosts one item header, with a logo when `logo` is set.
    struct LogoHeader {
        logo: bool,
    }

    impl gpui::Render for LogoHeader {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let mut header = MarketplaceItemHeader::new("name", "Notes", IconName::Puzzle);
            if self.logo {
                header = header.logo(Arc::new(Image::from_bytes(ImageFormat::Svg, LOGO.to_vec())));
            }
            header
        }
    }
}
