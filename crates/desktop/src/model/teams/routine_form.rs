//! The routine form's draft (screens 4.7): what the New and Edit routine page holds, checked before the service
//! sees it, and the schedule it reads as in words. No GPUI types here.

use jiff::civil::Date;
use massorca_protocol::methods::teams::{
    RoutineAddParams, RoutineCadence, RoutineUpdateParams, RoutineWeekday,
};

use super::types::{BotId, Cadence, Routine, RoutineId, Weekday};

/// The longest name a routine takes, in characters (screens 4.7).
pub const ROUTINE_NAME_LIMIT: usize = 64;
/// The longest instruction a routine takes, in characters (screens 4.7).
pub const ROUTINE_PROMPT_LIMIT: usize = 2_000;
/// Every weekday, Monday first, as the form's day chips show them.
pub const WEEKDAYS: [Weekday; 7] = [
    Weekday::Monday,
    Weekday::Tuesday,
    Weekday::Wednesday,
    Weekday::Thursday,
    Weekday::Friday,
    Weekday::Saturday,
    Weekday::Sunday,
];
/// The days a monthly routine can run on; every month has them all.
pub const MONTH_DAYS: std::ops::RangeInclusive<u8> = 1..=28;

/// How often a routine runs, as the form's segments choose it. The service has no weekday cadence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScheduleKind {
    Once,
    Daily,
    Weekly,
    Monthly,
}

impl ScheduleKind {
    /// Every kind, in the order the form's segments show them.
    pub const ALL: [ScheduleKind; 4] = [
        ScheduleKind::Once,
        ScheduleKind::Daily,
        ScheduleKind::Weekly,
        ScheduleKind::Monthly,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ScheduleKind::Once => "Once",
            ScheduleKind::Daily => "Daily",
            ScheduleKind::Weekly => "Weekly",
            ScheduleKind::Monthly => "Monthly",
        }
    }
}

/// The form field a problem belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutineField {
    Name,
    Prompt,
    Bot,
    Time,
    RunDate,
}

/// Why the form cannot save yet: the first field the service would refuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutineProblem {
    NameRequired,
    NameTooLong,
    PromptRequired,
    PromptTooLong,
    BotRequired,
    TimeInvalid,
    DateInvalid,
}

impl RoutineProblem {
    pub fn field(self) -> RoutineField {
        match self {
            RoutineProblem::NameRequired | RoutineProblem::NameTooLong => RoutineField::Name,
            RoutineProblem::PromptRequired | RoutineProblem::PromptTooLong => RoutineField::Prompt,
            RoutineProblem::BotRequired => RoutineField::Bot,
            RoutineProblem::TimeInvalid => RoutineField::Time,
            RoutineProblem::DateInvalid => RoutineField::RunDate,
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            RoutineProblem::NameRequired => "Give the routine a name.",
            RoutineProblem::NameTooLong => "The name takes at most 64 characters.",
            RoutineProblem::PromptRequired => "Say what the bot should do.",
            RoutineProblem::PromptTooLong => "The instruction takes at most 2,000 characters.",
            RoutineProblem::BotRequired => "Choose the bot that runs the routine.",
            RoutineProblem::TimeInvalid => "Write the time as 24-hour HH:MM, such as 09:00.",
            RoutineProblem::DateInvalid => "Write the date as YYYY-MM-DD, such as 2026-12-01.",
        }
    }
}

/// What the form's fields hold, as the user typed or chose them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutineDraft {
    pub name: String,
    pub prompt: String,
    pub bot: Option<BotId>,
    pub kind: ScheduleKind,
    pub weekday: Weekday,
    pub month_day: u8,
    /// The date of a once routine, as typed: `YYYY-MM-DD`.
    pub run_date: String,
    /// The time, as typed: 24-hour `HH:MM`.
    pub time: String,
}

impl RoutineDraft {
    /// The draft a New routine opens with: daily at 09:00, for the team's first bot when it has one.
    pub fn new_routine(first_bot: Option<BotId>) -> Self {
        Self {
            name: String::new(),
            prompt: String::new(),
            bot: first_bot,
            kind: ScheduleKind::Daily,
            weekday: Weekday::Monday,
            month_day: 1,
            run_date: String::new(),
            time: "09:00".to_owned(),
        }
    }

    /// The draft an Edit routine opens with: what the routine holds now. A weekday routine opens as daily.
    pub fn of(routine: &Routine) -> Self {
        let (kind, weekday, month_day) = match routine.cadence {
            Cadence::Daily | Cadence::Weekdays => (ScheduleKind::Daily, Weekday::Monday, 1),
            Cadence::Weekly(day) => (ScheduleKind::Weekly, day, 1),
            Cadence::Monthly(day) => (ScheduleKind::Monthly, Weekday::Monday, day),
            Cadence::Once => (ScheduleKind::Once, Weekday::Monday, 1),
        };
        Self {
            name: routine.name.clone(),
            prompt: routine.instruction.clone(),
            bot: routine.bot,
            kind,
            weekday,
            month_day,
            run_date: routine.run_date.clone().unwrap_or_default(),
            time: routine.time.clone(),
        }
    }

