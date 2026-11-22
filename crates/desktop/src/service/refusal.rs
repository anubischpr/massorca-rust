//! A service answer that says "no", typed so each kind picks its dialog (decision 0028).

use massorca_client::ClientError;
use massorca_protocol::codes;

/// Why a create or remove did not happen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The workspace has uncommitted changes; `force` discards them.
    Dirty,
    /// The worktree holds ignored files; `discard_ignored` deletes them.
    IgnoredFiles,
    /// The row's directory is not a git worktree; `forget_orphan` drops the row.
    Orphan,
    /// Git has the worktree locked. Nothing a dialog can waive.
    Locked { message: String },
    /// The project's own checkout. It is never removed.
    MainWorktree { message: String },
    /// The name is taken or not usable. Shown under the name field.
    NameProblem { message: String },
    /// The folder is missing, not a folder, already a project, or not a Git repository. Shown
    /// under the path field of "Add a project".
    PathProblem { message: String },
    /// Any other answer of the service.
    Failed { code: String, message: String },
    /// A git refusal that names the files it would affect (decision 0013), in `paths`.
    GitRefused {
        code: String,
        message: String,
        paths: Vec<String>,
    },
    /// The service could not be reached for this call.
    Unavailable { message: String },
}

impl Refusal {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::Unavailable {
            message: message.into(),
        }
    }

    /// The service's code for this refusal, when it gave one.
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Failed { code, .. } | Self::GitRefused { code, .. } => Some(code),
            _ => None,
        }
    }

    /// The text to show for this refusal.
    pub fn message(&self) -> String {
        match self {
            Self::Dirty => "This workspace has uncommitted changes.".to_owned(),
            Self::IgnoredFiles => "This workspace holds ignored files.".to_owned(),
            Self::Orphan => "This folder is not a git worktree any more.".to_owned(),
            Self::Locked { message }
            | Self::MainWorktree { message }
            | Self::NameProblem { message }
            | Self::PathProblem { message }
            | Self::Failed { message, .. }
            | Self::GitRefused { message, .. }
            | Self::Unavailable { message } => message.clone(),
        }
    }
}

impl From<&ClientError> for Refusal {
    fn from(error: &ClientError) -> Self {
        match error {
            ClientError::Remote(body) => match body.code.as_str() {
                codes::WORKSPACE_DIRTY => Self::Dirty,
                codes::WORKSPACE_IGNORED_FILES => Self::IgnoredFiles,
                codes::WORKSPACE_ORPHAN => Self::Orphan,
                codes::WORKSPACE_LOCKED => Self::Locked {
                    message: body.message.clone(),
                },
                codes::WORKSPACE_MAIN_WORKTREE => Self::MainWorktree {
                    message: body.message.clone(),
                },
                codes::WORKSPACE_NAME_TAKEN
                | codes::WORKSPACE_NAME_INVALID
                | codes::WORKSPACE_PATH_TAKEN => Self::NameProblem {
                    message: body.message.clone(),
                },
                codes::PATH_NOT_FOUND
                | codes::PATH_NOT_DIRECTORY
                | codes::PROJECT_EXISTS
                | codes::PROJECT_NOT_GIT => Self::PathProblem {
                    message: body.message.clone(),
                },
                codes::GIT_DISCARD_UNTRACKED
                | codes::GIT_DISCARD_STAGED
                | codes::GIT_PATH_CONFLICTED
                | codes::GIT_PATH_IS_DIRECTORY => Self::GitRefused {
                    code: body.code.clone(),
                    message: body.message.clone(),
                    paths: paths_of(body),
                },
                code => Self::Failed {
                    code: code.to_owned(),
                    message: body.message.clone(),
                },
            },
            ClientError::Disconnected
            | ClientError::Unavailable(_)
            | ClientError::StartFailed(_)
            | ClientError::UntrustedEndpoint(_)
            | ClientError::Handshake { .. }
            | ClientError::HostUnreachable(_) => Self::Unavailable {
                message: "The service cannot be reached right now.".to_owned(),
            },
            ClientError::Protocol(_)
            | ClientError::Io(_)
            | ClientError::ServerIdentityMismatch
            | ClientError::HostNotFound(_)
            | ClientError::HostExists(_)
            | ClientError::InvalidHostName(_) => Self::Failed {
                code: "client.failed".to_owned(),
                message: error.to_string(),
            },
        }
    }
}

/// The files a refusal names in `data.paths`; none when it names none.
fn paths_of(body: &massorca_protocol::ErrorBody) -> Vec<String> {
    body.data
        .as_ref()
        .and_then(|data| data.get("paths"))
        .and_then(|paths| paths.as_array())
        .map(|paths| {
            paths
                .iter()
                .filter_map(|path| path.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use massorca_protocol::ErrorBody;

    fn remote(code: &str) -> ClientError {
        ClientError::Remote(ErrorBody::new(code, "the service says no"))
    }

    #[test]
    fn each_waivable_code_maps_to_its_own_refusal() {
        assert_eq!(
            Refusal::from(&remote(codes::WORKSPACE_DIRTY)),
            Refusal::Dirty
        );
        assert_eq!(
            Refusal::from(&remote(codes::WORKSPACE_IGNORED_FILES)),
            Refusal::IgnoredFiles
        );
        assert_eq!(
            Refusal::from(&remote(codes::WORKSPACE_ORPHAN)),
            Refusal::Orphan
        );
    }

    #[test]
    fn locked_and_main_worktree_are_plain_refusals_with_the_service_text() {
        assert_eq!(
            Refusal::from(&remote(codes::WORKSPACE_LOCKED)),
            Refusal::Locked {
                message: "the service says no".into()
            }
        );
        assert!(matches!(
            Refusal::from(&remote(codes::WORKSPACE_MAIN_WORKTREE)),
            Refusal::MainWorktree { .. }
        ));
    }

    #[test]
    fn name_problems_are_kept_apart_for_the_name_field() {
        for code in [
            codes::WORKSPACE_NAME_TAKEN,
            codes::WORKSPACE_NAME_INVALID,
            codes::WORKSPACE_PATH_TAKEN,
        ] {
            assert!(matches!(
                Refusal::from(&remote(code)),
                Refusal::NameProblem { .. }
            ));
        }
    }

    #[test]
    fn unknown_codes_keep_their_code_and_a_lost_connection_is_unavailable() {
        assert_eq!(
            Refusal::from(&remote("project.not_found")),
            Refusal::Failed {
                code: "project.not_found".into(),
                message: "the service says no".into()
            }
        );
        assert!(matches!(
            Refusal::from(&ClientError::Disconnected),
            Refusal::Unavailable { .. }
        ));
    }

    #[test]
    fn an_unreachable_host_is_unavailable_and_a_key_mismatch_is_a_failure() {
        assert!(matches!(
            Refusal::from(&ClientError::HostUnreachable("timed out".into())),
            Refusal::Unavailable { .. }
        ));
        assert!(matches!(
            Refusal::from(&ClientError::ServerIdentityMismatch),
            Refusal::Failed { code, .. } if code == "client.failed"
        ));
    }

    #[test]
    fn path_and_project_refusals_show_under_the_path_field() {
        for code in [
            codes::PATH_NOT_FOUND,
            codes::PATH_NOT_DIRECTORY,
            codes::PROJECT_EXISTS,
            codes::PROJECT_NOT_GIT,
        ] {
            assert!(
                matches!(Refusal::from(&remote(code)), Refusal::PathProblem { .. }),
                "{code} belongs under the path field"
            );
        }
    }
}
