//! IconButton: a square button that shows only an icon
//! (docs/design/components/IconButton.md).
//!
//! Its accessible name is its tooltip text, so the constructor requires one.

use crate::components::button::{
    ButtonSize, ClickHandler, HOVER_GROUP, StatePreview, glyph, loading_slot,
};
use crate::icon::IconName;
use crate::style::{FocusRing, Surface};
use crate::theme::{ActiveTheme, Theme, box_shadows};
use crate::tokens::{Color, RADIUS_MD};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, ElementId, FocusHandle, InteractiveElement, IntoElement, ParentElement, RenderOnce,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use std::rc::Rc;

/// Transparent fill or border; keeps every variant the same size.
const NONE: Color = Color::rgba8(0, 0, 0, 0.0);

/// Ghost sits on its surface with no fill; outline has a raised fill and a border.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IconButtonVariant {
    #[default]
    Ghost,
    Outline,
}

/// What the icon button looks like right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Visual {
    Rest,
    Hover,
    Pressed,
    /// A toggle that is on: the selected fill and the accent icon.
    On,
    Disabled,
}

/// Solid colours of one icon button in one state.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Paint {
    fill: Color,
    border: Color,
    icon: Color,
}

fn paint(variant: IconButtonVariant, surface: Surface, visual: Visual, theme: &Theme) -> Paint {
    let p = &theme.palette;
    let outline = variant == IconButtonVariant::Outline;
    // An outline sits on the raised surface, so its state fills are the raised ones.
    let fills = if outline {
        Surface::Raised.fills(theme)
    } else {
        surface.fills(theme)
    };
    let border = if outline { p.border.strong } else { NONE };
    match visual {
        Visual::Disabled => Paint {
            fill: NONE,
            border: if outline { p.border.disabled } else { NONE },
            icon: p.text.disabled,
        },
        Visual::Rest => Paint {
            fill: if outline { fills.base } else { NONE },
            border,
            icon: p.text.secondary,
        },
        Visual::Hover => Paint {
            fill: fills.hover,
            border,
            icon: p.text.primary,
        },
        Visual::Pressed => Paint {
            fill: fills.pressed,
            border,
            icon: p.text.primary,
        },
        Visual::On => Paint {
            fill: fills.selected,
            border,
            icon: p.accent.fg,
        },
    }
}

/// A square icon-only button. Enter and Space activate it like a Button.
#[derive(IntoElement)]
pub struct IconButton {
    id: ElementId,
    icon: IconName,
    name: SharedString,
    variant: IconButtonVariant,
    size: ButtonSize,
    surface: Surface,
    toggled: Option<bool>,
    loading: bool,
    disabled: bool,
    reason: Option<SharedString>,
    preview: Option<StatePreview>,
    focus: Option<FocusHandle>,
    on_click: Option<ClickHandler>,
}

impl std::fmt::Debug for IconButton {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IconButton")
            .field("id", &self.id)
            .field("icon", &self.icon)
            .field("name", &self.name)
            .field("variant", &self.variant)
            .field("size", &self.size)
            .field("toggled", &self.toggled)
            .field("loading", &self.loading)
            .field("disabled", &self.disabled)
            .finish_non_exhaustive()
    }
}

impl IconButton {
    /// `name` is the accessible name and the tooltip text.
    pub fn new(id: impl Into<ElementId>, icon: IconName, name: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            icon,
            name: name.into(),
            variant: IconButtonVariant::default(),
            size: ButtonSize::default(),
            surface: Surface::default(),
            toggled: None,
            loading: false,
            disabled: false,
            reason: None,
            preview: None,
            focus: None,
            on_click: None,
        }
    }

    pub fn variant(mut self, variant: IconButtonVariant) -> Self {
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

    /// Makes the button a toggle that reports `aria-pressed`.
    pub fn toggle(mut self, pressed: bool) -> Self {
        self.toggled = Some(pressed);
        self
    }

    /// Busy: no activation, and a spinner replaces the icon after `SPINNER_DELAY`.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Why the button is disabled; the Tooltip shows it instead of the name.
    pub fn disabled_reason(mut self, reason: impl Into<SharedString>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Forces a visual state (gallery and visual checks only).
    pub fn preview(mut self, state: StatePreview) -> Self {
        self.preview = Some(state);
        self
    }

    /// Keyboard focus lives with the caller (for example a toolbar).
    pub fn focus_handle(mut self, handle: FocusHandle) -> Self {
        self.focus = Some(handle);
        self
    }

    pub fn on_click(mut self, listener: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(listener));
        self
    }

    /// The accessible name.
    pub fn name(&self) -> &SharedString {
        &self.name
    }

    /// The text the Tooltip shows: the disabled reason while disabled, else the name.
    pub fn tooltip_text(&self) -> &SharedString {
        match (&self.reason, self.disabled) {
            (Some(reason), true) => reason,
            _ => &self.name,
        }
    }

    /// `aria-pressed` of a toggle; `None` for a plain button.
    pub fn pressed(&self) -> Option<bool> {
        self.toggled
    }

    /// Whether a click or Enter/Space reaches `on_click`.
    pub fn activates(&self) -> bool {
        !self.disabled && !self.loading
    }

    fn visual(&self) -> Visual {
        if self.disabled {
            Visual::Disabled
        } else if self.toggled == Some(true) {
            Visual::On
        } else {
            Visual::Rest
        }
    }
}

