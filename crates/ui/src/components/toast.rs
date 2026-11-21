//! Toast: a short, passing message with a status glyph, an optional single action
//! and a close button (docs/design/components/Toast.md). It reports a result and
//! never asks for a decision. Toasts never take focus on their own; a toast can
//! hold focus once the user moves there, and Escape dismisses the focused toast.
//!
//! The stack's rules (arrival order, the visible limit, timers, duplicates) live
//! in [`ToastQueue`], a plain struct that takes the time it runs at, so tests
//! drive it with explicit instants.

use crate::components::badge::Badge;
use crate::components::button::{Button, ButtonSize, ButtonVariant, ClickHandler};
use crate::components::icon_button::{IconButton, IconButtonVariant};
use crate::components::switch::ease;
use crate::icon::{Icon, IconName, IconSize};
use crate::style::{FocusRing, Surface};
use crate::theme::{ActiveTheme, TextStyled, box_shadows};
use crate::tokens::{
    Color, DURATION_BASE, DURATION_MODERATE, EASE_ENTER, EASE_EXIT, LAYOUT_TOAST_WIDTH_DEFAULT,
    LAYOUT_TOAST_WIDTH_MAX, LAYOUT_TOAST_WIDTH_MIN, Palette, RADIUS_LG, SIZE_TOUCH_DEFAULT,
    SPACE_2, SPACE_3, SPACE_4, Z_TOAST,
};
use gpui::{
    Animation, AnimationExt, AnyElement, App, AppContext, Context, ElementId, Entity, FocusHandle,
    Global, InteractiveElement, IntoElement, ParentElement, Render, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, deferred, div, px,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Auto-dismiss time of an info, success or warning toast without an action (Toast.md).
pub const AUTO_DISMISS: Duration = Duration::from_secs(8);

/// Time an Undo toast stays up (design-system 4.8, `UNDO_TIMEOUT_MS`).
pub const UNDO_TIMEOUT_MS: u64 = 8_000;

/// An identical message within this window counts as a repeat (Toast.md, duplicates).
pub const DUPLICATE_WINDOW: Duration = Duration::from_secs(2);

/// At most this many toasts are visible at once; the rest wait their turn (Toast.md, stack).
pub const MAX_VISIBLE: usize = 3;

/// The kind of a toast: it sets the glyph and the colour, and whether it times out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToastKind {
    #[default]
    Info,
    Success,
    Warning,
    /// Stays until dismissed, so a failure is never missed.
    Error,
}

impl ToastKind {
    fn icon(self) -> IconName {
        match self {
            Self::Info => IconName::Info,
            Self::Success => IconName::CircleCheck,
            Self::Warning => IconName::TriangleAlert,
            Self::Error => IconName::OctagonAlert,
        }
    }

    fn colour(self, palette: &Palette) -> Color {
        match self {
            Self::Info => palette.status.info_fg,
            Self::Success => palette.status.success_fg,
            Self::Warning => palette.status.warning_fg,
            Self::Error => palette.status.danger_fg,
        }
    }
}

/// The single action of a toast. An Undo action times out; any other action stays until used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToastActionSpec {
    pub label: SharedString,
    pub undo: bool,
}

/// What a toast says: its kind, its message and its optional action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToastSpec {
    pub kind: ToastKind,
    pub message: SharedString,
    pub action: Option<ToastActionSpec>,
}

impl ToastSpec {
    pub fn new(kind: ToastKind, message: impl Into<SharedString>) -> Self {
        Self {
            kind,
            message: message.into(),
            action: None,
        }
    }

    /// An action button. `undo` marks an Undo action, which times out after `UNDO_TIMEOUT_MS`.
    pub fn with_action(mut self, label: impl Into<SharedString>, undo: bool) -> Self {
        self.action = Some(ToastActionSpec {
            label: label.into(),
            undo,
        });
        self
    }

    /// How long this toast stays before it dismisses itself, or `None` to stay until dismissed.
    fn timeout(&self) -> Option<Duration> {
        match &self.action {
            Some(action) if action.undo => Some(Duration::from_millis(UNDO_TIMEOUT_MS)),
            Some(_) => None,
            None => match self.kind {
                ToastKind::Error => None,
                _ => Some(AUTO_DISMISS),
            },
        }
    }
}

/// Identifies one toast in a [`ToastQueue`].
pub type ToastId = u64;

