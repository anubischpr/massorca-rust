//! The Calendar page's model (decision 0124): the week it shows, its runs by local day, and the query it sends. The
//! runs come from the service's shared schedule; the page keeps only what it shows.

mod state;
mod week;

pub use state::{CalendarLoad, CalendarMode, CalendarState};
pub use week::{day_heading, range_title, today, week_days};
