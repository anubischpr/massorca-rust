//! Select: one value from a known list, shown in a field-shaped trigger with a Menu below it
//! (docs/design/components/Select.md). Enter, Space or Down opens the menu; the menu's keyboard
//! model takes over; Enter chooses and Escape returns focus to the trigger. Closed and focused,
//! a letter selects the next option that starts with it.

use crate::components::menu::{
    MenuEntry, MenuEvent, MenuItem, MenuOrigin, MenuView, menu_overlay, type_ahead,
};
use crate::components::popover::TriggerBounds;
use crate::icon::{Icon, IconName, IconSize};
use crate::style::{FocusRing, StateFill, Surface};
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{RADIUS_MD, SIZE_CONTROL_DEFAULT, SPACE_1, SPACE_2, TYPE_BODY, TYPE_CAPTION};
use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, ClickEvent, Context, Entity, EventEmitter, FocusHandle, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, SharedString, StatefulInteractiveElement,
    Styled, Subscription, Window, div, px,
};
use std::fmt;

/// What a select reports to its owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectEvent {
    /// The value changed to the option at this index.
    Changed(usize),
}

/// Menu rows for `options`, with the selected option checked.
pub(crate) fn option_entries(options: &[SharedString], selected: Option<usize>) -> Vec<MenuEntry> {
    options
        .iter()
        .enumerate()
        .map(|(index, label)| {
            MenuItem::new(label.clone())
                .checked(selected == Some(index))
                .into()
        })
        .collect()
}

/// The option a letter selects while the select is closed: the next one that starts with it.
pub(crate) fn letter_choice(
    options: &[SharedString],
    selected: Option<usize>,
    letter: char,
) -> Option<usize> {
    type_ahead(&option_entries(options, selected), selected, letter)
}

/// A single-choice field. The owner subscribes to [`SelectEvent`] for changes.
pub struct SelectView {
    options: Vec<SharedString>,
    selected: Option<usize>,
    placeholder: SharedString,
    error: Option<SharedString>,
    disabled: bool,
    empty_text: SharedString,
    focus: FocusHandle,
    menu: Option<Entity<MenuView>>,
    _menu_events: Option<Subscription>,
    bounds: TriggerBounds,
}

impl EventEmitter<SelectEvent> for SelectView {}

impl fmt::Debug for SelectView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SelectView")
            .field("options", &self.options.len())
            .field("selected", &self.selected)
            .field("disabled", &self.disabled)
            .field("open", &self.is_open())
            .finish_non_exhaustive()
    }
}

impl SelectView {
    pub fn new(
        options: impl IntoIterator<Item = impl Into<SharedString>>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            options: options.into_iter().map(Into::into).collect(),
            selected: None,
            placeholder: "Choose".into(),
            error: None,
            disabled: false,
            empty_text: "Nothing to choose from".into(),
            focus: cx.focus_handle(),
            menu: None,
            _menu_events: None,
            bounds: TriggerBounds::default(),
        }
    }

    /// The option chosen at the start; an index past the end is ignored.
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index.filter(|index| *index < self.options.len());
        self
    }

    /// Shown in the trigger while nothing is chosen, naming the choice.
    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// A reason shown under the trigger in danger colour, with the error glyph.
    pub fn error(mut self, text: impl Into<SharedString>) -> Self {
        self.error = Some(text.into());
        self
    }

    /// The menu line when there are no options, saying why.
    pub fn empty_text(mut self, text: impl Into<SharedString>) -> Self {
        self.empty_text = text.into();
        self
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    pub fn selected_label(&self) -> Option<&SharedString> {
        self.selected.and_then(|index| self.options.get(index))
    }

    pub fn is_open(&self) -> bool {
        self.menu.is_some()
    }

    /// Sets the value without the menu or an event.
    pub fn set_selected(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        let index = index.filter(|index| *index < self.options.len());
        if self.selected != index {
            self.selected = index;
            cx.notify();
        }
    }

    /// Opens the menu under the trigger, with the selected option highlighted. Does nothing when
    /// disabled or already open.
    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.disabled || self.is_open() {
            return;
        }
        window.focus(&self.focus, cx);
        let entries = option_entries(&self.options, self.selected);
        let selected = self.selected;
        let empty = self.empty_text.clone();
        let width = self
            .bounds
            .get()
            .map_or(0.0, |trigger| f32::from(trigger.size.width));
        let menu = cx.new(|cx| {
            MenuView::new(entries, window, cx)
                .with_min_width(width)
                .with_empty_text(empty)
                .with_highlight(selected)
        });
        self._menu_events = Some(cx.subscribe(
            &menu,
            |this, _, event: &MenuEvent, cx| match *event {
                MenuEvent::Chosen(choice) => this.choose(choice.row, cx),
                MenuEvent::Dismissed => this.close(cx),
            },
        ));
        self.menu = Some(menu);
        cx.notify();
    }

    fn toggle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_open() {
            self.close(cx);
        } else {
            self.open(window, cx);
        }
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.menu.take().is_some() {
            self._menu_events = None;
            cx.notify();
        }
    }

    fn choose(&mut self, index: usize, cx: &mut Context<Self>) {
        self.close(cx);
        if index < self.options.len() && self.selected != Some(index) {
            self.selected = Some(index);
            cx.emit(SelectEvent::Changed(index));
        }
    }

    fn on_trigger_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled || self.is_open() {
            return;
        }
        let key = event.keystroke.key.as_str();
        if key == "down" {
            self.open(window, cx);
            cx.stop_propagation();
            return;
        }
        let mut chars = key.chars();
        let modifiers = event.keystroke.modifiers;
        if let (Some(letter), None) = (chars.next(), chars.next())
            && letter.is_alphanumeric()
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.platform
            && let Some(index) = letter_choice(&self.options, self.selected, letter)
        {
            self.choose(index, cx);
            cx.stop_propagation();
        }
    }
}

