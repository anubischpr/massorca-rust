//! The automation editor's data (decision 0123): the fields it holds and the rules a definition must meet before it is
//! sent. Plain data with no GPUI; the editor itself lives in `shell::automations`.

mod form;

pub use form::{AutomationForm, Cadence, Field, FormError, WEEKDAYS, weekday_label};
