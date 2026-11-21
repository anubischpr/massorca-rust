//! Popover: an anchored, non-modal panel beside its trigger (docs/design/components/Popover.md).
//! Info popovers never take focus; form and picker popovers move focus into their first control.
//! Escape returns focus to the trigger; a click outside dismisses without moving focus.

use crate::components::button::ButtonSize;
use crate::components::icon_button::IconButton;
use crate::components::switch::ease;
use crate::icon::IconName;
use crate::theme::{ActiveTheme, TextStyled, Theme, box_shadows};
use crate::tokens::{
    DURATION_BASE, EASE_ENTER, LAYOUT_POPOVER_WIDTH_LG, LAYOUT_POPOVER_WIDTH_MD,
    LAYOUT_POPOVER_WIDTH_SM, RADIUS_LG, SPACE_1, SPACE_2, SPACE_3, TYPE_TITLE_SM, Z_POPOVER,
};
use gpui::{
    Anchor, Animation, AnimationExt, AnyElement, App, Bounds, Div, ElementId, FocusHandle,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, anchored, deferred, div, point, px,
};
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

/// Gap between the trigger edge and the panel (Popover.md: space.2).
const ANCHOR_GAP: f32 = SPACE_2;
/// Height after which long content scrolls inside the panel, header fixed (Popover.md).
pub const BODY_MAX_HEIGHT: f32 = 360.0;
/// How far the panel travels while it fades in (Popover.md: 4 px).
const ENTER_TRAVEL: f32 = SPACE_1;

/// Runs when the popover is dismissed (Escape, its close button or a click outside).
pub(crate) type DismissHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// What the popover is for. The variant decides whether opening moves focus (handoff 2.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PopoverVariant {
    /// Read-only detail (usage windows, status). Never takes focus.
    #[default]
    Info,
    /// A short form; focus moves into its first field.
    Form,
    /// Hosts the Picker panel; focus moves into its search field.
    Picker,
    /// One view of a composer or context action. Cannot take focus on its own.
    Action,
}

impl PopoverVariant {
    /// Whether opening the popover moves focus into it.
    pub fn takes_focus(self) -> bool {
        matches!(self, Self::Form | Self::Picker)
    }
}

/// Width caps of the panel (Popover.md): usage detail, composer action, everything else.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PopoverWidth {
    /// Usage detail in the Activity rail (280 px).
    Small,
    /// Composer or context action (320 px).
    Medium,
    /// Any other content (360 px).
    #[default]
    Large,
}

impl PopoverWidth {
    pub fn px(self) -> f32 {
        match self {
            Self::Small => LAYOUT_POPOVER_WIDTH_SM,
            Self::Medium => LAYOUT_POPOVER_WIDTH_MD,
            Self::Large => LAYOUT_POPOVER_WIDTH_LG,
        }
    }
}

/// The side of the trigger the panel opens on. When there is no room there, the panel
/// moves to the other side (GPUI's attach fit).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PopoverPlacement {
    #[default]
    Below,
    Above,
}

impl PopoverPlacement {
    /// The trigger corner the panel attaches to, the panel corner that sits on it, and the
    /// signed gap that moves the panel away from the trigger.
    pub(crate) fn attachment(self) -> (Anchor, Anchor, f32) {
        match self {
            Self::Below => (Anchor::BottomLeft, Anchor::TopLeft, ANCHOR_GAP),
            Self::Above => (Anchor::TopLeft, Anchor::BottomLeft, -ANCHOR_GAP),
        }
    }
}

/// An open-state change seen between two renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpenEdge {
    Opened,
    Closed,
}

/// Remembers the last rendered open state, so that a change runs its effect once.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct OpenTracker {
    was_open: bool,
}

impl OpenTracker {
    /// Records `open` and reports the edge, if the state changed since the last call.
    pub(crate) fn observe(&mut self, open: bool) -> Option<OpenEdge> {
        let edge = match (self.was_open, open) {
            (false, true) => Some(OpenEdge::Opened),
            (true, false) => Some(OpenEdge::Closed),
            _ => None,
        };
        self.was_open = open;
        edge
    }
}