#[derive(Clone, Debug)]
struct Entry {
    id: ToastId,
    spec: ToastSpec,
    repeats: u32,
    last_push: Instant,
    /// Time left before the toast dismisses itself. It only runs while the toast is visible.
    remaining: Option<Duration>,
    hovered: bool,
    focused: bool,
    /// Time left in the leave animation, once the toast is dismissed.
    leaving: Option<Duration>,
}

impl Entry {
    fn paused(&self, window_active: bool) -> bool {
        self.hovered || self.focused || !window_active
    }

    /// The time that counts down for this toast right now: its leave time once it leaves.
    fn timer(&self) -> Option<Duration> {
        self.leaving.or(self.remaining)
    }

    fn timer_mut(&mut self) -> Option<&mut Duration> {
        if self.leaving.is_some() {
            self.leaving.as_mut()
        } else {
            self.remaining.as_mut()
        }
    }
}

/// The toast stack's rules as plain data: arrival order, the visible limit, the timers
/// (paused by hover, focus and window blur), duplicate counting and the leave animation.
/// Every method takes the instant it runs at; time only moves forward through those calls.
#[derive(Debug)]
pub struct ToastQueue {
    next_id: ToastId,
    clock: Instant,
    entries: Vec<Entry>,
    window_active: bool,
    leave: Duration,
}

impl ToastQueue {
    pub fn new(now: Instant) -> Self {
        Self {
            next_id: 1,
            clock: now,
            entries: Vec::new(),
            window_active: true,
            leave: DURATION_BASE,
        }
    }

    /// Reduced motion removes the leave fade: a dismissed toast goes at once. Timers are unchanged.
    pub fn set_reduced_motion(&mut self, reduced: bool) {
        self.leave = if reduced {
            Duration::ZERO
        } else {
            DURATION_BASE
        };
    }

    /// Adds a toast. An identical toast within `DUPLICATE_WINDOW` of the last one becomes a repeat.
    pub fn push(&mut self, now: Instant, spec: ToastSpec) -> ToastId {
        self.settle(now);
        if let Some(entry) = self.entries.iter_mut().find(|e| {
            e.leaving.is_none()
                && e.spec == spec
                && now.saturating_duration_since(e.last_push) <= DUPLICATE_WINDOW
        }) {
            entry.repeats += 1;
            entry.last_push = now;
            return entry.id;
        }
        let id = self.next_id;
        self.next_id += 1;
        let remaining = spec.timeout();
        self.entries.push(Entry {
            id,
            spec,
            repeats: 1,
            last_push: now,
            remaining,
            hovered: false,
            focused: false,
            leaving: None,
        });
        id
    }

    /// Moves the clock to `now`: expires timers and finishes leave animations.
    /// Returns the toasts removed from the stack.
    pub fn advance(&mut self, now: Instant) -> Vec<ToastId> {
        self.settle(now);
        self.expire();
        self.sweep()
    }

    /// Starts the leave animation of a toast. Returns `false` when it is unknown or already leaving.
    pub fn dismiss(&mut self, now: Instant, id: ToastId) -> bool {
        self.settle(now);
        let leave = self.leave;
        let Some(entry) = self
            .entries
            .iter_mut()
            .find(|e| e.id == id && e.leaving.is_none())
        else {
            return false;
        };
        entry.leaving = Some(leave);
        entry.remaining = None;
        self.sweep();
        true
    }

