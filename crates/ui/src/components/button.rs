//! Button: the shared text button (docs/design/components/Button.md).
//!
//! Enter and Space activate a focused button through GPUI's keyboard click, so
//! one `on_click` serves the pointer and the keyboard (handoff 2.3).

use crate::icon::{Icon, IconName, IconSize};
use crate::style::{FocusRing, Surface};
use crate::theme::{ActiveTheme, TextStyled, Theme, box_shadows};
use crate::tokens::{
    Color, DURATION_SPINNER, RADIUS_MD, SIZE_CONTROL_COMFORTABLE, SIZE_CONTROL_COMPACT,
    SIZE_CONTROL_DEFAULT, SPACE_1, SPACE_2, SPACE_3, SPACE_4, TYPE_BODY, TYPE_LABEL,
    TextStyleToken,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    Animation, AnimationExt, App, ElementId, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Transformation,
    Window, div, px, radians, svg,
};
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Delay before a loading spinner replaces the icon (design-system 4.8).
pub const SPINNER_DELAY: Duration = Duration::from_millis(200);

/// Fully transparent: "no fill" and "no border" keep every variant the same size.
const NONE: Color = Color::rgba8(0, 0, 0, 0.0);

/// The button's emphasis: one primary per view, secondary beside it, ghost in
/// rows and bars, danger for a destructive confirm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    #[default]
    Secondary,
    Ghost,
    Danger,
}

/// Height, padding, icon and label style of a button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonSize {
    Compact,
    #[default]
    Default,
    Comfortable,
}

impl ButtonSize {
    fn metrics(self) -> (f32, f32, IconSize, TextStyleToken) {
        match self {
            Self::Compact => (SIZE_CONTROL_COMPACT, SPACE_2, IconSize::Small, TYPE_LABEL),
            Self::Default => (SIZE_CONTROL_DEFAULT, SPACE_3, IconSize::Small, TYPE_LABEL),
            Self::Comfortable => (
                SIZE_CONTROL_COMFORTABLE,
                SPACE_4,
                IconSize::Medium,
                TYPE_BODY,
            ),
        }
    }
}

/// Forces a state so the gallery can show it without a pointer or keyboard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatePreview {
    Hover,
    Pressed,
    Focus,
}

/// What the button looks like right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Visual {
    Rest,
    Hover,
    Pressed,
    Selected,
    Disabled,
}

/// Solid colours of one button in one state (handoff 2.1: nothing blends at run time).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Paint {
    fill: Color,
    border: Color,
    label: Color,
}

fn paint(variant: ButtonVariant, surface: Surface, visual: Visual, theme: &Theme) -> Paint {
    let p = &theme.palette;
    let fills = surface.fills(theme);
    if visual == Visual::Disabled {
        return match variant {
            ButtonVariant::Secondary => Paint {
                fill: p.surface.raised,
                border: p.border.disabled,
                label: p.text.disabled,
            },
            ButtonVariant::Ghost => Paint {
                fill: NONE,
                border: NONE,
                label: p.text.disabled,
            },
            ButtonVariant::Primary | ButtonVariant::Danger => Paint {
                fill: p.fill.disabled,
                border: NONE,
                label: p.text.disabled,
            },
        };
    }
    match variant {
        ButtonVariant::Primary => Paint {
            fill: match visual {
                Visual::Hover => p.accent.hover,
                Visual::Pressed => p.accent.pressed,
                _ => p.accent.r#default,
            },
            border: NONE,
            label: p.text.on_accent,
        },
        ButtonVariant::Secondary => Paint {
            fill: match visual {
                Visual::Hover => p.fill.hover_raised,
                Visual::Pressed => p.fill.pressed_raised,
                _ => p.surface.raised,
            },
            border: p.border.strong,
            label: p.text.primary,
        },
        ButtonVariant::Ghost => match visual {
            Visual::Hover => Paint {
                fill: fills.hover,
                border: NONE,
                label: p.text.primary,
            },
            Visual::Pressed => Paint {
                fill: fills.pressed,
                border: NONE,
                label: p.text.primary,
            },
            Visual::Selected => Paint {
                fill: fills.selected,
                border: NONE,
                label: p.text.primary,
            },
            Visual::Rest | Visual::Disabled => Paint {
                fill: NONE,
                border: NONE,
                label: p.text.secondary,
            },
        },
        ButtonVariant::Danger => Paint {
            fill: match visual {
                Visual::Hover => p.status.danger_hover,
                Visual::Pressed => p.status.danger_pressed,
                _ => p.status.danger_solid,
            },
            border: NONE,
            label: p.status.danger_on,
        },
    }
}

