//! The day and time a room line asks for (decision 0121): a deterministic reading of explicit phrases, in English and
//! Spanish. A line with exactly one day and exactly one time is a request to schedule; anything else is not.

use jiff::ToSpan;
use jiff::civil::{Date, Time, Weekday};

/// When a room line asks its task to run, on the creator's clock.
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

/// Marks where a clause ends (`,`, `;`, `.`, `!`, `?`), so "at 9, run" reads 9 as a time.
const BREAK: &str = "|";

/// The moment `text` asks for, with `today` the creator's date: one day phrase (`YYYY-MM-DD`, today, tomorrow or a
/// weekday, which means its next one after today) and one time phrase (`HH:MM`, `9am`, `9:30 pm`, `at 9`, `a las 9`).
/// Two different days or two different times are not a request to schedule.
pub(super) fn scheduled_moment(text: &str, today: Date) -> Option<Moment> {
    let words = words(text);
    let mut dates = Vec::new();
    let mut times = Vec::new();
    for (index, word) in words.iter().enumerate() {
        let previous = index.checked_sub(1).and_then(|at| words.get(at));
        if let Some(date) = day_of(word, previous.map(String::as_str), today) {
            dates.push(date);
        }
        if let Some(time) = time_of(&words, index, today) {
            times.push(time);
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

/// Lowercase words, keeping `:` and `-` inside them, splitting a number from a following `am` or `pm`, and a [`BREAK`]
/// after a word that ends a clause.
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
        "tomorrow" => return today.checked_add(1.day()).ok(),
        // "por la mañana" is "in the morning", not tomorrow.
        "mañana" if previous != Some("la") => return today.checked_add(1.day()).ok(),
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

/// The time the word at `index` starts, read with its neighbours.
fn time_of(words: &[String], index: usize, today: Date) -> Option<Time> {
    let word = words[index].as_str();
    let next = words.get(index + 1).map(String::as_str);
    let meridiem = next.filter(|next| *next == "am" || *next == "pm");
    if let Some((hour, minute)) = word.split_once(':') {
        if hour.is_empty() || hour.len() > 2 || minute.len() != 2 {
            return None;
        }
        return clock(hour.parse().ok()?, minute.parse().ok()?, meridiem);
    }
    if word.is_empty() || word.len() > 2 || !word.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let hour = word.parse().ok()?;
    if meridiem.is_some() {
        return clock(hour, 0, meridiem);
    }
    // A bare hour counts only after "at" or "a las", and at the end of a clause or before a day, so "at 3 places" is not.
    let previous = |back: usize| {
        index
            .checked_sub(back)
            .and_then(|at| words.get(at))
            .map(String::as_str)
    };
    let introduced =
        previous(1) == Some("at") || (previous(1) == Some("las") && previous(2) == Some("a"));
    let ends = next.is_none_or(|next| {
        matches!(next, BREAK | "on" | "el" | "o'clock" | "h")
            || day_of(next, Some(word), today).is_some()
    });
    (introduced && ends).then(|| clock(hour, 0, None)).flatten()
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
    fn a_day_and_a_time_make_a_moment() {
        assert_eq!(
            at("Back up the docs tomorrow at 09:30"),
            moment("2030-01-03", "09:30")
        );
        assert_eq!(
            at("on 2030-02-14 at 6pm send the card"),
            moment("2030-02-14", "18:00")
        );
        assert_eq!(at("today 12am rotate logs"), moment("2030-01-02", "00:00"));
        assert_eq!(
            at("Friday at 9, run the release"),
            moment("2030-01-04", "09:00")
        );
        assert_eq!(
            at("run the release at 9 on friday"),
            moment("2030-01-04", "09:00")
        );
        assert_eq!(
            at("revisa el informe mañana a las 8"),
            moment("2030-01-03", "08:00")
        );
        assert_eq!(
            at("el miércoles a las 17:15, despliega"),
            moment("2030-01-09", "17:15")
        );
        assert_eq!(at("hoy 7:05 p.m. llama"), moment("2030-01-02", "19:05"));
    }

    #[test]
    fn a_weekday_is_its_next_one_after_today() {
        assert_eq!(at("wednesday 10:00 standup"), moment("2030-01-09", "10:00"));
        assert_eq!(at("thursday 10:00 standup"), moment("2030-01-03", "10:00"));
    }

    #[test]
    fn a_line_without_one_day_and_one_time_is_not_scheduled() {
        assert_eq!(at("Add magic link sign-in"), None);
        assert_eq!(at("tomorrow fix the parser"), None, "no time");
        assert_eq!(at("at 10:00 fix the parser"), None, "no day");
        assert_eq!(
            at("tomorrow add 3 tests at 3 places"),
            None,
            "a count, not a time"
        );
        assert_eq!(
            at("Friday at 9 run the release"),
            None,
            "a bare hour needs a clause end"
        );
        assert_eq!(at("tomorrow or friday at 9"), None, "two days");
        assert_eq!(at("today at 9:00 or 10:00"), None, "two times");
        assert_eq!(
            at("por la mañana a las 9 revisa"),
            None,
            "the morning, not tomorrow"
        );
        assert_eq!(at("today at 25:00"), None, "not a clock time");
        assert_eq!(at("2030-02-30 at 9am"), None, "not a date");
        assert_eq!(at("ratio 16:9 tomorrow"), None);
    }
}