/// Where a trigger was last painted, written by its wrapper and read by the overlay that
/// attaches to it on the next frame.
#[derive(Clone, Debug, Default)]
pub(crate) struct TriggerBounds(Rc<Cell<Option<Bounds<Pixels>>>>);

impl TriggerBounds {
    /// Records the first child's bounds (the trigger) each time `host` is painted.
    pub(crate) fn track(&self, host: Div) -> Div {
        let cell = self.0.clone();
        host.on_children_prepainted(move |children, _, _| {
            if let Some(first) = children.first() {
                cell.set(Some(*first));
            }
        })
    }

    /// The trigger's bounds from the last paint, if it has been painted.
    pub(crate) fn get(&self) -> Option<Bounds<Pixels>> {
        self.0.get()
    }
}

/// Keeps what the popover needs between frames: the open edge and the trigger's bounds.
#[derive(Debug, Default)]
struct PopoverState {
    open: OpenTracker,
    trigger_bounds: TriggerBounds,
}

/// An anchored panel next to its trigger. The consumer owns the open state and passes it in;
/// the panel reports Escape and outside clicks through `on_dismiss`.
#[derive(IntoElement)]
pub struct Popover {
    id: ElementId,
    trigger: AnyElement,
    open: bool,
    variant: PopoverVariant,
    width: PopoverWidth,
    placement: PopoverPlacement,
    title: Option<SharedString>,
    children: Vec<AnyElement>,
    footer: Option<AnyElement>,
    trigger_focus: Option<FocusHandle>,
    initial_focus: Option<FocusHandle>,
    on_dismiss: Option<DismissHandler>,
}

impl fmt::Debug for Popover {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Popover")
            .field("id", &self.id)
            .field("open", &self.open)
            .field("variant", &self.variant)
            .field("width", &self.width)
            .field("placement", &self.placement)
            .field("title", &self.title)
            .finish_non_exhaustive()
    }
}

impl Popover {
    pub fn new(id: impl Into<ElementId>, trigger: impl IntoElement) -> Self {
        Self {
            id: id.into(),
            trigger: trigger.into_any_element(),
            open: false,
            variant: PopoverVariant::default(),
            width: PopoverWidth::default(),
            placement: PopoverPlacement::default(),
            title: None,
            children: Vec::new(),
            footer: None,
            trigger_focus: None,
            initial_focus: None,
            on_dismiss: None,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn variant(mut self, variant: PopoverVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn width(mut self, width: PopoverWidth) -> Self {
        self.width = width;
        self
    }

    pub fn placement(mut self, placement: PopoverPlacement) -> Self {
        self.placement = placement;
        self
    }

    /// Header text (`title-sm`); the header adds a compact close button.
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// One body element, in order. The body scrolls past [`BODY_MAX_HEIGHT`].
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_any_element());
        self
    }

    /// Footer actions, right-aligned; put the primary action last.
    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    /// The trigger's focus handle; Escape and the close button return focus to it.
    pub fn trigger_focus(mut self, focus: FocusHandle) -> Self {
        self.trigger_focus = Some(focus);
        self
    }

    /// The first control of a form or picker, focused when the popover opens.
    pub fn initial_focus(mut self, focus: FocusHandle) -> Self {
        self.initial_focus = Some(focus);
        self
    }

    pub fn on_dismiss(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(listener));
        self
    }
}

