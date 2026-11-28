//! Projects: registering a repository or a folder, listing, showing and unregistering (ADR 0008).

use std::path::{Path, PathBuf};

use massorca_core_git::{WORKTREES_FOLDER, discard_new_repository};
use massorca_core_store::{ProjectKind as StoreKind, ProjectRow, StoreError, new_id, now_ms};
use massorca_protocol::codes;
use massorca_protocol::methods::projects::{
    Project, ProjectAddParams, ProjectIcon, ProjectInspection, ProjectKind, ProjectSetBaseParams,
    ProjectUpdateParams,
};

use crate::error::DomainError;
use crate::layout::WorkspaceLayout;
use crate::names;
use crate::paths;
use crate::project_icon_detection::detect_project_icon;
use crate::views;
use crate::workspaces::Workspaces;

/// What `remove_project` took away: the project, and the ids of the workspace rows forgotten with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedProject {
    pub project: Project,
    pub workspaces: Vec<String>,
}

impl Workspaces {
    /// The checkout that git reads of a project run in when no workspace is named: a git project's own directory. A
    /// folder project has none (`git.not_a_repository`), and a missing directory is `path.not_found` (decision 0112).
    pub fn project_checkout(&self, selector: &str) -> Result<PathBuf, DomainError> {
        let project = self.resolve_project(selector)?;
        if matches!(project.kind, StoreKind::Folder) {
            return Err(DomainError::new(
                codes::GIT_NOT_A_REPOSITORY,
                format!(
                    "project {:?} is a folder, not a git repository",
                    project.name
                ),
            ));
        }
        let checkout = PathBuf::from(&project.path);
        if !checkout.is_dir() {
            return Err(DomainError::new(
                codes::PATH_NOT_FOUND,
                format!(
                    "the directory of project {:?} is missing: {}",
                    project.name, project.path
                ),
            ));
        }
        Ok(checkout)
    }

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
        let icon = params
            .icon
            .map(|icon| icon.normalized().map_err(invalid_icon))
            .transpose()?;
        let path_key = paths::path_key(&root);
        if self.registered_at(&root)?.is_some() {
            return Err(DomainError::new(
                codes::PROJECT_EXISTS,
                format!("{} is already a project", root.display()),
            ));
        }
        if self.store.find_conversations_project(&path_key)?.is_some() {
            return Err(DomainError::new(
                codes::PROJECT_EXISTS,
                format!(
                    "{} is the folder conversations without a project are saved in",
                    root.display()
                ),
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
            None => self.default_workspaces_root(&name, &root, kind)?,
        };
        let icon = icon.or_else(|| detect_project_icon(&root));
        let row = ProjectRow {
            id: new_id(),
            kind,
            path: paths::path_text(&root)?,
            path_key,
            name,
            workspaces_root,
            added_at: now_ms(),
            base_ref: None,
            icon: stored_icon(icon.as_ref())?,
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
        Ok(self.project_view(&row))
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
            .map(|row| self.project_view(row))
            .collect())
    }

    /// The project `selector` names: id, then exact name, then path.
    pub fn show_project(&self, selector: &str) -> Result<Project, DomainError> {
        Ok(self.project_view(&self.resolve_project(selector)?))
    }

    /// A project's view with its remote's owner read from git now, so a changed remote shows on the next read.
    fn project_view(&self, row: &ProjectRow) -> Project {
        let remote_owner = match row.kind {
            StoreKind::Git => self.git.primary_remote_owner(Path::new(&row.path)),
            StoreKind::Folder => None,
        };
        Project {
            remote_owner,
            ..views::project(row)
        }
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
                    project: self.project_view(&row),
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
                project: self.project_view(&row),
                workspaces: Vec::new(),
            }),
            Ok(false) => Err(not_found(&row.name)),
            // A workspace inserted after the count is still referenced; the foreign key refuses.
            Err(StoreError::Constraint { .. }) => Err(has_workspaces(&row.name)),
            Err(error) => Err(error.into()),
        }
    }

    /// The workspaces root a project named `name` at `dir` gets when it names none: `<dir>/.worktrees` for a git project
    /// in the in-project layout (decision 0183), its own folder under the default root in the nested layout (and for a
    /// folder project, which has no worktrees), the default root itself in the flat one.
    fn default_workspaces_root(
        &self,
        name: &str,
        dir: &Path,
        kind: StoreKind,
    ) -> Result<String, DomainError> {
        if self.layout == WorkspaceLayout::InProject && kind == StoreKind::Git {
            return paths::path_text(&dir.join(WORKTREES_FOLDER));
        }
        if !self.default_root.is_absolute() {
            return Err(DomainError::new(
                codes::REQUEST_INVALID_PARAMS,
                "the default workspaces root must be an absolute path",
            ));
        }
        let root = match self.layout {
            WorkspaceLayout::Nested | WorkspaceLayout::InProject => self.default_root.join(name),
            WorkspaceLayout::Flat => self.default_root.clone(),
        };
        paths::path_text(&root)
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
        Ok(self.project_view(&ProjectRow {
            base_ref: params.base,
            ..row
        }))
    }

    /// Sets (`Some`), clears (`Some(None)`) or keeps (`None`) a project's icon. An invalid icon keeps the stored one.
    pub fn update_project(&self, params: ProjectUpdateParams) -> Result<Project, DomainError> {
        let _guard = self.lock_mutations();
        let row = self.resolve_project(&params.project)?;
        let Some(icon) = params.icon else {
            return Ok(self.project_view(&row));
        };
        let icon = icon
            .map(|icon| icon.normalized().map_err(invalid_icon))
            .transpose()?;
        let stored = stored_icon(icon.as_ref())?;
        if !self.store.set_project_icon(&row.id, stored.as_deref())? {
            return Err(not_found(&row.name));
        }
        Ok(self.project_view(&ProjectRow {
            icon: stored,
            ..row
        }))
    }

    /// Turns a folder project into a git project (decision 0115): `git init` and an empty first commit in its folder,
    /// then the workspaces root a new repository of that name gets. Its folder workspaces stay as they are.
    pub fn init_project_git(&self, selector: &str) -> Result<Project, DomainError> {
        let _guard = self.lock_mutations();
        let row = self.resolve_project(selector)?;
        if row.kind == StoreKind::Git {
            return Err(DomainError::new(
                codes::PROJECT_ALREADY_GIT,
                format!("project {:?} is already a git project", row.name),
            ));
        }
        let dir = paths::existing_directory(Path::new(&row.path))?;
        if dir.join(".git").symlink_metadata().is_ok() {
            return Err(DomainError::new(
                codes::PROJECT_ALREADY_GIT,
                format!("{} already has a .git entry", dir.display()),
            ));
        }
        if self.git.toplevel(&dir)?.is_some() {
            return Err(DomainError::new(
                codes::PROJECT_ALREADY_GIT,
                format!("{} is inside a git repository", dir.display()),
            ));
        }
        let workspaces_root = self.default_workspaces_root(&row.name, &dir, StoreKind::Git)?;
        self.git.init_repository(&dir)?;
        match self.store.set_project_git(&row.id, &workspaces_root) {
            Ok(true) => {}
            Ok(false) => {
                discard_new_repository(&dir);
                return Err(not_found(&row.name));
            }
            Err(error) => {
                discard_new_repository(&dir);
                return Err(error.into());
            }
        }
        Ok(views::project(&ProjectRow {
            kind: StoreKind::Git,
            workspaces_root,
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

/// The stored JSON of an icon; `None` is the default icon.
fn stored_icon(icon: Option<&ProjectIcon>) -> Result<Option<String>, DomainError> {
    icon.map(ProjectIcon::to_stored)
        .transpose()
        .map_err(|error| DomainError::new(codes::INTERNAL, error.to_string()))
}

fn invalid_icon(reason: String) -> DomainError {
    DomainError::new(
        codes::REQUEST_INVALID_PARAMS,
        format!("invalid icon: {reason}"),
    )
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