/// Process-wide clock for spinners, so they keep turning under reduced motion
/// (handoff 2.6); GPUI freezes repeating animations instead.
static SPINNER_EPOCH: OnceLock<Instant> = OnceLock::new();

/// Angle of a spinner in radians: one turn per `DURATION_SPINNER`.
pub(crate) fn spinner_angle() -> f32 {
    let epoch = *SPINNER_EPOCH.get_or_init(Instant::now);
    let period = DURATION_SPINNER.as_secs_f32();
    let phase = (epoch.elapsed().as_secs_f32() % period) / period;
    phase * std::f32::consts::TAU
}

/// The Lucide loader arc turning at the spinner angle.
pub(crate) fn spinner(size: f32, color: Color) -> impl IntoElement {
    svg()
        .path(IconName::LoaderCircle.path())
        .size(px(size))
        .flex_none()
        .text_color(color)
        .with_transformation(Transformation::rotate(radians(spinner_angle())))
}

/// The leading slot while loading: the icon until `SPINNER_DELAY`, then the spinner.
fn loading_slot(icon: Option<IconName>, size: IconSize, color: Color) -> impl IntoElement {
    div().flex_none().size(px(size.px())).with_animation(
        ("spinner-delay", 0usize),
        Animation::new(SPINNER_DELAY),
        move |slot, delta| {
            if delta >= 1.0 {
                slot.child(spinner(size.px(), color))
            } else {
                slot.children(icon.map(|name| Icon::new(name).size(size).color(color)))
            }
        },
    )
}

type ClickHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// A text button with an optional leading and trailing icon.
#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    variant: ButtonVariant,
    size: ButtonSize,
    surface: Surface,
    leading: Option<IconName>,
    trailing: Option<IconName>,
    loading: bool,
    selected: bool,
    disabled: bool,
    tooltip: Option<SharedString>,
    preview: Option<StatePreview>,
    focus: Option<FocusHandle>,
    on_click: Option<ClickHandler>,
}

