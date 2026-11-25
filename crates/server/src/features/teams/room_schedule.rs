//! The day and time a room line asks for (decision 0121): a deterministic reading of explicit phrases, in English and
//! Spanish. A line is a request to schedule only when it names exactly one day and exactly one time, and the time is
//! asked for ("at 9pm", "a las 17:00", or a line that says schedule or remind); anything else stays a task.

use jiff::ToSpan;
use jiff::civil::{Date, Time, Weekday};

/// When a room line asks its task to run, on the host's clock (the zone a routine without one runs on).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Moment {
    pub date: Date,
    pub time: Time,
}

impl Moment {
    /// `YYYY-MM-DD`, as `team.routine_add` takes it.
    pub fn run_date(&self) -> String {
        self.date.strftime("%Y-%m-%d").to_string()
    }

    /// `HH:MM`, as `team.routine_add` takes it.
    pub fn clock(&self) -> String {
        self.time.strftime("%H:%M").to_string()
    }
}

const WEEKDAYS: [(&[&str], Weekday); 7] = [
    (&["monday", "lunes"], Weekday::Monday),
    (&["tuesday", "martes"], Weekday::Tuesday),
    (&["wednesday", "miércoles", "miercoles"], Weekday::Wednesday),
    (&["thursday", "jueves"], Weekday::Thursday),
    (&["friday", "viernes"], Weekday::Friday),
    (&["saturday", "sábado", "sabado"], Weekday::Saturday),
    (&["sunday", "domingo"], Weekday::Sunday),
];

/// Words that ask for a schedule anywhere in a line, so its time needs no "at".
const SCHEDULE_VERBS: [&str; 10] = [
    "schedule",
    "remind",
    "programa",
    "programar",
    "prográmalo",
    "programalo",
    "recuerda",
    "recuérdame",
    "recuerdame",
    "recordar",
];

/// Words before "mañana" that make it "morning" ("esta mañana") or not tomorrow ("pasado mañana").
const NOT_TOMORROW: [&str; 6] = ["la", "esta", "de", "en", "por", "pasado"];

/// Marks where a clause ends (`,`, `;`, `.`, `!`, `?`), so "at 15, run" reads 15 as a time.
const BREAK: &str = "|";

/// The moment `text` asks for, with `today` the host's date: one day phrase (`YYYY-MM-DD`, today, tomorrow or a
/// weekday, which means its next one after today) and one asked-for time phrase (`HH:MM` with two hour digits, `9pm`,
/// `9:30 pm`, or a bare hour from 12 to 23 or 0 after "at" or "a las"). A bare hour from 1 to 11, or `H:MM` with one
/// hour digit from 1 to 9, says neither morning nor evening, so it is not read. Two different days or two different times are not a request to schedule.
pub(super) fn scheduled_moment(text: &str, today: Date) -> Option<Moment> {
    let words = words(text);
    let asked = words
        .iter()
        .any(|word| SCHEDULE_VERBS.contains(&word.as_str()));
    let mut dates = Vec::new();
    let mut times = Vec::new();
    for (index, word) in words.iter().enumerate() {
        let previous = index.checked_sub(1).and_then(|at| words.get(at));
        if let Some(date) = day_of(word, previous.map(String::as_str), today) {
            dates.push(date);
        }
        match time_of(&words, index, today, asked) {
            Some(Reading::Clock(time)) => times.push(time),
            // A line that asks for a time it does not make clear is not scheduled at all.
            Some(Reading::Unclear) => return None,
            None => {}
        }
    }
    Some(Moment {
        date: only(dates)?,
        time: only(times)?,
    })
}

/// The single value found, repeated or not; none, or two different ones, is `None`.
fn only<T: PartialEq>(found: Vec<T>) -> Option<T> {
    let mut found = found.into_iter();
    let first = found.next()?;
    found.all(|other| other == first).then_some(first)
}

