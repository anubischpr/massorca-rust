//! A service answer that says "no", typed so each kind picks its dialog (decision 0028).

use massorca_client::ClientError;
use massorca_protocol::pairing::OfferError;
use massorca_protocol::{ErrorBody, codes};

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

    /// The text to show for this refusal, as a sentence: its first letter capitalised.
    pub fn sentence(&self) -> String {
        sentence(&self.message())
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

/// Host codes the CLI reports for a client-side failure (`crates/cli/src/error.rs`); the service has no such codes.
const HOST_NOT_FOUND: &str = "host.not_found";
const HOST_EXISTS: &str = "host.exists";
const HOST_INVALID_NAME: &str = "host.invalid_name";
const HOST_IDENTITY_MISMATCH: &str = "host.identity_mismatch";

impl From<&ClientError> for Refusal {
    fn from(error: &ClientError) -> Self {
        match error {
            ClientError::Remote(body) => from_body(body),
            // Only a handshake the service never answered means it cannot be reached; any other refusal says why.
            ClientError::Handshake { error: body, .. }
                if body.code == codes::PROTOCOL_HANDSHAKE =>
            {
                unreachable_service()
            }
            ClientError::Handshake { error: body, .. } => from_body(body),
            ClientError::HostUnreachable(detail) => Self::Unavailable {
                message: sentence(&format!("the host is unreachable: {detail}")),
            },
            // The service the app started refused to (ADR 0073): its own code and message, as the CLI shows them.
            ClientError::StartRefused(body) | ClientError::ProfileRefused(body) => Self::Failed {
                code: body.code.clone(),
                message: body.message.clone(),
            },
            ClientError::Disconnected
            | ClientError::Unavailable(_)
            | ClientError::StartFailed(_)
            | ClientError::UntrustedEndpoint(_) => unreachable_service(),
            ClientError::ServerIdentityMismatch => host_failure(HOST_IDENTITY_MISMATCH, error),
            ClientError::HostNotFound(_) => host_failure(HOST_NOT_FOUND, error),
            ClientError::HostExists(_) => host_failure(HOST_EXISTS, error),
            ClientError::InvalidHostName(_) => host_failure(HOST_INVALID_NAME, error),
            ClientError::Protocol(_) | ClientError::Io(_) => Self::Failed {
                code: "client.failed".to_owned(),
                message: error.to_string(),
            },
        }
    }
}

/// A pairing offer that does not decode: its own sentence, under the code the CLI uses for it.
impl From<&OfferError> for Refusal {
    fn from(error: &OfferError) -> Self {
        let code = match error {
            OfferError::NotAnOffer => "pairing.not_an_offer",
            OfferError::Damaged => "pairing.damaged",
        };
        Self::Failed {
            code: code.to_owned(),
            message: sentence(&error.to_string()),
        }
    }
}

/// A service error body, by its code: the files it names, or the service's own sentence.
fn from_body(body: &ErrorBody) -> Refusal {
    match body.code.as_str() {
        codes::WORKSPACE_DIRTY => Refusal::Dirty,
        codes::WORKSPACE_IGNORED_FILES => Refusal::IgnoredFiles,
        codes::WORKSPACE_ORPHAN => Refusal::Orphan,
        codes::WORKSPACE_LOCKED => Refusal::Locked {
            message: body.message.clone(),
        },
        codes::WORKSPACE_MAIN_WORKTREE => Refusal::MainWorktree {
            message: body.message.clone(),
        },
        codes::WORKSPACE_NAME_TAKEN
        | codes::WORKSPACE_NAME_INVALID
        | codes::WORKSPACE_PATH_TAKEN => Refusal::NameProblem {
            message: body.message.clone(),
        },
        codes::PATH_NOT_FOUND
        | codes::PATH_NOT_DIRECTORY
        | codes::PROJECT_EXISTS
        | codes::PROJECT_NOT_GIT => Refusal::PathProblem {
            message: body.message.clone(),
        },
        codes::GIT_DISCARD_UNTRACKED
        | codes::GIT_DISCARD_STAGED
        | codes::GIT_PATH_CONFLICTED
        | codes::GIT_PATH_IS_DIRECTORY => Refusal::GitRefused {
            code: body.code.clone(),
            message: body.message.clone(),
            paths: paths_of(body),
        },
        code => Refusal::Failed {
            code: code.to_owned(),
            message: body.message.clone(),
        },
    }
}

fn unreachable_service() -> Refusal {
    Refusal::Unavailable {
        message: "The service cannot be reached right now.".to_owned(),
    }
}

fn host_failure(code: &str, error: &ClientError) -> Refusal {
    Refusal::Failed {
        code: code.to_owned(),
        message: sentence(&error.to_string()),
    }
}

/// The text with its first letter capitalised, for a sentence that starts a line.
pub(super) fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
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
    fn an_unreachable_host_is_named_as_the_host_in_a_sentence() {
        assert_eq!(
            Refusal::from(&ClientError::HostUnreachable("timed out".into())),
            Refusal::Unavailable {
                message: "The host is unreachable: timed out".into()
            }
        );
    }

    #[test]
    fn host_refusals_keep_their_codes_and_say_what_happened() {
        assert_eq!(
            Refusal::from(&ClientError::HostNotFound("vps".into())),
            Refusal::Failed {
                code: "host.not_found".into(),
                message: "No host named \"vps\"".into()
            }
        );
        assert!(matches!(
            Refusal::from(&ClientError::HostExists("vps".into())),
            Refusal::Failed { code, .. } if code == "host.exists"
        ));
        assert!(matches!(
            Refusal::from(&ClientError::InvalidHostName("Vps!".into())),
            Refusal::Failed { code, .. } if code == "host.invalid_name"
        ));
        assert!(matches!(
            Refusal::from(&ClientError::ServerIdentityMismatch),
            Refusal::Failed { code, .. } if code == "host.identity_mismatch"
        ));
    }

    #[test]
    fn a_refused_handshake_keeps_the_service_sentence_and_its_code() {
        let refused = ClientError::Handshake {
            error: Box::new(ErrorBody::new(
                codes::PAIRING_INVALID,
                "the pairing offer is not valid here; it may have been used already",
            )),
            supported: None,
        };
        assert_eq!(
            Refusal::from(&refused),
            Refusal::Failed {
                code: codes::PAIRING_INVALID.into(),
                message: "the pairing offer is not valid here; it may have been used already"
                    .into()
            }
        );
    }

    #[test]
    fn a_handshake_that_times_out_is_still_a_service_that_cannot_be_reached() {
        let timed_out = ClientError::Handshake {
            error: Box::new(ErrorBody::new(
                codes::PROTOCOL_HANDSHAKE,
                "the service did not answer hello in time",
            )),
            supported: None,
        };
        assert!(matches!(
            Refusal::from(&timed_out),
            Refusal::Unavailable { .. }
        ));
    }

    #[test]
    fn an_offer_that_does_not_decode_is_refused_with_its_sentence() {
        assert_eq!(
            Refusal::from(&OfferError::NotAnOffer),
            Refusal::Failed {
                code: "pairing.not_an_offer".into(),
                message: "This is not a MassOrca pairing offer".into()
            }
        );
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