    pub fn set_hovered(&mut self, now: Instant, id: ToastId, hovered: bool) {
        self.settle(now);
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.hovered = hovered;
        }
    }

    pub fn set_focused(&mut self, now: Instant, id: ToastId, focused: bool) {
        self.settle(now);
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.focused = focused;
        }
    }

    /// The window's activity: while it is not focused, no timer runs.
    pub fn set_window_active(&mut self, now: Instant, active: bool) {
        self.settle(now);
        self.window_active = active;
    }

    /// The toasts that are drawn, in arrival order: the visible ones and the leaving ones.
    pub fn displayed(&self) -> Vec<ToastId> {
        let visible = self.visible();
        self.entries
            .iter()
            .filter(|e| e.leaving.is_some() || visible.contains(&e.id))
            .map(|e| e.id)
            .collect()
    }

    /// The toasts that are visible: the first `MAX_VISIBLE` that are not leaving.
    pub fn visible(&self) -> Vec<ToastId> {
        self.entries
            .iter()
            .filter(|e| e.leaving.is_none())
            .take(MAX_VISIBLE)
            .map(|e| e.id)
            .collect()
    }

    pub fn contains(&self, id: ToastId) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn is_leaving(&self, id: ToastId) -> bool {
        self.entries
            .iter()
            .any(|e| e.id == id && e.leaving.is_some())
    }

    pub fn spec(&self, id: ToastId) -> Option<&ToastSpec> {
        self.entries.iter().find(|e| e.id == id).map(|e| &e.spec)
    }

    pub fn repeats(&self, id: ToastId) -> Option<u32> {
        self.entries.iter().find(|e| e.id == id).map(|e| e.repeats)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Runs the clock forward to `now` one expiry at a time, so a toast that becomes visible
    /// mid-way starts its timer at that moment. Leaving toasts fade; visible timers count down
    /// unless paused.
    fn settle(&mut self, now: Instant) {
        while self.clock < now {
            let visible = self.visible();
            let window_active = self.window_active;
            let counting = |entry: &Entry| {
                entry.leaving.is_some()
                    || (visible.contains(&entry.id)
                        && !entry.paused(window_active)
                        && entry.remaining.is_some())
            };
            let mut step = now.saturating_duration_since(self.clock);
            for entry in self.entries.iter().filter(|e| counting(e)) {
                if let Some(left) = entry.timer().filter(|left| !left.is_zero()) {
                    step = step.min(left);
                }
            }
            for entry in self.entries.iter_mut().filter(|e| counting(e)) {
                if let Some(left) = entry.timer_mut() {
                    *left = left.saturating_sub(step);
                }
            }
            self.clock += step;
            self.expire();
        }
    }

    /// Starts the leave animation of every visible toast whose timer has run out.
    fn expire(&mut self) {
        let visible = self.visible();
        let leave = self.leave;
        for entry in &mut self.entries {
            if visible.contains(&entry.id) && entry.remaining.is_some_and(|left| left.is_zero()) {
                entry.remaining = None;
                entry.leaving = Some(leave);
            }
        }
    }

    /// Drops the toasts whose leave animation has finished.
    fn sweep(&mut self) -> Vec<ToastId> {
        let mut removed = Vec::new();
        self.entries.retain(|e| {
            let done = e.leaving.is_some_and(|left| left.is_zero());
            if done {
                removed.push(e.id);
            }
            !done
        });
        removed
    }
}

/// One toast: glyph, message, a repeat counter, an optional action, and a close button.
#[derive(IntoElement)]
pub struct Toast {
    id: ElementId,
    kind: ToastKind,
    message: SharedString,
    repeats: u32,
    action: Option<(SharedString, ClickHandler)>,
    action_loading: bool,
    leaving: bool,
    focus: Option<FocusHandle>,
    on_dismiss: Option<ClickHandler>,
}

impl std::fmt::Debug for Toast {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Toast")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("message", &self.message)
            .field("repeats", &self.repeats)
            .field("leaving", &self.leaving)
            .finish_non_exhaustive()
    }
}

impl Toast {
    pub fn new(
        id: impl Into<ElementId>,
        kind: ToastKind,
        message: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            message: message.into(),
            repeats: 1,
            action: None,
            action_loading: false,
            leaving: false,
            focus: None,
            on_dismiss: None,
        }
    }

    /// The repeat counter: a chip "x2" and so on when the same message came again.
    pub fn repeats(mut self, repeats: u32) -> Self {
        self.repeats = repeats;
        self
    }

    pub fn action(
        mut self,
        label: impl Into<SharedString>,
        listener: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        self.action = Some((label.into(), std::rc::Rc::new(listener)));
        self
    }

    /// While the action's request runs, its button shows a spinner and cannot be activated.
    pub fn action_loading(mut self, loading: bool) -> Self {
        self.action_loading = loading;
        self
    }

    /// Fades the toast out instead of showing it.
    pub fn leaving(mut self, leaving: bool) -> Self {
        self.leaving = leaving;
        self
    }

    /// Keyboard focus lives with the caller; otherwise the toast keeps its own handle.
    pub fn focus_handle(mut self, handle: FocusHandle) -> Self {
        self.focus = Some(handle);
        self
    }

    /// Runs when the close button is pressed or Escape is pressed on the focused toast.
    pub fn on_dismiss(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_dismiss = Some(std::rc::Rc::new(listener));
        self
    }
}