/// Lowercase words, keeping `:`, `-` and `'` inside them (so "tomorrow's" is not "tomorrow"), splitting a number from a
/// following `am` or `pm`, and a [`BREAK`] after a word that ends a clause.
fn words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    for chunk in text.to_lowercase().split_whitespace() {
        let ends_clause = chunk.ends_with([',', ';', '.', '!', '?'])
            && !chunk.ends_with("a.m.")
            && !chunk.ends_with("p.m.");
        let word = chunk
            .trim_matches(|c: char| !c.is_alphanumeric())
            .replace("a.m", "am")
            .replace("p.m", "pm");
        let split = ["am", "pm"].into_iter().find_map(|suffix| {
            let number = word.strip_suffix(suffix)?;
            (!number.is_empty() && number.chars().all(|c| c.is_ascii_digit() || c == ':'))
                .then(|| (number.to_owned(), suffix))
        });
        match split {
            Some((number, suffix)) => {
                words.push(number);
                words.push(suffix.to_owned());
            }
            None if !word.is_empty() => words.push(word),
            None => {}
        }
        if ends_clause {
            words.push(BREAK.to_owned());
        }
    }
    words
}

fn day_of(word: &str, previous: Option<&str>, today: Date) -> Option<Date> {
    match word {
        "today" | "hoy" => return Some(today),
        "tomorrow" if previous != Some("after") => return today.checked_add(1.day()).ok(),
        "mañana" if previous.is_none_or(|before| !NOT_TOMORROW.contains(&before)) => {
            return today.checked_add(1.day()).ok();
        }
        _ => {}
    }
    if let Some((_, weekday)) = WEEKDAYS.iter().find(|(names, _)| names.contains(&word)) {
        return today.nth_weekday(1, *weekday).ok();
    }
    iso_date(word)
}

fn iso_date(word: &str) -> Option<Date> {
    let mut parts = word.split('-');
    let (year, month, day) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return None;
    }
    Date::new(year.parse().ok()?, month.parse().ok()?, day.parse().ok()?).ok()
}

/// The time the word at `index` asks for, read with its neighbours. It counts after "at" or "a las", or anywhere in a
/// line that says schedule or remind (`asked`).
/// An asked-for time: a clock time, or one that could be morning or evening.
enum Reading {
    Clock(Time),
    Unclear,
}

fn time_of(words: &[String], index: usize, today: Date, asked: bool) -> Option<Reading> {
    let word = words[index].as_str();
    let next = words.get(index + 1).map(String::as_str);
    let previous = |back: usize| {
        index
            .checked_sub(back)
            .and_then(|at| words.get(at))
            .map(String::as_str)
    };
    let cued = previous(1) == Some("at")
        || (matches!(previous(1), Some("las" | "la")) && previous(2) == Some("a"));
    let meridiem = next.filter(|next| *next == "am" || *next == "pm");
    if let Some((hour, minute)) = word.split_once(':') {
        if !(cued || asked) || hour.is_empty() || hour.len() > 2 || minute.len() != 2 {
            return None;
        }
        // "3:30" could be morning or evening; "03:30" and "3:30 pm" say which.
        let time = clock(hour.parse().ok()?, minute.parse().ok()?, meridiem)?;
        if hour.len() == 1 && hour != "0" && meridiem.is_none() {
            return Some(Reading::Unclear);
        }
        return Some(Reading::Clock(time));
    }
    if word.is_empty() || word.len() > 2 || !word.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let hour: i8 = word.parse().ok()?;
    if meridiem.is_some() {
        return (cued || asked)
            .then(|| clock(hour, 0, meridiem))
            .flatten()
            .map(Reading::Clock);
    }
    // A bare hour needs "at" or "a las", the end of a clause or a day after it ("at 15 places" is not a time), and an
    // hour that cannot be morning or evening.
    let ends = next.is_none_or(|next| {
        matches!(next, BREAK | "on" | "el" | "o'clock" | "h")
            || day_of(next, Some(word), today).is_some()
    });
    if !(cued && ends) {
        return None;
    }
    if (1..=11).contains(&hour) {
        return Some(Reading::Unclear);
    }
    clock(hour, 0, None).map(Reading::Clock)
}

