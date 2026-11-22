//! Tooltip: a short label that explains a control on hover or keyboard focus
//! (docs/design/components/Tooltip.md). Pointer tooltips use GPUI's tooltip, which waits
//! `duration.tooltip-delay` at rest and hides on press. Keyboard focus shows the same panel at
//! once, attached above the trigger. A tooltip never takes focus, and Escape hides it without
//! reaching the trigger's own action.

use crate::a11y::framed;
use crate::components::kbd::Kbd;
use crate::components::popover::{TriggerBounds, overlay_surface};
use crate::components::switch::ease;
use crate::theme::{ActiveTheme, TextStyled, Theme};
use crate::tokens::{
    DURATION_FAST, DURATION_TOOLTIP_DELAY, EASE_ENTER, LAYOUT_TOOLTIP_MAX_WIDTH, RADIUS_LG,
    SPACE_1, SPACE_2, TYPE_BODY_SM, Z_TOAST,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    Anchor, Animation, AnimationExt, AnyElement, AnyView, App, AppContext, Context, Div, ElementId,
    FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, RenderOnce,
    Role, SharedString, Stateful, StatefulInteractiveElement, Styled, Subscription, WeakEntity,
    Window, anchored, deferred, div, point, px,
};
use std::fmt;

/// Gap between the trigger edge and a keyboard tooltip (design-system space.1).
const ANCHOR_GAP: f32 = SPACE_1;

/// The label and optional shortcut of one tooltip. An empty label is never shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tooltip {
    text: SharedString,
    shortcut: Option<SharedString>,
}

impl Tooltip {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            text: text.into(),
            shortcut: None,
        }
    }

    /// A shortcut shown as a Kbd after the label, for example `Mod+K`.
    pub fn shortcut(mut self, binding: impl Into<SharedString>) -> Self {
        self.shortcut = Some(binding.into());
        self
    }

    pub fn text(&self) -> &SharedString {
        &self.text
    }

    pub fn shortcut_binding(&self) -> Option<&SharedString> {
        self.shortcut.as_ref()
    }

    pub fn has_text(&self) -> bool {
        !self.text.is_empty()
    }
}

impl Render for Tooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        panel(self, cx.theme())
            .id("tooltip")
            .role(Role::Tooltip)
            .aria_label(self.text.clone())
    }
}

/// The tooltip panel: a label that wraps to three lines, then truncates, and the shortcut.
fn panel(tooltip: &Tooltip, theme: &Theme) -> Div {
    overlay_surface(theme, RADIUS_LG, theme.shadows.e2)
        .flex()
        .items_center()
        .gap(px(SPACE_2))
        .max_w(px(LAYOUT_TOOLTIP_MAX_WIDTH))
        .px(px(SPACE_2))
        .py(px(SPACE_1))
        .debug_selector(|| "tooltip-panel".into())
        .child(
            div()
                .min_w_0()
                .line_clamp(3)
                .text_style(TYPE_BODY_SM)
                .text_color(theme.palette.text.primary)
                .child(tooltip.text.clone()),
        )
        .when_some(tooltip.shortcut.clone(), |el, binding| {
            el.child(Kbd::new(binding))
        })
}

/// The view GPUI's pointer tooltip shows for `tooltip`; a fresh view each time it appears.
pub fn tooltip_view(tooltip: Tooltip) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |_, cx| cx.new(|_| tooltip.clone()).into()
}

/// Keyboard-focus state of one trigger: shown while it has keyboard focus, hidden by Escape
/// until focus leaves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct KeyboardTip {
    focused_by_keyboard: bool,
    dismissed: bool,
}

impl KeyboardTip {
    /// Focus moved to or away from the trigger; `keyboard` is whether the input was a key.
    pub(crate) fn focus_changed(&mut self, focused: bool, keyboard: bool) {
        self.focused_by_keyboard = focused && keyboard;
        self.dismissed = false;
    }

    pub(crate) fn shows(&self, has_text: bool) -> bool {
        has_text && self.focused_by_keyboard && !self.dismissed
    }

    /// Escape hides a visible tooltip and reports that it consumed the key.
    pub(crate) fn escape(&mut self, has_text: bool) -> bool {
        let consumed = self.shows(has_text);
        if consumed {
            self.dismissed = true;
        }
        consumed
    }
}

