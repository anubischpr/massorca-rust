//! Search field: a text field for filtering, whose query settles after a pause.

use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, Render, SharedString, Styled, Subscription, Task,
    Window, div, px,
};

use super::text_field::{TextField, TextFieldEvent};
use crate::icon::IconName;
use crate::theme::{ActiveTheme, TextStyled};
use crate::tokens::{SPACE_1, TYPE_CAPTION};

/// Pause in typing before a search runs (design system 4.8, handoff 2.7).
pub const SEARCH_DEBOUNCE_MS: u64 = 200;

/// What a [`SearchField`] emits; subscribe with `cx.subscribe`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchFieldEvent {
    /// A query is ready to run: it settled after typing, or was cleared at once.
    Search(SharedString),
    /// Enter was pressed in the field.
    Submitted,
}

/// What a query edit asks the view to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QueryStep {
    /// The text already matches what ran, so nothing happens.
    Idle,
    /// Run this query now; clearing never waits.
    Apply(String),
    /// Run the pending query once the timer for this generation fires.
    Wait(u64),
}

/// Debounce state of a query: only the newest edit settles into a search.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QueryDebounce {
    applied: String,
    pending: Option<String>,
    generation: u64,
}

impl QueryDebounce {
    /// A debounce whose `query` has already run.
    pub fn starting_at(query: &str) -> Self {
        Self {
            applied: query.to_owned(),
            ..Self::default()
        }
    }

    pub fn edit(&mut self, text: &str) -> QueryStep {
        self.generation += 1;
        self.pending = None;
        if text == self.applied {
            return QueryStep::Idle;
        }
        if text.is_empty() {
            self.applied.clear();
            return QueryStep::Apply(String::new());
        }
        self.pending = Some(text.to_owned());
        QueryStep::Wait(self.generation)
    }

    /// Settles the pending query when its timer fires, unless a newer edit replaced it.
    pub fn settle(&mut self, generation: u64) -> Option<String> {
        if generation != self.generation {
            return None;
        }
        let query = self.pending.take()?;
        self.applied.clone_from(&query);
        Some(query)
    }

    /// Runs `text` without waiting; `true` when it differs from the last query.
    pub fn apply_now(&mut self, text: &str) -> bool {
        self.generation += 1;
        self.pending = None;
        if text == self.applied {
            return false;
        }
        self.applied = text.to_owned();
        true
    }
}

/// A text field for filtering; its query settles [`SEARCH_DEBOUNCE_MS`] after the last keystroke.
#[derive(Debug)]
pub struct SearchField {
    field: Entity<TextField>,
    debounce: QueryDebounce,
    pending: Option<Task<()>>,
    result_count: Option<usize>,
    _field_events: Subscription,
}

impl EventEmitter<SearchFieldEvent> for SearchField {}

impl SearchField {
    pub fn new(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self::with_query(placeholder, "", cx)
    }

    /// A search field that starts with `query`; that query counts as already run.
    pub fn with_query(
        placeholder: impl Into<SharedString>,
        query: &str,
        cx: &mut Context<Self>,
    ) -> Self {
        let placeholder = placeholder.into();
        let field = cx.new(|cx| {
            TextField::new(cx)
                .placeholder(placeholder)
                .leading_icon(IconName::Search)
                .clearable()
                .with_text(query)
        });
        let field_events = cx.subscribe(&field, Self::on_field_event);
        Self {
            field,
            debounce: QueryDebounce::starting_at(query),
            pending: None,
            result_count: None,
            _field_events: field_events,
        }
    }

    /// Sets the match count caption under the field.
    pub fn result_count(mut self, count: Option<usize>) -> Self {
        self.result_count = count;
        self
    }

    pub fn set_result_count(&mut self, count: Option<usize>, cx: &mut Context<Self>) {
        self.result_count = count;
        cx.notify();
    }

    pub fn set_disabled(&mut self, disabled: bool, cx: &mut Context<Self>) {
        self.field
            .update(cx, |field, cx| field.set_disabled(disabled, cx));
    }

