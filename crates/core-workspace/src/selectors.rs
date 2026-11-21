//! Selectors: a project or workspace is named by id, then exact name, then path (ADR 0008).

use std::path::Path;

use massorca_core_store::{ProjectRow, WorkspaceRow};
use massorca_protocol::codes;

use crate::error::DomainError;
use crate::paths;
use crate::workspaces::Workspaces;

impl Workspaces {
    /// Projects are unique by id, name and path, so each step matches at most one row.
    pub(crate) fn resolve_project(&self, selector: &str) -> Result<ProjectRow, DomainError> {
        refuse_relative_path(selector)?;
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

    /// Names are unique only within a project, so a bare name can match one workspace in each
    /// of several projects; that is `selector.ambiguous`, and the id or path still selects one.
    pub(crate) fn resolve_workspace(&self, selector: &str) -> Result<WorkspaceRow, DomainError> {
        refuse_relative_path(selector)?;
        if let Some(row) = self.store.get_workspace(selector)? {
            return Ok(row);
        }
        let mut named: Vec<WorkspaceRow> = self
            .store
            .list_workspaces(None)?
            .into_iter()
            .filter(|row| row.name == selector)
            .collect();
        if named.len() > 1 {
            return Err(DomainError::new(
                codes::SELECTOR_AMBIGUOUS,
                format!(
                    "workspace name {selector:?} is used in {} projects; select it by id or path",
                    named.len()
                ),
            ));
        }
        if let Some(row) = named.pop() {
            return Ok(row);
        }
        self.store
            .find_workspace_by_path_key(&paths::path_key(Path::new(selector)))?
            .ok_or_else(|| {
                DomainError::new(
                    codes::WORKSPACE_NOT_FOUND,
                    format!("no workspace named {selector:?}"),
                )
            })
    }
}

/// A selector that is a path must be absolute. A relative one would resolve against the
/// service's own directory, which the caller does not control, so it is refused with a reason.
fn refuse_relative_path(selector: &str) -> Result<(), DomainError> {
    let path_like = selector.contains(['/', '\\']) || selector.starts_with('.');
    if path_like && !Path::new(selector).is_absolute() {
        return Err(DomainError::new(
            codes::REQUEST_INVALID_PARAMS,
            format!(
                "selector {selector:?} is a relative path; give an absolute path, an id or a name"
            ),
        ));
    }
    Ok(())
}
