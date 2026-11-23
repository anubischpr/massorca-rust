//! One schedule engine for routines and automations (decision 0082).
//!
//! A [`Schedule`] is a cadence (daily, weekly on a weekday, monthly on day 1 to 28, or once on a date), a wall-clock
//! `HH:MM` and an optional IANA zone; no zone means the host's zone. [`Schedule::compile`] validates it into a
//! [`CompiledSchedule`], which answers occurrence questions. Instants are UTC epoch milliseconds (`i64`) so any
//! store can keep them. The crate knows nothing about teams, bots, tasks or automations.
//!
//! # Daylight-saving rules
//!
//! - A wall time that does not exist (the clock jumps forward over it) fires at the end of the gap: 02:30 on a night
//!   that jumps from 02:00 to 03:00 fires at 03:00 local time.
//! - A wall time that happens twice (the clock falls back over it) fires once, at the earlier instant. The repeat is
//!   not an occurrence.
//!
//! # Bounded searches
//!
//! Searches walk at most [`MAX_SEARCH_DAYS`] local days and stop at the end of representable time, so no question
//! loops forever; instants outside that range have no occurrence.


mod occurrence;
mod schedule;
mod zone;

pub use occurrence::{CompiledSchedule, MAX_SEARCH_DAYS};
pub use schedule::{Cadence, MAX_ZONE_NAME_LEN, Schedule, ScheduleError, Weekday};
pub use zone::ZoneSource;
