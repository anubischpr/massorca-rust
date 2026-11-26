//! The Calendar page's state (decision 0124): the week it shows, the mode and the Show done switch, and what the service
//! last answered. The shell changes it and sends the query it builds; an answer to a query that is no longer asked is
//! dropped.

use jiff::Span;
use jiff::civil::Date;
use massorca_protocol::methods::calendar::{
    CalendarKind, CalendarOccurrence, CalendarOccurrenceList, CalendarOccurrencesParams,
};

use super::week::{column_of, fetch_window, week_start};

/// Which runs the page lists: every routine and automation, Bot Teams routines only, or Workspaces automations only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CalendarMode {
    #[default]
    All,
    Routines,
    Automations,
}

impl CalendarMode {
    /// The modes in the order the page's chips show them.
    pub const ALL: [Self; 3] = [Self::All, Self::Routines, Self::Automations];

    pub fn kind(self) -> Option<CalendarKind> {
        match self {
            Self::All => None,
            Self::Routines => Some(CalendarKind::Routine),
            Self::Automations => Some(CalendarKind::Automation),
        }
    }

    /// The chip's words, as the Calendar artboard words them: "Team" for Bot Teams routines.
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Routines => "Team",
            Self::Automations => "Automations",
        }
    }
}

/// What the service last said about the shown week, for the query it was asked.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CalendarLoad {
    /// Nothing asked yet.
    #[default]
    Idle,
    /// A read for this query is on its way.
    Loading(CalendarOccurrencesParams),
    Loaded(CalendarOccurrenceList),
    /// The read failed; the text is the reason.
    Failed(String),
    /// The service predates `calendar/1`, so it cannot answer.
    Unsupported,
}

/// The Calendar page's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarState {
    week: Date,
    mode: CalendarMode,
    show_done: bool,
    load: CalendarLoad,
}

impl CalendarState {
    /// The page opens on the week of `today`, with every kind listed and done runs hidden.
    pub fn new(today: Date) -> Self {
        Self {
            week: week_start(today),
            mode: CalendarMode::default(),
            show_done: false,
            load: CalendarLoad::default(),
        }
    }

    /// The Monday the shown week starts on.
    pub fn week(&self) -> Date {
        self.week
    }

    pub fn mode(&self) -> CalendarMode {
        self.mode
    }

    pub fn show_done(&self) -> bool {
        self.show_done
    }

    pub fn load(&self) -> &CalendarLoad {
        &self.load
    }

    pub fn previous_week(&mut self) {
        self.shift_week(-7);
    }

    pub fn next_week(&mut self) {
        self.shift_week(7);
    }

    /// Shows the week of `today`.
    pub fn go_to_today(&mut self, today: Date) {
        self.week = week_start(today);
        self.forget_load();
    }

    pub fn set_mode(&mut self, mode: CalendarMode) {
        self.mode = mode;
        self.forget_load();
    }

    pub fn set_show_done(&mut self, show: bool) {
        self.show_done = show;
        self.forget_load();
    }

    /// The query for the shown week under the current mode and Show done; `None` when no window can be built.
    pub fn query(&self) -> Option<CalendarOccurrencesParams> {
        let (from_ms, to_ms) = fetch_window(self.week)?;
        Some(CalendarOccurrencesParams {
            from_ms,
            to_ms,
            kind: self.mode.kind(),
            include_done: self.show_done,
        })
    }

    /// Records that `query` is on its way.
    pub fn start_loading(&mut self, query: CalendarOccurrencesParams) {
        self.load = CalendarLoad::Loading(query);
    }

    /// Records the answer to `query`. An answer to a query no longer asked is dropped.
    pub fn set_loaded(&mut self, query: &CalendarOccurrencesParams, list: CalendarOccurrenceList) {
        if self.load == CalendarLoad::Loading(query.clone()) {
            self.load = CalendarLoad::Loaded(list);
        }
    }

    /// Records that `query` failed. An answer to a query no longer asked is dropped.
    pub fn set_failed(&mut self, query: &CalendarOccurrencesParams, reason: String) {
        if self.load == CalendarLoad::Loading(query.clone()) {
            self.load = CalendarLoad::Failed(reason);
        }
    }

    pub fn set_unsupported(&mut self) {
        self.load = CalendarLoad::Unsupported;
    }

