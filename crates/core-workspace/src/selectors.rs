//! Selectors: a project or workspace is named by id, then exact name, then path (ADR 0008).

use std::path::Path;

use massorca_core_store::ProjectRow;
use massorca_protocol::codes;

use crate::error::DomainError;
use crate::paths;
use crate::workspaces::Workspaces;

impl Workspaces {
    /// Projects are unique by id, name and path, so each step matches at most one row.
    pub(crate) fn resolve_project(&self, selector: &str) -> Result<ProjectRow, DomainError> {
        if let Some(row) = self.store.get_project(selector)? {
            return Ok(row);
        }
        if let Some(row) = self.store.find_project_by_name(selector)? {
            return Ok(row);
        }
        self.store
            .find_project_by_path_key(&paths::path_key(Path::new(selector)))?
            .ok_or_else(|| {
                DomainError::new(
                    codes::PROJECT_NOT_FOUND,
                    format!("no project named {selector:?}"),
                )
            })
    }

}
