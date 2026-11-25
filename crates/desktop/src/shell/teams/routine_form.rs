//! The New and Edit routine form (screens 4.7): its draft and fields, and the saving of the draft. The model checks
//! the draft; the service or the sample store saves it. The page that draws the form is in `routine_form_page`.

use std::fmt;

use super::super::Shell;
use super::routine_service::ROUTINE_GONE;
use crate::model::teams::{
    BotId, MONTH_DAYS, RoutineDraft, RoutineField, RoutineId, ScheduleKind, Team, TeamId, TeamPage,
    Weekday,
};
use crate::service::ServiceHandle;
use gpui::{Context, Entity, Subscription, prelude::*};
use jiff::tz::TimeZone;
use massorca_ui::components::select::{SelectEvent, SelectView};
use massorca_ui::components::text_field::{TextField, TextFieldEvent};

/// The routine form on screen: the team and routine it is for, its draft, and the fields that edit it.
pub(in crate::shell) struct RoutineForm {
    pub(in crate::shell) team: TeamId,
    /// The routine being edited; `None` for a New routine.
    pub(in crate::shell) routine: Option<RoutineId>,
    pub(in crate::shell) draft: RoutineDraft,
    /// The first problem with the draft, or the sentence the service answered with.
    pub(in crate::shell) error: Option<String>,
    /// Whether a save is waiting for the service.
    pub(in crate::shell) busy: bool,
    /// The team's bots in the order the Bot select lists them.
    pub(in crate::shell) bots: Vec<BotId>,
    pub(in crate::shell) name: Entity<TextField>,
    pub(in crate::shell) prompt: Entity<TextField>,
    pub(in crate::shell) time: Entity<TextField>,
    pub(in crate::shell) run_date: Entity<TextField>,
    pub(in crate::shell) bot: Entity<SelectView>,
    pub(in crate::shell) month_day: Entity<SelectView>,
    _subscriptions: Vec<Subscription>,
}

impl RoutineForm {
    fn new(
        team: &Team,
        routine: Option<RoutineId>,
        draft: RoutineDraft,
        cx: &mut Context<Shell>,
    ) -> Self {
        let bots: Vec<BotId> = team.bots.iter().map(|bot| bot.id).collect();
        let mut subscriptions = Vec::new();
        let name = text_field(
            cx,
            &mut subscriptions,
            TextInput {
                field: RoutineField::Name,
                label: "Name",
                placeholder: "Name the routine",
                helper: None,
                text: &draft.name,
            },
        );
        let prompt = text_field(
            cx,
            &mut subscriptions,
            TextInput {
                field: RoutineField::Prompt,
                label: "What should the bot do?",
                placeholder: "Describe the task",
                helper: Some("Write it as you would tell a person. Up to 2,000 characters."),
                text: &draft.prompt,
            },
        );
        let time = text_field(
            cx,
            &mut subscriptions,
            TextInput {
                field: RoutineField::Time,
                label: "Time",
                placeholder: "09:00",
                helper: Some("24-hour time, on this computer's clock."),
                text: &draft.time,
            },
        );
        let run_date = text_field(
            cx,
            &mut subscriptions,
            TextInput {
                field: RoutineField::RunDate,
                label: "Date",
                placeholder: "YYYY-MM-DD",
                helper: None,
                text: &draft.run_date,
            },
        );
        let bot_names: Vec<String> = team.bots.iter().map(|bot| bot.name.clone()).collect();
        let selected_bot = draft
            .bot
            .and_then(|id| bots.iter().position(|known| *known == id));
        let bot = cx.new(|cx| {
            SelectView::new(bot_names, cx)
                .label("Bot")
                .placeholder("Choose a bot")
                .selected(selected_bot)
        });
        subscriptions.push(cx.subscribe(&bot, |shell, _, event: &SelectEvent, cx| {
            let SelectEvent::Changed(index) = *event;
            shell.routine_bot_chosen(index, cx);
        }));
        let day_names = MONTH_DAYS.map(|day| day.to_string());
        let selected_day = usize::from(draft.month_day).checked_sub(1);
        let month_day = cx.new(|cx| {
            SelectView::new(day_names, cx)
                .label("Day of the month")
                .selected(selected_day)
        });
        subscriptions.push(
            cx.subscribe(&month_day, |shell, _, event: &SelectEvent, cx| {
                let SelectEvent::Changed(index) = *event;
                shell.routine_month_day_chosen(index, cx);
            }),
        );
        Self {
            team: team.id,
            routine,
            draft,
            error: None,
            busy: false,
            bots,
            name,
            prompt,
            time,
            run_date,
            bot,
            month_day,
            _subscriptions: subscriptions,
        }
    }
}

