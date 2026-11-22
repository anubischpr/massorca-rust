//! Resolves the selectors of Bot Teams calls: an id first, then an exact name within the scope the call names.

use massorca_core_store::{Store, TeamRow};
use massorca_protocol::{ErrorBody, codes};

use super::store_error;

/// The team `selector` names: its id, then its exact name.
pub(crate) fn team(store: &Store, selector: &str) -> Result<TeamRow, ErrorBody> {
    if let Some(row) = store.get_team(selector).map_err(store_error)? {
        return Ok(row);
    }
    store
        .find_team_by_name(selector)
        .map_err(store_error)?
        .ok_or_else(|| ErrorBody::new(codes::TEAM_NOT_FOUND, format!("no team named {selector:?}")))
}