impl RenderOnce for Toast {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let reduced = cx.reduce_motion();
        let focus = self.focus.clone().unwrap_or_else(|| {
            window
                .use_keyed_state((self.id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
                .read(cx)
                .clone()
        });
        let on_dismiss = self.on_dismiss.clone();
        let leaving = self.leaving;
        let id = self.id.clone();

        let mut panel = div()
            .id(self.id.clone())
            .relative()
            .track_focus(&focus)
            .focus_ring(&theme)
            .w(px(LAYOUT_TOAST_WIDTH_DEFAULT))
            .min_w(px(LAYOUT_TOAST_WIDTH_MIN))
            .max_w(px(LAYOUT_TOAST_WIDTH_MAX))
            .min_h(px(SIZE_TOUCH_DEFAULT))
            .flex()
            .items_center()
            .gap(px(SPACE_2))
            .p(px(SPACE_3))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(p.border.default)
            .bg(p.surface.overlay)
            .shadow(box_shadows(theme.shadows.e2))
            .text_color(p.text.primary)
            .child(
                div().flex_none().child(
                    Icon::new(self.kind.icon())
                        .size(IconSize::Small)
                        .color(self.kind.colour(&p)),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_body()
                    .line_clamp(3)
                    .child(self.message),
            );
        if self.repeats > 1 {
            panel = panel.child(
                div()
                    .flex_none()
                    .child(Badge::text(format!("x{}", self.repeats))),
            );
        }
        if let Some((label, on_click)) = self.action {
            let action = Button::new((self.id.clone(), "action"), label)
                .variant(ButtonVariant::Ghost)
                .size(ButtonSize::Compact)
                .surface(Surface::Overlay)
                .loading(self.action_loading)
                .on_click(move |window, cx| on_click(window, cx));
            panel = panel.child(
                div()
                    .debug_selector(|| "toast-action".into())
                    .flex_none()
                    .child(action),
            );
        }
        let mut dismiss = IconButton::new((self.id.clone(), "dismiss"), IconName::X, "Dismiss")
            .variant(IconButtonVariant::Ghost)
            .size(ButtonSize::Compact)
            .surface(Surface::Overlay);
        if let Some(on_dismiss) = on_dismiss.clone() {
            dismiss = dismiss.on_click(move |window, cx| on_dismiss(window, cx));
        }
        panel = panel.child(
            div()
                .debug_selector(|| "toast-dismiss".into())
                .flex_none()
                .child(dismiss),
        );
        if let Some(on_dismiss) = on_dismiss {
            panel = panel.on_key_down(move |event, window, cx| {
                if event.keystroke.key == "escape" {
                    on_dismiss(window, cx);
                    cx.stop_propagation();
                }
            });
        }

        if reduced {
            panel.into_any_element()
        } else if leaving {
            panel
                .with_animation(
                    (id, "leave"),
                    Animation::new(DURATION_BASE).with_easing(|t| ease(EASE_EXIT, t)),
                    |toast, t| toast.opacity(1.0 - t),
                )
                .into_any_element()
        } else {
            panel
                .with_animation(
                    (id, "enter"),
                    Animation::new(DURATION_MODERATE).with_easing(|t| ease(EASE_ENTER, t)),
                    |toast, t| toast.opacity(t).top(px(SPACE_2 * (1.0 - t))),
                )
                .into_any_element()
        }
    }
}

/// How often the stack reads the clock while toasts are up.
const TICK: Duration = Duration::from_millis(100);

/// The app's toast stack. It draws every toast bottom right of its nearest positioned ancestor
/// (the canvas), `space.4` from the right edge and from the bottom, newest at the bottom. It runs
/// its clock only while toasts are up.
pub struct ToastStack {
    queue: ToastQueue,
    actions: HashMap<ToastId, ClickHandler>,
    focus: HashMap<ToastId, FocusHandle>,
    ticking: bool,
}

impl std::fmt::Debug for ToastStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToastStack")
            .field("queue", &self.queue)
            .finish_non_exhaustive()
    }
}

/// The stack's clock: the executor's, which tests can move with `advance_clock`.
fn now(cx: &App) -> Instant {
    cx.background_executor().now()
}