    /// The runs of the shown week by local day, Monday first, each day earliest first. Empty until a read is loaded.
    pub fn days(&self) -> [Vec<CalendarOccurrence>; 7] {
        let mut days: [Vec<CalendarOccurrence>; 7] = Default::default();
        if let CalendarLoad::Loaded(list) = &self.load {
            for run in &list.occurrences {
                if let Some(column) = column_of(&run.local_date, self.week) {
                    days[column].push(run.clone());
                }
            }
            for day in &mut days {
                day.sort_by(|a, b| {
                    a.local_time
                        .cmp(&b.local_time)
                        .then_with(|| a.name.cmp(&b.name))
                });
            }
        }
        days
    }

    fn shift_week(&mut self, days: i64) {
        if let Ok(moved) = self.week.checked_add(Span::new().days(days)) {
            self.week = moved;
            self.forget_load();
        }
    }

    /// The query changed, so what was asked or loaded belongs to another query; the shell asks again at once.
    fn forget_load(&mut self) {
        self.load = CalendarLoad::Idle;
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use massorca_protocol::methods::calendar::{
        CalendarKind, CalendarOccurrence, CalendarOccurrenceList,
    };

    use super::{CalendarLoad, CalendarMode, CalendarState};

    fn run(name: &str, local_date: &str, local_time: &str) -> CalendarOccurrence {
        CalendarOccurrence {
            kind: CalendarKind::Routine,
            source_id: name.to_owned(),
            name: name.to_owned(),
            team_id: Some("team".to_owned()),
            project_id: None,
            at_ms: 0,
            local_date: local_date.to_owned(),
            local_time: local_time.to_owned(),
            zone: Some("UTC".to_owned()),
            done: false,
        }
    }

    fn list(runs: Vec<CalendarOccurrence>) -> CalendarOccurrenceList {
        CalendarOccurrenceList {
            occurrences: runs,
            truncated: false,
        }
    }

    #[test]
    fn the_page_opens_on_this_week_with_every_kind_listed_and_done_runs_hidden() {
        let state = CalendarState::new(date(2026, 11, 25));
        assert_eq!(state.week(), date(2026, 11, 23));
        assert_eq!(state.mode(), CalendarMode::All);
        assert!(!state.show_done());
        let query = state.query().expect("a window");
        assert_eq!(query.kind, None);
        assert!(!query.include_done);
        assert_eq!(query.to_ms - query.from_ms, 9 * 86_400_000);
    }

    #[test]
    fn a_mode_asks_only_for_its_kind() {
        let mut state = CalendarState::new(date(2026, 11, 25));
        state.set_mode(CalendarMode::Routines);
        assert_eq!(
            state.query().expect("a window").kind,
            Some(CalendarKind::Routine)
        );
        state.set_mode(CalendarMode::Automations);
        assert_eq!(
            state.query().expect("a window").kind,
            Some(CalendarKind::Automation)
        );
    }

    #[test]
    fn the_weeks_move_by_seven_days_and_today_returns_to_its_monday() {
        let mut state = CalendarState::new(date(2026, 11, 25));
        state.next_week();
        assert_eq!(state.week(), date(2026, 11, 30));
        state.previous_week();
        state.previous_week();
        assert_eq!(state.week(), date(2026, 11, 16));
        state.go_to_today(date(2026, 11, 25));
        assert_eq!(state.week(), date(2026, 11, 23));
    }

    #[test]
    fn a_loaded_list_is_placed_on_its_local_day_and_the_days_outside_the_week_are_left_out() {
        let mut state = CalendarState::new(date(2026, 11, 25));
        let query = state.query().expect("a window");
        state.start_loading(query.clone());
        state.set_loaded(
            &query,
            list(vec![
                run("late", "2026-11-25", "18:00"),
                run("early", "2026-11-25", "09:00"),
                run("outside", "2026-12-01", "09:00"),
            ]),
        );
        let days = state.days();
        let names: Vec<&str> = days[2].iter().map(|run| run.name.as_str()).collect();
        assert_eq!(names, vec!["early", "late"]);
        assert_eq!(days.iter().map(Vec::len).sum::<usize>(), 2);
    }

    #[test]
    fn an_answer_to_a_query_no_longer_asked_is_not_shown() {
        let mut state = CalendarState::new(date(2026, 11, 25));
        let asked = state.query().expect("a window");
        state.start_loading(asked.clone());
        state.next_week();
        state.set_loaded(&asked, list(vec![run("old", "2026-11-25", "09:00")]));
        assert!(!matches!(state.load(), CalendarLoad::Loaded(_)));
        assert!(state.days().iter().all(Vec::is_empty));
    }
}
