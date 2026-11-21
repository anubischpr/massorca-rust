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
    SPACE_2, SPACE_3,
};
use gpui::{
    Animation, AnimationExt, App, ElementId, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, SharedString, Styled, Window, div, px,
};
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