impl RenderOnce for IconButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let (square, _, icon_size, _) = self.size.metrics();
        let interactive = self.activates();
        let shown = match self.preview {
            Some(StatePreview::Hover) => Visual::Hover,
            Some(StatePreview::Pressed) => Visual::Pressed,
            _ => self.visual(),
        };
        let rest = paint(self.variant, self.surface, shown, &theme);
        let hover = paint(self.variant, self.surface, Visual::Hover, &theme);
        let pressed = paint(self.variant, self.surface, Visual::Pressed, &theme);
        let hover_icon = if interactive { hover.icon } else { rest.icon };
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
            .group(HOVER_GROUP)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(square))
            .rounded(px(RADIUS_MD))
            .border_1()
            .border_color(rest.border)
            .bg(rest.fill)
            .when(self.preview == Some(StatePreview::Focus), |el| {
                el.shadow(box_shadows(theme.shadows.focus))
            });
        if interactive && self.preview.is_none() {
            root = root
                .cursor_pointer()
                .hover(move |s| s.bg(hover.fill))
                .active(move |s| s.bg(pressed.fill));
        }
        if let Some(on_click) = self.on_click.filter(|_| interactive) {
            root = root.on_click(move |_, window, cx| on_click(window, cx));
        }

        if self.loading {
            root.child(loading_slot(Some(self.icon), icon_size, rest.icon))
        } else {
            root.child(glyph(self.icon, icon_size, rest.icon, hover_icon))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::icon::IconSize;
    use crate::tokens::Appearance;

    fn dark() -> Theme {
        Theme::new(Appearance::Dark)
    }

    #[test]
    fn square_follows_the_control_and_icon_tokens() {
        assert_eq!(ButtonSize::Compact.metrics().0, 24.0);
        assert_eq!(ButtonSize::Default.metrics().0, 28.0);
        assert_eq!(ButtonSize::Comfortable.metrics().0, 32.0);
        assert_eq!(ButtonSize::Comfortable.metrics().2, IconSize::Medium);
    }

    #[test]
    fn ghost_is_transparent_at_rest_and_outline_is_raised_with_a_strong_border() {
        let t = dark();
        let ghost = paint(IconButtonVariant::Ghost, Surface::Canvas, Visual::Rest, &t);
        assert_eq!(ghost.fill.a, 0.0);
        assert_eq!(ghost.border.a, 0.0);
        assert_eq!(ghost.icon, t.palette.text.secondary);
        let outline = paint(
            IconButtonVariant::Outline,
            Surface::Canvas,
            Visual::Rest,
            &t,
        );
        assert_eq!(outline.fill, t.palette.surface.raised);
        assert_eq!(outline.border, t.palette.border.strong);
    }

    #[test]
    fn title_bar_ghost_lights_the_frame_fill_on_hover() {
        let t = dark();
        let hover = paint(IconButtonVariant::Ghost, Surface::Frame, Visual::Hover, &t);
        assert_eq!(hover.fill, t.palette.fill.hover_frame);
        assert_eq!(hover.icon, t.palette.text.primary);
    }

    #[test]
    fn toggle_on_uses_the_selected_fill_and_the_accent_icon() {
        let t = dark();
        let on = paint(IconButtonVariant::Ghost, Surface::Canvas, Visual::On, &t);
        assert_eq!(on.fill, t.palette.fill.selected_canvas);
        assert_eq!(on.icon, t.palette.accent.fg);
    }

    #[test]
    fn disabled_has_no_fill_and_an_outline_keeps_its_disabled_border() {
        let t = dark();
        let ghost = paint(
            IconButtonVariant::Ghost,
            Surface::Canvas,
            Visual::Disabled,
            &t,
        );
        assert_eq!(ghost.fill.a, 0.0);
        assert_eq!(ghost.icon, t.palette.text.disabled);
        let outline = paint(
            IconButtonVariant::Outline,
            Surface::Canvas,
            Visual::Disabled,
            &t,
        );
        assert_eq!(outline.border, t.palette.border.disabled);
    }

    #[test]
    fn name_is_the_tooltip_unless_a_disabled_reason_is_set() {
        let plain = IconButton::new("a", IconName::Plus, "New terminal");
        assert_eq!(plain.name().as_ref(), "New terminal");
        assert_eq!(plain.tooltip_text().as_ref(), "New terminal");
        let disabled = IconButton::new("a", IconName::Plus, "New terminal")
            .disabled(true)
            .disabled_reason("Host unreachable.");
        assert_eq!(disabled.name().as_ref(), "New terminal");
        assert_eq!(disabled.tooltip_text().as_ref(), "Host unreachable.");
        let enabled = IconButton::new("a", IconName::Plus, "New terminal")
            .disabled_reason("Host unreachable.");
        assert_eq!(enabled.tooltip_text().as_ref(), "New terminal");
    }

    #[test]
    fn toggle_reports_its_pressed_state_and_a_plain_button_reports_none() {
        assert_eq!(IconButton::new("a", IconName::Plus, "x").pressed(), None);
        let on = IconButton::new("a", IconName::Plus, "x").toggle(true);
        assert_eq!(on.pressed(), Some(true));
    }

    #[test]
    fn disabled_and_loading_buttons_do_not_activate() {
        let name = "Refresh";
        assert!(IconButton::new("a", IconName::RefreshCw, name).activates());
        assert!(
            !IconButton::new("a", IconName::RefreshCw, name)
                .disabled(true)
                .activates()
        );
        assert!(
            !IconButton::new("a", IconName::RefreshCw, name)
                .loading(true)
                .activates()
        );
    }
}
