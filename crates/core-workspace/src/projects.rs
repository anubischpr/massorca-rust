//! Projects: registering a repository or a folder, listing, showing and unregistering (ADR 0008).

use std::path::{Path, PathBuf};

use massorca_core_store::{ProjectKind as StoreKind, ProjectRow, StoreError, new_id, now_ms};
use massorca_protocol::codes;
use massorca_protocol::methods::projects::{
    Project, ProjectAddParams, ProjectInspection, ProjectKind, ProjectSetBaseParams,
};

use crate::error::DomainError;
use crate::layout::WorkspaceLayout;
use crate::names;
use crate::paths;
use crate::views;
use crate::workspaces::Workspaces;

/// What `remove_project` took away: the project, and the ids of the workspace rows forgotten with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedProject {
    pub project: Project,
    pub workspaces: Vec<String>,
}

impl Workspaces {
    /// Registers a git repository (at its main worktree) or a folder. Without `kind`, a directory
    /// inside a repository is a git project and any other directory is a folder.
    pub fn add_project(&self, params: ProjectAddParams) -> Result<Project, DomainError> {
        let _guard = self.lock_mutations();
        let (kind, root) = self.locate_project(&params.path, params.kind)?;
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
        if self.registered_at(&root)?.is_some() {
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
            None => {
                if !self.default_root.is_absolute() {
                    return Err(DomainError::new(
                        codes::REQUEST_INVALID_PARAMS,
                        "the default workspaces root must be an absolute path",
                    ));
                }
                let root = match self.layout {
                    WorkspaceLayout::Nested => self.default_root.join(&name),
                    WorkspaceLayout::Flat => self.default_root.clone(),
                };
                paths::path_text(&root)?
            }
        };
        let row = ProjectRow {
            id: new_id(),
            kind,
            path: paths::path_text(&root)?,
            path_key,
            name,
            workspaces_root,
            added_at: now_ms(),
            base_ref: None,
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

    /// What `add_project` without a kind would register for `path`, and the project already registered there.
    /// Validates the path exactly as `add_project` does and writes nothing.
    pub fn inspect_project(&self, path: &str) -> Result<ProjectInspection, DomainError> {
        let (kind, root) = self.locate_project(path, None)?;
        Ok(ProjectInspection {
            path: paths::path_text(&root)?,
            kind: views::project_kind(kind),
            registered: self.registered_at(&root)?.map(|row| row.id),
        })
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

    /// The project `selector` names: id, then exact name, then path.
    pub fn show_project(&self, selector: &str) -> Result<Project, DomainError> {
        Ok(views::project(&self.resolve_project(selector)?))
    }

    /// Unregisters a project. Refused while it has workspaces, unless `forget_workspaces` drops their
    /// records with it in one transaction. Never deletes files, worktrees or branches.
    pub fn remove_project(
        &self,
        selector: &str,
        forget_workspaces: bool,
    ) -> Result<RemovedProject, DomainError> {
        let _guard = self.lock_mutations();
        let row = self.resolve_project(selector)?;
        if forget_workspaces {
            return match self.store.forget_project(&row.id)? {
                Some(workspaces) => Ok(RemovedProject {
                    project: views::project(&row),
                    workspaces,
                }),
                None => Err(not_found(&row.name)),
            };
        }
        if self.store.count_workspaces(&row.id)? > 0 {
            return Err(has_workspaces(&row.name));
        }
        match self.store.delete_project(&row.id) {
            Ok(true) => Ok(RemovedProject {
                project: views::project(&row),
                workspaces: Vec::new(),
            }),
            Ok(false) => Err(not_found(&row.name)),
            // A workspace inserted after the count is still referenced; the foreign key refuses.
            Err(StoreError::Constraint { .. }) => Err(has_workspaces(&row.name)),
            Err(error) => Err(error.into()),
        }
    }

    /// The kind and root directory a project at `path` registers as. Without `kind`, a directory inside a
    /// repository is a git project at its main worktree and any other directory is a folder.
    fn locate_project(
        &self,
        path: &str,
        kind: Option<ProjectKind>,
    ) -> Result<(StoreKind, PathBuf), DomainError> {
        if kind == Some(ProjectKind::Unknown) {
            return Err(DomainError::new(
                codes::REQUEST_INVALID_PARAMS,
                "unknown project kind",
            ));
        }
        let given = paths::existing_directory(&paths::absolute(path)?)?;
        Ok(match kind {
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
        })
    }

    /// The project registered at `root`, compared by path key.
    fn registered_at(&self, root: &Path) -> Result<Option<ProjectRow>, DomainError> {
        Ok(self
            .store
            .find_project_by_path_key(&paths::path_key(root))?)
    }

    /// Sets the base a project's new workspaces start from, or clears it with `None`. A folder
    /// project has none, and a base must resolve to a commit, as a workspace's base does.
    pub fn set_project_base(&self, params: ProjectSetBaseParams) -> Result<Project, DomainError> {
        let _guard = self.lock_mutations();
        let row = self.resolve_project(&params.project)?;
        if row.kind == StoreKind::Folder {
            return Err(DomainError::new(
                codes::REQUEST_INVALID_PARAMS,
                format!(
                    "project {:?} is a folder; only a git project has a base",
                    row.name
                ),
            ));
        }
        if let Some(name) = params.base.as_deref() {
            let repo = Path::new(&row.path);
            let base = self.git.qualify_base(repo, name)?;
            if !self.git.ref_exists(repo, &base.revision)? {
                return Err(DomainError::new(
                    codes::BASE_NOT_FOUND,
                    format!("base {name:?} does not resolve to a commit"),
                ));
            }
        }
        if !self
            .store
            .set_project_base(&row.id, params.base.as_deref())?
        {
            return Err(DomainError::new(
                codes::PROJECT_NOT_FOUND,
                format!("no project named {:?}", row.name),
            ));
        }
        Ok(views::project(&ProjectRow {
            base_ref: params.base,
            ..row
        }))
    }

    /// The canonical main worktree of the repository containing `dir`, or `None` outside one. A linked
    /// worktree stands for its main repository, so it is never registered as a project of its own.
    fn repository_root(&self, dir: &Path) -> Result<Option<PathBuf>, DomainError> {
        if self.git.toplevel(dir)?.is_none() {
            return Ok(None);
        }
        match self.git.main_worktree(dir)? {
            Some(main) => Ok(Some(paths::canonical(&main))),
            None => Err(DomainError::new(
                codes::REQUEST_INVALID_PARAMS,
                format!(
                    "{} is a linked worktree of a bare repository, which has no main worktree to register",
                    dir.display()
                ),
            )),
        }
    }
}

fn not_git(dir: &Path) -> DomainError {
    DomainError::new(
        codes::PROJECT_NOT_GIT,
        format!("{} is not inside a git repository", dir.display()),
    )
}

fn not_found(name: &str) -> DomainError {
    DomainError::new(
        codes::PROJECT_NOT_FOUND,
        format!("no project named {name:?}"),
    )
}

fn has_workspaces(name: &str) -> DomainError {
    DomainError::new(
        codes::PROJECT_HAS_WORKSPACES,
        format!("project {name:?} still has workspaces; remove them first"),
    )
}