impl ToastStack {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            queue: ToastQueue::new(now(cx)),
            actions: HashMap::new(),
            focus: HashMap::new(),
            ticking: false,
        }
    }

    /// Shows a toast. An identical toast within `DUPLICATE_WINDOW` counts as a repeat.
    pub fn show(
        &mut self,
        spec: ToastSpec,
        on_action: Option<ClickHandler>,
        cx: &mut Context<Self>,
    ) -> ToastId {
        let id = self.queue.push(now(cx), spec);
        if let Some(handler) = on_action {
            self.actions.entry(id).or_insert(handler);
        }
        self.focus.entry(id).or_insert_with(|| cx.focus_handle());
        self.start_ticking(cx);
        cx.notify();
        id
    }

    /// Starts a toast's leave animation, as its close button or Escape does.
    pub fn dismiss(&mut self, id: ToastId, cx: &mut Context<Self>) {
        self.queue.dismiss(now(cx), id);
        self.prune();
        cx.notify();
    }

    fn set_hovered(&mut self, id: ToastId, hovered: bool, cx: &mut Context<Self>) {
        self.queue.set_hovered(now(cx), id, hovered);
        cx.notify();
    }

    fn start_ticking(&mut self, cx: &mut Context<Self>) {
        if self.ticking {
            return;
        }
        self.ticking = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let keep = this.update(cx, |stack, cx| stack.tick(cx)).unwrap_or(false);
                if !keep {
                    break;
                }
            }
        })
        .detach();
    }

    /// Moves the clock on. Returns whether toasts are still up, so the ticker keeps running.
    fn tick(&mut self, cx: &mut Context<Self>) -> bool {
        self.queue.advance(now(cx));
        self.prune();
        cx.notify();
        self.ticking = !self.queue.is_empty();
        self.ticking
    }

    /// Forgets the actions and focus handles of toasts the queue has removed.
    fn prune(&mut self) {
        let queue = &self.queue;
        self.actions.retain(|id, _| queue.contains(*id));
        self.focus.retain(|id, _| queue.contains(*id));
    }
}

impl Render for ToastStack {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let at = now(cx);
        self.queue.set_reduced_motion(cx.reduce_motion());
        self.queue.set_window_active(at, window.is_window_active());
        for id in self.queue.displayed() {
            if let Some(focus) = self.focus.get(&id) {
                let focused = focus.contains_focused(window, cx);
                self.queue.set_focused(at, id, focused);
            }
        }

        let stack = cx.entity();
        let slots: Vec<AnyElement> = self
            .queue
            .displayed()
            .into_iter()
            .filter_map(|id| {
                let spec = self.queue.spec(id)?.clone();
                let focus = self.focus.get(&id)?.clone();
                let on_dismiss = stack.clone();
                let mut toast = Toast::new(("toast", id), spec.kind, spec.message.clone())
                    .repeats(self.queue.repeats(id).unwrap_or(1))
                    .leaving(self.queue.is_leaving(id))
                    .focus_handle(focus)
                    .on_dismiss(move |_, cx| on_dismiss.update(cx, |s, cx| s.dismiss(id, cx)));
                if let Some(action) = &spec.action {
                    let handler = self.actions.get(&id).cloned();
                    let on_action = stack.clone();
                    toast = toast.action(action.label.clone(), move |window, cx| {
                        if let Some(handler) = &handler {
                            handler(window, cx);
                        }
                        on_action.update(cx, |s, cx| s.dismiss(id, cx));
                    });
                }
                let on_hover = stack.clone();
                Some(
                    div()
                        .id(("toast-slot", id))
                        .on_hover(move |hovered, _, cx| {
                            on_hover.update(cx, |s, cx| s.set_hovered(id, *hovered, cx))
                        })
                        .child(toast)
                        .into_any_element(),
                )
            })
            .collect();

        deferred(
            div()
                .absolute()
                .right(px(SPACE_4))
                .bottom(px(SPACE_4))
                .w(px(LAYOUT_TOAST_WIDTH_DEFAULT))
                .flex()
                .flex_col()
                .items_end()
                .gap(px(SPACE_2))
                .children(slots),
        )
        .with_priority(Z_TOAST as usize)
    }
}

struct ToastStackGlobal(Entity<ToastStack>);

impl Global for ToastStackGlobal {}

/// Shows toasts from anywhere with `&mut App`. The shared stack is created on first use.
#[derive(Debug)]
pub struct Toasts;

impl Toasts {
    /// The shared stack. Put it inside the canvas, which must be positioned, so it sits bottom right.
    pub fn stack(cx: &mut App) -> Entity<ToastStack> {
        if let Some(global) = cx.try_global::<ToastStackGlobal>() {
            return global.0.clone();
        }
        let stack = cx.new(ToastStack::new);
        cx.set_global(ToastStackGlobal(stack.clone()));
        stack
    }

    pub fn show(cx: &mut App, spec: ToastSpec) -> ToastId {
        Self::stack(cx).update(cx, |stack, cx| stack.show(spec, None, cx))
    }