/// Keeps the trigger's keyboard state, its bounds and its focus subscriptions between frames.
#[derive(Default)]
struct TriggerState {
    keyboard: KeyboardTip,
    bounds: TriggerBounds,
    _focus_subscriptions: Vec<Subscription>,
}

/// A trigger with its tooltip. Pass the trigger's focus handle so keyboard focus can show the
/// tooltip; without it, only the pointer shows one.
#[derive(IntoElement)]
pub struct TooltipTrigger {
    id: ElementId,
    tooltip: Tooltip,
    focus: Option<FocusHandle>,
    child: AnyElement,
}

impl fmt::Debug for TooltipTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TooltipTrigger")
            .field("id", &self.id)
            .field("tooltip", &self.tooltip)
            .finish_non_exhaustive()
    }
}

impl TooltipTrigger {
    pub fn new(
        id: impl Into<ElementId>,
        text: impl Into<SharedString>,
        child: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            tooltip: Tooltip::new(text),
            focus: None,
            child: child.into_any_element(),
        }
    }

    pub fn shortcut(mut self, binding: impl Into<SharedString>) -> Self {
        self.tooltip = self.tooltip.shortcut(binding);
        self
    }

    pub fn focus_handle(mut self, focus: FocusHandle) -> Self {
        self.focus = Some(focus);
        self
    }
}

impl RenderOnce for TooltipTrigger {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let focus = self.focus.clone();
        let state = window.use_keyed_state(self.id.clone(), cx, move |window, cx| {
            let mut state = TriggerState::default();
            if let Some(focus) = focus {
                let entered: WeakEntity<TriggerState> = cx.weak_entity();
                let left: WeakEntity<TriggerState> = cx.weak_entity();
                state._focus_subscriptions.push(window.on_focus_in(
                    &focus,
                    cx,
                    move |window, cx| {
                        let keyboard = window.last_input_was_keyboard();
                        let _ = entered.update(cx, |state, cx| {
                            state.keyboard.focus_changed(true, keyboard);
                            cx.notify();
                        });
                    },
                ));
                state._focus_subscriptions.push(window.on_focus_out(
                    &focus,
                    cx,
                    move |_, _, cx| {
                        let _ = left.update(cx, |state, cx| {
                            state.keyboard.focus_changed(false, false);
                            cx.notify();
                        });
                    },
                ));
            }
            state
        });
        let (keyboard, bounds) = {
            let state = state.read(cx);
            (state.keyboard, state.bounds.clone())
        };
        let has_text = self.tooltip.has_text();
        let shown = keyboard.shows(has_text);

        let escape_state = state.clone();
        let mut host = bounds
            .track(div().relative())
            .id(self.id.clone())
            .on_key_down(move |event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    escape_state.update(cx, |state, cx| {
                        if state.keyboard.escape(has_text) {
                            cx.stop_propagation();
                            cx.notify();
                        }
                    });
                }
            });
        if has_text {
            host = host
                .tooltip(tooltip_view(self.tooltip.clone()))
                .tooltip_show_delay(DURATION_TOOLTIP_DELAY);
        }
        host = host.child(self.child);
        if shown && let Some(target) = bounds.get() {
            let panel = panel(&self.tooltip, &theme);
            let layer = anchored()
                .attach(target, Anchor::TopCenter)
                .anchor(Anchor::BottomCenter)
                .offset(point(px(0.), px(-ANCHOR_GAP)));
            host = host.child(
                deferred(layer.child(keyboard_tip(
                    ElementId::from((self.id, "keyboard-tip")),
                    panel,
                    self.tooltip.text.clone(),
                    cx,
                )))
                .priority(Z_TOAST as usize),
            );
        }
        host
    }
}

/// The keyboard tip's frame: it carries the tooltip role and text around the fading panel.
fn keyboard_tip(id: ElementId, panel: Div, text: SharedString, cx: &App) -> Stateful<Div> {
    framed(id.clone(), Role::Tooltip, Some(text), appear(id, panel, cx))
}