impl Render for SelectView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = *cx.theme();
        let p = theme.palette;
        let open = self.is_open();
        let value = self.selected_label().cloned();
        let shown = value.clone().unwrap_or_else(|| self.placeholder.clone());
        let (fill, text, edge) = if self.disabled {
            (p.fill.disabled, p.text.disabled, p.border.disabled)
        } else {
            let text = if value.is_some() {
                p.text.primary
            } else {
                p.text.tertiary
            };
            let edge = if self.error.is_some() {
                p.status.danger_fg
            } else if open {
                p.border.focus
            } else {
                p.border.strong
            };
            (p.surface.sunken, text, edge)
        };

        let mut trigger = div()
            .id("select-trigger")
            .debug_selector(|| "select-trigger".into())
            .track_focus(&self.focus)
            .tab_stop(!self.disabled)
            .focus_ring(&theme)
            .flex()
            .items_center()
            .justify_between()
            .gap(px(SPACE_2))
            .h(px(SIZE_CONTROL_DEFAULT))
            .px(px(SPACE_2))
            .rounded(px(RADIUS_MD))
            .bg(fill)
            .border_color(edge)
            .text_style(TYPE_BODY)
            .text_color(text)
            .when(open, |el| el.border_2())
            .when(!open, |el| el.border_1())
            .child(div().flex_1().min_w_0().truncate().child(shown))
            .child(
                Icon::new(IconName::ChevronsUpDown)
                    .size(IconSize::Small)
                    .color(if open { p.accent.fg } else { p.text.secondary }),
            );
        if !self.disabled {
            trigger = trigger
                .cursor_pointer()
                .state_fills(&theme, Surface::Raised)
                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.toggle(window, cx);
                }))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    this.on_trigger_key(event, window, cx);
                }));
        }

        let mut host = self.bounds.track(div().relative()).child(trigger);
        if let Some(error) = self.error.clone() {
            host = host.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(SPACE_1))
                    .pt(px(SPACE_1))
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
        if let Some(menu) = self.menu.clone() {
            match self.bounds.get() {
                Some(bounds) => {
                    host = host.child(menu_overlay(&menu, MenuOrigin::Below(bounds), cx));
                }
                // The trigger is measured on the first paint; the menu follows on the next frame.
                None => window.request_animation_frame(),
            }
        }
        host
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn branches() -> Vec<SharedString> {
        ["main", "feature/overlays", "release/2026.11"]
            .into_iter()
            .map(SharedString::from)
            .collect()
    }

    #[test]
    fn only_the_selected_option_is_checked() {
        let entries = option_entries(&branches(), Some(1));
        let checked: Vec<Option<bool>> = entries
            .iter()
            .map(|entry| match entry {
                MenuEntry::Item(item) => item.is_checked(),
                _ => None,
            })
            .collect();
        assert_eq!(checked, vec![Some(false), Some(true), Some(false)]);
    }

    #[test]
    fn a_letter_selects_the_next_option_that_starts_with_it() {
        let options = branches();
        assert_eq!(letter_choice(&options, None, 'f'), Some(1));
        assert_eq!(letter_choice(&options, Some(1), 'r'), Some(2));
        assert_eq!(
            letter_choice(&options, Some(2), 'm'),
            Some(0),
            "wraps to the start"
        );
        assert_eq!(letter_choice(&options, None, 'z'), None);
    }

    #[test]
    fn an_empty_option_list_has_nothing_to_choose() {
        assert_eq!(letter_choice(&[], None, 'a'), None);
        assert!(option_entries(&[], None).is_empty());
    }
}

#[cfg(test)]
mod element_tests {
    use super::*;
    use crate::components::button::test_support::{focus_and_draw, press_key};
    use gpui::{AppContext, Context, Render, TestAppContext, VisualTestContext};
    use std::cell::RefCell;
    use std::rc::Rc;

