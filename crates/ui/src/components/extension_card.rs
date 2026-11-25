//! Extension card (docs/design/components/ExtensionCard.md): one extension, skill, tool, agent or
//! service in the Marketplace, with the one action that fits its state. Desktop only.

use crate::components::agent_icon::{AgentIcon, AgentIconSize, AgentKind};
use crate::components::badge::{Badge, BadgeTone};
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::icon_button::{IconButton, IconButtonVariant};
use crate::components::progress::percent_text;
use crate::components::skeleton::{LineWidth, Skeleton};
use crate::components::switch::Switch;
use crate::icon::{Icon, IconName, IconSize};
use crate::style::{FocusRing, Surface};
use crate::theme::{ActiveTheme, TextStyled, box_shadows};
use crate::tokens::{
    Color, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM, SIZE_AVATAR_LG, SIZE_CONTROL_COMPACT,
    SIZE_PROGRESS_HEIGHT, SPACE_1, SPACE_2, SPACE_3, SPACE_4, TYPE_BODY, TYPE_BODY_SM,
    TYPE_CAPTION, TYPE_MONO_SM, TYPE_TITLE_SM,
};
use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, ImageSource, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, Role, SharedString, Stateful, StatefulInteractiveElement, Styled,
    Window, div, img, px, relative,
};
use std::rc::Rc;
use std::sync::Arc;

/// The narrowest card; the grid never draws a narrower one.
pub const CARD_MIN_WIDTH: f32 = 280.0;
/// Chips drawn before the rest fold into "+N": two rows of two at the minimum width.
const VISIBLE_CHIPS: usize = 4;

/// Columns of the extension grid for a page width: one under 560 px, two from 560, three from 880.
pub fn grid_columns(width: f32) -> usize {
    if width >= 880.0 {
        3
    } else if width >= 560.0 {
        2
    } else {
        1
    }
}

/// The chips drawn and the number folded into "+N".
pub fn visible_chips(total: usize) -> (usize, usize) {
    let shown = total.min(VISIBLE_CHIPS);
    (shown, total - shown)
}

/// The state line of a host that cannot be reached: the last known state and its age.
pub fn last_known_line(enabled: bool, age: &str) -> String {
    let state = if enabled { "enabled" } else { "disabled" };
    format!("Last known: {state}, {age}")
}

/// What the card's footer shows: the one action that fits the extension's state.
#[derive(Clone, Debug, PartialEq)]
pub enum ExtensionState {
    /// In the catalog and not installed: the install action.
    Catalog,
    /// Installed; `update` is the version on offer, which replaces the switch.
    Installed {
        enabled: bool,
        update: Option<SharedString>,
    },
    /// Installing, from 0 to 100 percent.
    Installing { percent: u8 },
    /// Installed but failed to start; the reason is one line.
    Failed { reason: SharedString },
    /// Needs a newer MassOrca, given as its version.
    Incompatible { requires: SharedString },
}

/// What a press on the card asks for. The caller acts on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionAction {
    Open,
    Install,
    Update,
    Cancel,
    Toggle(bool),
    Settings,
    More,
    ViewLog,
}

pub(super) type ActionHandler = Rc<dyn Fn(ExtensionAction, &mut Window, &mut App)>;

/// A press handler for one action.
pub(super) fn route(
    handler: &Option<ActionHandler>,
    action: ExtensionAction,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| {
        if let Some(handler) = &handler {
            handler(action, window, cx);
        }
    }
}

/// A press handler for a plain element, whose click also carries the event.
pub(super) fn route_click(
    handler: &Option<ActionHandler>,
    action: ExtensionAction,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |_, window, cx| {
        if let Some(handler) = &handler {
            handler(action, window, cx);
        }
    }
}

/// The name's press: it opens the item page, and stops here so the card does not open the page a second time.
fn name_press(
    handler: &Option<ActionHandler>,
) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |_, window, cx| {
        cx.stop_propagation();
        if let Some(handler) = &handler {
            handler(ExtensionAction::Open, window, cx);
        }
    }
}

