//! Selects one automation by id, then by exact name, as the protocol describes (decision 0123).

use massorca_core_store::{AutomationRow, Store};
use massorca_protocol::{ErrorBody, codes};

use crate::features::teams::{invalid, store_error};

pub(super) fn automation(store: &Store, selector: &str) -> Result<AutomationRow, ErrorBody> {
    if let Some(row) = store.get_automation(selector).map_err(store_error)? {
        return Ok(row);
    }
    let mut named = store
        .list_automations()
        .map_err(store_error)?
        .into_iter()
        .filter(|row| row.name == selector);
    match (named.next(), named.next()) {
        (Some(row), None) => Ok(row),
        (Some(_), Some(_)) => Err(invalid(format!(
            "more than one automation is called {selector:?}; name it by its id"
        ))),
        (None, _) => Err(ErrorBody::new(
            codes::AUTOMATION_NOT_FOUND,
            format!("no automation {selector:?}"),
        )),
    }
}
