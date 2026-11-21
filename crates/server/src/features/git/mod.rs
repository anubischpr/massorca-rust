//! The `git/1` capability (decisions 0010-0015): one module per command group, sharing workspace resolution here.

mod commit;
mod diff;
mod index;
mod log;
mod status;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, Weak};

use massorca_core_git::Git;
use massorca_core_workspace::DomainError;
use massorca_protocol::methods::workspaces::{WorkspaceKind, WorkspaceState};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::owner::Owner;
use crate::registry::RegistrationHandle;

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    let mut handles = vec![
        registries
            .capabilities
            .register(capabilities::GIT, Owner::Core)?,
    ];
    handles.extend(status::register(registries)?);
    handles.extend(diff::register(registries)?);
    handles.extend(log::register(registries)?);
    handles.extend(index::register(registries)?);
    handles.extend(commit::register(registries)?);
    Ok(handles)
}

/// The git runner and the work tree a workspace selector names.
#[derive(Debug, Clone)]
pub(crate) struct Repository {
    pub(crate) git: Git,
    pub(crate) dir: PathBuf,
    pub(crate) workspace_id: String,
}

/// One lock per workspace, held by every git mutation on it, so two clients cannot interleave a stage and a
/// commit (decision 0010). Entries are dropped with their last holder.
static MUTATION_LOCKS: LazyLock<Mutex<HashMap<String, Weak<tokio::sync::Mutex<()>>>>> =
    LazyLock::new(Mutex::default);

impl Repository {
    /// Waits for the workspace's mutation lock; reads never take it.
    pub(crate) async fn lock(&self) -> tokio::sync::OwnedMutexGuard<()> {
        let lock = {
            let mut locks = MUTATION_LOCKS
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            locks.retain(|_, lock| lock.strong_count() > 0);
            match locks.get(&self.workspace_id).and_then(Weak::upgrade) {
                Some(lock) => lock,
                None => {
                    let lock = Arc::new(tokio::sync::Mutex::new(()));
                    locks.insert(self.workspace_id.clone(), Arc::downgrade(&lock));
                    lock
                }
            }
        };
        lock.lock_owned().await
    }
}

/// Resolves `selector` to a git work tree. A folder workspace, a missing directory and a workspace being removed are refused.
pub(crate) async fn repository(
    ctx: &RequestCtx,
    selector: String,
) -> Result<Repository, ErrorBody> {
    let workspaces = Arc::clone(&ctx.state.workspaces);
    let git = workspaces.git().clone();
    let workspace = blocking(move || {
        workspaces
            .show_workspace(&selector)
            .map_err(ErrorBody::from)
    })
    .await?;
    match workspace.kind {
        WorkspaceKind::Worktree => {}
        _ => {
            return Err(ErrorBody::new(
                codes::GIT_NOT_A_REPOSITORY,
                format!(
                    "workspace `{}` is a folder, not a git worktree",
                    workspace.name
                ),
            ));
        }
    }
    if workspace.state == WorkspaceState::Removing {
        return Err(ErrorBody::new(
            codes::WORKSPACE_NOT_FOUND,
            format!("workspace `{}` is being removed", workspace.name),
        ));
    }
    if workspace.missing {
        return Err(ErrorBody::new(
            codes::PATH_NOT_FOUND,
            format!(
                "the directory of workspace `{}` is missing: {}",
                workspace.name, workspace.path
            ),
        ));
    }
    Ok(Repository {
        git,
        dir: PathBuf::from(workspace.path),
        workspace_id: workspace.id,
    })
}

/// Checks a workspace-relative path from a request: no absolute path, no `..` segment, no NUL (decision 0010).
pub(crate) fn relative_path(path: &str) -> Result<&str, ErrorBody> {
    let outside = path.is_empty()
        || path.contains('\0')
        || path.starts_with('/')
        || path.starts_with('\\')
        || std::path::Path::new(path).is_absolute()
        || path.get(1..2) == Some(":")
        || path.split(['/', '\\']).any(|segment| segment == "..");
    if outside {
        return Err(ErrorBody::new(
            codes::GIT_PATH_OUTSIDE_WORKTREE,
            format!("`{path}` is not a path inside the workspace"),
        ));
    }
    Ok(path)
}

/// Checks that `revision` names a commit; a leading dash is refused before git can read it as an option.
pub(crate) fn require_commit(repository: &Repository, revision: &str) -> Result<(), ErrorBody> {
    if repository
        .git
        .ref_exists(&repository.dir, revision)
        .map_err(git_error)?
    {
        Ok(())
    } else {
        Err(ErrorBody::new(
            codes::GIT_REVISION_NOT_FOUND,
            format!("`{revision}` does not name a commit"),
        ))
    }
}

/// Runs one git call on the blocking pool and maps its error to the protocol.
pub(crate) async fn blocking<T, F>(call: F) -> Result<T, ErrorBody>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, ErrorBody> + Send + 'static,
{
    tokio::task::spawn_blocking(call)
        .await
        .unwrap_or_else(|error| {
            Err(ErrorBody::new(
                codes::INTERNAL,
                format!("the git task failed: {error}"),
            ))
        })
}

/// A git failure as the protocol reports it.
pub(crate) fn git_error(error: massorca_core_git::GitError) -> ErrorBody {
    match massorca_core_git::failure_code(&error) {
        Some(code) => ErrorBody::new(code, error.to_string()),
        None => DomainError::from(error).into(),
    }
}
