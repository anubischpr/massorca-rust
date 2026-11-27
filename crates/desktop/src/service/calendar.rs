//! The Calendar's service call (decision 0124): `calendar.occurrences`, answered on its oneshot. A service that predates
//! `calendar/1` is refused with `service.capability_missing`, which the page shows as a note rather than a failure.

use massorca_client::Client;
use massorca_protocol::capabilities;
use massorca_protocol::methods::calendar::{CalendarOccurrenceList, CalendarOccurrencesParams};
use tokio::sync::oneshot;

use super::bridge::{Command, ServiceHandle};
use super::refusal::Refusal;

/// The code a service that does not offer `calendar/1` is refused with.
pub const CAPABILITY_MISSING: &str = "service.capability_missing";

/// One `calendar.*` call, answered on its oneshot with the service's answer or the reason it was refused.
pub(crate) enum CalendarCommand {
    Occurrences {
        params: CalendarOccurrencesParams,
        reply: oneshot::Sender<Result<CalendarOccurrenceList, Refusal>>,
    },
}

/// Runs one calendar call on its own task and answers it.
pub(super) fn spawn_call(client: &Client, command: CalendarCommand) {
    let client = client.clone();
    tokio::spawn(async move {
        match command {
            CalendarCommand::Occurrences { params, reply } => {
                let answer = if client.has_capability(capabilities::CALENDAR) {
                    client
                        .calendar_occurrences(&params)
                        .await
                        .map_err(|error| Refusal::from(&error))
                } else {
                    Err(capability_missing())
                };
                let _ = reply.send(answer);
            }
        }
    });
}

/// Answers a call the service cannot take, with the reason.
pub(super) fn refuse(command: CalendarCommand, refusal: Refusal) {
    match command {
        CalendarCommand::Occurrences { reply, .. } => {
            let _ = reply.send(Err(refusal));
        }
    }
}

fn capability_missing() -> Refusal {
    Refusal::Failed {
        code: CAPABILITY_MISSING.to_owned(),
        message: "This service does not offer the calendar yet.".to_owned(),
    }
}

impl ServiceHandle {
    /// `calendar.occurrences`: the runs of routines and automations in a window of up to 62 days.
    pub async fn calendar_occurrences(
        &self,
        params: CalendarOccurrencesParams,
    ) -> Result<CalendarOccurrenceList, Refusal> {
        self.ask(|reply| Command::Calendar(CalendarCommand::Occurrences { params, reply }))
            .await
    }
}