impl fmt::Debug for RoutineForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RoutineForm")
            .field("team", &self.team)
            .field("routine", &self.routine)
            .field("draft", &self.draft)
            .finish_non_exhaustive()
    }
}

/// One text field of the form: which draft field it edits and what it shows.
struct TextInput<'a> {
    field: RoutineField,
    label: &'static str,
    placeholder: &'static str,
    helper: Option<&'static str>,
    text: &'a str,
}

/// A text field whose changes land in the draft, and whose Enter saves the routine.
fn text_field(
    cx: &mut Context<Shell>,
    subscriptions: &mut Vec<Subscription>,
    input: TextInput<'_>,
) -> Entity<TextField> {
    let field = cx.new(|cx| {
        let field = TextField::new(cx)
            .label(input.label)
            .placeholder(input.placeholder)
            .with_text(input.text);
        match input.helper {
            Some(helper) => field.helper(helper),
            None => field,
        }
    });
    let edited = input.field;
    subscriptions.push(
        cx.subscribe(
            &field,
            move |shell, entity, event: &TextFieldEvent, cx| match event {
                TextFieldEvent::Changed => {
                    let text = entity.read(cx).text().to_owned();
                    shell.routine_text_changed(edited, text, cx);
                }
                TextFieldEvent::Submitted => shell.save_routine_form(cx),
            },
        ),
    );
    field
}

impl Shell {
    /// Opens the New or Edit routine page with its draft filled from the team, or from the routine it edits.
    pub(super) fn open_routine_form(&mut self, page: TeamPage, cx: &mut Context<Self>) {
        let (team_id, routine) = match page {
            TeamPage::NewRoutine(team) => (team, None),
            TeamPage::EditRoutine(team, routine) => (team, Some(routine)),
            _ => return,
        };
        let Some(team) = self.model.teams().team(team_id).cloned() else {
            return;
        };
        let draft = match routine {
            None => RoutineDraft::new_routine(team.bots.first().map(|bot| bot.id)),
            Some(id) => match team.routines.iter().find(|item| item.id == id) {
                Some(item) => RoutineDraft::of(item),
                None => return,
            },
        };
        self.routine_form = Some(RoutineForm::new(&team, routine, draft, cx));
        self.model.open_team_page(page);
        cx.notify();
    }

    /// Leaves the form for the team's Routines page, dropping what was typed.
    pub(super) fn close_routine_form(&mut self, cx: &mut Context<Self>) {
        if let Some(form) = self.routine_form.take() {
            self.model.open_team_page(TeamPage::Routines(form.team));
        }
        cx.notify();
    }

    /// Stores one text field's text in the draft and clears the form's problem.
    pub(super) fn routine_text_changed(
        &mut self,
        field: RoutineField,
        text: String,
        cx: &mut Context<Self>,
    ) {
        let Some(form) = self.routine_form.as_mut() else {
            return;
        };
        match field {
            RoutineField::Name => form.draft.name = text,
            RoutineField::Prompt => form.draft.prompt = text,
            RoutineField::Time => form.draft.time = text,
            RoutineField::RunDate => form.draft.run_date = text,
            RoutineField::Bot => {}
        }
        form.error = None;
        cx.notify();
    }