/// A group of controls on the card: a press on it is the control's own, so the card does not also open its page.
fn control_group(id: &ElementId, name: &'static str) -> Stateful<Div> {
    div()
        .id((id.clone(), name))
        .on_click(|_, _, cx| cx.stop_propagation())
}

/// The switch handler: a toggle carries the state it switches to.
pub(super) fn route_toggle(
    handler: &Option<ActionHandler>,
) -> impl Fn(bool, &mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |checked, window, cx| {
        if let Some(handler) = &handler {
            handler(ExtensionAction::Toggle(checked), window, cx);
        }
    }
}

/// One extension as a card: an icon, the name, its publisher, version and description, the chips
/// of what it adds, and a footer with the action for its state.
#[derive(IntoElement)]
pub struct ExtensionCard {
    id: ElementId,
    name: SharedString,
    publisher: SharedString,
    source: SharedString,
    version: SharedString,
    description: SharedString,
    icon: IconName,
    logo: Option<ImageSource>,
    agent: Option<AgentKind>,
    adds: Vec<(IconName, SharedString)>,
    state: ExtensionState,
    install_host: Option<SharedString>,
    has_settings: bool,
    last_known: Option<SharedString>,
    loading: bool,
    on_action: Option<ActionHandler>,
}

impl std::fmt::Debug for ExtensionCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtensionCard")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl ExtensionCard {
    pub fn new(
        id: impl Into<ElementId>,
        name: impl Into<SharedString>,
        state: ExtensionState,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            publisher: SharedString::default(),
            source: SharedString::default(),
            version: SharedString::default(),
            description: SharedString::default(),
            icon: IconName::Puzzle,
            logo: None,
            agent: None,
            adds: Vec::new(),
            state,
            install_host: None,
            has_settings: false,
            last_known: None,
            loading: false,
            on_action: None,
        }
    }

    pub fn publisher(mut self, publisher: impl Into<SharedString>) -> Self {
        self.publisher = publisher.into();
        self
    }

    /// Where the extension comes from, such as "Marketplace".
    pub fn source(mut self, source: impl Into<SharedString>) -> Self {
        self.source = source.into();
        self
    }

    pub fn version(mut self, version: impl Into<SharedString>) -> Self {
        self.version = version.into();
        self
    }

    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.description = description.into();
        self
    }

    /// The icon, a Lucide glyph drawn in the secondary text colour.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = icon;
        self
    }

    /// The extension's logo, drawn in the plate in place of the icon.
    pub fn logo(mut self, logo: impl Into<ImageSource>) -> Self {
        self.logo = Some(logo.into());
        self
    }

    /// The agent's own mark, the one the sidebar and model picker draw; it takes the plate over the icon and logo.
    pub fn agent(mut self, agent: AgentKind) -> Self {
        self.agent = Some(agent);
        self
    }

    /// A chip for what the extension contributes, such as "3 commands".
    pub fn add(mut self, icon: IconName, label: impl Into<SharedString>) -> Self {
        self.adds.push((icon, label.into()));
        self
    }

    /// The host a catalog install lands on; the card shows it only when more than one host exists.
    pub fn install_host(mut self, host: impl Into<SharedString>) -> Self {
        self.install_host = Some(host.into());
        self
    }

    /// Shows the Settings button, for an extension with a settings section.
    pub fn settings(mut self, has_settings: bool) -> Self {
        self.has_settings = has_settings;
        self
    }

    /// The host cannot be reached: the card shows the last known state with this age.
    pub fn last_known(mut self, age: impl Into<SharedString>) -> Self {
        self.last_known = Some(age.into());
        self
    }

    /// Replaces the card with a skeleton while its data loads.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Runs with the action a press asks for; the caller keeps the state and updates it.
    pub fn on_action(
        mut self,
        listener: impl Fn(ExtensionAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(listener));
        self
    }

    /// The footer: the state on the left, the actions on the right, and a row under them.
    fn footer(&self, p: &Palette, handler: &Option<ActionHandler>) -> Footer {
        let id = &self.id;
        let stale = self.last_known.is_some();
        let mut right = Vec::new();
        let mut below = None;
        let left: AnyElement = match &self.state {
            ExtensionState::Catalog => {
                right.push(
                    div()
                        .debug_selector(|| "install".into())
                        .child(
                            Button::new((id.clone(), "install"), "Review and install")
                                .variant(ButtonVariant::Primary)
                                .on_click(route(handler, ExtensionAction::Install)),
                        )
                        .into_any_element(),
                );
                match &self.install_host {
                    Some(host) => Badge::text(format!("Installs on {host}")).into_any_element(),
                    None => div().into_any_element(),
                }
            }
            ExtensionState::Installed {
                enabled,
                update: None,
            } => {
                right.extend(self.management(handler));
                let switch = Switch::new((id.clone(), "switch"), *enabled)
                    .label("Enabled")
                    .disabled(stale)
                    .on_change(route_toggle(handler));
                let mut row = control_group(id, "switch-row")
                    .flex()
                    .items_center()
                    .gap(px(SPACE_2))
                    .child(div().debug_selector(|| "switch".into()).child(switch));
                if !*enabled {
                    row = row.child(Badge::text("Disabled").tone(BadgeTone::Neutral));
                }
                row.into_any_element()
            }
            ExtensionState::Installed {
                update: Some(version),
                ..
            } => {
                right.push(
                    Button::new((id.clone(), "update"), "Update")
                        .variant(ButtonVariant::Secondary)
                        .disabled(stale)
                        .on_click(route(handler, ExtensionAction::Update))
                        .into_any_element(),
                );
                Badge::text(format!("Update {version}"))
                    .tone(BadgeTone::Info)
                    .into_any_element()
            }
            ExtensionState::Installing { percent } => {
                right.push(
                    Button::new((id.clone(), "cancel"), "Cancel")
                        .variant(ButtonVariant::Ghost)
                        .on_click(route(handler, ExtensionAction::Cancel))
                        .into_any_element(),
                );
                below = Some(install_bar((id.clone(), "progress"), *percent, p));
                div()
                    .min_w_0()
                    .truncate()
                    .text_style(TYPE_BODY_SM)
                    .tabular_figures()
                    .text_color(p.text.secondary)
                    .child(installing_text(*percent))
                    .into_any_element()
            }
            ExtensionState::Failed { reason } => {
                right.push(
                    Button::new((id.clone(), "log"), "View log")
                        .variant(ButtonVariant::Ghost)
                        .on_click(route(handler, ExtensionAction::ViewLog))
                        .into_any_element(),
                );
                below = Some(
                    div()
                        .min_w_0()
                        .truncate()
                        .rounded(px(RADIUS_SM))
                        .bg(p.status.danger_bg)
                        .px(px(SPACE_2))
                        .py(px(SPACE_1))
                        .text_style(TYPE_BODY_SM)
                        .text_color(p.status.danger_fg)
                        .child(reason.clone())
                        .into_any_element(),
                );
                status_word(
                    IconName::OctagonAlert,
                    p.status.danger_fg,
                    "Failed to start",
                )
            }
            ExtensionState::Incompatible { requires } => status_word(
                IconName::TriangleAlert,
                p.status.warning_fg,
                &format!("Needs MassOrca {requires} or later"),
            ),
        };
        if let Some(age) = &self.last_known {
            let line = last_known_line(self.enabled(), age);
            below = Some(
                div()
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.tertiary)
                    .child(line)
                    .into_any_element(),
            );
        }
        Footer { left, right, below }
    }

    /// Settings and More, for an installed extension.
    fn management(&self, handler: &Option<ActionHandler>) -> Vec<AnyElement> {
        let mut actions = Vec::new();
        if self.has_settings {
            actions.push(
                Button::new((self.id.clone(), "settings"), "Settings")
                    .variant(ButtonVariant::Ghost)
                    .size(ButtonSize::Compact)
                    .leading_icon(IconName::Settings)
                    .on_click(route(handler, ExtensionAction::Settings))
                    .into_any_element(),
            );
        }
        actions.push(
            IconButton::new((self.id.clone(), "more"), IconName::Ellipsis, "More")
                .variant(IconButtonVariant::Ghost)
                .size(ButtonSize::Compact)
                .on_click(route(handler, ExtensionAction::More))
                .into_any_element(),
        );
        actions
    }

    fn enabled(&self) -> bool {
        matches!(self.state, ExtensionState::Installed { enabled: true, .. })
    }
}

