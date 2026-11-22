//! Work panel tabs (docs/design/components/WorkPanelTabs.md): the tab strip of the Work panel,
//! one tab per terminal, browser page, Markdown or file and mobile emulator. The strip scrolls
//! sideways when tabs overflow and tabs reorder by dragging. With no tabs it offers the four kinds.

use crate::components::agent_icon::NameTooltip;
use crate::components::button::{Button, ButtonSize, ButtonVariant};
use crate::components::icon_button::{IconButton, IconButtonVariant};
use crate::components::segmented_control::{segment_key, tab_segment};
use crate::icon::{Icon, IconName, IconSize};
use crate::style::FocusRing;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{
    BORDER_WIDTH_THICK, DURATION_TOOLTIP_DELAY, LAYOUT_TAB_MAX_WIDTH, RADIUS_FULL, RADIUS_LG,
    SIZE_CONTROL_DEFAULT, SIZE_INDICATOR_DOT, SPACE_1, SPACE_2, TYPE_LABEL,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, Context, ElementId, FocusHandle, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, Render, RenderOnce, Role, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use std::rc::Rc;
use std::sync::Arc;

/// The kinds of tab, in the order the empty state and the "+" menu list them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkTabKind {
    Terminal,
    Browser,
    Markdown,
    MobileEmulator,
}

impl WorkTabKind {
    pub const ALL: [WorkTabKind; 4] = [
        WorkTabKind::Terminal,
        WorkTabKind::Browser,
        WorkTabKind::Markdown,
        WorkTabKind::MobileEmulator,
    ];

    /// The copy of the empty state and the menu.
    pub fn label(self) -> &'static str {
        match self {
            Self::Terminal => "New terminal",
            Self::Browser => "New browser tab",
            Self::Markdown => "New Markdown",
            Self::MobileEmulator => "New mobile emulator",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Terminal => IconName::SquareTerminal,
            Self::Browser => IconName::Globe,
            Self::Markdown => IconName::FileText,
            Self::MobileEmulator => IconName::Smartphone,
        }
    }
}

/// One tab: its kind, its title, and whether it has unsaved changes or is still loading.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkTab {
    kind: WorkTabKind,
    title: SharedString,
    dirty: bool,
    loading: bool,
    path: Option<SharedString>,
}

impl WorkTab {
    pub fn new(kind: WorkTabKind, title: impl Into<SharedString>) -> Self {
        Self {
            kind,
            title: title.into(),
            dirty: false,
            loading: false,
            path: None,
        }
    }

    /// Unsaved changes: a dot, and "unsaved changes" joins the accessible name.
    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    /// The label shows at once while the body loads.
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// The full path of a file tab, shown as its tooltip.
    pub fn path(mut self, path: impl Into<SharedString>) -> Self {
        self.path = Some(path.into());
        self
    }

    fn accessible_name(&self) -> String {
        if self.dirty {
            format!("{}, unsaved changes", self.title)
        } else {
            self.title.to_string()
        }
    }
}

/// What a press on the strip asks for. The caller acts on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkTabsAction {
    Select(usize),
    Close(usize),
    /// The "+" button: the caller opens its menu of kinds.
    Add,
    /// A kind chosen from the empty state.
    New(WorkTabKind),
    /// A tab dropped at another position.
    Move {
        from: usize,
        to: usize,
    },
}

type ActionHandler = Rc<dyn Fn(WorkTabsAction, &mut Window, &mut App)>;

/// The payload of a tab drag: where it came from and what it is called.
#[derive(Clone, Debug)]
struct DraggedTab {
    from: usize,
    title: SharedString,
}

/// The floating label that follows the pointer during a drag.
struct DragPreview(SharedString);

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.theme().palette;
        div()
            .flex()
            .items_center()
            .h(px(SIZE_CONTROL_DEFAULT))
            .px(px(SPACE_2))
            .rounded(px(RADIUS_LG))
            .border_1()
            .border_color(p.border.default)
            .bg(p.surface.overlay)
            .text_style(TYPE_LABEL)
            .text_color(p.text.primary)
            .child(self.0.clone())
    }
}

