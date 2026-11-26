//! Typed helper over the `calendar/1` method: the runs of routines and automations in a window (decision 0124).

use massorca_protocol::methods::calendar::{
    self, CalendarOccurrenceList, CalendarOccurrencesParams,
};

use crate::client::Client;
use crate::error::ClientError;

impl Client {
    /// `calendar.occurrences`: the runs in a window of up to 62 days, earliest first, with done routine runs only when asked.
    pub async fn calendar_occurrences(
        &self,
        params: &CalendarOccurrencesParams,
    ) -> Result<CalendarOccurrenceList, ClientError> {
        self.call(calendar::OCCURRENCES, params).await
    }
}
