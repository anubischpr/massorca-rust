//! Card: a raised container that groups one related set of information or
//! actions (docs/design/components/Card.md). It is a surface, not a link: an
//! interactive card is one focus and click target, and its inner controls stop
//! click propagation. A card with no content draws nothing (A35).

use crate::components::button::ClickHandler;
use crate::components::skeleton::{LineWidth, Skeleton};
use crate::components::switch::ease;
use crate::icon::{Icon, IconName, IconSize};
use crate::style::{FocusRing, Surface};
use crate::theme::{ActiveTheme, TextStyled, box_shadows};
use crate::tokens::{
    BORDER_WIDTH_DEFAULT, BORDER_WIDTH_THICK, DURATION_CARD_INSERT, EASE_ENTER, RADIUS_LG, SPACE_2,
    SPACE_3, SPACE_4, TYPE_BODY, TYPE_TITLE_SM,
};

use gpui::{
    Animation, AnimationExt, AnyElement, App, ElementId, Empty, FocusHandle, InteractiveElement,
    IntoElement, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled,
    Window, div, px,
};

/// How a card answers the pointer and the keyboard.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CardVariant {
    /// Not a focus target as a whole; its controls are (summary, settings and host cards).
    #[default]
    Static,
    /// One focus and click target; Enter opens it (routine and workspace cards).
    Interactive,
}

/// Inner padding: `space.3` for summary and settings cards, `space.4` for host and routine cards.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CardPadding {
    #[default]
    Compact,
    Comfortable,
}

impl CardPadding {
    pub fn px(self) -> f32 {
        match self {
            Self::Compact => SPACE_3,
            Self::Comfortable => SPACE_4,
        }
    }
}

/// A raised card: an optional header (icon, title, value, actions), a body and footer actions.
#[derive(IntoElement)]
pub struct Card {
    id: ElementId,
    variant: CardVariant,
    padding: CardPadding,
    icon: Option<IconName>,
    title: Option<SharedString>,
    value: Option<SharedString>,
    header_actions: Vec<AnyElement>,
    body: Vec<AnyElement>,
    footer: Vec<AnyElement>,
    selected: bool,
    disabled: bool,
    loading: bool,
    inserting: bool,
    focus: Option<FocusHandle>,
    on_click: Option<ClickHandler>,
}

impl std::fmt::Debug for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Card")
            .field("id", &self.id)
            .field("variant", &self.variant)
            .field("padding", &self.padding)
            .field("title", &self.title)
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .field("loading", &self.loading)
            .finish_non_exhaustive()
    }
}

impl Card {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            variant: CardVariant::default(),
            padding: CardPadding::default(),
            icon: None,
            title: None,
            value: None,
            header_actions: Vec::new(),
            body: Vec::new(),
            footer: Vec::new(),
            selected: false,
            disabled: false,
            loading: false,
            inserting: false,
            focus: None,
            on_click: None,
        }
    }

    pub fn variant(mut self, variant: CardVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn padding(mut self, padding: CardPadding) -> Self {
        self.padding = padding;
        self
    }

    /// The header icon, on the left of the title (A23).
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// A short number or word on the trailing edge of the header, with tabular figures.
    pub fn value(mut self, value: impl Into<SharedString>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// A header control such as an overflow IconButton. Clicks on it never open the card.
    pub fn header_action(mut self, action: impl IntoElement) -> Self {
        self.header_actions.push(action.into_any_element());
        self
    }

    /// A body row.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.body.push(child.into_any_element());
        self
    }

    /// A footer control, right-aligned and `space.2` apart.
    pub fn footer_action(mut self, action: impl IntoElement) -> Self {
        self.footer.push(action.into_any_element());
        self
    }

    /// The selected state: raised selected fill and a 2 px accent bar on the leading edge.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Dims the content and removes hover and activation.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Replaces the body with skeleton lines while its content loads.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Plays the summary insert: fade in and rise `space.2` over `duration.card-insert`
    /// (instant with reduced motion).
    pub fn inserting(mut self, inserting: bool) -> Self {
        self.inserting = inserting;
        self
    }

    /// Keyboard focus lives with the caller; otherwise the card keeps its own handle.
    pub fn focus_handle(mut self, handle: FocusHandle) -> Self {
        self.focus = Some(handle);
        self
    }

    pub fn on_click(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(std::rc::Rc::new(listener));
        self
    }

    /// Whether the card has anything to show. A card without content is never drawn (A35).
    pub fn has_content(&self) -> bool {
        self.title.is_some()
            || self.value.is_some()
            || self.loading
            || !self.body.is_empty()
            || !self.footer.is_empty()
    }
}