/// The Work panel's tab strip, or its empty state when there are no tabs.
#[derive(IntoElement)]
pub struct WorkPanelTabs {
    id: ElementId,
    tabs: Vec<WorkTab>,
    selected: usize,
    /// Kinds the empty state offers as disabled, each with the reason it is off.
    unavailable: Vec<(WorkTabKind, SharedString)>,
    on_action: Option<ActionHandler>,
}

impl std::fmt::Debug for WorkPanelTabs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkPanelTabs")
            .field("id", &self.id)
            .field("tabs", &self.tabs.len())
            .field("selected", &self.selected)
            .finish_non_exhaustive()
    }
}

impl WorkPanelTabs {
    pub fn new(id: impl Into<ElementId>, tabs: Vec<WorkTab>) -> Self {
        Self {
            id: id.into(),
            tabs,
            selected: 0,
            unavailable: Vec::new(),
            on_action: None,
        }
    }

    pub fn selected(mut self, index: usize) -> Self {
        self.selected = index;
        self
    }

    /// Runs with the action a press asks for; the caller keeps the tabs and updates them.
    pub fn on_action(
        mut self,
        listener: impl Fn(WorkTabsAction, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_action = Some(Rc::new(listener));
        self
    }

    /// Offers `kind` in the empty state as disabled, with `reason` as its tooltip: its layer does
    /// not exist or is not ready, so the action waits (A43).
    pub fn unavailable(mut self, kind: WorkTabKind, reason: impl Into<SharedString>) -> Self {
        self.unavailable.push((kind, reason.into()));
        self
    }
}

/// A press handler for a button or a plain element.
fn emit(
    handler: &Option<ActionHandler>,
    action: WorkTabsAction,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let handler = handler.clone();
    move |window, cx| {
        if let Some(handler) = &handler {
            handler(action, window, cx);
        }
    }
}

/// The element id of tab `index`; its focus handle is keyed by the same id.
fn tab_key(strip: &ElementId, index: usize) -> ElementId {
    ElementId::NamedChild(Arc::new(strip.clone()), format!("tab-{index}").into())
}

/// The empty state: a centred column of four buttons, each with its icon, and no title. A kind
/// that is unavailable is drawn disabled, with its reason as the tooltip.
fn empty_state(
    id: &ElementId,
    handler: &Option<ActionHandler>,
    unavailable: &[(WorkTabKind, SharedString)],
) -> AnyElement {
    let mut column = div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(SPACE_2))
        .size_full();
    for kind in WorkTabKind::ALL {
        let button = Button::new((id.clone(), kind.label()), kind.label())
            .variant(ButtonVariant::Secondary)
            .size(ButtonSize::Default)
            .leading_icon(kind.icon());
        let button = match unavailable.iter().find(|(offered, _)| *offered == kind) {
            Some((_, reason)) => button.disabled(true).tooltip(reason.clone()),
            None => button.on_click(emit(handler, WorkTabsAction::New(kind))),
        };
        column = column.child(
            div()
                .debug_selector(move || kind.label().into())
                .child(button),
        );
    }
    column.into_any_element()
}

