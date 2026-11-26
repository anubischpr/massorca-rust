//! The Automations page's data (decision 0123): the list the service reported, the editor's fields and rules, and the
//! schedule text the page shows. Plain data with no GPUI; the page and the editor live in `shell::automations`.

mod form;
mod schedule;
mod state;

pub use form::{AutomationForm, Cadence, Field, FormError, WEEKDAYS, weekday_label};
pub use schedule::{next_run_text, schedule_summary};
pub use state::{AutomationsState, Editor};
