//! Shared styling rules every component uses: the surface a control sits on
//! (state fills are solid per surface, handoff 2.1) and the focus ring
//! (handoff 2.2).

use crate::theme::{Theme, box_shadows};
use crate::tokens::Color;
use gpui::{InteractiveElement, StatefulInteractiveElement, Styled};

/// The surface a component is drawn on. Hover, selected and pressed fills are
/// precomputed for each surface; nothing blends at run time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Surface {
    /// The L-shaped frame: Activity rail, Title bar, Sidebar.
    Frame,
    /// The canvas inside the frame.
    #[default]
    Canvas,
    /// Cards and fields.
    Raised,
    /// Popovers, menus, dialogs, toasts.
    Overlay,
}

/// The three state fills and the subtle accent fill of one surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StateFills {
    pub base: Color,
    pub hover: Color,
    pub selected: Color,
    pub pressed: Color,
    pub accent_subtle: Color,
}

impl Surface {
    pub fn fills(self, theme: &Theme) -> StateFills {
        let p = &theme.palette;
        match self {
            Surface::Frame => StateFills {
                base: p.surface.frame,
                hover: p.fill.hover_frame,
                selected: p.fill.selected_frame,
                pressed: p.fill.pressed_frame,
                accent_subtle: p.accent.subtle_frame,
            },
            Surface::Canvas => StateFills {
                base: p.surface.canvas,
                hover: p.fill.hover_canvas,
                selected: p.fill.selected_canvas,
                pressed: p.fill.pressed_canvas,
                accent_subtle: p.accent.subtle_canvas,
            },
            Surface::Raised => StateFills {
                base: p.surface.raised,
                hover: p.fill.hover_raised,
                selected: p.fill.selected_raised,
                pressed: p.fill.pressed_raised,
                accent_subtle: p.accent.subtle_raised,
            },
            Surface::Overlay => StateFills {
                base: p.surface.overlay,
                hover: p.fill.hover_overlay,
                selected: p.fill.selected_overlay,
                pressed: p.fill.pressed_overlay,
                accent_subtle: p.accent.subtle_overlay,
            },
        }
    }
}

/// The one focus ring of the design system: 2 px `border.focus` outside a
/// 2 px gap, shown on keyboard focus only (`focus-visible`).
pub trait FocusRing: InteractiveElement + Styled + Sized {
    fn focus_ring(self, theme: &Theme) -> Self {
        let rings = box_shadows(theme.shadows.focus);
        self.focus_visible(move |s| s.shadow(rings))
    }
}

impl<T: InteractiveElement + Styled> FocusRing for T {}

/// Hover and pressed fills for a clickable element on `surface`.
pub trait StateFill: StatefulInteractiveElement + Styled + Sized {
    fn state_fills(self, theme: &Theme, surface: Surface) -> Self {
        let f = surface.fills(theme);
        self.hover(move |s| s.bg(f.hover))
            .active(move |s| s.bg(f.pressed))
    }
}

impl<T: StatefulInteractiveElement + Styled> StateFill for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::Appearance;

    #[test]
    fn each_surface_has_distinct_solid_fills() {
        for appearance in [Appearance::Dark, Appearance::Light] {
            let theme = Theme::new(appearance);
            for s in [
                Surface::Frame,
                Surface::Canvas,
                Surface::Raised,
                Surface::Overlay,
            ] {
                let f = s.fills(&theme);
                assert_ne!(f.base, f.hover, "{s:?}");
                assert_ne!(f.hover, f.pressed, "{s:?}");
                assert_eq!(f.hover.a, 1.0, "state fills are solid");
            }
        }
    }
}