/// Fades a tooltip in over `duration.fast`; reduced motion shows it at once.
fn appear(id: ElementId, panel: Div, cx: &App) -> AnyElement {
    if cx.reduce_motion() {
        return panel.into_any_element();
    }
    panel
        .with_animation(id, Animation::new(DURATION_FAST), |el, delta| {
            el.opacity(ease(EASE_ENTER, delta))
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::Button;
    use crate::components::button::test_support::{focus_and_draw, press_key};

    #[test]
    fn an_empty_label_is_never_shown() {
        assert!(!Tooltip::new("").has_text());
        assert!(Tooltip::new("New terminal").has_text());
    }

    #[test]
    fn the_shortcut_is_kept_beside_the_label() {
        let tooltip = Tooltip::new("Command palette").shortcut("Mod+K");
        assert_eq!(tooltip.text().as_ref(), "Command palette");
        assert_eq!(
            tooltip.shortcut_binding().map(|s| s.as_ref()),
            Some("Mod+K")
        );
        assert_eq!(Tooltip::new("Copy").shortcut_binding(), None);
    }

    #[test]
    fn keyboard_focus_shows_the_tip_but_pointer_focus_does_not() {
        let mut tip = KeyboardTip::default();
        tip.focus_changed(true, false);
        assert!(!tip.shows(true), "a mouse click does not show it");
        tip.focus_changed(true, true);
        assert!(tip.shows(true));
        tip.focus_changed(false, true);
        assert!(!tip.shows(true), "blur hides it");
    }

    #[test]
    fn escape_hides_a_visible_tip_once_and_only_then() {
        let mut tip = KeyboardTip::default();
        assert!(!tip.escape(true), "nothing to dismiss before focus");
        tip.focus_changed(true, true);
        assert!(tip.escape(true));
        assert!(!tip.shows(true));
        assert!(!tip.escape(true), "already dismissed");
        tip.focus_changed(true, true);
        assert!(tip.shows(true), "refocusing shows it again");
    }

    #[test]
    fn an_empty_tip_never_shows_or_consumes_escape() {
        let mut tip = KeyboardTip::default();
        tip.focus_changed(true, true);
        assert!(!tip.shows(false));
        assert!(!tip.escape(false));
    }

    /// Hosts one trigger whose button holds the focus the test moves.
    struct Harness {
        trigger: FocusHandle,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().p(px(SPACE_2)).child(
                TooltipTrigger::new(
                    "tooltip-under-test",
                    "New terminal",
                    Button::new("tooltip-button", "New terminal")
                        .focus_handle(self.trigger.clone()),
                )
                .focus_handle(self.trigger.clone()),
            )
        }
    }

    #[gpui::test]
    fn keyboard_focus_shows_the_tip_and_escape_hides_it(cx: &mut gpui::TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            trigger: cx.focus_handle(),
        });
        cx.run_until_parked();
        let trigger = view.read_with(cx, |h, _| h.trigger.clone());

        // The window must be active for focus events, and keyboard modality must be set
        // before the focus change, so the listener sees it.
        cx.update(|window, _| window.activate_window());
        press_key(cx, "f1");
        focus_and_draw(cx, &trigger);
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("tooltip-panel").is_some());

        press_key(cx, "escape");
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("tooltip-panel").is_none());
    }
}

#[cfg(test)]
mod a11y_tests {
    use super::*;
    use crate::a11y::probe::{reported, root, view};
    use gpui::{TestAppContext, accesskit::Role};

    #[gpui::test]
    fn a_tooltip_is_a_tooltip_named_by_its_text(cx: &mut TestAppContext) {
        let node = view(cx, |_| Tooltip::new("Open settings")).expect("the tooltip is reported");
        assert_eq!(node.role(), Role::Tooltip);
        assert_eq!(node.label(), Some("Open settings"));
    }

    #[gpui::test]
    fn the_keyboard_tip_is_a_tooltip_named_by_its_text(cx: &mut TestAppContext) {
        let node = root(cx, |_, cx| {
            reported(keyboard_tip(
                ElementId::from("tip"),
                div(),
                "Open settings".into(),
                cx,
            ))
        })
        .expect("the keyboard tip is reported");
        assert_eq!(node.role(), Role::Tooltip);
        assert_eq!(node.label(), Some("Open settings"));
    }
}
