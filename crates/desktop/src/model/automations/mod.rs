//! The Automations page's data (decision 0123): the list the service reported, the editor's fields and rules, the
//! schedule text the page shows, and the run table of one automation (decision 0125). Plain data with no GPUI; the page and the editor live in `shell::automations`.

mod form;
mod runs;
mod schedule;
mod state;

pub use form::{AutomationForm, Cadence, Field, FormError, WEEKDAYS, weekday_label};
pub use runs::{RunHistory, run_detail_text, run_duration_text, run_started_text, run_status_text};
pub use schedule::{next_run_text, schedule_summary};
pub use state::{AutomationsState, Editor};