impl RenderOnce for Popover {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| PopoverState::default());
        let bounds = state.read(cx).trigger_bounds.clone();
        let edge = state.update(cx, |state, _| state.open.observe(self.open));
        if edge == Some(OpenEdge::Opened)
            && self.variant.takes_focus()
            && let Some(focus) = &self.initial_focus
        {
            window.focus(focus, cx);
        }

        let escape = dismiss_with_focus(self.trigger_focus.clone(), self.on_dismiss.clone());
        let mut host = bounds.track(div().relative()).child(self.trigger);

        if !self.open {
            return host;
        }
        let key_escape = escape.clone();
        host = host.on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" {
                cx.stop_propagation();
                key_escape(window, cx);
            }
        });

        let outside = bounds.clone();
        let on_outside = self.on_dismiss.clone();
        let mut panel = overlay_surface(&theme, RADIUS_LG, theme.shadows.e2)
            .w(px(self.width.px()))
            .flex()
            .flex_col()
            .gap(px(SPACE_2))
            .p(px(SPACE_3))
            .debug_selector(|| "popover-panel".into())
            .on_mouse_down_out(move |event, window, cx| {
                let on_trigger = outside.get().is_some_and(|b| b.contains(&event.position));
                if !on_trigger && let Some(on_dismiss) = &on_outside {
                    on_dismiss(window, cx);
                }
            });
        if let Some(title) = self.title {
            let close = escape.clone();
            panel = panel.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(SPACE_2))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_style(TYPE_TITLE_SM)
                            .text_color(theme.palette.text.primary)
                            .child(title),
                    )
                    .child(
                        IconButton::new((self.id.clone(), "close"), IconName::X, "Close")
                            .size(ButtonSize::Compact)
                            .on_click(move |window, cx| close(window, cx)),
                    ),
            );
        }
        panel = panel.child(
            div()
                .id((self.id.clone(), "body"))
                .flex()
                .flex_col()
                .gap(px(SPACE_2))
                .max_h(px(BODY_MAX_HEIGHT))
                .overflow_y_scroll()
                .children(self.children),
        );
        if let Some(footer) = self.footer {
            panel = panel.child(div().flex().justify_end().gap(px(SPACE_2)).child(footer));
        }

        let (trigger_corner, panel_corner, gap) = self.placement.attachment();
        let layer = match bounds.get() {
            Some(target) => anchored()
                .attach(target, trigger_corner)
                .anchor(panel_corner)
                .offset(point(px(0.), px(gap))),
            // The trigger has not been painted yet: sit in place for this one frame.
            None => {
                window.request_animation_frame();
                anchored().anchor(panel_corner)
            }
        };
        host.child(
            deferred(layer.child(fade_in((self.id, "enter"), panel, cx)))
                .priority(Z_POPOVER as usize),
        )
    }
}

/// Dismisses by returning focus to the trigger first, then telling the owner.
fn dismiss_with_focus(
    trigger_focus: Option<FocusHandle>,
    on_dismiss: Option<DismissHandler>,
) -> DismissHandler {
    Rc::new(move |window, cx| {
        if let Some(focus) = &trigger_focus {
            window.focus(focus, cx);
        }
        if let Some(on_dismiss) = &on_dismiss {
            on_dismiss(window, cx);
        }
    })
}

/// The fill, border, radius and shadow every overlay panel shares (Popover, Menu, Select,
/// Dialog, Tooltip), with the caller's radius and shadow tier.
pub(crate) fn overlay_surface(
    theme: &Theme,
    radius: f32,
    shadow: &[crate::tokens::ShadowToken],
) -> Div {
    div()
        .bg(theme.palette.surface.overlay)
        .border_1()
        .border_color(theme.palette.border.default)
        .rounded(px(radius))
        .shadow(box_shadows(shadow))
}