    /// The first thing the fields hold that the service would refuse, if any.
    pub fn check(&self) -> Result<(), RoutineProblem> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(RoutineProblem::NameRequired);
        }
        if name.chars().count() > ROUTINE_NAME_LIMIT {
            return Err(RoutineProblem::NameTooLong);
        }
        let prompt = self.prompt.trim();
        if prompt.is_empty() {
            return Err(RoutineProblem::PromptRequired);
        }
        if prompt.chars().count() > ROUTINE_PROMPT_LIMIT {
            return Err(RoutineProblem::PromptTooLong);
        }
        if self.bot.is_none() {
            return Err(RoutineProblem::BotRequired);
        }
        if parse_time(&self.time).is_none() {
            return Err(RoutineProblem::TimeInvalid);
        }
        if self.kind == ScheduleKind::Once && parse_date(&self.run_date).is_none() {
            return Err(RoutineProblem::DateInvalid);
        }
        Ok(())
    }

    /// The cadence the draft names.
    pub fn cadence(&self) -> Cadence {
        match self.kind {
            ScheduleKind::Once => Cadence::Once,
            ScheduleKind::Daily => Cadence::Daily,
            ScheduleKind::Weekly => Cadence::Weekly(self.weekday),
            ScheduleKind::Monthly => Cadence::Monthly(self.month_day),
        }
    }

    /// The preview line under the schedule: "Runs every Monday at 09:00 on VPS".
    pub fn preview(&self, host: &str) -> String {
        let words = schedule_words(self.cadence(), &self.wire_time(), self.run_date_kept());
        format!("Runs {} on {host}", lower_first(&words))
    }

    /// Applies the draft to a routine: its name, instruction, bot, schedule and time. Pause and runs stay.
    pub fn apply(&self, routine: &mut Routine) {
        routine.name = self.name.trim().to_owned();
        routine.instruction = self.prompt.trim().to_owned();
        routine.bot = self.bot;
        routine.cadence = self.cadence();
        routine.time = self.wire_time();
        routine.run_date = self.run_date_kept();
    }

    /// A routine of `host` made from the draft: enabled, with no run yet. The sample store makes it without a service.
    pub fn new_routine_on(&self, id: RoutineId, host: &str) -> Routine {
        let mut routine = Routine {
            id,
            name: String::new(),
            instruction: String::new(),
            cadence: Cadence::Daily,
            time: String::new(),
            bot: None,
            host: host.to_owned(),
            enabled: true,
            last_run: None,
            run_date: None,
            next_run_at_ms: None,
        };
        self.apply(&mut routine);
        routine
    }

    /// The `team.routine_add` params: the service ids of the team and bot, and the creator's zone when known.
    pub fn add_params(&self, team: String, bot: String, zone: Option<String>) -> RoutineAddParams {
        RoutineAddParams {
            team,
            bot,
            name: self.name.trim().to_owned(),
            prompt: self.prompt.trim().to_owned(),
            cadence: self.wire_cadence(),
            time: self.wire_time(),
            weekday: self.wire_weekday(),
            month_day: self.wire_month_day(),
            run_date: self.run_date_kept(),
            zone,
        }
    }

    /// The `team.routine_update` params: every field the form holds. The zone stays as it is.
    pub fn update_params(&self, team: String, routine: String, bot: String) -> RoutineUpdateParams {
        RoutineUpdateParams {
            team,
            routine,
            bot: Some(bot),
            name: Some(self.name.trim().to_owned()),
            prompt: Some(self.prompt.trim().to_owned()),
            cadence: Some(self.wire_cadence()),
            time: Some(self.wire_time()),
            weekday: self.wire_weekday(),
            month_day: self.wire_month_day(),
            run_date: self.run_date_kept(),
            zone: None,
        }
    }

    fn wire_cadence(&self) -> RoutineCadence {
        match self.kind {
            ScheduleKind::Once => RoutineCadence::Once,
            ScheduleKind::Daily => RoutineCadence::Daily,
            ScheduleKind::Weekly => RoutineCadence::Weekly,
            ScheduleKind::Monthly => RoutineCadence::Monthly,
        }
    }

    fn wire_weekday(&self) -> Option<RoutineWeekday> {
        (self.kind == ScheduleKind::Weekly).then(|| wire_weekday(self.weekday))
    }

    fn wire_month_day(&self) -> Option<u8> {
        (self.kind == ScheduleKind::Monthly).then_some(self.month_day)
    }

    fn run_date_kept(&self) -> Option<String> {
        (self.kind == ScheduleKind::Once).then(|| self.run_date.trim().to_owned())
    }

    /// The time as `HH:MM`, padded; a time that does not parse is sent as typed and refused by the service.
    fn wire_time(&self) -> String {
        match parse_time(&self.time) {
            Some((hour, minute)) => format!("{hour:02}:{minute:02}"),
            None => self.time.trim().to_owned(),
        }
    }
}

