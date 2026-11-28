//! The Windows path limit for in-project worktrees (decision 0183): a worktree deeper than `MAX_PATH` breaks git and
//! most tools there, so the create is refused before git runs, naming the path and how to pick another root.

use std::path::Path;

use massorca_protocol::codes;

use crate::error::DomainError;

/// Windows' classic `MAX_PATH`, in UTF-16 units.
const WINDOWS_MAX_PATH: usize = 260;

/// The path limit of the platform the service runs on; `None` where there is none worth checking.
pub(crate) fn platform_path_limit() -> Option<usize> {
    cfg!(windows).then_some(WINDOWS_MAX_PATH)
}

/// Refuses `path` with `workspace.path_too_long` when it is longer than `limit`.
pub(crate) fn check_worktree_path_length(
    path: &Path,
    limit: Option<usize>,
) -> Result<(), DomainError> {
    let Some(limit) = limit else {
        return Ok(());
    };
    let length = path.to_string_lossy().encode_utf16().count();
    if length <= limit {
        return Ok(());
    }
    Err(DomainError::new(
        codes::WORKSPACE_PATH_TOO_LONG,
        format!(
            "{} is {length} characters, over the {limit}-character Windows path limit; set \
             workspaces.default_root and workspaces.layout, or give the project a shorter workspaces_root",
            path.display()
        ),
    ))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use massorca_protocol::codes;

    use super::check_worktree_path_length;

    #[test]
    fn a_path_at_the_limit_passes_and_one_over_it_is_refused_by_name() {
        let path = Path::new("/p/.worktrees/abc");
        let length = path.to_string_lossy().len();
        check_worktree_path_length(path, Some(length)).expect("at the limit");
        let error = check_worktree_path_length(path, Some(length - 1)).expect_err("over");
        assert_eq!(error.code(), codes::WORKSPACE_PATH_TOO_LONG);
        assert!(
            error.message().contains(".worktrees"),
            "{}",
            error.message()
        );
        assert!(
            error.message().contains("workspaces.default_root"),
            "{}",
            error.message()
        );
        assert!(
            error.message().contains("workspaces_root"),
            "{}",
            error.message()
        );
    }

    #[test]
    fn no_limit_passes_every_path() {
        check_worktree_path_length(Path::new(&"x".repeat(1000)), None).expect("no limit");
    }
}