/// A row whose clicks stop here, so a control inside a card never opens the card.
fn isolated_row(id: &ElementId, name: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id((id.clone(), name))
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .on_click(|_, _, cx| cx.stop_propagation())
}

impl RenderOnce for Card {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if !self.has_content() {
            return Empty.into_any_element();
        }
        let theme = *cx.theme();
        let p = theme.palette;
        let fills = Surface::Raised.fills(&theme);
        let interactive = self.variant == CardVariant::Interactive;
        let live = interactive && !self.disabled;
        let focus = interactive.then(|| {
            self.focus.clone().unwrap_or_else(|| {
                window
                    .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
                    .read(cx)
                    .clone()
            })
        });
        let title_color = if self.disabled {
            p.text.disabled
        } else {
            p.text.primary
        };
        let has_header = self.icon.is_some()
            || self.title.is_some()
            || self.value.is_some()
            || !self.header_actions.is_empty();
        let has_body = self.loading || !self.body.is_empty();

        let mut root = div()
            .id(self.id.clone())
            .relative()
            .flex()
            .flex_col()
            .min_w_0()
            .p(px(self.padding.px()))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(p.border.default)
            .bg(if self.selected {
                fills.selected
            } else {
                p.surface.raised
            })
            .shadow(box_shadows(theme.shadows.e1))
            .text_color(title_color);
        if let Some(focus) = &focus {
            root = root.track_focus(focus).tab_stop(live).focus_ring(&theme);
        }
        if live {
            root = root
                .cursor_pointer()
                .hover(move |s| s.bg(fills.hover))
                .active(move |s| s.bg(fills.pressed));
        }
        if let Some(on_click) = self.on_click.filter(|_| live) {
            root = root.on_click(move |_, window, cx| on_click(window, cx));
        }
        if self.selected {
            root = root.child(
                div()
                    .absolute()
                    .left(px(0.))
                    .top(px(0.))
                    .bottom(px(0.))
                    .w(px(BORDER_WIDTH_THICK))
                    .bg(p.accent.default),
            );
        }

