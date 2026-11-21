//! The F6 region cycle in the window: one focus handle per zone, and the move between them
//! (handoff 2.2). The order and the next-zone rule live in the model.

use super::Shell;
use crate::model::region_cycle::{Zone, next_zone};
use gpui::{App, Context, FocusHandle, Window};

/// One focus handle per zone, so F6 can move focus into a zone and see where focus is.
#[derive(Debug)]
pub(super) struct ZoneFocus {
    handles: Vec<FocusHandle>,
}

impl ZoneFocus {
    pub(super) fn new(cx: &mut App) -> Self {
        Self {
            handles: Zone::ORDER.iter().map(|_| cx.focus_handle()).collect(),
        }
    }

    pub(super) fn get(&self, zone: Zone) -> &FocusHandle {
        &self.handles[zone.index()]
    }
}

impl Shell {
    /// F6 moves focus to the next zone on screen, Shift+F6 to the previous one. The cycle
    /// stands still while the palette is open, because the palette owns focus then.
    pub(super) fn cycle_region(
        &mut self,
        backwards: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.model.palette().is_some() {
            return;
        }
        let zones = self.model.focus_zones();
        let current = zones
            .iter()
            .copied()
            .find(|zone| self.zones.get(*zone).contains_focused(window, cx));
        let Some(next) = next_zone(&zones, current, backwards) else {
            return;
        };
        let target = match next {
            Zone::Rail => self.rail.focus_target().clone(),
            zone => self.zones.get(zone).clone(),
        };
        window.focus(&target, cx);
        cx.notify();
    }
}
