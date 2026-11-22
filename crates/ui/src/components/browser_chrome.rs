//! Browser chrome (docs/design/components/BrowserChrome.md): the toolbar of a browser tab in the
//! Work panel. It has back, forward, reload or stop, the address bar, a zoom chip, find, grab and
//! more, and an agent chip while an agent drives the tab. The page is drawn by the web engine
//! below the toolbar; `content` is that slot, and the toolbar's menus open over the toolbar row.

use crate::components::agent_icon::NameTooltip;
use crate::components::badge::{Badge, BadgeTone};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::icon_button::{IconButton, IconButtonVariant};
use crate::components::text_field::TextField;
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    DURATION_TOOLTIP_DELAY, Palette, RADIUS_MD, RADIUS_SM, SIZE_CONTROL_DEFAULT, SIZE_TOOLBAR,
    SPACE_1, SPACE_2, TYPE_LABEL,
};
use gpui::{
    AnyElement, App, AppContext, ElementId, Entity, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    px, relative,
};
use std::rc::Rc;

/// What typed text means: an address to open, or words to search for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddressIntent {
    Open(String),
    Search(String),
}

/// Reads what the user typed. A scheme, localhost or a dotted name without spaces is an address;
/// anything else is a search, and empty input is nothing.
pub fn resolve_address(input: &str) -> Option<AddressIntent> {
    let text = input.trim();
    if text.is_empty() {
        None
    } else if text.contains("://") {
        Some(AddressIntent::Open(text.to_owned()))
    } else if text.starts_with("localhost") || text.starts_with("127.0.0.1") {
        Some(AddressIntent::Open(format!("http://{text}")))
    } else if text.contains('.') && !text.contains(char::is_whitespace) {
        Some(AddressIntent::Open(format!("https://{text}")))
    } else {
        Some(AddressIntent::Search(text.to_owned()))
    }
}

/// The connection of the page. A local page shows no indicator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteSecurity {
    Local,
    Secure,
    NotSecure,
    CertificateError,
}

impl SiteSecurity {
    /// The glyph and the word: the word is the tooltip, and it is shown for the two problems.
    fn indicator(self) -> Option<(IconName, &'static str)> {
        match self {
            Self::Local => None,
            Self::Secure => Some((IconName::Lock, "Secure connection")),
            Self::NotSecure => Some((IconName::TriangleAlert, "Not secure")),
            Self::CertificateError => Some((IconName::OctagonAlert, "Certificate error")),
        }
    }
}

/// What a press on the chrome asks for. The caller acts on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowserAction {
    Back,
    Forward,
    Reload,
    Stop,
    Find,
    Grab,
    More,
    ResetZoom,
    TakeOver,
    /// Enter in the address bar.
    Navigate(AddressIntent),
    /// Escape in the address bar: the text went back to the loaded address; focus goes to the page.
    FocusPage,
}

type ActionHandler = Rc<dyn Fn(BrowserAction, &mut Window, &mut App)>;

/// A press handler for a button.
fn emit(
    handler: &Option<ActionHandler>,
    action: BrowserAction,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| {
        if let Some(handler) = &handler {
            handler(action.clone(), window, cx);
        }
    }
}

/// A toolbar control wrapped so a test can find it by its selector.
fn wrapped(selector: &'static str, control: impl IntoElement) -> AnyElement {
    div()
        .debug_selector(move || selector.into())
        .child(control)
        .into_any_element()
}

/// The toolbar of one browser tab, with the page's slot below it.
#[derive(IntoElement)]
pub struct BrowserChrome {
    id: ElementId,
    address: Entity<TextField>,
    current_url: SharedString,
    security: SiteSecurity,
    host: Option<SharedString>,
    loading: bool,
    progress: Option<f32>,
    can_back: bool,
    can_forward: bool,
    zoom_percent: u32,
    agent: bool,
    compact: bool,
    content: Option<AnyElement>,
    on_action: Option<ActionHandler>,
}

impl std::fmt::Debug for BrowserChrome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserChrome")
            .field("id", &self.id)
            .field("current_url", &self.current_url)
            .field("security", &self.security)
            .field("loading", &self.loading)
            .finish_non_exhaustive()
    }
}