        if has_header {
            let mut header = div().flex().items_center().gap(px(SPACE_2)).min_w_0();
            if let Some(icon) = self.icon {
                header = header.child(
                    Icon::new(icon)
                        .size(IconSize::Small)
                        .color(p.text.secondary),
                );
            }
            if let Some(title) = self.title {
                header = header.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_style(TYPE_TITLE_SM)
                        .text_color(title_color)
                        .child(title),
                );
            }
            if let Some(value) = self.value {
                header = header.child(
                    div()
                        .flex_none()
                        .text_body()
                        .tabular_figures()
                        .text_color(title_color)
                        .child(value),
                );
            }
            if !self.header_actions.is_empty() {
                header = header.child(
                    isolated_row(&self.id, "header-actions")
                        .flex_none()
                        .children(self.header_actions),
                );
            }
            root = root.child(header);
        }

        if has_header && has_body {
            root = root.child(
                div()
                    .h(px(BORDER_WIDTH_DEFAULT))
                    .my(px(SPACE_2))
                    .bg(p.border.subtle),
            );
        }
        if self.loading {
            root = root.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(SPACE_2))
                    .child(Skeleton::line(LineWidth::Full))
                    .child(Skeleton::line(LineWidth::Most))
                    .child(Skeleton::line(LineWidth::Partial)),
            );
        } else if !self.body.is_empty() {
            root = root.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(SPACE_2))
                    .min_w_0()
                    .text_style(TYPE_BODY)
                    .children(self.body),
            );
        }
        if !self.footer.is_empty() {
            root = root.child(
                isolated_row(&self.id, "footer-actions")
                    .justify_end()
                    .mt(px(SPACE_2))
                    .children(self.footer),
            );
        }

        if self.inserting && !cx.reduce_motion() {
            root.with_animation(
                (self.id.clone(), "card-insert"),
                Animation::new(DURATION_CARD_INSERT).with_easing(|t| ease(EASE_ENTER, t)),
                |card, t| card.opacity(t).top(px(SPACE_2 * (1.0 - t))),
            )
            .into_any_element()
        } else {
            root.into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::{
        Button,
        test_support::{focus_and_draw, press_key},
    };
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn padding_follows_the_summary_and_host_tokens() {
        assert_eq!(CardPadding::Compact.px(), SPACE_3);
        assert_eq!(CardPadding::Comfortable.px(), SPACE_4);
        assert_eq!(CardPadding::default(), CardPadding::Compact);
    }

    #[test]
    fn a_card_without_content_has_nothing_to_show() {
        assert!(!Card::new("empty").has_content());
        assert!(!Card::new("icon-only").icon(IconName::Server).has_content());
        assert!(Card::new("titled").title("Plan").has_content());
        assert!(Card::new("loading").loading(true).has_content());
        assert!(Card::new("body").child("2 of 5 done").has_content());
    }

    #[test]
    fn selected_and_disabled_flags_are_stored() {
        let card = Card::new("c").selected(true).disabled(true);
        assert!(card.selected && card.disabled);
        assert_eq!(Card::new("c").variant, CardVariant::Static);
    }

    #[gpui::test]
    fn an_empty_card_draws_no_height(cx: &mut gpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| EmptyHarness);
        cx.run_until_parked();
        let bounds = cx.debug_bounds("wrapper").expect("wrapper is drawn");
        assert_eq!(bounds.size.height, px(0.));
    }

    #[gpui::test]
    fn pointer_and_enter_open_an_interactive_card(cx: &mut gpui::TestAppContext) {
        let opens = Rc::new(Cell::new(0_u32));
        let (view, cx) = cx.add_window_view(|_, cx| Harness::new(cx, opens.clone(), false));
        cx.run_until_parked();

        let center = cx.debug_bounds("wrapper").expect("card is drawn").center();
        cx.simulate_click(center, gpui::Modifiers::none());
        assert_eq!(opens.get(), 1);

        let focus = view.read_with(cx, |h, _| h.focus.clone());
        focus_and_draw(cx, &focus);
        press_key(cx, "enter");
        assert_eq!(opens.get(), 2);
    }

    #[gpui::test]
    fn a_header_action_does_not_open_the_card(cx: &mut gpui::TestAppContext) {
        let opens = Rc::new(Cell::new(0_u32));
        let (view, cx) = cx.add_window_view(|_, cx| Harness::new(cx, opens.clone(), false));
        cx.run_until_parked();

        let action = cx.debug_bounds("action").expect("action is drawn").center();
        cx.simulate_click(action, gpui::Modifiers::none());
        assert_eq!(view.read_with(cx, |h, _| h.action_clicks.get()), 1);
        assert_eq!(opens.get(), 0);
    }

    #[gpui::test]
    fn a_disabled_card_ignores_pointer_and_keys(cx: &mut gpui::TestAppContext) {
        let opens = Rc::new(Cell::new(0_u32));
        let (view, cx) = cx.add_window_view(|_, cx| Harness::new(cx, opens.clone(), true));
        cx.run_until_parked();

        let center = cx.debug_bounds("wrapper").expect("card is drawn").center();
        cx.simulate_click(center, gpui::Modifiers::none());
        let focus = view.read_with(cx, |h, _| h.focus.clone());
        focus_and_draw(cx, &focus);
        press_key(cx, "enter");
        assert_eq!(opens.get(), 0);
    }

    /// Hosts a card with no content, which must draw nothing.
    struct EmptyHarness;

    impl gpui::Render for EmptyHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            div().child(
                div()
                    .debug_selector(|| "wrapper".into())
                    .child(Card::new("empty")),
            )
        }
    }

    /// Hosts one interactive card whose header holds a button of its own.
    struct Harness {
        focus: FocusHandle,
        opens: Rc<Cell<u32>>,
        action_clicks: Rc<Cell<u32>>,
        disabled: bool,
    }

    impl Harness {
        fn new(cx: &mut gpui::Context<Self>, opens: Rc<Cell<u32>>, disabled: bool) -> Self {
            Self {
                focus: cx.focus_handle(),
                opens,
                action_clicks: Rc::new(Cell::new(0)),
                disabled,
            }
        }
    }

    impl gpui::Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let opens = self.opens.clone();
            let action_clicks = self.action_clicks.clone();
            div().child(
                div().w(px(240.)).debug_selector(|| "wrapper".into()).child(
                    Card::new("routine")
                        .variant(CardVariant::Interactive)
                        .focus_handle(self.focus.clone())
                        .disabled(self.disabled)
                        .title("Nightly build")
                        .header_action(
                            div().debug_selector(|| "action".into()).child(
                                Button::new("run", "Run").on_click(move |_, _| {
                                    action_clicks.set(action_clicks.get() + 1)
                                }),
                            ),
                        )
                        .child("Runs at 02:00")
                        .on_click(move |_, _| opens.set(opens.get() + 1)),
                ),
            )
        }
    }
}
