//! An automation as the protocol shows it: its project's name while the project exists, and the zone it runs in.

use massorca_core_store::{AutomationRow, Store};
use massorca_protocol::ErrorBody;
use massorca_protocol::methods::automations::{self, Automation};

use super::definition::{HostZone, cadence_of, weekday_of};
use crate::features::teams::store_error;

pub(super) fn automation_view(
    store: &Store,
    row: &AutomationRow,
    host: &HostZone,
) -> Result<Automation, ErrorBody> {
    let project_name = match row.project_id.as_deref() {
        Some(id) => store
            .get_project(id)
            .map_err(store_error)?
            .map(|project| project.name),
        None => None,
    };
    Ok(Automation {
        id: row.id.clone(),
        project_id: row.project_id.clone(),
        project_name,
        name: row.name.clone(),
        prompt: row.prompt.clone(),
        agent: row.agent_kind.clone(),
        cadence: cadence_of(&row.cadence),
        time: row.time.clone(),
        weekday: row.weekday.as_deref().map(weekday_of),
        cron: row.cron.clone(),
        zone: row.zone.clone(),
        zone_used: row.zone.is_none().then(|| host.label()),
        grace_minutes: row
            .grace_minutes
            .unwrap_or(automations::DEFAULT_GRACE_MINUTES),
        enabled: row.enabled,
        next_run_at_ms: row.next_run_at,
        last_run_at_ms: row.last_run_at,
        created_at_ms: row.created_at,
        updated_at_ms: row.updated_at,
    })
}