/// The footer parts: the state, the actions and an optional row under them.
struct Footer {
    left: AnyElement,
    right: Vec<AnyElement>,
    below: Option<AnyElement>,
}

/// The installing status: "Installing, 40%".
fn installing_text(percent: u8) -> String {
    format!("Installing, {}", percent_text(f32::from(percent) / 100.0))
}

/// The 4 px install bar across the card, announced as progress.
fn install_bar(id: impl Into<ElementId>, percent: u8, p: &Palette) -> AnyElement {
    let fraction = f32::from(percent.min(100)) / 100.0;
    div()
        .id(id)
        .role(Role::ProgressIndicator)
        .aria_label("Installing")
        .aria_numeric_value(f64::from(percent.min(100)))
        .w_full()
        .h(px(SIZE_PROGRESS_HEIGHT))
        .rounded_full()
        .overflow_hidden()
        .bg(p.progress.track)
        .child(
            div()
                .h_full()
                .w(relative(fraction))
                .rounded_full()
                .bg(p.accent.default),
        )
        .into_any_element()
}

/// The loading card: a plate and two text bars, then a footer bar (ExtensionCard.md States).
fn loading_body(p: &Palette) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(SPACE_3))
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(SPACE_3))
                .child(
                    div()
                        .flex_none()
                        .size(px(SIZE_AVATAR_LG))
                        .rounded(px(RADIUS_MD))
                        .bg(p.fill.skeleton),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(SPACE_2))
                        .pt(px(SPACE_1))
                        .child(Skeleton::line(LineWidth::Partial))
                        .child(Skeleton::line(LineWidth::Most)),
                ),
        )
        .child(
            div()
                .h(px(SIZE_CONTROL_COMPACT))
                .w_full()
                .rounded(px(RADIUS_SM))
                .bg(p.fill.skeleton),
        )
        .into_any_element()
}

