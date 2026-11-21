//! Projects: registering a repository or a folder, listing, showing and unregistering (ADR 0008).

use std::path::{Path, PathBuf};

use massorca_core_store::{ProjectKind as StoreKind, ProjectRow, StoreError, new_id, now_ms};
use massorca_protocol::codes;
use massorca_protocol::methods::projects::{Project, ProjectAddParams, ProjectKind};

use crate::error::DomainError;
use crate::names;
use crate::paths;
use crate::views;
use crate::workspaces::Workspaces;

impl Workspaces {
    /// Registers a git repository (at its toplevel) or a folder. Without `kind`, a directory
    /// inside a repository is a git project and any other directory is a folder.
    pub fn add_project(&self, params: ProjectAddParams) -> Result<Project, DomainError> {
        let _guard = self.lock_mutations();
        if params.kind == Some(ProjectKind::Unknown) {
            return Err(DomainError::new(
                codes::REQUEST_INVALID_PARAMS,
                "unknown project kind",
            ));
        }
        let given = paths::existing_directory(Path::new(&params.path))?;
        let (kind, root) = match params.kind {
            Some(ProjectKind::Folder) => (StoreKind::Folder, given),
            Some(ProjectKind::Git) => {
                let top = self
                    .repository_root(&given)?
                    .ok_or_else(|| not_git(&given))?;
                (StoreKind::Git, top)
            }
            _ => match self.repository_root(&given)? {
                Some(top) => (StoreKind::Git, top),
                None => (StoreKind::Folder, given),
            },
        };
        let name = match params.name {
            Some(name) => name,
            None => root
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_owned)
                .ok_or_else(|| {
                    DomainError::new(
                        codes::REQUEST_INVALID_PARAMS,
                        format!("{} has no name; give one", root.display()),
                    )
                })?,
        };
        names::project_name(&name)?;
        let path_key = paths::path_key(&root);
        if self.store.find_project_by_path_key(&path_key)?.is_some() {
            return Err(DomainError::new(
                codes::PROJECT_EXISTS,
                format!("{} is already a project", root.display()),
            ));
        }
        if self.store.find_project_by_name(&name)?.is_some() {
            return Err(DomainError::new(
                codes::PROJECT_EXISTS,
                format!("a project named {name:?} already exists"),
            ));
        }
        let workspaces_root = match params.workspaces_root {
            Some(text) => {
                paths::absolute(&text)?;
                text
            }
            None => paths::path_text(&self.default_root.join(&name))?,
        };
        let row = ProjectRow {
            id: new_id(),
            kind,
            path: paths::path_text(&root)?,
            path_key,
            name,
            workspaces_root,
            added_at: now_ms(),
        };
        self.store
            .insert_project(&row)
            .map_err(|error| match error {
                StoreError::Conflict { .. } => DomainError::new(
                    codes::PROJECT_EXISTS,
                    "a project with this path or name was added concurrently",
                ),
                other => other.into(),
            })?;
        Ok(views::project(&row))
    }

    /// Every project, ordered by name.
    pub fn list_projects(&self) -> Result<Vec<Project>, DomainError> {
        Ok(self
            .store
            .list_projects()?
            .iter()
            .map(views::project)
            .collect())
    }

    /// The canonical toplevel of the repository containing `dir`, or `None` outside one.
    fn repository_root(&self, dir: &Path) -> Result<Option<PathBuf>, DomainError> {
        Ok(self.git.toplevel(dir)?.map(|top| paths::canonical(&top)))
    }
}

fn not_git(dir: &Path) -> DomainError {
    DomainError::new(
        codes::PROJECT_NOT_GIT,
        format!("{} is not inside a git repository", dir.display()),
    )
}

