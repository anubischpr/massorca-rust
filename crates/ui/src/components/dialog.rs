//! Dialog and confirm: a centred modal for one decision (docs/design/components/Dialog.md).
//! Escape closes unless the dialog is busy, and Tab and Shift+Tab cycle inside it. Focus starts
//! on Cancel for destructive dialogs and on the primary action otherwise, and returns to the
//! element that opened the dialog when it closes. A destructive dialog never starts on its
//! danger action, so Enter cannot run it by accident.

use crate::components::button::{Button, ButtonVariant};
use crate::components::popover::{DismissHandler, OpenEdge, OpenTracker, overlay_surface};
use crate::components::switch::ease;
use crate::icon::{Icon, IconName, IconSize};
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    DURATION_MODERATE, EASE_ENTER, LAYOUT_DIALOG_WIDTH_LG, LAYOUT_DIALOG_WIDTH_MD,
    LAYOUT_DIALOG_WIDTH_SM, RADIUS_XL, SPACE_1, SPACE_2, SPACE_3, SPACE_4, TYPE_BODY, TYPE_CAPTION,
    TYPE_TITLE, Z_DIALOG,
};
use gpui::{
    Animation, AnimationExt, AnyElement, App, ElementId, FocusHandle, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, ParentElement, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, deferred, div, px,
};
use std::fmt;
use std::rc::Rc;

/// Rows of a loss list shown before the rest are summarised (Dialog.md: up to five rows).
pub const LOSS_ROWS: usize = 5;
/// Share of the window height the body may take before it scrolls (Dialog.md: 40 percent).
const BODY_HEIGHT_SHARE: f32 = 0.4;

/// What the dialog decides. The variant sets the action's style and where focus starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DialogVariant {
    /// A confirm with one sentence on what happens.
    #[default]
    Confirm,
    /// A confirm that names what is lost; the action is danger-styled.
    Destructive,
    /// What failed and what is kept; the action is "Retry" or "Close".
    Error,
    /// One text field with its label; focus goes into the field.
    OneField,
}

impl DialogVariant {
    /// Destructive dialogs start on Cancel, so Enter never reaches the danger action.
    pub fn starts_on_cancel(self) -> bool {
        matches!(self, Self::Destructive)
    }

    /// A click on the scrim closes a confirm or a form with nothing typed, never the others.
    pub fn scrim_dismisses(self, typed: bool) -> bool {
        matches!(self, Self::Confirm | Self::OneField) && !typed
    }

    fn action_style(self) -> ButtonVariant {
        match self {
            Self::Destructive => ButtonVariant::Danger,
            Self::Confirm | Self::Error | Self::OneField => ButtonVariant::Primary,
        }
    }
}

/// Panel width (Dialog.md): the default, the single-field and confirm width, and permission review.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DialogWidth {
    /// 400 px, for confirms that need less room.
    Small,
    /// 440 px, the desktop default.
    #[default]
    Medium,
    /// 560 px, for permission review.
    Large,
}

impl DialogWidth {
    pub fn px(self) -> f32 {
        match self {
            Self::Small => LAYOUT_DIALOG_WIDTH_SM,
            Self::Medium => LAYOUT_DIALOG_WIDTH_MD,
            Self::Large => LAYOUT_DIALOG_WIDTH_LG,
        }
    }
}

/// How many loss rows show, and how many more are summarised in a last row ("and 4 more").
pub(crate) fn loss_rows(total: usize) -> (usize, Option<usize>) {
    if total <= LOSS_ROWS {
        (total, None)
    } else {
        (LOSS_ROWS - 1, Some(total - (LOSS_ROWS - 1)))
    }
}

/// The next focus stop after `current` among `count` stops, wrapping at both ends.
pub(crate) fn cycle(current: Option<usize>, count: usize, backwards: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(match (current, backwards) {
        (None, false) => 0,
        (None, true) => count - 1,
        (Some(index), false) => (index + 1) % count,
        (Some(index), true) => (index + count - 1) % count,
    })
}

/// What the dialog keeps between frames: the open edge, the focus stops it owns and the
/// element that had focus when it opened.
struct DialogState {
    open: OpenTracker,
    cancel: FocusHandle,
    action: FocusHandle,
    opener: Option<FocusHandle>,
}