    /// The query as typed, which may not have run yet.
    pub fn text<'a>(&self, cx: &'a App) -> &'a str {
        self.field.read(cx).text()
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.field.read(cx).focus_handle(cx)
    }

    /// Sets the query at once, without the pause, and emits `Search` when it changed.
    pub fn set_query(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.pending = None;
        if self.debounce.apply_now(text) {
            cx.emit(SearchFieldEvent::Search(text.to_owned().into()));
        }
        self.field
            .update(cx, |field, cx| field.set_text(text, window, cx));
    }

    fn on_field_event(
        &mut self,
        field: Entity<TextField>,
        event: &TextFieldEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            TextFieldEvent::Submitted => cx.emit(SearchFieldEvent::Submitted),
            TextFieldEvent::Changed => {
                let text = field.read(cx).text().to_owned();
                match self.debounce.edit(&text) {
                    QueryStep::Idle => self.pending = None,
                    QueryStep::Apply(query) => {
                        self.pending = None;
                        cx.emit(SearchFieldEvent::Search(query.into()));
                    }
                    QueryStep::Wait(generation) => {
                        let timer = cx
                            .background_executor()
                            .timer(Duration::from_millis(SEARCH_DEBOUNCE_MS));
                        self.pending = Some(cx.spawn(async move |this, cx| {
                            timer.await;
                            this.update(cx, |this, cx| this.settle(generation, cx)).ok();
                        }));
                    }
                }
            }
        }
    }

    fn settle(&mut self, generation: u64, cx: &mut Context<Self>) {
        if let Some(query) = self.debounce.settle(generation) {
            cx.emit(SearchFieldEvent::Search(query.into()));
        }
    }
}

impl Render for SearchField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = cx.theme().palette;
        let count = self.result_count.map(count_label);
        div()
            .flex()
            .flex_col()
            .gap(px(SPACE_1))
            .w_full()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && !this.field.read(cx).text().is_empty() {
                    this.set_query("", window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(self.field.clone())
            .when_some(count, |root, label| {
                root.child(
                    div()
                        .text_style(TYPE_CAPTION)
                        .text_color(palette.text.tertiary)
                        .tabular_figures()
                        .child(label),
                )
            })
    }
}