    /// Shows a toast whose action runs `on_action`; acting on the toast dismisses it.
    pub fn show_with_action(
        cx: &mut App,
        spec: ToastSpec,
        on_action: impl Fn(&mut Window, &mut App) + 'static,
    ) -> ToastId {
        let handler: ClickHandler = std::rc::Rc::new(on_action);
        Self::stack(cx).update(cx, |stack, cx| stack.show(spec, Some(handler), cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::test_support::{focus_and_draw, press_key};
    use std::cell::Cell;
    use std::rc::Rc;

    fn info(message: &str) -> ToastSpec {
        ToastSpec::new(ToastKind::Info, message.to_string())
    }

    fn at(t0: Instant, secs: f32) -> Instant {
        t0 + Duration::from_secs_f32(secs)
    }

    #[test]
    fn an_info_toast_clears_after_eight_seconds() {
        let t0 = Instant::now();
        let mut queue = ToastQueue::new(t0);
        let id = queue.push(t0, info("Copied"));
        queue.advance(at(t0, 7.9));
        assert_eq!(queue.visible(), vec![id]);
        queue.advance(at(t0, 8.0));
        assert!(queue.is_leaving(id), "leaving once the timer runs out");
        queue.advance(at(t0, 8.0) + DURATION_BASE);
        assert!(queue.is_empty());
    }

    #[test]
    fn an_error_toast_stays_until_dismissed() {
        let t0 = Instant::now();
        let mut queue = ToastQueue::new(t0);
        let id = queue.push(
            t0,
            ToastSpec::new(ToastKind::Error, "Could not rename the session"),
        );
        queue.advance(at(t0, 600.0));
        assert_eq!(queue.visible(), vec![id]);
        assert!(queue.dismiss(at(t0, 601.0), id));
        assert!(queue.is_leaving(id), "fades for the leave time first");
        queue.advance(at(t0, 601.0) + DURATION_BASE);
        assert!(queue.is_empty());
    }

    #[test]
    fn an_undo_toast_times_out_and_other_actions_stay() {
        let t0 = Instant::now();
        let mut queue = ToastQueue::new(t0);
        let undo = queue.push(
            t0,
            ToastSpec::new(ToastKind::Success, "Removed from queue").with_action("Undo", true),
        );
        let retry = queue.push(
            t0,
            ToastSpec::new(ToastKind::Error, "Setup script failed").with_action("Open log", false),
        );
        queue.advance(at(t0, 8.0));
        assert!(queue.is_leaving(undo));
        assert!(
            !queue.is_leaving(retry),
            "an action toast stays until acted on"
        );
        assert_eq!(UNDO_TIMEOUT_MS, 8_000, "design-system 4.8");
    }

    #[test]
    fn hovering_pauses_the_timer_and_leaving_resumes_it() {
        let t0 = Instant::now();
        let mut queue = ToastQueue::new(t0);
        let id = queue.push(t0, info("Copied"));
        queue.set_hovered(at(t0, 1.0), id, true);
        queue.advance(at(t0, 100.0));
        assert!(!queue.is_leaving(id), "paused while hovered");
        queue.set_hovered(at(t0, 100.0), id, false);
        queue.advance(at(t0, 106.9));
        assert!(!queue.is_leaving(id), "seven seconds were left");
        queue.advance(at(t0, 107.0));
        assert!(queue.is_leaving(id));
    }

    #[test]
    fn focus_and_window_blur_pause_the_timer_together() {
        let t0 = Instant::now();
        let mut queue = ToastQueue::new(t0);
        let id = queue.push(t0, info("Copied"));
        queue.set_focused(at(t0, 2.0), id, true);
        queue.set_window_active(at(t0, 3.0), false);
        queue.set_focused(at(t0, 10.0), id, false);
        queue.advance(at(t0, 20.0));
        assert!(!queue.is_leaving(id), "the window is still blurred");
        queue.set_window_active(at(t0, 20.0), true);
        queue.advance(at(t0, 25.9));
        assert!(!queue.is_leaving(id), "six seconds were left at 2 s");
        queue.advance(at(t0, 26.0));
        assert!(queue.is_leaving(id));
    }

    #[test]
    fn three_toasts_are_visible_and_the_rest_wait_their_turn() {
        let t0 = Instant::now();
        let mut queue = ToastQueue::new(t0);
        let ids: Vec<_> = ["a", "b", "c", "d"]
            .iter()
            .map(|m| queue.push(t0, info(m)))
            .collect();
        assert_eq!(queue.visible(), ids[..3].to_vec());
        assert_eq!(queue.displayed(), ids[..3].to_vec());
        assert_eq!(queue.repeats(ids[3]), Some(1));
    }

    #[test]
    fn a_waiting_toast_starts_its_timer_only_when_it_becomes_visible() {
        let t0 = Instant::now();
        let mut queue = ToastQueue::new(t0);
        let ids: Vec<_> = ["a", "b", "c", "d"]
            .iter()
            .map(|m| queue.push(t0, info(m)))
            .collect();
        queue.dismiss(at(t0, 5.0), ids[0]);
        assert_eq!(queue.visible(), vec![ids[1], ids[2], ids[3]]);
        queue.advance(at(t0, 12.9));
        assert!(!queue.is_leaving(ids[3]), "its eight seconds start at 5 s");
        queue.advance(at(t0, 13.0));
        assert!(queue.is_leaving(ids[3]));
    }

    #[test]
    fn an_identical_message_within_two_seconds_is_a_repeat() {
        let t0 = Instant::now();
        let mut queue = ToastQueue::new(t0);
        let first = queue.push(t0, info("Copied"));
        assert_eq!(queue.push(at(t0, 1.5), info("Copied")), first);
        assert_eq!(queue.repeats(first), Some(2));
        assert_eq!(
            queue.push(at(t0, 3.4), info("Copied")),
            first,
            "the window slides"
        );
        assert_eq!(queue.repeats(first), Some(3));
        let later = queue.push(at(t0, 5.5), info("Copied"));
        assert_ne!(later, first, "more than two seconds since the last repeat");
        assert_eq!(queue.displayed().len(), 2);
    }

    #[test]
    fn toasts_are_displayed_in_arrival_order() {
        let t0 = Instant::now();
        let mut queue = ToastQueue::new(t0);
        let first = queue.push(t0, info("one"));
        let second = queue.push(t0, info("two"));
        assert_eq!(queue.displayed(), vec![first, second]);
    }

    #[test]
    fn reduced_motion_removes_a_dismissed_toast_at_once() {
        let t0 = Instant::now();
        let mut queue = ToastQueue::new(t0);
        queue.set_reduced_motion(true);
        let id = queue.push(t0, info("Copied"));
        assert!(queue.dismiss(t0, id));
        assert!(queue.is_empty());
    }

    #[test]
    fn dismissing_an_unknown_toast_does_nothing() {
        let t0 = Instant::now();
        let mut queue = ToastQueue::new(t0);
        assert!(!queue.dismiss(t0, 99));
        let id = queue.push(t0, info("Copied"));
        assert!(queue.dismiss(t0, id));
        assert!(!queue.dismiss(t0, id), "already leaving");
    }

    #[test]
    fn advance_reports_the_toasts_it_removed() {
        let t0 = Instant::now();
        let mut queue = ToastQueue::new(t0);
        let id = queue.push(t0, info("Copied"));
        assert!(queue.advance(at(t0, 8.0)).is_empty(), "still fading");
        assert_eq!(queue.advance(at(t0, 8.0) + DURATION_BASE), vec![id]);
    }

    /// Moves the test clock on in ticker-sized steps, letting the stack's timer run at each one.
    fn advance_in_ticks(cx: &mut gpui::TestAppContext, by: Duration) {
        let mut left = by;
        while !left.is_zero() {
            let step = left.min(TICK);
            cx.executor().advance_clock(step);
            cx.run_until_parked();
            left -= step;
        }
    }

    #[gpui::test]
    fn an_info_toast_leaves_the_stack_on_the_test_clock(cx: &mut gpui::TestAppContext) {
        let stack = cx.update(|cx| {
            Toasts::show(cx, ToastSpec::new(ToastKind::Info, "Copied"));
            Toasts::stack(cx)
        });
        cx.run_until_parked();
        assert_eq!(stack.read_with(cx, |s, _| s.queue.visible().len()), 1);

        advance_in_ticks(cx, Duration::from_secs(8));
        assert_eq!(
            stack.read_with(cx, |s, _| s.queue.visible().len()),
            0,
            "fading"
        );

        advance_in_ticks(cx, DURATION_BASE + TICK);
        assert!(stack.read_with(cx, |s, _| s.queue.is_empty()));
        assert!(
            !stack.read_with(cx, |s, _| s.ticking),
            "the clock stops with the stack"
        );
    }

    #[gpui::test]
    fn an_error_toast_stays_on_the_test_clock(cx: &mut gpui::TestAppContext) {
        let stack = cx.update(|cx| {
            Toasts::show(
                cx,
                ToastSpec::new(ToastKind::Error, "Could not rename the session"),
            );
            Toasts::stack(cx)
        });
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_secs(60));
        cx.run_until_parked();
        assert_eq!(stack.read_with(cx, |s, _| s.queue.visible().len()), 1);
    }

    #[gpui::test]
    fn acting_on_a_toast_runs_its_handler_and_dismisses_it(cx: &mut gpui::TestAppContext) {
        let acted = Rc::new(Cell::new(0_u32));
        let (stack, cx) = cx.add_window_view(|_, cx| ToastStack::new(cx));
        let handler: ClickHandler = {
            let acted = acted.clone();
            Rc::new(move |_, _| acted.set(acted.get() + 1))
        };
        let id = stack.update(cx, |s, cx| {
            let spec =
                ToastSpec::new(ToastKind::Success, "Removed from queue").with_action("Undo", true);
            s.show(spec, Some(handler), cx)
        });
        cx.run_until_parked();

        let center = cx
            .debug_bounds("toast-action")
            .expect("action is drawn")
            .center();
        cx.simulate_click(center, gpui::Modifiers::none());
        cx.run_until_parked();
        assert_eq!(acted.get(), 1);
        assert!(stack.read_with(cx, |s, _| s.queue.is_leaving(id)));
    }

    #[gpui::test]
    fn escape_dismisses_the_focused_toast_in_the_stack(cx: &mut gpui::TestAppContext) {
        let (stack, cx) = cx.add_window_view(|_, cx| ToastStack::new(cx));
        let id = stack.update(cx, |s, cx| {
            s.show(
                ToastSpec::new(ToastKind::Error, "Could not rename the session"),
                None,
                cx,
            )
        });
        cx.run_until_parked();
        let focus = stack.read_with(cx, |s, _| s.focus[&id].clone());
        focus_and_draw(cx, &focus);
        press_key(cx, "escape");
        cx.run_until_parked();
        assert!(stack.read_with(cx, |s, _| s.queue.is_leaving(id)));
    }

    #[gpui::test]
    fn a_new_toast_takes_no_focus(cx: &mut gpui::TestAppContext) {
        let (stack, cx) = cx.add_window_view(|_, cx| ToastStack::new(cx));
        stack.update(cx, |s, cx| {
            s.show(ToastSpec::new(ToastKind::Info, "Copied"), None, cx);
        });
        cx.run_until_parked();
        assert!(cx.update(|window, cx| window.focused(cx)).is_none());
    }

    #[gpui::test]
    fn escape_on_the_focused_toast_dismisses_it(cx: &mut gpui::TestAppContext) {
        let dismissed = Rc::new(Cell::new(0_u32));
        let (view, cx) =
            cx.add_window_view(|_, cx| Harness::new(cx, dismissed.clone(), Rc::new(Cell::new(0))));
        cx.run_until_parked();
        let focus = view.read_with(cx, |h, _| h.focus.clone());
        focus_and_draw(cx, &focus);
        press_key(cx, "escape");
        assert_eq!(dismissed.get(), 1);
    }

    #[gpui::test]
    fn the_action_runs_once_and_does_not_dismiss_by_itself(cx: &mut gpui::TestAppContext) {
        let dismissed = Rc::new(Cell::new(0_u32));
        let acted = Rc::new(Cell::new(0_u32));
        let (_, cx) =
            cx.add_window_view(|_, cx| Harness::new(cx, dismissed.clone(), acted.clone()));
        cx.run_until_parked();
        let center = cx
            .debug_bounds("toast-action")
            .expect("action is drawn")
            .center();
        cx.simulate_click(center, gpui::Modifiers::none());
        assert_eq!(acted.get(), 1);
        assert_eq!(
            dismissed.get(),
            0,
            "the stack dismisses an acted toast, not the element"
        );
    }

    /// Hosts one error toast with an action, a focus handle and two counters.
    struct Harness {
        focus: FocusHandle,
        dismissed: Rc<Cell<u32>>,
        acted: Rc<Cell<u32>>,
    }

    impl Harness {
        fn new(
            cx: &mut gpui::Context<Self>,
            dismissed: Rc<Cell<u32>>,
            acted: Rc<Cell<u32>>,
        ) -> Self {
            Self {
                focus: cx.focus_handle(),
                dismissed,
                acted,
            }
        }
    }

    impl gpui::Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let (dismissed, acted) = (self.dismissed.clone(), self.acted.clone());
            div().w(px(480.)).child(
                Toast::new("toast", ToastKind::Error, "Could not rename the session")
                    .action("Retry", move |_, _| acted.set(acted.get() + 1))
                    .focus_handle(self.focus.clone())
                    .on_dismiss(move |_, _| dismissed.set(dismissed.get() + 1)),
            )
        }
    }
}