/// A modal decision. The owner keeps `open` and handles `on_action` and `on_dismiss`; closing
/// the dialog (setting `open` to false) returns focus to the opener.
#[derive(IntoElement)]
pub struct Dialog {
    id: ElementId,
    open: bool,
    variant: DialogVariant,
    width: DialogWidth,
    title: SharedString,
    body: Vec<AnyElement>,
    loss: Vec<SharedString>,
    error: Option<SharedString>,
    action_label: SharedString,
    cancel_label: SharedString,
    busy: bool,
    action_disabled: bool,
    field: Option<FocusHandle>,
    return_focus: Option<FocusHandle>,
    dismiss_on_scrim: bool,
    inline: bool,
    on_action: Option<DismissHandler>,
    on_dismiss: Option<DismissHandler>,
}

impl fmt::Debug for Dialog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Dialog")
            .field("id", &self.id)
            .field("open", &self.open)
            .field("variant", &self.variant)
            .field("title", &self.title)
            .field("busy", &self.busy)
            .finish_non_exhaustive()
    }
}

impl Dialog {
    /// A dialog titled with a question or a short noun phrase, such as "Delete secret DB_PASSWORD?".
    pub fn new(id: impl Into<ElementId>, title: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            open: false,
            variant: DialogVariant::default(),
            width: DialogWidth::default(),
            title: title.into(),
            body: Vec::new(),
            loss: Vec::new(),
            error: None,
            action_label: "OK".into(),
            cancel_label: "Cancel".into(),
            busy: false,
            action_disabled: false,
            field: None,
            return_focus: None,
            dismiss_on_scrim: false,
            inline: false,
            on_action: None,
            on_dismiss: None,
        }
    }

    pub fn open(mut self, open: bool) -> Self {
        self.open = open;
        self
    }

    pub fn variant(mut self, variant: DialogVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn width(mut self, width: DialogWidth) -> Self {
        self.width = width;
        self
    }

    /// One body element, in order. The body scrolls past 40 percent of the window height.
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.body.push(child.into_any_element());
        self
    }

    /// The loss list of a destructive dialog, one line per kind of loss, with counts.
    pub fn loss(mut self, rows: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.loss = rows.into_iter().map(Into::into).collect();
        self
    }

    /// The status line above the footer, in danger colour with its glyph.
    pub fn error(mut self, text: impl Into<SharedString>) -> Self {
        self.error = Some(text.into());
        self
    }

    /// The action's label; name the loss for destructive dialogs, for example "Delete secret".
    pub fn action(mut self, label: impl Into<SharedString>) -> Self {
        self.action_label = label.into();
        self
    }

    pub fn cancel_label(mut self, label: impl Into<SharedString>) -> Self {
        self.cancel_label = label.into();
        self
    }

    /// While busy the action shows a spinner, both buttons are disabled and Escape is ignored.
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Disables the action without busy, for example when the host is unreachable.
    pub fn action_disabled(mut self, disabled: bool) -> Self {
        self.action_disabled = disabled;
        self
    }

    /// The first field of a one-field dialog; focus moves into it on open.
    pub fn field(mut self, focus: FocusHandle) -> Self {
        self.field = Some(focus);
        self
    }

    /// Where focus goes on close, instead of the element that had it when the dialog opened.
    pub fn return_focus(mut self, focus: FocusHandle) -> Self {
        self.return_focus = Some(focus);
        self
    }

    /// Lets a click on the scrim dismiss. Set it only when nothing is typed; destructive and
    /// error dialogs never close from the scrim.
    pub fn dismiss_on_scrim(mut self, enabled: bool) -> Self {
        self.dismiss_on_scrim = enabled;
        self
    }

    /// Draws the panel in place, without the scrim or the overlay layer, for galleries and docs.
    pub fn inline(mut self) -> Self {
        self.inline = true;
        self
    }

    pub fn on_action(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_action = Some(Rc::new(listener));
        self
    }

    pub fn on_dismiss(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(Rc::new(listener));
        self
    }
}