/// One tab: icon, title, a dirty dot, a close mark, and the drop indicator while a tab hovers.
fn tab_element(
    id: &ElementId,
    index: usize,
    tab: WorkTab,
    selected: bool,
    handle: &FocusHandle,
    handler: &Option<ActionHandler>,
    theme: &crate::theme::Theme,
) -> AnyElement {
    let p = theme.palette;
    let ink = if selected {
        p.text.primary
    } else if tab.loading {
        p.text.tertiary
    } else {
        p.text.secondary
    };
    let name = tab.accessible_name();
    let tip = tab.path.clone().unwrap_or_else(|| tab.title.clone());
    let dragged = DraggedTab {
        from: index,
        title: tab.title.clone(),
    };
    let close = div()
        .id((tab_key(id, index), "close-wrap"))
        .debug_selector(move || format!("close-{index}"))
        .flex_none()
        .on_click(|_, _, cx| cx.stop_propagation())
        .child(
            IconButton::new((tab_key(id, index), "close"), IconName::X, "Close tab")
                .variant(IconButtonVariant::Ghost)
                .size(ButtonSize::Compact)
                .on_click(emit(handler, WorkTabsAction::Close(index))),
        );
    let drop_target = handler.clone();
    let mut element = div()
        .id(tab_key(id, index))
        .debug_selector(move || format!("work-tab-{index}"))
        .role(Role::Tab)
        .aria_label(name)
        .aria_selected(selected)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(SPACE_1))
        .h(px(SIZE_CONTROL_DEFAULT))
        .px(px(SPACE_2))
        .max_w(px(LAYOUT_TAB_MAX_WIDTH))
        .track_focus(handle)
        .focus_ring(theme)
        .cursor_pointer()
        .text_style(TYPE_LABEL)
        .text_color(ink)
        .when(selected, |el| el.bg(p.fill.selected_canvas))
        .when(!selected, |el| el.hover(move |s| s.bg(p.fill.hover_canvas)))
        .active(move |s| s.bg(p.fill.pressed_canvas))
        .tooltip_show_delay(DURATION_TOOLTIP_DELAY)
        .tooltip(move |_, cx| cx.new(|_| NameTooltip(tip.clone())).into())
        .on_drag(dragged, |tab: &DraggedTab, _, _, cx| {
            cx.new(|_| DragPreview(tab.title.clone()))
        })
        .drag_over::<DraggedTab>(move |style, _, _, _| {
            style.border_l_2().border_color(p.accent.default)
        })
        .on_drop::<DraggedTab>(move |drag, window, cx| {
            if let Some(handler) = &drop_target {
                handler(
                    WorkTabsAction::Move {
                        from: drag.from,
                        to: index,
                    },
                    window,
                    cx,
                );
            }
        })
        .child(Icon::new(tab.kind.icon()).size(IconSize::Small).color(ink))
        .child(div().min_w_0().truncate().child(tab.title.clone()))
        .when(tab.dirty, |el| {
            el.child(
                div()
                    .flex_none()
                    .size(px(SIZE_INDICATOR_DOT))
                    .rounded(px(RADIUS_FULL))
                    .bg(p.text.secondary),
            )
        })
        .child(close);
    if let Some(handler) = handler.clone() {
        element = element
            .on_click(move |_, window, cx| handler(WorkTabsAction::Select(index), window, cx));
    }
    if selected {
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
    element.into_any_element()
}

impl RenderOnce for WorkPanelTabs {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let id = self.id.clone();
        let handler = self.on_action.clone();
        if self.tabs.is_empty() {
            return empty_state(&id, &handler, &self.unavailable);
        }
        let count = self.tabs.len();
        let selected = self.selected.min(count - 1);
        let enabled = vec![true; count];
        let stop = tab_segment(selected, &enabled);
        let handles: Vec<FocusHandle> = (0..count)
            .map(|index| {
                window
                    .use_keyed_state(tab_key(&id, index), cx, |_, cx| cx.focus_handle())
                    .read(cx)
                    .clone()
                    .tab_stop(stop == Some(index))
            })
            .collect();

        let arrow_handles = handles.clone();
        let mut row = div()
            .id((id.clone(), "tabs"))
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .overflow_x_scroll()
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
        for (index, (tab, handle)) in self.tabs.into_iter().zip(&handles).enumerate() {
            row = row.child(tab_element(
                &id,
                index,
                tab,
                index == selected,
                handle,
                &handler,
                &theme,
            ));
        }
        let add = IconButton::new((id.clone(), "add"), IconName::Plus, "New tab")
            .variant(IconButtonVariant::Ghost)
            .size(ButtonSize::Default)
            .on_click(emit(&handler, WorkTabsAction::Add));