impl BrowserChrome {
    /// `address` is the caller's text field; `current_url` is the loaded address, which Escape restores.
    pub fn new(
        id: impl Into<ElementId>,
        address: Entity<TextField>,
        current_url: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            address,
            current_url: current_url.into(),
            security: SiteSecurity::Local,
            host: None,
            loading: false,
            progress: None,
            can_back: false,
            can_forward: false,
            zoom_percent: 100,
            agent: false,
            compact: false,
            content: None,
            on_action: None,
        }
    }

    pub fn security(mut self, security: SiteSecurity) -> Self {
        self.security = security;
        self
    }

    /// A page on another host shows a chip such as "on VPS", or "this PC" for a local page.
    pub fn host(mut self, host: impl Into<SharedString>) -> Self {
        self.host = Some(host.into());
        self
    }

    /// Loading turns Reload into Stop and shows the progress line; `progress` makes it determinate.
    pub fn loading(mut self, loading: bool, progress: Option<f32>) -> Self {
        self.loading = loading;
        self.progress = progress.map(|value| value.clamp(0.0, 1.0));
        self
    }

    pub fn history(mut self, can_back: bool, can_forward: bool) -> Self {
        self.can_back = can_back;
        self.can_forward = can_forward;
        self
    }

    /// A zoom other than 100 percent shows its chip; a press on it resets the zoom.
    pub fn zoom(mut self, percent: u32) -> Self {
        self.zoom_percent = percent;
        self
    }

    /// An agent drives this tab: its chip shows, and a press on "Take over" hands the tab back.
    pub fn agent(mut self, agent: bool) -> Self {
        self.agent = agent;
        self
    }

    /// A narrow tab: find and grab move into More, and the zoom chip is hidden.
    pub fn compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    /// The page slot, filled by the engine or by an error state.
    pub fn content(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    pub fn on_action(
        mut self,
        listener: impl Fn(BrowserAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(listener));
        self
    }
}

/// The site indicator at the start of the address bar, with its word as tooltip and name.
fn site_indicator(id: &ElementId, security: SiteSecurity, p: &Palette) -> Option<AnyElement> {
    let (icon, word) = security.indicator()?;
    let ink = match security {
        SiteSecurity::NotSecure => p.status.warning_fg,
        SiteSecurity::CertificateError => p.status.danger_fg,
        _ => p.text.secondary,
    };
    let shown = matches!(
        security,
        SiteSecurity::NotSecure | SiteSecurity::CertificateError
    );
    let name = word.to_owned();
    let mut indicator = div()
        .id((id.clone(), "security"))
        .role(Role::Image)
        .aria_label(word)
        .tooltip_show_delay(DURATION_TOOLTIP_DELAY)
        .tooltip(move |_, cx| cx.new(|_| NameTooltip(name.clone().into())).into())
        .flex_none()
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .text_color(ink)
        .child(Icon::new(icon).size(IconSize::Small).color(ink));
    if shown {
        indicator = indicator.text_style(TYPE_LABEL).child(word);
    }
    Some(indicator.into_any_element())
}

impl RenderOnce for BrowserChrome {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let id = self.id.clone();
        let handler = self.on_action.clone();
        let address = self.address.clone();

        let reload_or_stop = if self.loading {
            wrapped(
                "stop",
                IconButton::new((id.clone(), "stop"), IconName::X, "Stop")
                    .variant(IconButtonVariant::Ghost)
                    .size(ButtonSize::Default)
                    .on_click(emit(&handler, BrowserAction::Stop)),
            )
        } else {
            wrapped(
                "reload",
                IconButton::new((id.clone(), "reload"), IconName::RefreshCw, "Reload")
                    .variant(IconButtonVariant::Ghost)
                    .size(ButtonSize::Default)
                    .on_click(emit(&handler, BrowserAction::Reload)),
            )
        };

