//! Project sidebar tabs (docs/design/components/ProjectSidebarTabs.md): the icon-only tab strip at
//! the top of the Project sidebar. It holds Files, Sessions, Source control and Checks, then any
//! extension tabs. Arrow keys move focus; Enter or Space selects.

use crate::components::agent_icon::NameTooltip;
use crate::components::badge::{Badge, BadgeTone};
use crate::components::segmented_control::{segment_key, tab_segment};
use crate::icon::{Icon, IconName, IconSize};
use crate::style::FocusRing;
use crate::theme::ActiveTheme;
use crate::tokens::{
    BORDER_WIDTH_THICK, DURATION_TOOLTIP_DELAY, RADIUS_MD, SIZE_CONTROL_DEFAULT, SIZE_TOOLBAR,
    SPACE_1, SPACE_2, SPACE_HALF,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, ElementId, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    px,
};
use std::rc::Rc;
use std::sync::Arc;

/// The longest tab label; extension labels are cut to this.
pub const LABEL_MAX_CHARS: usize = 24;

/// One tab: an icon, its name (also its tooltip) and an optional count.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectTab {
    pub label: SharedString,
    pub icon: IconName,
    /// The count and the word that follows it in the name, such as "2 failing".
    pub count: Option<(u32, &'static str)>,
    /// A failing count is drawn in the danger colour.
    pub failing: bool,
}

impl ProjectTab {
    pub fn new(label: &str, icon: IconName) -> Self {
        Self {
            label: label
                .chars()
                .take(LABEL_MAX_CHARS)
                .collect::<String>()
                .into(),
            icon,
            count: None,
            failing: false,
        }
    }

    /// A count badge; zero draws nothing.
    pub fn count(mut self, count: u32, word: &'static str) -> Self {
        self.count = (count > 0).then_some((count, word));
        self
    }

    pub fn failing(mut self) -> Self {
        self.failing = true;
        self
    }

    /// The label, with the count after it: "Checks, 2 failing".
    pub fn accessible_name(&self) -> String {
        match self.count {
            Some((count, word)) => format!("{}, {count} {word}", self.label),
            None => self.label.to_string(),
        }
    }
}

/// The four core tabs in their fixed order: changed files and failing runs are the counts.
pub fn core_tabs(changed_files: u32, failing_runs: u32) -> Vec<ProjectTab> {
    vec![
        ProjectTab::new("Files", IconName::FolderTree),
        ProjectTab::new("Sessions", IconName::History),
        ProjectTab::new("Source control", IconName::GitBranch).count(changed_files, "changed"),
        ProjectTab::new("Checks", IconName::BadgeCheck)
            .count(failing_runs, "failing")
            .failing(),
    ]
}

type SelectHandler = Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// The strip of icon tabs. The caller keeps the selected index and updates it in `on_select`.
#[derive(IntoElement)]
pub struct ProjectSidebarTabs {
    id: ElementId,
    tabs: Vec<ProjectTab>,
    selected: usize,
    on_select: Option<SelectHandler>,
}

impl std::fmt::Debug for ProjectSidebarTabs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectSidebarTabs")
            .field("id", &self.id)
            .field("tabs", &self.tabs.len())
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl ProjectSidebarTabs {
    pub fn new(id: impl Into<ElementId>, tabs: Vec<ProjectTab>) -> Self {
        Self {
            id: id.into(),
            tabs,
            selected: 0,
            on_select: None,
        }
    }

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }

    /// Runs when a tab is chosen by a press, Enter or Space; arrow keys only move focus.
    pub fn on_select(mut self, listener: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(listener));
        self
    }
}