/// The caption under the field: "No matches", "1 match" or "12 matches".
fn count_label(count: usize) -> String {
    match count {
        0 => "No matches".to_owned(),
        1 => "1 match".to_owned(),
        n => format!("{n} matches"),
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use gpui::{TestAppContext, VisualTestContext};

    use super::*;
    use crate::components::text_field;

    #[test]
    fn typing_settles_once_with_the_latest_query() {
        let mut debounce = QueryDebounce::default();
        assert_eq!(debounce.edit("a"), QueryStep::Wait(1));
        assert_eq!(debounce.edit("ab"), QueryStep::Wait(2));
        assert_eq!(debounce.edit("abc"), QueryStep::Wait(3));
        assert_eq!(debounce.settle(1), None);
        assert_eq!(debounce.settle(2), None);
        assert_eq!(debounce.settle(3), Some("abc".to_owned()));
        assert_eq!(debounce.settle(3), None);
    }

    #[test]
    fn clearing_runs_at_once_and_typing_back_cancels_the_wait() {
        let mut debounce = QueryDebounce::starting_at("abc");
        assert_eq!(debounce.edit("abcd"), QueryStep::Wait(1));
        assert_eq!(debounce.edit("abc"), QueryStep::Idle);
        assert_eq!(debounce.settle(1), None);
        assert_eq!(debounce.edit(""), QueryStep::Apply(String::new()));
        assert_eq!(debounce.edit(""), QueryStep::Idle);
    }

    #[test]
    fn set_query_runs_without_waiting_and_only_when_it_changes() {
        let mut debounce = QueryDebounce::starting_at("a");
        assert!(debounce.apply_now("ab"));
        assert!(!debounce.apply_now("ab"));
        assert_eq!(debounce.edit("ab"), QueryStep::Idle);
    }

    #[test]
    fn match_count_reads_naturally() {
        assert_eq!(count_label(0), "No matches");
        assert_eq!(count_label(1), "1 match");
        assert_eq!(count_label(12), "12 matches");
    }

    /// Subscribes to a search field and collects what it emits.
    fn record(
        search: &Entity<SearchField>,
        cx: &mut VisualTestContext,
    ) -> Rc<RefCell<Vec<SearchFieldEvent>>> {
        let events = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        cx.update(|_, cx| {
            cx.subscribe(search, move |_, event: &SearchFieldEvent, _| {
                sink.borrow_mut().push(event.clone())
            })
            .detach();
        });
        events
    }

    /// Focuses a search field and redraws, so its input handler is installed.
    fn focus_and_draw(search: &Entity<SearchField>, cx: &mut VisualTestContext) {
        search.update_in(cx, |search, window, cx| {
            let handle = search.focus_handle(cx);
            window.focus(&handle, cx);
            window.refresh();
        });
        cx.run_until_parked();
    }

    fn query_of(search: &Entity<SearchField>, cx: &VisualTestContext) -> String {
        search.read_with(cx, |search, cx| search.text(cx).to_owned())
    }

    #[gpui::test]
    fn typing_waits_out_the_debounce_then_searches_once(cx: &mut TestAppContext) {
        let clock = cx.executor();
        let (search, cx) = cx.add_window_view(|_, cx| SearchField::new("Search files", cx));
        let events = record(&search, cx);
        focus_and_draw(&search, cx);
        cx.simulate_input("abc");
        clock.advance_clock(Duration::from_millis(SEARCH_DEBOUNCE_MS - 1));
        cx.run_until_parked();
        assert!(events.borrow().is_empty());
        clock.advance_clock(Duration::from_millis(1));
        cx.run_until_parked();
        assert_eq!(
            *events.borrow(),
            vec![SearchFieldEvent::Search("abc".into())]
        );
    }

    #[gpui::test]
    fn each_keystroke_restarts_the_pause(cx: &mut TestAppContext) {
        let clock = cx.executor();
        let (search, cx) = cx.add_window_view(|_, cx| SearchField::new("Search files", cx));
        let events = record(&search, cx);
        focus_and_draw(&search, cx);
        cx.simulate_input("a");
        clock.advance_clock(Duration::from_millis(SEARCH_DEBOUNCE_MS - 50));
        cx.simulate_input("b");
        clock.advance_clock(Duration::from_millis(SEARCH_DEBOUNCE_MS - 50));
        cx.run_until_parked();
        assert!(events.borrow().is_empty());
        clock.advance_clock(Duration::from_millis(50));
        cx.run_until_parked();
        assert_eq!(
            *events.borrow(),
            vec![SearchFieldEvent::Search("ab".into())]
        );
    }

    #[gpui::test]
    fn escape_clears_a_query_at_once(cx: &mut TestAppContext) {
        let (search, cx) =
            cx.add_window_view(|_, cx| SearchField::with_query("Search files", "abc", cx));
        let events = record(&search, cx);
        focus_and_draw(&search, cx);
        cx.simulate_keystrokes("escape");
        assert_eq!(query_of(&search, cx), "");
        assert_eq!(*events.borrow(), vec![SearchFieldEvent::Search("".into())]);
    }

    #[gpui::test]
    fn escape_on_an_empty_field_leaves_it_alone(cx: &mut TestAppContext) {
        let (search, cx) = cx.add_window_view(|_, cx| SearchField::new("Search files", cx));
        let events = record(&search, cx);
        focus_and_draw(&search, cx);
        cx.simulate_keystrokes("escape");
        assert!(events.borrow().is_empty());
        assert_eq!(query_of(&search, cx), "");
    }

    #[gpui::test]
    fn enter_is_forwarded_as_submitted(cx: &mut TestAppContext) {
        cx.update(text_field::bind_keys);
        let (search, cx) = cx.add_window_view(|_, cx| SearchField::new("Search files", cx));
        let events = record(&search, cx);
        focus_and_draw(&search, cx);
        cx.simulate_keystrokes("enter");
        assert_eq!(*events.borrow(), vec![SearchFieldEvent::Submitted]);
    }
}