impl RenderOnce for Dialog {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| DialogState {
            open: OpenTracker::default(),
            cancel: cx.focus_handle(),
            action: cx.focus_handle(),
            opener: None,
        });
        let (cancel, action, opener) = {
            let state = state.read(cx);
            (
                state.cancel.clone(),
                state.action.clone(),
                state.opener.clone(),
            )
        };
        match state.update(cx, |state, _| state.open.observe(self.open)) {
            Some(OpenEdge::Opened) => {
                let opener = window.focused(cx);
                state.update(cx, |state, _| state.opener = opener);
                let first = if self.variant.starts_on_cancel() {
                    cancel.clone()
                } else {
                    self.field.clone().unwrap_or_else(|| action.clone())
                };
                window.focus(&first, cx);
            }
            Some(OpenEdge::Closed) => {
                if let Some(target) = self.return_focus.clone().or(opener) {
                    window.focus(&target, cx);
                }
            }
            None => {}
        }
        if !self.open {
            return div().into_any_element();
        }

        // The focus stops in Tab order; Tab cycles through them and never leaves the dialog.
        let mut stops = Vec::new();
        stops.extend(self.field.clone());
        stops.push(cancel.clone());
        stops.push(action.clone());
        let busy = self.busy;
        let dismiss = self.on_dismiss.clone();
        let mut panel = overlay_surface(&theme, RADIUS_XL, theme.shadows.e3)
            .w(px(self.width.px()))
            .flex()
            .flex_col()
            .gap(px(SPACE_3))
            .p(px(SPACE_4))
            .debug_selector(|| "dialog-panel".into())
            .occlude()
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "escape" => {
                        cx.stop_propagation();
                        if !busy && let Some(dismiss) = &dismiss {
                            dismiss(window, cx);
                        }
                    }
                    "tab" => {
                        cx.stop_propagation();
                        let current = stops.iter().position(|stop| stop.is_focused(window));
                        if let Some(next) =
                            cycle(current, stops.len(), event.keystroke.modifiers.shift)
                        {
                            window.focus(&stops[next], cx);
                        }
                    }
                    _ => {}
                }
            });

        panel = panel.child(
            div()
                .min_w_0()
                .text_style(TYPE_TITLE)
                .text_color(p.text.primary)
                .child(self.title.clone()),
        );

        let body_height = f32::from(window.viewport_size().height) * BODY_HEIGHT_SHARE;
        panel = panel.child(
            div()
                .id((self.id.clone(), "body"))
                .flex()
                .flex_col()
                .gap(px(SPACE_2))
                .max_h(px(body_height))
                .overflow_y_scroll()
                .text_style(TYPE_BODY)
                .text_color(p.text.secondary)
                .children(self.body),
        );

        if !self.loss.is_empty() {
            let (shown, more) = loss_rows(self.loss.len());
            let mut list = div()
                .flex()
                .flex_col()
                .gap(px(SPACE_1))
                .text_style(TYPE_BODY)
                .text_color(p.text.primary);
            for row in self.loss.iter().take(shown) {
                list = list.child(div().tabular_figures().child(row.clone()));
            }
            if let Some(more) = more {
                list = list.child(
                    div()
                        .tabular_figures()
                        .text_color(p.text.secondary)
                        .child(format!("and {more} more")),
                );
            }
            panel = panel.child(list);
        }

        if let Some(error) = self.error.clone() {
            panel = panel.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(SPACE_1))
                    .text_style(TYPE_CAPTION)
                    .text_color(p.status.danger_fg)
                    .child(
                        Icon::new(IconName::OctagonAlert)
                            .size(IconSize::XSmall)
                            .color(p.status.danger_fg),
                    )
                    .child(error),
            );
        }

        let dismiss = self.on_dismiss.clone();
        let cancel_button = Button::new((self.id.clone(), "cancel"), self.cancel_label.clone())
            .variant(ButtonVariant::Secondary)
            .focus_handle(cancel)
            .disabled(self.busy)
            .on_click(move |window, cx| {
                if let Some(dismiss) = &dismiss {
                    dismiss(window, cx);
                }
            });
        let on_action = self.on_action.clone();
        let action_button = Button::new((self.id.clone(), "action"), self.action_label.clone())
            .variant(self.variant.action_style())
            .focus_handle(action)
            .disabled(self.busy || self.action_disabled)
            .loading(self.busy)
            .on_click(move |window, cx| {
                if let Some(on_action) = &on_action {
                    on_action(window, cx);
                }
            });
        panel = panel.child(
            div()
                .flex()
                .justify_end()
                .gap(px(SPACE_2))
                .child(
                    div()
                        .debug_selector(|| "dialog-cancel".into())
                        .child(cancel_button),
                )
                .child(
                    div()
                        .debug_selector(|| "dialog-action".into())
                        .child(action_button),
                ),
        );

        let scrim_closes = self.dismiss_on_scrim && self.variant.scrim_dismisses(false);
        let outside = self.on_dismiss.clone();

        let panel = if cx.reduce_motion() {
            panel.into_any_element()
        } else {
            // Opacity only: GPUI in the pinned version has no scale transform for divs.
            panel
                .with_animation(
                    (self.id.clone(), "enter"),
                    Animation::new(DURATION_MODERATE),
                    |el, delta| el.opacity(ease(EASE_ENTER, delta)),
                )
                .into_any_element()
        };
        if self.inline {
            return panel;
        }
        // The scrim is a sibling below the panel, not its parent: only a press on the scrim
        // itself dismisses. A press on the panel, or on an overlay opened from it, never reaches it.
        let scrim = div()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .size_full()
            .bg(p.scrim)
            .occlude()
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if scrim_closes && let Some(dismiss) = &outside {
                    dismiss(window, cx);
                }
            });
        let layer = div()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(scrim)
            .child(panel);
        deferred(layer)
            .priority(Z_DIALOG as usize)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loss_list_shows_five_rows_or_four_and_a_summary() {
        assert_eq!(loss_rows(0), (0, None));
        assert_eq!(loss_rows(5), (5, None));
        assert_eq!(loss_rows(6), (4, Some(2)), "four rows and 'and 2 more'");
        assert_eq!(loss_rows(8), (4, Some(4)), "the summary reads 'and 4 more'");
    }

    #[test]
    fn only_a_destructive_dialog_starts_on_cancel() {
        assert!(DialogVariant::Destructive.starts_on_cancel());
        assert!(!DialogVariant::Confirm.starts_on_cancel());
        assert!(!DialogVariant::Error.starts_on_cancel());
        assert!(!DialogVariant::OneField.starts_on_cancel());
    }

    #[test]
    fn the_scrim_closes_only_an_untyped_confirm_or_form() {
        assert!(DialogVariant::Confirm.scrim_dismisses(false));
        assert!(DialogVariant::OneField.scrim_dismisses(false));
        assert!(
            !DialogVariant::OneField.scrim_dismisses(true),
            "typed text is kept"
        );
        assert!(!DialogVariant::Destructive.scrim_dismisses(false));
        assert!(!DialogVariant::Error.scrim_dismisses(false));
    }

    #[test]
    fn destructive_dialogs_use_the_danger_action() {
        assert_eq!(
            DialogVariant::Destructive.action_style(),
            ButtonVariant::Danger
        );
        assert_eq!(
            DialogVariant::Confirm.action_style(),
            ButtonVariant::Primary
        );
        assert_eq!(DialogVariant::Error.action_style(), ButtonVariant::Primary);
    }

    #[test]
    fn tab_cycles_forward_and_back_through_the_stops_and_wraps() {
        assert_eq!(cycle(None, 2, false), Some(0));
        assert_eq!(cycle(None, 2, true), Some(1));
        assert_eq!(cycle(Some(0), 2, false), Some(1));
        assert_eq!(cycle(Some(1), 2, false), Some(0), "wraps forward");
        assert_eq!(cycle(Some(0), 2, true), Some(1), "wraps back");
        assert_eq!(cycle(Some(0), 0, false), None);
    }

    #[test]
    fn widths_are_the_three_dialog_caps() {
        assert_eq!(DialogWidth::Small.px(), 400.0);
        assert_eq!(DialogWidth::Medium.px(), 440.0);
        assert_eq!(DialogWidth::Large.px(), 560.0);
        assert_eq!(DialogWidth::default().px(), 440.0);
    }
}