impl RenderOnce for ProjectSidebarTabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let count = self.tabs.len();
        let selected = self.selected.min(count.saturating_sub(1));
        let enabled = vec![true; count];
        let stop = tab_segment(selected, &enabled);
        let handles: Vec<FocusHandle> = (0..count)
            .map(|index| {
                let key = tab_id(&self.id, index);
                window
                    .use_keyed_state(key, cx, |_, cx| cx.focus_handle())
                    .read(cx)
                    .clone()
                    .tab_stop(stop == Some(index))
            })
            .collect();

        let arrow_handles = handles.clone();
        let mut strip = div()
            .id(self.id.clone())
            .role(Role::TabList)
            .aria_label("Project sidebar")
            .flex()
            .items_center()
            // 36 px strip (28 px tabs, 4 px above and below) so its rule meets the Work panel's.
            .flex_none()
            .h(px(SIZE_TOOLBAR))
            .px(px(SPACE_2))
            .py(px(SPACE_1))
            .gap(px(SPACE_HALF))
            .border_b_1()
            .border_color(p.border.subtle)
            .bg(p.surface.canvas)
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let current = arrow_handles
                    .iter()
                    .position(|handle| handle.is_focused(window))
                    .unwrap_or(selected);
                let Some(next) = segment_key(current, &enabled, &event.keystroke.key) else {
                    return;
                };
                cx.stop_propagation();
                arrow_handles[next].focus(window, cx);
            });

        for (index, tab) in self.tabs.into_iter().enumerate() {
            let is_selected = index == selected;
            let ink = if is_selected {
                p.text.primary
            } else {
                p.text.secondary
            };
            let name = tab.accessible_name();
            let mut element = div()
                .id(tab_id(&self.id, index))
                .debug_selector(move || format!("tab-{index}"))
                .role(Role::Tab)
                .aria_label(name.clone())
                .aria_selected(is_selected)
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(SIZE_CONTROL_DEFAULT))
                .rounded(px(RADIUS_MD))
                .track_focus(&handles[index])
                .focus_ring(&theme)
                .text_color(ink)
                .cursor_pointer()
                .when(is_selected, |el| el.bg(p.fill.selected_canvas))
                .when(!is_selected, |el| {
                    el.hover(move |s| s.bg(p.fill.hover_canvas))
                })
                .active(move |s| s.bg(p.fill.pressed_canvas))
                .tooltip_show_delay(DURATION_TOOLTIP_DELAY)
                .tooltip(move |_, cx| cx.new(|_| NameTooltip(name.clone().into())).into())
                .child(Icon::new(tab.icon).size(IconSize::Small).color(ink));
            if let Some(on_select) = self.on_select.clone() {
                element = element.on_click(move |_, window, cx| on_select(index, window, cx));
            }
            if is_selected {
                element = element.child(
                    div()
                        .absolute()
                        .left(px(0.))
                        .right(px(0.))
                        .bottom(px(0.))
                        .h(px(BORDER_WIDTH_THICK))
                        .bg(p.accent.default),
                );
            }
            if let Some((count, _)) = tab.count {
                let tone = if tab.failing {
                    BadgeTone::Danger
                } else {
                    BadgeTone::Neutral
                };
                element = element.child(
                    div()
                        .absolute()
                        .top(px(0.))
                        .right(px(0.))
                        .child(Badge::count(count).tone(tone).solid(tab.failing)),
                );
            }
            strip = strip.child(element);
        }
        strip
    }
}

/// The element id of tab `index`; its focus handle is keyed by the same id.
fn tab_id(strip: &ElementId, index: usize) -> ElementId {
    ElementId::NamedChild(Arc::new(strip.clone()), format!("tab-{index}").into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::button::test_support::press_key;
    use std::cell::RefCell;

    #[test]
    fn core_tabs_keep_their_order_and_show_counts_only_when_nonzero() {
        let tabs = core_tabs(3, 0);
        let labels: Vec<&str> = tabs.iter().map(|tab| tab.label.as_ref()).collect();
        assert_eq!(labels, ["Files", "Sessions", "Source control", "Checks"]);
        assert_eq!(tabs[2].accessible_name(), "Source control, 3 changed");
        assert_eq!(tabs[3].accessible_name(), "Checks");
        assert!(core_tabs(0, 2)[3].failing);
    }

    #[test]
    fn labels_are_capped_at_twenty_four_characters() {
        let tab = ProjectTab::new(&"x".repeat(40), IconName::Puzzle);
        assert_eq!(tab.label.len(), LABEL_MAX_CHARS);
    }

    #[gpui::test]
    fn a_press_selects_and_arrows_only_move_focus(cx: &mut gpui::TestAppContext) {
        let chosen = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| TabsHarness {
            chosen: chosen.clone(),
        });
        cx.run_until_parked();

        cx.update(|window, cx| window.focus_next(cx));
        press_key(cx, "right");
        assert!(chosen.borrow().is_empty(), "arrows do not select");
        press_key(cx, "enter");
        assert_eq!(*chosen.borrow(), vec![1]);

        let sessions = cx.debug_bounds("tab-1").expect("tab is drawn").center();
        cx.simulate_click(sessions, gpui::Modifiers::none());
        assert_eq!(*chosen.borrow(), vec![1, 1]);
    }

    /// Hosts the core tabs with Files selected and records every selection.
    struct TabsHarness {
        chosen: Rc<RefCell<Vec<usize>>>,
    }

    impl gpui::Render for TabsHarness {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let chosen = self.chosen.clone();
            div().child(
                ProjectSidebarTabs::new("tabs", core_tabs(0, 0))
                    .selected(0)
                    .on_select(move |index, _, _| chosen.borrow_mut().push(index)),
            )
        }
    }
}
