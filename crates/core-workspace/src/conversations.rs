//! Workspaces of conversations without a project (decision 0094). Each conversation is a folder of its own inside the
//! conversations folder, a folder workspace of a hidden project that no project list shows.

use std::path::Path;

use massorca_core_store::{ProjectKind as StoreKind, ProjectRow, new_id, now_ms};
use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};

use crate::error::DomainError;
use crate::paths;
use crate::workspaces::Workspaces;

impl Workspaces {
    /// The workspace of one conversation without a project: the folder `root/<name>`, made with its parents, as a folder
    /// workspace of the hidden project of `root`. That project is made on the first conversation of `root`. A vendor CLI
    /// keeps its history per folder, so each conversation gets a folder of its own.
    pub fn conversation_workspace(
        &self,
        root: &Path,
        name: &str,
    ) -> Result<Workspace, DomainError> {
        let _guard = self.lock_mutations();
        std::fs::create_dir_all(root).map_err(|error| made_no_folder(root, &error))?;
        let root = paths::canonical(root);
        if let Some(project) = self
            .store
            .find_project_by_path_key(&paths::path_key(&root))?
        {
            return Err(DomainError::new(
                codes::CONFIG_INVALID_VALUE,
                format!(
                    "the conversations folder {} is the project {:?}; set conversations.folder to another folder or remove that project",
                    root.display(),
                    project.name
                ),
            ));
        }
        let project = match self
            .store
            .find_conversations_project(&paths::path_key(&root))?
        {
            Some(row) => row,
            None => {
                let row = ProjectRow {
                    id: new_id(),
                    kind: StoreKind::Folder,
                    path: paths::path_text(&root)?,
                    path_key: paths::path_key(&root),
                    // The name only has to be unique; nothing shows it.
                    name: format!("conversations-{}", new_id()),
                    workspaces_root: paths::path_text(&root)?,
                    added_at: now_ms(),
                    base_ref: None,
                    icon: None,
                };
                self.store.insert_conversations_project(&row)?;
                row
            }
        };
        let folder = root.join(name);
        std::fs::create_dir_all(&folder).map_err(|error| made_no_folder(&folder, &error))?;
        let params = WorkspaceCreateParams {
            in_place: false,
            project: project.id.clone(),
            name: name.to_owned(),
            branch: None,
            base: None,
            existing_branch: false,
            refresh_base: false,
            path: Some(paths::path_text(&folder)?),
            request_id: None,
            no_setup: false,
            unique_name: false,
            name_generated: false,
        };
        self.create_folder(&project, &params, false)
    }
}

fn made_no_folder(path: &Path, error: &std::io::Error) -> DomainError {
    DomainError::new(
        codes::PATH_NOT_FOUND,
        format!(
            "could not make the conversations folder {}: {error}",
            path.display()
        ),
    )
}