#[cfg(test)]
mod element_tests {
    use super::*;
    use crate::components::button::Button;
    use crate::components::button::test_support::{focus_and_draw, press_key};
    use crate::components::menu::{
        MenuChoice, MenuEntry, MenuEvent, MenuItem, MenuOrigin, MenuView, menu_overlay,
    };
    use crate::components::popover::OverlayLayer;
    use gpui::{
        AppContext, Context, Entity, Modifiers, Render, TestAppContext, VisualTestContext, point,
    };
    use std::cell::{Cell, RefCell};

    type Count = Rc<Cell<u32>>;

    /// Hosts an opener button and one dialog whose state the test drives. When a menu is set,
    /// the dialog body opens it at the window's top-left corner, over the scrim.
    struct Harness {
        opener: FocusHandle,
        open: bool,
        busy: bool,
        variant: DialogVariant,
        dismiss_on_scrim: bool,
        menu: Option<Entity<MenuView>>,
        actions: Count,
        dismissals: Count,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let actions = self.actions.clone();
            let dismissals = self.dismissals.clone();
            let mut dialog = Dialog::new("dialog-under-test", "Delete secret DB_PASSWORD?")
                .variant(self.variant)
                .open(self.open)
                .busy(self.busy)
                .dismiss_on_scrim(self.dismiss_on_scrim)
                .loss(["3 projects", "2 routines stop"])
                .action("Delete secret (3 projects use it)")
                .on_action(move |_, _| actions.set(actions.get() + 1))
                .on_dismiss(move |_, _| dismissals.set(dismissals.get() + 1));
            if let Some(menu) = self.menu.clone() {
                let origin = MenuOrigin::At(point(px(20.), px(20.)));
                dialog = dialog.child(menu_overlay(&menu, origin, cx));
            }
            div()
                .p(px(SPACE_2))
                .child(Button::new("dialog-opener", "Delete").focus_handle(self.opener.clone()))
                .child(dialog)
        }
    }

    /// Opens a window with the harness, focuses its opener and returns the window context.
    fn open_window(
        cx: &mut TestAppContext,
        variant: DialogVariant,
    ) -> (
        Entity<Harness>,
        &mut VisualTestContext,
        Count,
        Count,
        FocusHandle,
    ) {
        let actions: Count = Rc::default();
        let dismissals: Count = Rc::default();
        let (view, cx) = cx.add_window_view(|_, cx| Harness {
            opener: cx.focus_handle(),
            open: false,
            busy: false,
            variant,
            dismiss_on_scrim: false,
            menu: None,
            actions: actions.clone(),
            dismissals: dismissals.clone(),
        });
        cx.run_until_parked();
        let opener = view.read_with(cx, |h, _| h.opener.clone());
        focus_and_draw(cx, &opener);
        (view, cx, actions, dismissals, opener)
    }

    fn set_open(view: &Entity<Harness>, cx: &mut VisualTestContext, open: bool) {
        view.update(cx, |harness, cx| {
            harness.open = open;
            cx.notify();
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
    }

    /// Opens a two-row menu on `layer` from the dialog body, scrim dismissal enabled, and returns
    /// the choices the menu reports.
    fn open_menu_on(
        view: &Entity<Harness>,
        cx: &mut VisualTestContext,
        layer: OverlayLayer,
    ) -> Rc<RefCell<Vec<MenuEvent>>> {
        let entries: Vec<MenuEntry> = vec![
            MenuItem::new("Rename").into(),
            MenuItem::new("Duplicate").into(),
        ];
        let menu = cx
            .update(|window, cx| cx.new(|cx| MenuView::new(entries, window, cx).with_layer(layer)));
        let choices: Rc<RefCell<Vec<MenuEvent>>> = Rc::default();
        let sink = choices.clone();
        cx.update(|_, cx| {
            cx.subscribe(&menu, move |_, event: &MenuEvent, _| {
                sink.borrow_mut().push(*event);
            })
            .detach();
        });
        view.update(cx, |harness, cx| {
            harness.dismiss_on_scrim = true;
            harness.menu = Some(menu);
            cx.notify();
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        choices
    }

    #[gpui::test]
    fn a_destructive_dialog_starts_on_cancel_so_enter_cancels(cx: &mut TestAppContext) {
        let (view, cx, actions, dismissals, _opener) = open_window(cx, DialogVariant::Destructive);
        set_open(&view, cx, true);
        press_key(cx, "enter");
        assert_eq!(dismissals.get(), 1, "Enter on the default focus cancels");
        assert_eq!(
            actions.get(),
            0,
            "the danger action is never the Enter target"
        );
    }

    #[gpui::test]
    fn escape_dismisses_and_is_ignored_while_busy(cx: &mut TestAppContext) {
        let (view, cx, _actions, dismissals, _opener) = open_window(cx, DialogVariant::Confirm);
        set_open(&view, cx, true);
        press_key(cx, "escape");
        assert_eq!(dismissals.get(), 1);

        view.update(cx, |harness, cx| {
            harness.busy = true;
            cx.notify();
        });
        set_open(&view, cx, true);
        press_key(cx, "escape");
        assert_eq!(dismissals.get(), 1, "Escape does nothing while busy");
    }

    #[gpui::test]
    fn tab_and_shift_tab_cycle_between_cancel_and_the_action(cx: &mut TestAppContext) {
        let (view, cx, actions, dismissals, _opener) = open_window(cx, DialogVariant::Confirm);
        set_open(&view, cx, true);
        // A confirm starts on its action; Tab wraps to Cancel, Shift+Tab returns to the action.
        press_key(cx, "tab");
        press_key(cx, "enter");
        assert_eq!(dismissals.get(), 1);
        assert_eq!(actions.get(), 0);

        set_open(&view, cx, true);
        press_key(cx, "shift-tab");
        press_key(cx, "enter");
        assert_eq!(actions.get(), 1);
    }

    #[gpui::test]
    fn clicking_the_action_runs_it(cx: &mut TestAppContext) {
        let (view, cx, actions, dismissals, _opener) = open_window(cx, DialogVariant::Destructive);
        set_open(&view, cx, true);
        let action = cx
            .debug_bounds("dialog-action")
            .expect("the action is drawn")
            .center();
        cx.simulate_click(action, Modifiers::none());
        assert_eq!(actions.get(), 1);
        assert_eq!(dismissals.get(), 0);
    }

    #[gpui::test]
    fn closing_the_dialog_returns_focus_to_the_opener(cx: &mut TestAppContext) {
        let (view, cx, _actions, _dismissals, opener) = open_window(cx, DialogVariant::Confirm);
        set_open(&view, cx, true);
        cx.update(|window, _| assert!(!opener.is_focused(window), "focus moved into the dialog"));
        set_open(&view, cx, false);
        cx.update(|window, _| assert!(opener.is_focused(window), "focus is back on the opener"));
    }

    #[gpui::test]
    fn a_press_on_the_scrim_dismisses_only_when_allowed_and_a_press_on_the_panel_never_does(
        cx: &mut TestAppContext,
    ) {
        let (view, cx, _actions, dismissals, _opener) = open_window(cx, DialogVariant::Confirm);
        set_open(&view, cx, true);
        cx.simulate_click(point(px(4.), px(4.)), Modifiers::none());
        assert_eq!(dismissals.get(), 0, "scrim dismissal is off by default");

        view.update(cx, |harness, cx| {
            harness.dismiss_on_scrim = true;
            cx.notify();
        });
        set_open(&view, cx, true);
        let panel = cx.debug_bounds("dialog-panel").expect("the panel is drawn");
        cx.simulate_click(panel.origin + point(px(24.), px(24.)), Modifiers::none());
        assert_eq!(
            dismissals.get(),
            0,
            "a press on the panel is not outside it"
        );

        cx.simulate_click(point(px(4.), px(4.)), Modifiers::none());
        assert_eq!(dismissals.get(), 1, "a press on the scrim dismisses");
    }

    #[gpui::test]
    fn a_menu_opened_from_a_dialog_takes_clicks_above_the_scrim(cx: &mut TestAppContext) {
        let (view, cx, _actions, dismissals, _opener) = open_window(cx, DialogVariant::Confirm);
        set_open(&view, cx, true);
        let choices = open_menu_on(&view, cx, OverlayLayer::Dialog);

        let row = cx
            .debug_bounds("menu-item-1")
            .expect("the menu row is drawn")
            .center();
        cx.simulate_click(row, Modifiers::none());
        assert_eq!(
            *choices.borrow(),
            vec![MenuEvent::Chosen(MenuChoice {
                row: 1,
                child: None
            })],
            "the menu row, not the scrim, takes the click"
        );
        assert_eq!(
            dismissals.get(),
            0,
            "a press on the menu does not dismiss the dialog"
        );
    }
}