    pub(super) fn routine_kind_chosen(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(kind) = ScheduleKind::ALL.get(index).copied() else {
            return;
        };
        if let Some(form) = self.routine_form.as_mut() {
            form.draft.kind = kind;
            form.error = None;
        }
        cx.notify();
    }

    pub(super) fn routine_weekday_chosen(&mut self, day: Weekday, cx: &mut Context<Self>) {
        if let Some(form) = self.routine_form.as_mut() {
            form.draft.weekday = day;
        }
        cx.notify();
    }

    fn routine_bot_chosen(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(form) = self.routine_form.as_mut() {
            form.draft.bot = form.bots.get(index).copied();
            form.error = None;
        }
        cx.notify();
    }

    fn routine_month_day_chosen(&mut self, index: usize, cx: &mut Context<Self>) {
        let day = u8::try_from(index)
            .ok()
            .and_then(|index| MONTH_DAYS.start().checked_add(index))
            .filter(|day| MONTH_DAYS.contains(day));
        if let (Some(form), Some(day)) = (self.routine_form.as_mut(), day) {
            form.draft.month_day = day;
        }
        cx.notify();
    }

    /// Checks the draft, then saves it: through the service when one is attached, else in the sample store.
    pub(super) fn save_routine_form(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.routine_form.as_mut() else {
            return;
        };
        if form.busy {
            return;
        }
        let (team, routine, draft) = (form.team, form.routine, form.draft.clone());
        if let Err(problem) = draft.check() {
            form.error = Some(problem.message().to_owned());
            cx.notify();
            return;
        }
        let Some(bot) = draft.bot else {
            return;
        };
        match self.service.clone() {
            Some(service) => self.send_routine_save(service, team, routine, draft, bot, cx),
            None => {
                let answer = match routine {
                    None => self.model.teams_mut().add_routine(team, &draft).map(drop),
                    Some(id) => self.model.teams_mut().update_routine(team, id, &draft),
                };
                let error = answer.err().map(|error| error.message().to_owned());
                self.routine_saved(team, error, cx);
            }
        }
    }

    fn send_routine_save(
        &mut self,
        service: ServiceHandle,
        team: TeamId,
        routine: Option<RoutineId>,
        draft: RoutineDraft,
        bot: BotId,
        cx: &mut Context<Self>,
    ) {
        let (Some(team_key), Some(bot_key)) = (self.team_key(team, cx), self.bot_key(bot, cx))
        else {
            self.routine_saved(team, Some(ROUTINE_GONE.to_owned()), cx);
            return;
        };
        let routine_key = match routine {
            None => None,
            Some(id) => match self.routine_key(id, cx) {
                Some(key) => Some(key),
                None => {
                    self.routine_saved(team, Some(ROUTINE_GONE.to_owned()), cx);
                    return;
                }
            },
        };
        if let Some(form) = self.routine_form.as_mut() {
            form.busy = true;
        }
        cx.spawn(async move |this, cx| {
            let answer = match routine_key {
                Some(key) => service
                    .update_routine(draft.update_params(team_key, key, bot_key))
                    .await
                    .map(drop),
                None => {
                    let zone = TimeZone::system().iana_name().map(str::to_owned);
                    service
                        .add_routine(draft.add_params(team_key, bot_key, zone))
                        .await
                        .map(drop)
                }
            };
            this.update(cx, |shell, cx| {
                let error = answer.err().map(|refusal| refusal.message());
                shell.routine_saved(team, error, cx);
            })
            .ok();
        })
        .detach();
    }

    /// Closes the form on a saved routine and shows the Routines page; otherwise the form says why not.
    fn routine_saved(&mut self, team: TeamId, error: Option<String>, cx: &mut Context<Self>) {
        match error {
            None => {
                self.routine_form = None;
                self.model.open_team_page(TeamPage::Routines(team));
            }
            Some(message) => {
                if let Some(form) = self.routine_form.as_mut() {
                    form.busy = false;
                    form.error = Some(message);
                }
            }
        }
        cx.notify();
    }
}