/// A glyph with a word, the state of a card that needs a decision.
pub(super) fn status_word(icon: IconName, ink: Color, word: &str) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .min_w_0()
        .text_style(TYPE_BODY_SM)
        .text_color(ink)
        .child(Icon::new(icon).size(IconSize::Small).color(ink))
        .child(word.to_owned())
        .into_any_element()
}

/// One chip of what the extension adds: an icon and a label in a compact pill.
fn chip(icon: IconName, label: SharedString, p: &Palette) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(SPACE_1))
        .h(px(SIZE_CONTROL_COMPACT))
        .px(px(SPACE_2))
        .rounded(px(RADIUS_SM))
        .border_1()
        .border_color(p.border.subtle)
        .bg(p.surface.sunken)
        .text_style(TYPE_CAPTION)
        .text_color(p.text.secondary)
        .child(
            Icon::new(icon)
                .size(IconSize::XSmall)
                .color(p.text.secondary),
        )
        .child(label)
        .into_any_element()
}

impl RenderOnce for ExtensionCard {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let fills = Surface::Raised.fills(&theme);
        let id = self.id.clone();
        let card_focus = window
            .use_keyed_state(
                ElementId::NamedChild(Arc::new(id.clone()), "card".into()),
                cx,
                |_, cx| cx.focus_handle(),
            )
            .read(cx)
            .clone();
        let card = div()
            .id(id.clone())
            .role(Role::ListItem)
            .aria_label(self.name.clone())
            .track_focus(&card_focus)
            .flex()
            .flex_col()
            .gap(px(SPACE_3))
            .min_w(px(CARD_MIN_WIDTH))
            .p(px(SPACE_4))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(p.border.default)
            .bg(p.surface.raised)
            .shadow(box_shadows(theme.shadows.e1))
            .hover(move |s| s.bg(fills.hover));
        if self.loading {
            return card.child(loading_body(&p)).into_any_element();
        }