/// Fades a panel in and moves it up by 4 px over `duration.base` (`ease.enter`). Reduced
/// motion shows it at once.
pub(crate) fn fade_in(id: impl Into<ElementId>, panel: Div, cx: &App) -> AnyElement {
    if cx.reduce_motion() {
        return panel.into_any_element();
    }
    panel
        .with_animation(id, Animation::new(DURATION_BASE), |el, delta| {
            let t = ease(EASE_ENTER, delta);
            el.opacity(t).top(px(ENTER_TRAVEL * (1.0 - t)))
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::Button;
    use crate::components::button::test_support::{focus_and_draw, press_key};
    use gpui::{Context, Render};

    #[test]
    fn only_forms_and_pickers_move_focus_into_the_panel() {
        assert!(!PopoverVariant::Info.takes_focus());
        assert!(PopoverVariant::Form.takes_focus());
        assert!(PopoverVariant::Picker.takes_focus());
        assert!(!PopoverVariant::Action.takes_focus());
    }

    #[test]
    fn widths_are_the_three_popover_caps() {
        assert_eq!(PopoverWidth::Small.px(), 280.0);
        assert_eq!(PopoverWidth::Medium.px(), 320.0);
        assert_eq!(PopoverWidth::Large.px(), 360.0);
        assert_eq!(PopoverWidth::default().px(), 360.0);
    }

    #[test]
    fn below_attaches_the_panel_top_under_the_trigger_bottom() {
        assert_eq!(
            PopoverPlacement::Below.attachment(),
            (Anchor::BottomLeft, Anchor::TopLeft, ANCHOR_GAP)
        );
        assert_eq!(
            PopoverPlacement::Above.attachment(),
            (Anchor::TopLeft, Anchor::BottomLeft, -ANCHOR_GAP)
        );
    }

    #[test]
    fn open_tracker_reports_each_change_once() {
        let mut tracker = OpenTracker::default();
        assert_eq!(tracker.observe(false), None);
        assert_eq!(tracker.observe(true), Some(OpenEdge::Opened));
        assert_eq!(tracker.observe(true), None);
        assert_eq!(tracker.observe(false), Some(OpenEdge::Closed));
        assert_eq!(tracker.observe(false), None);
    }

    /// Hosts one trigger and one popover whose state the test drives.
    struct Harness {
        trigger: FocusHandle,
        field: FocusHandle,
        open: bool,
        variant: PopoverVariant,
        dismissals: Rc<Cell<u32>>,
    }

    impl Harness {
        fn new(
            cx: &mut Context<Self>,
            variant: PopoverVariant,
            open: bool,
            dismissals: Rc<Cell<u32>>,
        ) -> Self {
            Self {
                trigger: cx.focus_handle(),
                field: cx.focus_handle(),
                open,
                variant,
                dismissals,
            }
        }
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let dismissals = self.dismissals.clone();
            let field = self.field.clone();
            div().p(px(SPACE_3)).child(
                Popover::new(
                    "popover-under-test",
                    Button::new("trigger", "Usage").focus_handle(self.trigger.clone()),
                )
                .open(self.open)
                .variant(self.variant)
                .title("Claude usage")
                .trigger_focus(self.trigger.clone())
                .initial_focus(self.field.clone())
                .child(div().track_focus(&field).child("Resets in 2 days"))
                .on_dismiss(move |_, _| dismissals.set(dismissals.get() + 1)),
            )
        }
    }

    #[gpui::test]
    fn escape_dismisses_and_returns_focus_to_the_trigger(cx: &mut gpui::TestAppContext) {
        let dismissals = Rc::new(Cell::new(0_u32));
        let (view, cx) = cx.add_window_view(|_, cx| {
            Harness::new(cx, PopoverVariant::Info, true, dismissals.clone())
        });
        cx.run_until_parked();
        let trigger = view.read_with(cx, |h, _| h.trigger.clone());
        focus_and_draw(cx, &trigger);
        press_key(cx, "escape");
        assert_eq!(dismissals.get(), 1);
        cx.update(|window, _| assert!(trigger.is_focused(window)));
    }

    #[gpui::test]
    fn info_popover_never_takes_focus_when_it_opens(cx: &mut gpui::TestAppContext) {
        let dismissals = Rc::new(Cell::new(0_u32));
        let (view, cx) = cx.add_window_view(|_, cx| {
            Harness::new(cx, PopoverVariant::Info, false, dismissals.clone())
        });
        cx.run_until_parked();
        view.update(cx, |h, cx| {
            h.open = true;
            cx.notify();
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        let field = view.read_with(cx, |h, _| h.field.clone());
        cx.update(|window, _| assert!(!field.is_focused(window)));
    }

    #[gpui::test]
    fn form_popover_moves_focus_into_its_first_field_on_open(cx: &mut gpui::TestAppContext) {
        let dismissals = Rc::new(Cell::new(0_u32));
        let (view, cx) = cx.add_window_view(|_, cx| {
            Harness::new(cx, PopoverVariant::Form, false, dismissals.clone())
        });
        cx.run_until_parked();
        view.update(cx, |h, cx| {
            h.open = true;
            cx.notify();
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        let field = view.read_with(cx, |h, _| h.field.clone());
        cx.update(|window, _| assert!(field.is_focused(window)));
    }
}