    type Events = Rc<RefCell<Vec<SelectEvent>>>;

    /// Hosts the select so that it paints and can be clicked.
    struct Harness {
        select: Option<Entity<SelectView>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().p(px(SPACE_2)).children(self.select.clone())
        }
    }

    fn options() -> Vec<SharedString> {
        ["main", "feature/overlays", "release/2026.11"]
            .into_iter()
            .map(SharedString::from)
            .collect()
    }

    /// Attaches a select built by `build` to the harness, records its events and draws it.
    fn attach(
        view: &Entity<Harness>,
        cx: &mut VisualTestContext,
        build: impl FnOnce(&mut Context<SelectView>) -> SelectView + 'static,
    ) -> (Entity<SelectView>, Events) {
        let select = cx.update(|_, cx| cx.new(build));
        let events: Events = Rc::default();
        let log = events.clone();
        cx.update(|_, cx| {
            cx.subscribe(&select, move |_, event: &SelectEvent, _| {
                log.borrow_mut().push(*event);
            })
            .detach();
        });
        view.update(cx, |harness, cx| {
            harness.select = Some(select.clone());
            cx.notify();
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        (select, events)
    }

    fn trigger_centre(cx: &mut VisualTestContext) -> gpui::Point<gpui::Pixels> {
        cx.debug_bounds("select-trigger")
            .expect("trigger is drawn")
            .center()
    }

    #[gpui::test]
    fn a_click_opens_the_menu_and_choosing_a_row_changes_the_value(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, _| Harness { select: None });
        cx.run_until_parked();
        let (select, events) = attach(&view, cx, |cx| {
            SelectView::new(options(), cx).placeholder("Choose a branch")
        });
        let centre = trigger_centre(cx);
        cx.simulate_click(centre, gpui::Modifiers::none());
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(select.read_with(cx, |s, _| s.is_open()));

        let row = cx
            .debug_bounds("menu-item-2")
            .expect("the menu rows are drawn")
            .center();
        cx.simulate_click(row, gpui::Modifiers::none());
        assert_eq!(select.read_with(cx, |s, _| s.selected_index()), Some(2));
        assert!(!select.read_with(cx, |s, _| s.is_open()));
        assert_eq!(*events.borrow(), vec![SelectEvent::Changed(2)]);
    }

    #[gpui::test]
    fn down_opens_the_menu_and_enter_chooses_the_highlighted_row(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, _| Harness { select: None });
        cx.run_until_parked();
        let (select, events) = attach(&view, cx, |cx| SelectView::new(options(), cx));
        let focus = select.read_with(cx, |s, _| s.focus.clone());
        focus_and_draw(cx, &focus);

        press_key(cx, "down");
        assert!(select.read_with(cx, |s, _| s.is_open()));
        press_key(cx, "down");
        press_key(cx, "enter");
        assert_eq!(select.read_with(cx, |s, _| s.selected_index()), Some(1));
        assert_eq!(*events.borrow(), vec![SelectEvent::Changed(1)]);
    }

    #[gpui::test]
    fn escape_closes_without_a_change_and_returns_focus_to_the_trigger(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, _| Harness { select: None });
        cx.run_until_parked();
        let (select, events) = attach(&view, cx, |cx| SelectView::new(options(), cx));
        let focus = select.read_with(cx, |s, _| s.focus.clone());
        focus_and_draw(cx, &focus);
        press_key(cx, "down");
        assert!(select.read_with(cx, |s, _| s.is_open()));

        press_key(cx, "escape");
        assert!(!select.read_with(cx, |s, _| s.is_open()));
        assert_eq!(select.read_with(cx, |s, _| s.selected_index()), None);
        assert!(events.borrow().is_empty());
        cx.update(|window, _| assert!(focus.is_focused(window)));
    }

    #[gpui::test]
    fn a_disabled_select_does_not_open(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, _| Harness { select: None });
        cx.run_until_parked();
        let (select, _events) = attach(&view, cx, |cx| {
            SelectView::new(options(), cx).disabled(true)
        });
        let centre = trigger_centre(cx);
        cx.simulate_click(centre, gpui::Modifiers::none());
        assert!(!select.read_with(cx, |s, _| s.is_open()));
    }

    #[gpui::test]
    fn a_letter_on_the_closed_trigger_selects_the_next_match(cx: &mut TestAppContext) {
        let (view, cx) = cx.add_window_view(|_, _| Harness { select: None });
        cx.run_until_parked();
        let (select, events) = attach(&view, cx, |cx| SelectView::new(options(), cx));
        let focus = select.read_with(cx, |s, _| s.focus.clone());
        focus_and_draw(cx, &focus);

        press_key(cx, "r");
        assert_eq!(select.read_with(cx, |s, _| s.selected_index()), Some(2));
        assert_eq!(*events.borrow(), vec![SelectEvent::Changed(2)]);
    }
}