        let handler = self.on_action.clone();
        let card = card
            .cursor_pointer()
            .on_click(route_click(&handler, ExtensionAction::Open));
        let disabled_extension =
            matches!(self.state, ExtensionState::Installed { enabled: false, .. });
        let name_color = if disabled_extension {
            p.text.secondary
        } else {
            p.text.primary
        };
        let name_focus = window
            .use_keyed_state(
                ElementId::NamedChild(Arc::new(id.clone()), "name".into()),
                cx,
                |_, cx| cx.focus_handle(),
            )
            .read(cx)
            .clone();
        let name = div()
            .id((id.clone(), "name"))
            .debug_selector(|| "name".into())
            .track_focus(&name_focus)
            .tab_stop(true)
            .focus_ring(&theme)
            .cursor_pointer()
            .min_w_0()
            .truncate()
            .text_style(TYPE_TITLE_SM)
            .text_color(name_color)
            .hover(|s| s.underline())
            .active(move |s| s.bg(fills.pressed))
            .on_click(name_press(&handler))
            .child(self.name.clone());
        let plate = div()
            .id((id.clone(), "icon"))
            .flex_none()
            .size(px(SIZE_AVATAR_LG))
            .rounded(px(RADIUS_MD))
            .bg(p.surface.sunken)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .child(match (self.agent, self.logo.clone()) {
                (Some(agent), _) => div()
                    .debug_selector(|| "card-agent".into())
                    .child(AgentIcon::new(agent).size(AgentIconSize::Large))
                    .into_any_element(),
                (None, Some(logo)) => div()
                    .debug_selector(|| "card-logo".into())
                    .size_full()
                    .child(img(logo).size_full())
                    .into_any_element(),
                (None, None) => Icon::new(self.icon)
                    .size(IconSize::Large)
                    .color(p.text.secondary)
                    .into_any_element(),
            });