/// The hour and minute a 24-hour `H:MM` or `HH:MM` text names.
pub fn parse_time(text: &str) -> Option<(u8, u8)> {
    let (hour, minute) = text.trim().split_once(':')?;
    let shaped = (1..=2).contains(&hour.len())
        && minute.len() == 2
        && hour.bytes().all(|byte| byte.is_ascii_digit())
        && minute.bytes().all(|byte| byte.is_ascii_digit());
    if !shaped {
        return None;
    }
    let (hour, minute): (u8, u8) = (hour.parse().ok()?, minute.parse().ok()?);
    (hour < 24 && minute < 60).then_some((hour, minute))
}

/// The calendar date a `YYYY-MM-DD` text names, if it is one.
pub fn parse_date(text: &str) -> Option<Date> {
    let text = text.trim();
    if text.len() != 10 {
        return None;
    }
    text.parse::<Date>().ok()
}

/// The schedule in words, with the time and, for a once routine, its date: "Every Monday at 09:00".
pub fn schedule_words(cadence: Cadence, time: &str, run_date: Option<String>) -> String {
    match (cadence, run_date) {
        (Cadence::Once, Some(date)) => format!("Once on {date} at {time}"),
        _ => cadence.words(time),
    }
}

/// The service's weekday for a desktop weekday.
pub fn wire_weekday(day: Weekday) -> RoutineWeekday {
    match day {
        Weekday::Monday => RoutineWeekday::Monday,
        Weekday::Tuesday => RoutineWeekday::Tuesday,
        Weekday::Wednesday => RoutineWeekday::Wednesday,
        Weekday::Thursday => RoutineWeekday::Thursday,
        Weekday::Friday => RoutineWeekday::Friday,
        Weekday::Saturday => RoutineWeekday::Saturday,
        Weekday::Sunday => RoutineWeekday::Sunday,
    }
}