impl std::fmt::Debug for Button {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Button")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("variant", &self.variant)
            .field("size", &self.size)
            .field("loading", &self.loading)
            .field("disabled", &self.disabled)
            .finish_non_exhaustive()
    }
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            variant: ButtonVariant::default(),
            size: ButtonSize::default(),
            surface: Surface::default(),
            leading: None,
            trailing: None,
            loading: false,
            selected: false,
            disabled: false,
            tooltip: None,
            preview: None,
            focus: None,
            on_click: None,
        }
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    /// The surface the button sits on; ghost hover and selected fills follow it.
    pub fn surface(mut self, surface: Surface) -> Self {
        self.surface = surface;
        self
    }

    pub fn leading_icon(mut self, icon: IconName) -> Self {
        self.leading = Some(icon);
        self
    }

    pub fn trailing_icon(mut self, icon: IconName) -> Self {
        self.trailing = Some(icon);
        self
    }

    /// Busy: no activation, and a spinner replaces the icon after `SPINNER_DELAY`.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Open or pressed-in state of a ghost button (its menu is open).
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Tooltip text. The Tooltip component draws it; while disabled it is the reason.
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    /// The text the Tooltip shows, if any.
    pub fn tooltip_text(&self) -> Option<&SharedString> {
        self.tooltip.as_ref()
    }

    /// Forces a visual state (gallery and visual checks only).
    pub fn preview(mut self, state: StatePreview) -> Self {
        self.preview = Some(state);
        self
    }

    /// Keyboard focus lives with the caller (for example a dialog's initial focus).
    pub fn focus_handle(mut self, handle: FocusHandle) -> Self {
        self.focus = Some(handle);
        self
    }

    pub fn on_click(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(listener));
        self
    }

    /// Whether a click or Enter/Space reaches `on_click`.
    pub fn activates(&self) -> bool {
        !self.disabled && !self.loading
    }

    fn visual(&self) -> Visual {
        if self.disabled {
            Visual::Disabled
        } else if self.selected {
            Visual::Selected
        } else {
            Visual::Rest
        }
    }
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let (height, padding, icon_size, label_style) = self.size.metrics();
        let interactive = self.activates();
        let shown = match self.preview {
            Some(StatePreview::Hover) => Visual::Hover,
            Some(StatePreview::Pressed) => Visual::Pressed,
            _ => self.visual(),
        };
        let rest = paint(self.variant, self.surface, shown, &theme);
        let focus = self.focus.clone().unwrap_or_else(|| {
            window
                .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
                .read(cx)
                .clone()
        });
        if self.loading {
            window.request_animation_frame();
        }

        let mut root = div()
            .id(self.id.clone())
            .track_focus(&focus)
            .tab_stop(true)
            .focus_ring(&theme)
            .flex()
            .min_w_0()
            .items_center()
            .justify_center()
            .gap(px(SPACE_1))
            .h(px(height))
            .px(px(padding))
            .rounded(px(RADIUS_MD))
            .border_1()
            .border_color(rest.border)
            .bg(rest.fill)
            .text_style(label_style)
            .text_color(rest.label)
            .when(self.preview == Some(StatePreview::Focus), |el| {
                el.shadow(box_shadows(theme.shadows.focus))
            });

        if interactive && self.preview.is_none() {
            let hover = paint(self.variant, self.surface, Visual::Hover, &theme);
            let pressed = paint(self.variant, self.surface, Visual::Pressed, &theme);
            root = root
                .cursor_pointer()
                .hover(move |s| s.bg(hover.fill).text_color(hover.label))
                .active(move |s| s.bg(pressed.fill).text_color(pressed.label));
        }
        if let Some(on_click) = self.on_click.filter(|_| interactive) {
            root = root.on_click(move |_, window, cx| on_click(window, cx));
        }

        if self.loading {
            root = root.child(loading_slot(self.leading, icon_size, rest.label));
        } else if let Some(icon) = self.leading {
            root = root.child(Icon::new(icon).size(icon_size).color(rest.label));
        }
        root = root.child(div().min_w_0().truncate().child(self.label));
        if let Some(icon) = self.trailing {
            root = root.child(Icon::new(icon).size(icon_size).color(rest.label));
        }
        root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::Appearance;
    use std::cell::Cell;

    fn dark() -> Theme {
        Theme::new(Appearance::Dark)
    }

    #[test]
    fn sizes_follow_the_control_tokens() {
        assert_eq!(ButtonSize::Compact.metrics().0, 24.0);
        assert_eq!(ButtonSize::Default.metrics().0, 28.0);
        assert_eq!(ButtonSize::Comfortable.metrics().0, 32.0);
        assert_eq!(ButtonSize::Compact.metrics().2, IconSize::Small);
        assert_eq!(ButtonSize::Comfortable.metrics().2, IconSize::Medium);
        assert_eq!(ButtonSize::Default.metrics().3, TYPE_LABEL);
        assert_eq!(ButtonSize::Comfortable.metrics().3, TYPE_BODY);
    }

    #[test]
    fn primary_is_accent_with_on_accent_label() {
        let t = dark();
        let rest = paint(ButtonVariant::Primary, Surface::Canvas, Visual::Rest, &t);
        assert_eq!(rest.fill, t.palette.accent.r#default);
        assert_eq!(rest.label, t.palette.text.on_accent);
        let hover = paint(ButtonVariant::Primary, Surface::Canvas, Visual::Hover, &t);
        assert_eq!(hover.fill, t.palette.accent.hover);
        let pressed = paint(ButtonVariant::Primary, Surface::Canvas, Visual::Pressed, &t);
        assert_eq!(pressed.fill, t.palette.accent.pressed);
    }

    #[test]
    fn ghost_has_no_fill_or_border_at_rest_and_lights_its_own_surface() {
        let t = dark();
        let rest = paint(ButtonVariant::Ghost, Surface::Canvas, Visual::Rest, &t);
        assert_eq!(rest.fill.a, 0.0);
        assert_eq!(rest.border.a, 0.0);
        assert_eq!(rest.label, t.palette.text.secondary);
        let hover = paint(ButtonVariant::Ghost, Surface::Canvas, Visual::Hover, &t);
        assert_eq!(hover.fill, t.palette.fill.hover_canvas);
        assert_eq!(hover.label, t.palette.text.primary);
        let frame = paint(ButtonVariant::Ghost, Surface::Frame, Visual::Hover, &t);
        assert_eq!(frame.fill, t.palette.fill.hover_frame);
        let selected = paint(ButtonVariant::Ghost, Surface::Canvas, Visual::Selected, &t);
        assert_eq!(selected.fill, t.palette.fill.selected_canvas);
    }

    #[test]
    fn danger_and_secondary_use_their_documented_tokens() {
        let t = dark();
        let danger = paint(ButtonVariant::Danger, Surface::Canvas, Visual::Hover, &t);
        assert_eq!(danger.fill, t.palette.status.danger_hover);
        assert_eq!(danger.label, t.palette.status.danger_on);
        let secondary = paint(ButtonVariant::Secondary, Surface::Canvas, Visual::Rest, &t);
        assert_eq!(secondary.fill, t.palette.surface.raised);
        assert_eq!(secondary.border, t.palette.border.strong);
    }

    #[test]
    fn disabled_uses_disabled_tokens_for_every_variant() {
        let t = dark();
        for variant in [
            ButtonVariant::Primary,
            ButtonVariant::Secondary,
            ButtonVariant::Ghost,
            ButtonVariant::Danger,
        ] {
            let p = paint(variant, Surface::Canvas, Visual::Disabled, &t);
            assert_eq!(p.label, t.palette.text.disabled, "{variant:?}");
        }
        let secondary = paint(
            ButtonVariant::Secondary,
            Surface::Canvas,
            Visual::Disabled,
            &t,
        );
        assert_eq!(secondary.border, t.palette.border.disabled);
    }

    #[test]
    fn disabled_and_loading_buttons_do_not_activate() {
        let plain = Button::new("a", "Save");
        assert!(plain.activates());
        assert!(!Button::new("a", "Save").disabled(true).activates());
        assert!(!Button::new("a", "Save").loading(true).activates());
    }

    #[test]
    fn disabled_reason_is_stored_as_tooltip_text() {
        let button = Button::new("a", "Open")
            .disabled(true)
            .tooltip("Host unreachable.");
        assert_eq!(
            button.tooltip_text().map(|s| s.as_ref()),
            Some("Host unreachable.")
        );
        assert_eq!(Button::new("a", "Open").tooltip_text(), None);
    }

    #[test]
    fn spinner_angle_stays_within_one_turn() {
        let angle = spinner_angle();
        assert!((0.0..std::f32::consts::TAU).contains(&angle), "{angle}");
    }

    #[gpui::test]
    fn pointer_click_and_enter_and_space_each_fire_once(cx: &mut gpui::TestAppContext) {
        let clicks = Rc::new(Cell::new(0_u32));
        let (view, cx) = cx.add_window_view(|_, cx| Harness::new(cx, clicks.clone(), false, false));
        cx.run_until_parked();

        let center = cx.debug_bounds("button").expect("button is drawn").center();
        cx.simulate_click(center, gpui::Modifiers::none());
        assert_eq!(clicks.get(), 1);

        let focus = view.read_with(cx, |h, _| h.focus.clone());
        focus_and_draw(cx, &focus);
        press_key(cx, "enter");
        assert_eq!(clicks.get(), 2);
        press_key(cx, "space");
        assert_eq!(clicks.get(), 3);
    }

    #[gpui::test]
    fn disabled_and_loading_buttons_ignore_pointer_and_keys(cx: &mut gpui::TestAppContext) {
        for (disabled, loading) in [(true, false), (false, true)] {
            let clicks = Rc::new(Cell::new(0_u32));
            let (view, cx) =
                cx.add_window_view(|_, cx| Harness::new(cx, clicks.clone(), disabled, loading));
            cx.run_until_parked();
            let center = cx.debug_bounds("button").expect("button is drawn").center();
            cx.simulate_click(center, gpui::Modifiers::none());
            let focus = view.read_with(cx, |h, _| h.focus.clone());
            focus_and_draw(cx, &focus);
            press_key(cx, "enter");
            press_key(cx, "space");
            assert_eq!(clicks.get(), 0, "disabled={disabled} loading={loading}");
        }
    }

    /// Focuses `focus`, then paints, so the button registers its key handlers.
    fn focus_and_draw(cx: &mut gpui::VisualTestContext, focus: &FocusHandle) {
        cx.update(|window, cx| window.focus(focus, cx));
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }

    /// Presses and releases one key: GPUI turns a focused Enter or Space release into a click.
    fn press_key(cx: &mut gpui::VisualTestContext, key: &str) {
        let os = cx.update(|_, cx| cx.platform_info().os);
        let keystroke = gpui::Keystroke::parse_with_os(key, os).unwrap();
        cx.simulate_event(gpui::KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(gpui::KeyUpEvent { keystroke });
    }

    /// Hosts one button with a focus handle and a click counter.
    struct Harness {
        focus: FocusHandle,
        clicks: Rc<Cell<u32>>,
        disabled: bool,
        loading: bool,
    }

    impl Harness {
        fn new(
            cx: &mut gpui::Context<Self>,
            clicks: Rc<Cell<u32>>,
            disabled: bool,
            loading: bool,
        ) -> Self {
            Self {
                focus: cx.focus_handle(),
                clicks,
                disabled,
                loading,
            }
        }
    }

    impl gpui::Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let clicks = self.clicks.clone();
            div().child(
                div().debug_selector(|| "button".into()).child(
                    Button::new("under-test", "Save")
                        .focus_handle(self.focus.clone())
                        .disabled(self.disabled)
                        .loading(self.loading)
                        .on_click(move |_, _| clicks.set(clicks.get() + 1)),
                ),
            )
        }
    }
}
