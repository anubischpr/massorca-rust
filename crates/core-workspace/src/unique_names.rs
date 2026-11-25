//! `unique_name` on a create: a typed name whose name, branch or folder is taken moves on to `name-2` ..
//! `name-100`, the scheme the reference app uses. Every comparison folds case, as the reference app does. A rename of a
//! generated name (decision 0111) runs the same search without the folder, which it never moves.

use std::collections::HashSet;
use std::path::Path;

use massorca_core_store::{ProjectKind as StoreKind, ProjectRow};
use massorca_protocol::codes;
use massorca_protocol::methods::workspaces::WorkspaceCreateParams;

use crate::branch_prefix;
use crate::create::{invalid, is_occupied};
use crate::error::DomainError;
use crate::names;
use crate::paths;
use crate::workspaces::Workspaces;

/// The highest suffix tried.
const MAX_SUFFIX: usize = 100;

/// The longest workspace name, which a suffixed name must keep to.
const MAX_NAME: usize = 64;

impl Workspaces {
    /// The first of `params.name`, `name-2` .. `name-100` that no workspace, branch or folder of `project` takes.
    /// Called under the mutation lock, so a concurrent create sees this one's row and moves past it.
    pub(crate) fn unique_name(
        &self,
        project: &ProjectRow,
        params: &WorkspaceCreateParams,
    ) -> Result<String, DomainError> {
        if params.branch.is_some() || params.existing_branch || params.path.is_some() {
            return Err(invalid(
                "unique_name applies only when the service derives the branch and the folder",
            ));
        }
        let base = names::clean_workspace_name(&params.name)?;
        self.free_name(project, &base, Folders::Checked)
    }

    /// The first of `base`, `base-2` .. `base-100` that no workspace or branch (local or remote-tracking) of `project`
    /// takes, nor a folder when `folders` says so. Called under the mutation lock.
    pub(crate) fn free_name(
        &self,
        project: &ProjectRow,
        base: &str,
        folders: Folders,
    ) -> Result<String, DomainError> {
        let rows = self.store.list_workspaces(Some(&project.id))?;
        let taken_names: HashSet<String> = rows.iter().map(|row| row.name.to_lowercase()).collect();
        let free = match project.kind {
            // A folder workspace's folder is the project's or a given one, never derived from the name.
            StoreKind::Folder => {
                first_free_name(base, |name| taken_names.contains(&name.to_lowercase()))
            }
            StoreKind::Git => {
                let repo = Path::new(&project.path);
                let segment = self.branch_prefix.segment(&self.git, repo)?;
                let mut branches: HashSet<String> = self
                    .git
                    .branches(repo)?
                    .into_iter()
                    .map(|branch| branch.name.to_lowercase())
                    .collect();
                branches.extend(
                    self.git
                        .remote_branch_names(repo)?
                        .into_iter()
                        .map(|branch| branch.to_lowercase()),
                );
                let claimed: HashSet<String> = match folders {
                    Folders::Checked => self
                        .store
                        .list_workspaces(None)?
                        .into_iter()
                        .map(|row| row.path_key)
                        .collect(),
                    Folders::Ignored => HashSet::new(),
                };
                let root = paths::absolute(&project.workspaces_root)?;
                first_free_name(base, |name| {
                    let branch = branch_prefix::branch_name(segment.as_deref(), name);
                    let folder_taken = || {
                        let folder = root.join(name);
                        claimed.contains(&paths::path_key(&folder)) || is_occupied(&folder)
                    };
                    taken_names.contains(&name.to_lowercase())
                        || branches.contains(&branch.to_lowercase())
                        || (matches!(folders, Folders::Checked) && folder_taken())
                })
            }
        };
        free.ok_or_else(|| {
            DomainError::new(
                codes::WORKSPACE_NAME_TAKEN,
                format!("no free name was found from {base:?} to {base:?}-{MAX_SUFFIX}"),
            )
        })
    }
}

/// Whether a candidate's folder under the workspaces root counts: a create makes it, a rename never moves it.
#[derive(Clone, Copy)]
pub(crate) enum Folders {
    Checked,
    Ignored,
}

/// `base`, then `base-2` .. `base-100`: the first that `is_taken` does not refuse. A suffixed name cuts the base, and
/// any `-` or `.` that ends the cut, so it stays within the length limit.
fn first_free_name(base: &str, is_taken: impl Fn(&str) -> bool) -> Option<String> {
    std::iter::once(base.to_owned())
        .chain((2..=MAX_SUFFIX).map(|suffix| suffixed(base, suffix)))
        .find(|candidate| !is_taken(candidate))
}

fn suffixed(base: &str, suffix: usize) -> String {
    let suffix = format!("-{suffix}");
    // A cleaned name is ASCII, so any byte offset is a character boundary.
    let cut = base
        .get(..base.len().min(MAX_NAME - suffix.len()))
        .unwrap_or(base);
    format!("{}{suffix}", cut.trim_end_matches(['-', '.']))
}

#[cfg(test)]
mod tests {
    use super::{MAX_NAME, first_free_name};
    use crate::names::workspace_name;

    #[test]
    fn the_candidates_run_from_the_name_to_the_hundredth_suffix() {
        assert_eq!(first_free_name("a", |_| false).as_deref(), Some("a"));
        assert_eq!(
            first_free_name("a", |name| name != "a-100").as_deref(),
            Some("a-100")
        );
        assert_eq!(first_free_name("a", |_| true), None);
    }

    #[test]
    fn every_suffixed_long_name_is_a_valid_workspace_name() {
        let base = format!("{}.-xy", "b".repeat(60));
        assert_eq!(base.len(), MAX_NAME);
        let last = first_free_name(&base, |name| name != format!("{}-100", "b".repeat(60)))
            .expect("the last suffix is free");
        workspace_name(&last).expect("valid");
        for suffix in [2, 10, 100] {
            let name = super::suffixed(&base, suffix);
            assert!(name.len() <= MAX_NAME, "{name}");
            workspace_name(&name).expect(&name);
        }
    }
}