        let byline = [self.publisher.to_string(), self.source.to_string()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
        let header = div()
            .flex()
            .items_start()
            .gap(px(SPACE_3))
            .child(plate)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(SPACE_1))
                    .child(name)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(SPACE_2))
                            .min_w_0()
                            .text_style(TYPE_BODY_SM)
                            .text_color(p.text.secondary)
                            .child(div().flex_1().min_w_0().truncate().child(byline))
                            .child(
                                div()
                                    .flex_none()
                                    .text_style(TYPE_MONO_SM)
                                    .tabular_figures()
                                    .text_color(p.text.tertiary)
                                    .child(self.version.clone()),
                            ),
                    ),
            );
        let mut card = card.child(header).child(
            div()
                .debug_selector(|| "description".into())
                .min_w_0()
                .text_style(TYPE_BODY)
                .text_color(p.text.secondary)
                .line_clamp(2)
                .child(self.description.clone()),
        );

        if !self.adds.is_empty() {
            let (shown, folded) = visible_chips(self.adds.len());
            let mut row = div().flex().flex_wrap().gap(px(SPACE_1));
            for (icon, label) in self.adds.iter().take(shown) {
                row = row.child(chip(*icon, label.clone(), &p));
            }
            if folded > 0 {
                row = row.child(
                    div()
                        .flex()
                        .items_center()
                        .h(px(SIZE_CONTROL_COMPACT))
                        .text_style(TYPE_CAPTION)
                        .text_color(p.text.tertiary)
                        .child(format!("+{folded}")),
                );
            }
            card = card.child(row);
        }

        let footer = self.footer(&p, &handler);
        card = card.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(SPACE_2))
                .min_w_0()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(SPACE_2))
                        .min_w_0()
                        .child(footer.left),
                )
                .child(
                    control_group(&id, "actions")
                        .flex()
                        .items_center()
                        .gap(px(SPACE_2))
                        .flex_none()
                        .children(footer.right),
                ),
        );
        if let Some(below) = footer.below {
            card = card.child(below);
        }
        card.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::test_support::press_key;
    use std::cell::RefCell;

    #[test]
    fn grid_has_one_two_or_three_columns_by_width() {
        assert_eq!(grid_columns(559.0), 1);
        assert_eq!(grid_columns(560.0), 2);
        assert_eq!(grid_columns(879.0), 2);
        assert_eq!(grid_columns(880.0), 3);
    }

    #[test]
    fn chips_fold_the_rest_into_a_count() {
        assert_eq!(visible_chips(2), (2, 0));
        assert_eq!(visible_chips(7), (4, 3));
    }

    #[test]
    fn installing_reads_one_status_with_its_percent() {
        assert_eq!(installing_text(40), "Installing, 40%");
        assert_eq!(installing_text(100), "Installing, 100%");
    }

    #[test]
    fn last_known_line_names_the_state_and_its_age() {
        assert_eq!(
            last_known_line(true, "5 min ago"),
            "Last known: enabled, 5 min ago"
        );
        assert_eq!(
            last_known_line(false, "1 h ago"),
            "Last known: disabled, 1 h ago"
        );
    }

    #[gpui::test]
    fn name_and_switch_send_their_actions(cx: &mut gpui::TestAppContext) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| ActionHarness {
            seen: seen.clone(),
            catalog: false,
        });
        cx.run_until_parked();

        let name = cx.debug_bounds("name").expect("name is drawn").center();
        cx.simulate_click(name, gpui::Modifiers::none());
        let switch = cx.debug_bounds("switch").expect("switch is drawn");
        let mut toggle = switch.center();
        toggle.x = switch.right() - px(16.);
        cx.simulate_click(toggle, gpui::Modifiers::none());

        assert_eq!(
            *seen.borrow(),
            vec![ExtensionAction::Open, ExtensionAction::Toggle(false)]
        );
    }

    #[gpui::test]
    fn install_sends_the_install_action(cx: &mut gpui::TestAppContext) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| ActionHarness {
            seen: seen.clone(),
            catalog: true,
        });
        cx.run_until_parked();

        let install = cx
            .debug_bounds("install")
            .expect("install is drawn")
            .center();
        cx.simulate_click(install, gpui::Modifiers::none());

        assert_eq!(*seen.borrow(), vec![ExtensionAction::Install]);
    }

    #[gpui::test]
    fn an_agent_card_draws_the_agents_own_mark(cx: &mut gpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| AgentHarness);
        cx.run_until_parked();

        assert!(
            cx.debug_bounds("card-agent").is_some(),
            "the agent's mark is drawn in the plate"
        );
    }

    /// Hosts one catalog card for an agent, as the Marketplace draws the Gentle Shell package.
    struct AgentHarness;

    impl gpui::Render for AgentHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            div().child(
                ExtensionCard::new("gentle-shell", "Gentle Shell", ExtensionState::Catalog)
                    .agent(AgentKind::GentleShell),
            )
        }
    }

    #[gpui::test]
    fn a_press_on_the_card_body_opens_the_item_page(cx: &mut gpui::TestAppContext) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| ActionHarness {
            seen: seen.clone(),
            catalog: true,
        });
        cx.run_until_parked();

        let body = cx
            .debug_bounds("description")
            .expect("description is drawn")
            .center();
        cx.simulate_click(body, gpui::Modifiers::none());

        assert_eq!(*seen.borrow(), vec![ExtensionAction::Open]);
    }

    #[gpui::test]
    fn enter_on_the_focused_card_opens_the_item_page(cx: &mut gpui::TestAppContext) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| ActionHarness {
            seen: seen.clone(),
            catalog: true,
        });
        cx.run_until_parked();

        // A press on the body focuses the card; Enter on the focused card then opens it.
        let body = cx
            .debug_bounds("description")
            .expect("description is drawn")
            .center();
        cx.simulate_click(body, gpui::Modifiers::none());
        seen.borrow_mut().clear();
        press_key(cx, "enter");

        assert_eq!(*seen.borrow(), vec![ExtensionAction::Open]);
    }

    /// Hosts one card, installed and enabled or in the catalog, and records the actions it sends.
    struct ActionHarness {
        seen: Rc<RefCell<Vec<ExtensionAction>>>,
        catalog: bool,
    }

    impl gpui::Render for ActionHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let seen = self.seen.clone();
            let state = if self.catalog {
                ExtensionState::Catalog
            } else {
                ExtensionState::Installed {
                    enabled: true,
                    update: None,
                }
            };
            div().child(
                ExtensionCard::new("acme", "Acme Metrics", state)
                    .settings(true)
                    .on_action(move |action, _, _| seen.borrow_mut().push(action)),
            )
        }
    }
}
