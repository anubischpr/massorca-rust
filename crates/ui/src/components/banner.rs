//! Banner: an inline strip at the top of a region that says something true about
//! the region's state (docs/design/components/Banner.md). It pushes content and
//! never floats; it never takes focus on its own, and its buttons are ordinary
//! tab stops in reading order.

use crate::components::agent_status::{AgentState, AgentStatus, AgentStatusSize};
use crate::components::button::{Button, ButtonSize, ButtonVariant, ClickHandler};
use crate::components::icon_button::{IconButton, IconButtonVariant};
use crate::components::progress::Spinner;
use crate::components::switch::ease;
use crate::icon::{Icon, IconName, IconSize};
use crate::style::Surface;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    Color, DURATION_MODERATE, EASE_ENTER, Palette, RADIUS_MD, SIZE_TITLEBAR_WINDOWS, SPACE_1,
    SPACE_2, SPACE_3, TYPE_BODY_SM, TYPE_BODY_STRONG,
};
use gpui::{
    Animation, AnimationExt, AnyElement, App, ElementId, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};

/// A banner carries at most this many actions (Banner.md, anatomy); extra ones are ignored.
pub const MAX_ACTIONS: usize = 2;

/// What the banner reports. Sets its fill, border and glyph.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BannerVariant {
    #[default]
    Info,
    Success,
    Warning,
    Danger,
    /// The host cannot be reached: the dashed ring with a slash, never worded as stopped.
    Unreachable,
}

impl BannerVariant {
    /// The fill and the border of the variant (`status.*`, or `agent.unreachable.*`).
    fn surface(self, palette: &Palette) -> (Color, Color) {
        match self {
            Self::Info => (palette.status.info_bg, palette.status.info_border),
            Self::Success => (palette.status.success_bg, palette.status.success_border),
            Self::Warning => (palette.status.warning_bg, palette.status.warning_border),
            Self::Danger => (palette.status.danger_bg, palette.status.danger_border),
            Self::Unreachable => (
                palette.agent.unreachable_bg,
                palette.agent.unreachable_border,
            ),
        }
    }

    /// The glyph colour of the variant.
    fn glyph_color(self, palette: &Palette) -> Color {
        match self {
            Self::Info => palette.status.info_fg,
            Self::Success => palette.status.success_fg,
            Self::Warning => palette.status.warning_fg,
            Self::Danger => palette.status.danger_fg,
            Self::Unreachable => palette.agent.unreachable,
        }
    }

    /// The Lucide glyph of a status variant; unreachable draws the shared AgentStatus glyph.
    fn icon(self) -> Option<IconName> {
        match self {
            Self::Info => Some(IconName::Info),
            Self::Success => Some(IconName::CircleCheck),
            Self::Warning => Some(IconName::TriangleAlert),
            Self::Danger => Some(IconName::OctagonAlert),
            Self::Unreachable => None,
        }
    }
}

/// An inline notice: a glyph, an optional title, a message, up to two actions, an optional
/// Details disclosure and an optional dismiss control.
#[derive(IntoElement)]
pub struct Banner {
    id: ElementId,
    variant: BannerVariant,
    title: Option<SharedString>,
    message: SharedString,
    detail: Option<SharedString>,
    actions: Vec<Button>,
    more: Option<(SharedString, ClickHandler)>,
    loading: bool,
    edge_to_edge: bool,
    on_dismiss: Option<ClickHandler>,
}

impl std::fmt::Debug for Banner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Banner")
            .field("id", &self.id)
            .field("variant", &self.variant)
            .field("message", &self.message)
            .field("actions", &self.actions.len())
            .field("loading", &self.loading)
            .finish_non_exhaustive()
    }
}

impl Banner {
    pub fn new(
        id: impl Into<ElementId>,
        variant: BannerVariant,
        message: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            variant,
            title: None,
            message: message.into(),
            detail: None,
            actions: Vec::new(),
            more: None,
            loading: false,
            edge_to_edge: false,
            on_dismiss: None,
        }
    }

    /// A short bold lead above the message.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Text behind a Details disclosure: host name, last contact, error text or retry timer.
    pub fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// An action button; at most `MAX_ACTIONS` are kept, the rest are ignored.
    pub fn action(mut self, button: Button) -> Self {
        if self.actions.len() < MAX_ACTIONS {
            self.actions.push(button);
        }
        self
    }

    /// A "+N more" text button that lists the notices a container hid. Nothing is drawn at zero.
    pub fn more(
        mut self,
        count: usize,
        listener: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        if count > 0 {
            self.more = Some((format!("+{count} more").into(), std::rc::Rc::new(listener)));
        }
        self
    }

    /// Busy: a spinner replaces the glyph after the spinner delay (connecting, reconnecting).
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Spans its container with square corners and a bottom border only.
    pub fn edge_to_edge(mut self, edge_to_edge: bool) -> Self {
        self.edge_to_edge = edge_to_edge;
        self
    }

    /// Makes the banner dismissible with an x control. Without a handler it stays until its state changes.
    pub fn on_dismiss(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(std::rc::Rc::new(listener));
        self
    }
}