fn clock(hour: i8, minute: i8, meridiem: Option<&str>) -> Option<Time> {
    let hour = match meridiem {
        Some(_) if !(1..=12).contains(&hour) => return None,
        Some("am") => hour % 12,
        Some(_) => hour % 12 + 12,
        None => hour,
    };
    Time::new(hour, minute, 0, 0).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Wednesday.
    const TODAY: Date = Date::constant(2030, 1, 2);

    fn at(text: &str) -> Option<(String, String)> {
        scheduled_moment(text, TODAY).map(|moment| (moment.run_date(), moment.clock()))
    }

    fn moment(date: &str, time: &str) -> Option<(String, String)> {
        Some((date.to_owned(), time.to_owned()))
    }

    #[test]
    fn a_day_and_an_asked_for_time_make_a_moment() {
        assert_eq!(
            at("Back up the docs tomorrow at 09:30"),
            moment("2030-01-03", "09:30")
        );
        assert_eq!(
            at("on 2030-02-14 at 6pm send the card"),
            moment("2030-02-14", "18:00")
        );
        assert_eq!(
            at("today at 12am rotate logs"),
            moment("2030-01-02", "00:00")
        );
        assert_eq!(
            at("Friday at 21, run the release"),
            moment("2030-01-04", "21:00")
        );
        assert_eq!(
            at("run the release at 17 on friday"),
            moment("2030-01-04", "17:00")
        );
        assert_eq!(
            at("revisa el informe mañana a las 20"),
            moment("2030-01-03", "20:00")
        );
        assert_eq!(
            at("el miércoles a las 17:15, despliega"),
            moment("2030-01-09", "17:15")
        );
        assert_eq!(
            at("hoy a las 7:05 p.m. llama"),
            moment("2030-01-02", "19:05")
        );
        assert_eq!(
            at("Schedule a backup tomorrow 9am"),
            moment("2030-01-03", "09:00"),
            "a schedule verb asks for the time"
        );
        assert_eq!(
            at("recuérdame el viernes 10:30 revisar la factura"),
            moment("2030-01-04", "10:30")
        );
    }

    #[test]
    fn a_weekday_is_its_next_one_after_today() {
        assert_eq!(
            at("wednesday at 10:00 standup"),
            moment("2030-01-09", "10:00")
        );
        assert_eq!(
            at("thursday at 10:00 standup"),
            moment("2030-01-03", "10:00")
        );
    }

    #[test]
    fn a_time_that_is_only_mentioned_is_not_scheduled() {
        assert_eq!(
            at("The 5pm deploy today will break login, please fix now"),
            None
        );
        assert_eq!(at("tomorrow's standup moved; the 10:00 report fails"), None);
        assert_eq!(
            at("esta mañana a las 9:00 falló el backup, revísalo"),
            None,
            "this morning, not tomorrow"
        );
        assert_eq!(at("today 12am rotate logs"), None, "no at");
        assert_eq!(at("pasado mañana a las 18:00 despliega"), None);
        assert_eq!(at("the day after tomorrow at 18:00 deploy"), None);
    }

    #[test]
    fn a_bare_hour_that_could_be_morning_or_evening_is_not_read() {
        assert_eq!(at("tomorrow at 3"), None);
        assert_eq!(at("mañana a las 5"), None);
        assert_eq!(at("Friday at 9, run the release"), None);
        assert_eq!(at("tomorrow at 3:30, call the client"), None);
        assert_eq!(at("mañana a las 5:30 llama al cliente"), None);
        assert_eq!(
            at("today at 9:00 or at 10:00"),
            None,
            "an unclear time is not dropped for a clear one"
        );
        assert_eq!(
            at("tomorrow at 03:30, call the client"),
            moment("2030-01-03", "03:30"),
            "two hour digits are a 24-hour clock"
        );
        assert_eq!(at("tomorrow at 3pm"), moment("2030-01-03", "15:00"));
    }

    #[test]
    fn a_line_without_one_day_and_one_time_is_not_scheduled() {
        assert_eq!(at("Add magic link sign-in"), None);
        assert_eq!(at("tomorrow fix the parser"), None, "no time");
        assert_eq!(at("at 10:00 fix the parser"), None, "no day");
        assert_eq!(
            at("tomorrow add 3 tests at 15 places"),
            None,
            "a count, not a time"
        );
        assert_eq!(at("tomorrow or friday at 18:00"), None, "two days");
        assert_eq!(at("today at 09:00 or at 10:00"), None, "two times");
        assert_eq!(at("today at 25:00"), None, "not a clock time");
        assert_eq!(at("2030-02-30 at 9am"), None, "not a date");
        assert_eq!(at("ratio 16:9 tomorrow"), None);
    }
}