        // Enter navigates and Escape restores the loaded address, with its text selected.
        let typed = address.clone();
        let typed_url = self.current_url.clone();
        let typed_handler = handler.clone();
        let mut bar = div()
            .id((id.clone(), "address"))
            .role(Role::TextInput)
            .aria_label("Address")
            .flex_1()
            .min_w(px(160.))
            .h(px(SIZE_CONTROL_DEFAULT))
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .px(px(SPACE_2))
            .rounded(px(RADIUS_MD))
            .bg(p.surface.sunken)
            .border_1()
            .border_color(p.border.strong)
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "enter" => {
                        let text = typed.read(cx).text().to_owned();
                        if let Some(intent) = resolve_address(&text) {
                            cx.stop_propagation();
                            if let Some(handler) = &typed_handler {
                                handler(BrowserAction::Navigate(intent), window, cx);
                            }
                        }
                    }
                    "escape" => {
                        cx.stop_propagation();
                        typed.update(cx, |field, cx| {
                            field.set_text(typed_url.as_ref(), window, cx)
                        });
                        if let Some(handler) = &typed_handler {
                            handler(BrowserAction::FocusPage, window, cx);
                        }
                    }
                    _ => {}
                }
            });
        if let Some(indicator) = site_indicator(&id, self.security, &p) {
            bar = bar.child(indicator);
        }
        if let Some(host) = &self.host {
            bar = bar.child(
                Badge::text(format!("on {host}"))
                    .tone(BadgeTone::Neutral)
                    .glyph(IconName::Server),
            );
        }
        bar = bar.child(address.clone());

        let focus_address = address.clone();
        let mut row = div()
            .id((id.clone(), "toolbar"))
            .role(Role::Toolbar)
            .aria_label("Browser toolbar")
            .relative()
            .h(px(SIZE_TOOLBAR))
            .flex()
            .items_center()
            .gap(px(SPACE_1))
            .px(px(SPACE_1))
            .bg(p.surface.canvas)
            .border_b_1()
            .border_color(p.border.subtle)
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let modifiers = event.keystroke.modifiers;
                if (modifiers.control || modifiers.platform) && event.keystroke.key == "l" {
                    cx.stop_propagation();
                    focus_address.update(cx, |field, cx| field.focus(window, cx));
                }
            })
            .child(wrapped(
                "back",
                IconButton::new((id.clone(), "back"), IconName::ArrowLeft, "Back")
                    .variant(IconButtonVariant::Ghost)
                    .size(ButtonSize::Default)
                    .disabled(!self.can_back)
                    .on_click(emit(&handler, BrowserAction::Back)),
            ))
            .child(wrapped(
                "forward",
                IconButton::new((id.clone(), "forward"), IconName::ArrowRight, "Forward")
                    .variant(IconButtonVariant::Ghost)
                    .size(ButtonSize::Default)
                    .disabled(!self.can_forward)
                    .on_click(emit(&handler, BrowserAction::Forward)),
            ))
            .child(reload_or_stop)
            .child(bar);
        if self.zoom_percent != 100 && !self.compact {
            row = row.child(
                div()
                    .id((id.clone(), "zoom"))
                    .debug_selector(|| "zoom".into())
                    .flex_none()
                    .h(px(SIZE_CONTROL_DEFAULT))
                    .px(px(SPACE_2))
                    .flex()
                    .items_center()
                    .rounded(px(RADIUS_SM))
                    .bg(p.surface.sunken)
                    .text_style(TYPE_LABEL)
                    .text_color(p.text.secondary)
                    .cursor_pointer()
                    .on_click(emit_click(&handler, BrowserAction::ResetZoom))
                    .child(format!("{}%", self.zoom_percent)),
            );
        }
        if !self.compact {
            row = row
                .child(wrapped(
                    "find",
                    IconButton::new((id.clone(), "find"), IconName::Search, "Find in page")
                        .variant(IconButtonVariant::Ghost)
                        .size(ButtonSize::Default)
                        .on_click(emit(&handler, BrowserAction::Find)),
                ))
                .child(wrapped(
                    "grab",
                    IconButton::new(
                        (id.clone(), "grab"),
                        IconName::Crosshair,
                        "Grab and annotate",
                    )
                    .variant(IconButtonVariant::Ghost)
                    .size(ButtonSize::Default)
                    .on_click(emit(&handler, BrowserAction::Grab)),
                ));
        }
        if self.agent {
            row = row.child(
                div()
                    .flex_none()
                    .h(px(SIZE_CONTROL_DEFAULT))
                    .px(px(SPACE_2))
                    .flex()
                    .items_center()
                    .gap(px(SPACE_1))
                    .rounded(px(RADIUS_SM))
                    .border_1()
                    .border_color(p.border.default)
                    .bg(p.accent.subtle_canvas)
                    .text_style(TYPE_LABEL)
                    .text_color(p.text.primary)
                    .child(
                        Icon::new(IconName::Bot)
                            .size(IconSize::XSmall)
                            .color(p.text.secondary),
                    )
                    .child("Claude is controlling this tab")
                    .child(
                        Button::new((id.clone(), "take-over"), "Take over")
                            .variant(ButtonVariant::Ghost)
                            .size(ButtonSize::Compact)
                            .on_click(emit(&handler, BrowserAction::TakeOver)),
                    ),
            );
        }
        row = row.child(wrapped(
            "more",
            IconButton::new((id.clone(), "more"), IconName::Ellipsis, "More")
                .variant(IconButtonVariant::Ghost)
                .size(ButtonSize::Default)
                .on_click(emit(&handler, BrowserAction::More)),
        ));
        if self.loading {
            let fraction = self.progress.unwrap_or(0.3);
            row = row.child(
                div()
                    .absolute()
                    .left(px(0.))
                    .bottom(px(0.))
                    .h(px(2.))
                    .w(relative(fraction))
                    .bg(p.accent.default),
            );
        }

        div()
            .flex()
            .flex_col()
            .size_full()
            .min_h_0()
            .child(row)
            .child(
                div()
                    .id((id.clone(), "content"))
                    .debug_selector(|| "browser-content".into())
                    .flex_1()
                    .min_h_0()
                    .children(self.content),
            )
    }
}