impl RenderOnce for Banner {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let (fill, border) = self.variant.surface(&p);
        let detail_open = self
            .detail
            .as_ref()
            .map(|_| window.use_keyed_state((self.id.clone(), "detail-open"), cx, |_, _| false));
        let open = detail_open.as_ref().is_some_and(|state| *state.read(cx));

        let glyph: AnyElement = if self.loading {
            Spinner::new()
                .size(IconSize::Small)
                .color(p.agent.running)
                .delayed((self.id.clone(), "spinner"))
                .into_any_element()
        } else if let Some(icon) = self.variant.icon() {
            Icon::new(icon)
                .size(IconSize::Small)
                .color(self.variant.glyph_color(&p))
                .into_any_element()
        } else {
            AgentStatus::new(AgentState::Unreachable)
                .size(AgentStatusSize::Small)
                .into_any_element()
        };

        let mut text = div().flex_1().min_w_0().flex().flex_col().gap(px(SPACE_1));
        if let Some(title) = self.title {
            text = text.child(
                div()
                    .text_style(TYPE_BODY_STRONG)
                    .text_color(p.text.primary)
                    .child(title),
            );
        }
        text = text.child(
            div()
                .text_body()
                .text_color(p.text.primary)
                .line_clamp(3)
                .child(self.message),
        );
        if let (true, Some(detail)) = (open, self.detail.clone()) {
            text = text.child(
                div()
                    .debug_selector(|| "banner-detail".into())
                    .text_style(TYPE_BODY_SM)
                    .text_color(p.text.secondary)
                    .child(detail),
            );
        }