/// The text with its first letter lowercase, for use mid-sentence.
fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> RoutineDraft {
        RoutineDraft {
            name: "Morning digest".into(),
            prompt: "Summarize the board".into(),
            bot: Some(BotId(2)),
            ..RoutineDraft::new_routine(None)
        }
    }

    fn routine(cadence: Cadence) -> Routine {
        Routine {
            cadence,
            ..draft().new_routine_on(RoutineId(7), "VPS")
        }
    }

    #[test]
    fn a_new_routine_starts_daily_at_nine_for_the_first_bot() {
        let fresh = RoutineDraft::new_routine(Some(BotId(4)));
        assert_eq!(fresh.kind, ScheduleKind::Daily);
        assert_eq!(fresh.time, "09:00");
        assert_eq!(fresh.bot, Some(BotId(4)));
        assert_eq!(RoutineDraft::new_routine(None).bot, None);
    }

    #[test]
    fn check_names_the_first_field_the_service_would_refuse() {
        let ok = draft();
        assert_eq!(ok.check(), Ok(()));
        let blank = RoutineDraft {
            name: "  ".into(),
            ..draft()
        };
        assert_eq!(blank.check(), Err(RoutineProblem::NameRequired));
        let long = RoutineDraft {
            name: "n".repeat(ROUTINE_NAME_LIMIT + 1),
            ..draft()
        };
        assert_eq!(long.check(), Err(RoutineProblem::NameTooLong));
        let silent = RoutineDraft {
            prompt: String::new(),
            ..draft()
        };
        assert_eq!(silent.check(), Err(RoutineProblem::PromptRequired));
        let no_bot = RoutineDraft {
            bot: None,
            ..draft()
        };
        assert_eq!(no_bot.check(), Err(RoutineProblem::BotRequired));
        let late = RoutineDraft {
            time: "25:00".into(),
            ..draft()
        };
        assert_eq!(late.check(), Err(RoutineProblem::TimeInvalid));
        let undated = RoutineDraft {
            kind: ScheduleKind::Once,
            run_date: "2026-13-01".into(),
            ..draft()
        };
        assert_eq!(undated.check(), Err(RoutineProblem::DateInvalid));
        assert_eq!(RoutineProblem::DateInvalid.field(), RoutineField::RunDate);
    }

    #[test]
    fn the_time_reads_as_twenty_four_hours_and_is_sent_padded() {
        assert_eq!(parse_time("9:05"), Some((9, 5)));
        assert_eq!(parse_time("23:59"), Some((23, 59)));
        assert_eq!(parse_time("24:00"), None);
        assert_eq!(parse_time("9:5"), None);
        assert_eq!(parse_time("+9:05"), None);
        let early = RoutineDraft {
            time: "9:05".into(),
            ..draft()
        };
        assert_eq!(early.add_params("t".into(), "b".into(), None).time, "09:05");
    }

    #[test]
    fn the_preview_says_when_the_routine_runs_on_its_host() {
        let weekly = RoutineDraft {
            kind: ScheduleKind::Weekly,
            weekday: Weekday::Monday,
            ..draft()
        };
        assert_eq!(weekly.preview("VPS"), "Runs every Monday at 09:00 on VPS");
        let monthly = RoutineDraft {
            kind: ScheduleKind::Monthly,
            month_day: 15,
            ..draft()
        };
        assert_eq!(
            monthly.preview("VPS"),
            "Runs on day 15 of every month at 09:00 on VPS"
        );
        let once = RoutineDraft {
            kind: ScheduleKind::Once,
            run_date: "2026-12-01".into(),
            ..draft()
        };
        assert_eq!(
            once.preview("VPS"),
            "Runs once on 2026-12-01 at 09:00 on VPS"
        );
        assert_eq!(draft().preview("VPS"), "Runs every day at 09:00 on VPS");
    }

    #[test]
    fn add_params_send_only_the_fields_the_cadence_uses() {
        let weekly = RoutineDraft {
            kind: ScheduleKind::Weekly,
            weekday: Weekday::Friday,
            month_day: 9,
            run_date: "2026-12-01".into(),
            ..draft()
        };
        let params = weekly.add_params("t1".into(), "b1".into(), Some("Europe/Madrid".into()));
        assert_eq!(params.cadence, RoutineCadence::Weekly);
        assert_eq!(params.weekday, Some(RoutineWeekday::Friday));
        assert_eq!((params.month_day, params.run_date.as_deref()), (None, None));
        assert_eq!(params.zone.as_deref(), Some("Europe/Madrid"));
        let once = RoutineDraft {
            kind: ScheduleKind::Once,
            run_date: " 2026-12-01 ".into(),
            ..draft()
        };
        let params = once.add_params("t1".into(), "b1".into(), None);
        assert_eq!(params.weekday, None);
        assert_eq!(params.run_date.as_deref(), Some("2026-12-01"));
    }

    #[test]
    fn update_params_carry_the_form_and_leave_the_zone_alone() {
        let monthly = RoutineDraft {
            kind: ScheduleKind::Monthly,
            month_day: 3,
            ..draft()
        };
        let params = monthly.update_params("t1".into(), "r1".into(), "b2".into());
        assert_eq!(params.bot.as_deref(), Some("b2"));
        assert_eq!(params.cadence, Some(RoutineCadence::Monthly));
        assert_eq!(params.month_day, Some(3));
        assert_eq!(params.weekday, None);
        assert_eq!(params.zone, None);
    }

    #[test]
    fn an_edit_opens_with_the_routine_and_applying_keeps_its_pause_and_runs() {
        let mut existing = routine(Cadence::Weekly(Weekday::Thursday));
        existing.enabled = false;
        existing.last_run = Some(super::super::types::RunOutcome::Done);
        existing.next_run_at_ms = Some(42);
        let opened = RoutineDraft::of(&existing);
        assert_eq!(opened.kind, ScheduleKind::Weekly);
        assert_eq!(opened.weekday, Weekday::Thursday);
        assert_eq!(opened.cadence(), existing.cadence);
        let renamed = RoutineDraft {
            name: "Evening digest".into(),
            ..opened
        };
        renamed.apply(&mut existing);
        assert_eq!(existing.name, "Evening digest");
        assert!(!existing.enabled);
        assert_eq!(
            existing.last_run,
            Some(super::super::types::RunOutcome::Done)
        );
        assert_eq!(existing.next_run_at_ms, Some(42));
    }

    #[test]
    fn a_once_routine_keeps_its_date_and_its_words_name_it() {
        let once = RoutineDraft {
            kind: ScheduleKind::Once,
            run_date: "2026-12-01".into(),
            ..draft()
        };
        let made = once.new_routine_on(RoutineId(9), "VPS");
        assert_eq!(made.run_date.as_deref(), Some("2026-12-01"));
        assert_eq!(
            schedule_words(made.cadence, &made.time, made.run_date.clone()),
            "Once on 2026-12-01 at 09:00"
        );
        assert_eq!(
            schedule_words(Cadence::Once, "09:00", None),
            "Once at 09:00"
        );
    }
}
