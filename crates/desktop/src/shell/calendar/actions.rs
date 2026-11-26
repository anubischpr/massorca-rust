//! The Calendar's reads and controls on the shell (decision 0124). A change of week, mode or Show done asks the service
//! again for the week shown; an answer to a query that is no longer asked is dropped by the state.

use gpui::{Context, WeakEntity};
use massorca_protocol::methods::calendar::{
    CalendarKind, CalendarOccurrence, CalendarOccurrenceList, CalendarOccurrencesParams,
};

use super::super::Shell;
use super::CalendarFrame;
use crate::model::calendar::{CalendarMode, today};
use crate::model::canvas::Canvas;
use crate::model::navigation::Page;
use crate::model::teams::{TeamId, TeamPage};
use crate::service::{CAPABILITY_MISSING, Refusal};

/// What the page says when no service is connected.
const NOT_CONNECTED: &str = "Connect to MassOrca to read the calendar.";

impl Shell {
    /// Reads the shown week when the Calendar comes on screen, so a return shows the week as it is now.
    pub(in crate::shell) fn ensure_calendar_loaded(&mut self, cx: &mut Context<Self>) {
        let on_screen = self.model.canvas() == Canvas::Page(Page::Calendar);
        let opened = on_screen && !self.calendar_on_screen;
        self.calendar_on_screen = on_screen;
        if opened {
            self.request_calendar(cx);
        }
    }

    /// The page's frame for this render.
    pub(in crate::shell) fn calendar_frame(&self, shell: &WeakEntity<Shell>) -> CalendarFrame {
        CalendarFrame {
            state: self.calendar.clone(),
            connected: self.service.is_some(),
            shell: shell.clone(),
        }
    }

    pub(in crate::shell) fn calendar_previous_week(&mut self, cx: &mut Context<Self>) {
        self.calendar.previous_week();
        self.request_calendar(cx);
    }

    pub(in crate::shell) fn calendar_next_week(&mut self, cx: &mut Context<Self>) {
        self.calendar.next_week();
        self.request_calendar(cx);
    }

    pub(in crate::shell) fn calendar_today(&mut self, cx: &mut Context<Self>) {
        self.calendar.go_to_today(today());
        self.request_calendar(cx);
    }

    pub(in crate::shell) fn set_calendar_mode(
        &mut self,
        mode: CalendarMode,
        cx: &mut Context<Self>,
    ) {
        self.calendar.set_mode(mode);
        self.request_calendar(cx);
    }

    pub(in crate::shell) fn set_calendar_show_done(&mut self, show: bool, cx: &mut Context<Self>) {
        self.calendar.set_show_done(show);
        self.request_calendar(cx);
    }

    pub(in crate::shell) fn retry_calendar(&mut self, cx: &mut Context<Self>) {
        self.request_calendar(cx);
    }

    /// Opens a routine on its team's Routines page, or an automation on the Automations page.
    pub(in crate::shell) fn open_calendar_run(
        &mut self,
        run: &CalendarOccurrence,
        cx: &mut Context<Self>,
    ) {
        match run.kind {
            CalendarKind::Automation => self.model.open_page(Page::Automations),
            CalendarKind::Routine => {
                let team = run
                    .team_id
                    .as_deref()
                    .and_then(|id| self.store.read(cx).state().team_ids.get(id));
                if let Some(number) = team {
                    self.model
                        .open_team_page(TeamPage::Routines(TeamId(number)));
                }
            }
            CalendarKind::Unknown => {}
        }
        cx.notify();
    }

    /// Asks the service for the shown week under the current mode and Show done.
    fn request_calendar(&mut self, cx: &mut Context<Self>) {
        let Some(query) = self.calendar.query() else {
            return;
        };
        self.calendar.start_loading(query.clone());
        let Some(service) = self.service.clone() else {
            self.calendar.set_failed(&query, NOT_CONNECTED.to_owned());
            cx.notify();
            return;
        };
        cx.spawn(async move |this, cx| {
            let answer = service.calendar_occurrences(query.clone()).await;
            this.update(cx, |shell, cx| shell.calendar_read(&query, answer, cx))
                .ok();
        })
        .detach();
        cx.notify();
    }

    fn calendar_read(
        &mut self,
        query: &CalendarOccurrencesParams,
        answer: Result<CalendarOccurrenceList, Refusal>,
        cx: &mut Context<Self>,
    ) {
        match answer {
            Ok(list) => self.calendar.set_loaded(query, list),
            Err(refusal) if refusal.code() == Some(CAPABILITY_MISSING) => {
                self.calendar.set_unsupported();
            }
            Err(refusal) => self.calendar.set_failed(query, refusal.message()),
        }
        cx.notify();
    }
}