        let mut actions = div().flex().flex_none().items_center().gap(px(SPACE_1));
        for button in self.actions {
            actions = actions.child(button.size(ButtonSize::Compact));
        }
        if let (Some(state), Some(_)) = (detail_open, self.detail.as_ref()) {
            let toggle = Button::new((self.id.clone(), "detail-toggle"), "Details")
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Compact)
                .selected(open)
                .on_click(move |_, cx| {
                    state.update(cx, |open, cx| {
                        *open = !*open;
                        cx.notify();
                    })
                });
            actions = actions.child(
                div()
                    .debug_selector(|| "banner-detail-toggle".into())
                    .child(toggle),
            );
        }
        if let Some((label, listener)) = self.more {
            let more = Button::new((self.id.clone(), "more"), label)
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Compact)
                .on_click(move |window, cx| listener(window, cx));
            actions = actions.child(more);
        }

        let mut root = div()
            .id(self.id.clone())
            .flex()
            .items_start()
            .gap(px(SPACE_2))
            .min_h(px(SIZE_TITLEBAR_WINDOWS))
            .px(px(SPACE_3))
            .py(px(SPACE_2))
            .bg(fill)
            .text_color(p.text.primary)
            .child(div().flex_none().pt(px(SPACE_1)).child(glyph))
            .child(text)
            .child(actions);
        if self.edge_to_edge {
            root = root.border_b_1().border_color(border);
        } else {
            root = root.rounded(px(RADIUS_MD)).border_1().border_color(border);
        }
        if let Some(on_dismiss) = self.on_dismiss {
            let escape = on_dismiss.clone();
            root = root
                .on_key_down(move |event, window, cx| {
                    if event.keystroke.key == "escape" {
                        escape(window, cx);
                        cx.stop_propagation();
                    }
                })
                .child(
                    div().debug_selector(|| "banner-dismiss".into()).child(
                        IconButton::new((self.id.clone(), "dismiss"), IconName::X, "Dismiss")
                            .variant(IconButtonVariant::Ghost)
                            .size(ButtonSize::Compact)
                            .surface(Surface::Canvas)
                            .on_click(move |window, cx| on_dismiss(window, cx)),
                    ),
                );
        }
        if cx.reduce_motion() {
            root.into_any_element()
        } else {
            root.with_animation(
                (self.id.clone(), "enter"),
                Animation::new(DURATION_MODERATE).with_easing(|t| ease(EASE_ENTER, t)),
                |banner, t| banner.opacity(t),
            )
            .into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::test_support::{focus_and_draw, press_key};
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn each_variant_uses_its_status_tokens() {
        let p = Palette::DARK;
        assert_eq!(
            BannerVariant::Info.surface(&p),
            (p.status.info_bg, p.status.info_border)
        );
        assert_eq!(
            BannerVariant::Danger.surface(&p),
            (p.status.danger_bg, p.status.danger_border)
        );
        assert_eq!(
            BannerVariant::Unreachable.surface(&p),
            (p.agent.unreachable_bg, p.agent.unreachable_border)
        );
        assert_eq!(
            BannerVariant::Unreachable.glyph_color(&p),
            p.agent.unreachable
        );
        assert_eq!(BannerVariant::Unreachable.icon(), None);
        assert_eq!(BannerVariant::Warning.icon(), Some(IconName::TriangleAlert));
        assert_eq!(BannerVariant::default(), BannerVariant::Info);
    }

    #[test]
    fn at_most_two_actions_are_kept() {
        let banner = Banner::new("b", BannerVariant::Warning, "Sign in")
            .action(Button::new("a", "Sign in"))
            .action(Button::new("b", "Later"))
            .action(Button::new("c", "Never"));
        assert_eq!(banner.actions.len(), MAX_ACTIONS);
    }

    #[test]
    fn more_is_drawn_only_for_hidden_notices() {
        assert!(Banner::new("b", BannerVariant::Info, "x").more.is_none());
        let counted = Banner::new("b", BannerVariant::Info, "x").more(1, |_, _| {});
        assert_eq!(
            counted.more.map(|(label, _)| label.to_string()),
            Some("+1 more".into())
        );
    }

    #[gpui::test]
    fn details_toggle_reveals_and_hides_the_detail_line(cx: &mut gpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| Harness::new(None, Rc::new(Cell::new(0))));
        cx.run_until_parked();
        assert!(cx.debug_bounds("banner-detail").is_none());

        let toggle = cx
            .debug_bounds("banner-detail-toggle")
            .expect("toggle is drawn")
            .center();
        cx.simulate_click(toggle, gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("banner-detail").is_some());
    }

    #[gpui::test]
    fn escape_on_a_focused_action_dismisses_the_banner(cx: &mut gpui::TestAppContext) {
        let dismissed = Rc::new(Cell::new(0_u32));
        let (view, cx) =
            cx.add_window_view(|_, cx| Harness::new(Some(cx.focus_handle()), dismissed.clone()));
        cx.run_until_parked();
        let focus = view.read_with(cx, |h, _| {
            h.focus.clone().expect("harness has a focus handle")
        });
        focus_and_draw(cx, &focus);
        press_key(cx, "escape");
        assert_eq!(dismissed.get(), 1);
    }

    #[gpui::test]
    fn the_dismiss_control_dismisses_the_banner(cx: &mut gpui::TestAppContext) {
        let dismissed = Rc::new(Cell::new(0_u32));
        let (_, cx) = cx.add_window_view(|_, _| Harness::new(None, dismissed.clone()));
        cx.run_until_parked();
        let dismiss = cx
            .debug_bounds("banner-dismiss")
            .expect("dismiss is drawn")
            .center();
        cx.simulate_click(dismiss, gpui::Modifiers::none());
        assert_eq!(dismissed.get(), 1);
    }

    #[gpui::test]
    fn a_banner_without_a_handler_has_no_dismiss_control(cx: &mut gpui::TestAppContext) {
        let (_, cx) = cx.add_window_view(|_, _| Harness::sticky());
        cx.run_until_parked();
        assert!(cx.debug_bounds("banner-dismiss").is_none());
    }

    /// Hosts one dismissible banner with an action button; the action can take focus.
    struct Harness {
        focus: Option<gpui::FocusHandle>,
        dismissed: Rc<Cell<u32>>,
        sticky: bool,
    }

    impl Harness {
        fn new(focus: Option<gpui::FocusHandle>, dismissed: Rc<Cell<u32>>) -> Self {
            Self {
                focus,
                dismissed,
                sticky: false,
            }
        }

        fn sticky() -> Self {
            Self {
                focus: None,
                dismissed: Rc::new(Cell::new(0)),
                sticky: true,
            }
        }
    }

    impl gpui::Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let dismissed = self.dismissed.clone();
            let mut action = Button::new("retry", "Retry");
            if let Some(focus) = self.focus.clone() {
                action = action.focus_handle(focus);
            }
            let mut banner = Banner::new(
                "notice",
                BannerVariant::Unreachable,
                "VPS can't be reached.",
            )
            .detail("Last contact 5 min ago")
            .action(action);
            if !self.sticky {
                banner = banner.on_dismiss(move |_, _| dismissed.set(dismissed.get() + 1));
            }
            div().w(px(480.)).child(banner)
        }
    }
}
