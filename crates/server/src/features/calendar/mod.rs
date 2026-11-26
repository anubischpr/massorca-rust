//! `calendar.occurrences` (capability `calendar/1`, decision 0124): the runs of routines and automations in one window,
//! and the done routine runs in it. Read-only, as the routine and automation lists are.

mod routine_runs;
mod window;

use massorca_core_store::now_ms;
use massorca_protocol::methods::calendar::{
    self, CalendarOccurrenceList, CalendarOccurrencesParams,
};
use massorca_protocol::{ErrorBody, capabilities};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::teams::blocking;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let methods = &registries.methods;
    let cap = capabilities::CALENDAR;
    Ok(vec![
        registries.capabilities.register(cap, Owner::Core)?,
        methods.register(
            Owner::Core,
            calendar::OCCURRENCES,
            cap,
            Permission::Read,
            occurrences,
        )?,
    ])
}

/// Reads one window on the blocking pool. The clock is read when the call arrives, so upcoming runs start from then.
async fn occurrences(
    ctx: RequestCtx,
    params: CalendarOccurrencesParams,
) -> Result<CalendarOccurrenceList, ErrorBody> {
    let now = now_ms();
    blocking(&ctx, move |store, _agents| {
        window::read(store, now, &params)
    })
    .await
}