        div()
            .id(id.clone())
            .role(Role::TabList)
            .aria_label("Work panel")
            .flex()
            .items_center()
            .h(px(SIZE_CONTROL_DEFAULT))
            .border_b_1()
            .border_color(p.border.subtle)
            .bg(p.surface.canvas)
            .child(row)
            .child(add)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn kinds_list_the_spec_copy_in_order() {
        let labels: Vec<&str> = WorkTabKind::ALL.iter().map(|kind| kind.label()).collect();
        assert_eq!(
            labels,
            [
                "New terminal",
                "New browser tab",
                "New Markdown",
                "New mobile emulator"
            ]
        );
    }

    #[test]
    fn a_dirty_tab_says_so_in_its_name() {
        let plain = WorkTab::new(WorkTabKind::Markdown, "notes.md");
        let dirty = plain.clone().dirty(true);
        assert_eq!(plain.accessible_name(), "notes.md");
        assert_eq!(dirty.accessible_name(), "notes.md, unsaved changes");
    }

    #[gpui::test]
    fn select_close_and_add_send_their_actions_without_crossing(cx: &mut gpui::TestAppContext) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| StripHarness {
            seen: seen.clone(),
            tabs: 3,
            unavailable: Vec::new(),
        });
        cx.run_until_parked();

        let tab = cx
            .debug_bounds("work-tab-1")
            .expect("tab is drawn")
            .center();
        cx.simulate_click(tab, gpui::Modifiers::none());
        let close = cx.debug_bounds("close-1").expect("close is drawn").center();
        cx.simulate_click(close, gpui::Modifiers::none());

        assert_eq!(
            *seen.borrow(),
            vec![WorkTabsAction::Select(1), WorkTabsAction::Close(1)],
            "closing a tab does not also select it"
        );
    }

    #[gpui::test]
    fn the_empty_state_offers_the_four_kinds(cx: &mut gpui::TestAppContext) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| StripHarness {
            seen: seen.clone(),
            tabs: 0,
            unavailable: Vec::new(),
        });
        cx.run_until_parked();

        let browser = cx
            .debug_bounds("New browser tab")
            .expect("browser action is drawn")
            .center();
        cx.simulate_click(browser, gpui::Modifiers::none());
        assert_eq!(
            *seen.borrow(),
            vec![WorkTabsAction::New(WorkTabKind::Browser)]
        );
    }

    #[gpui::test]
    fn an_unavailable_kind_is_disabled_and_sends_nothing(cx: &mut gpui::TestAppContext) {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (_, cx) = cx.add_window_view(|_, _| StripHarness {
            seen: seen.clone(),
            tabs: 0,
            unavailable: vec![(WorkTabKind::Browser, "Browser tabs come later.")],
        });
        cx.run_until_parked();

        let browser = cx
            .debug_bounds("New browser tab")
            .expect("browser action is drawn")
            .center();
        cx.simulate_click(browser, gpui::Modifiers::none());
        let terminal = cx
            .debug_bounds("New terminal")
            .expect("terminal action is drawn")
            .center();
        cx.simulate_click(terminal, gpui::Modifiers::none());

        assert_eq!(
            *seen.borrow(),
            vec![WorkTabsAction::New(WorkTabKind::Terminal)],
            "the disabled kind sends nothing; the enabled one still does"
        );
    }

    /// Hosts a strip of `tabs` terminals, or the empty state, and records its actions.
    struct StripHarness {
        seen: Rc<RefCell<Vec<WorkTabsAction>>>,
        tabs: usize,
        unavailable: Vec<(WorkTabKind, &'static str)>,
    }

    impl Render for StripHarness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let seen = self.seen.clone();
            let tabs = (0..self.tabs)
                .map(|index| WorkTab::new(WorkTabKind::Terminal, format!("shell {index}")))
                .collect();
            let strip = self.unavailable.iter().fold(
                WorkPanelTabs::new("work", tabs)
                    .selected(0)
                    .on_action(move |action, _, _| seen.borrow_mut().push(action)),
                |strip, (kind, reason)| strip.unavailable(*kind, *reason),
            );
            div().child(strip)
        }
    }
}