/// A press handler for a plain element, whose click also carries the event.
fn emit_click(
    handler: &Option<ActionHandler>,
    action: BrowserAction,
) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |_, window, cx| {
        if let Some(handler) = &handler {
            handler(action.clone(), window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn typed_text_is_an_address_or_a_search() {
        assert_eq!(resolve_address("   "), None);
        assert_eq!(
            resolve_address("localhost:3000"),
            Some(AddressIntent::Open("http://localhost:3000".into()))
        );
        assert_eq!(
            resolve_address("docs.rs/tokio"),
            Some(AddressIntent::Open("https://docs.rs/tokio".into()))
        );
        assert_eq!(
            resolve_address("http://example.test/a"),
            Some(AddressIntent::Open("http://example.test/a".into()))
        );
        assert_eq!(
            resolve_address("rust borrow checker"),
            Some(AddressIntent::Search("rust borrow checker".into()))
        );
    }

    #[test]
    fn only_the_two_problems_show_their_word() {
        assert_eq!(SiteSecurity::Local.indicator(), None);
        assert_eq!(
            SiteSecurity::NotSecure.indicator(),
            Some((IconName::TriangleAlert, "Not secure"))
        );
        assert_eq!(
            SiteSecurity::CertificateError.indicator(),
            Some((IconName::OctagonAlert, "Certificate error"))
        );
    }

    #[gpui::test]
    fn history_buttons_respect_their_flags_and_reload_sends(cx: &mut gpui::TestAppContext) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, cx| ChromeHarness::new(cx, seen.clone()));
        cx.run_until_parked();

        let back = cx.debug_bounds("back").expect("back is drawn").center();
        cx.simulate_click(back, gpui::Modifiers::none());
        assert!(seen.borrow().is_empty(), "back has no history to go to");

        let reload = cx.debug_bounds("reload").expect("reload is drawn").center();
        cx.simulate_click(reload, gpui::Modifiers::none());
        assert_eq!(*seen.borrow(), vec![BrowserAction::Reload]);
    }

    /// Hosts one chrome with an address field and no history, recording its actions.
    struct ChromeHarness {
        address: Entity<TextField>,
        seen: Rc<RefCell<Vec<BrowserAction>>>,
    }

    impl ChromeHarness {
        fn new(cx: &mut gpui::Context<Self>, seen: Rc<RefCell<Vec<BrowserAction>>>) -> Self {
            Self {
                address: cx.new(TextField::new),
                seen,
            }
        }
    }

    impl gpui::Render for ChromeHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let seen = self.seen.clone();
            BrowserChrome::new("chrome", self.address.clone(), "http://localhost:3000")
                .host("this PC")
                .on_action(move |action, _, _| seen.borrow_mut().push(action))
        }
    }
}
