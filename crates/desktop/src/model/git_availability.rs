//! Git on this computer as the Settings row says it (GIT-01): the service's answer, read on each
//! connect, as a state word and the sentence under the row's name. Free of GPUI.

use massorca_protocol::methods::service::GitStatus as ServiceGitStatus;

use crate::service::Refusal;

/// What the service said about this computer's Git.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GitState {
    /// No answer on this connection yet, or the service is not connected.
    NotChecked,
    /// A git at or above the baseline, with its version.
    Present { version: String },
    /// No git program runs on this computer.
    Missing,
    /// A git below the baseline; every git command is refused.
    TooOld { version: String, required: String },
    /// The service could not check Git, and says why.
    Unknown { reason: String },
    /// The service predates the Git report.
    Unreported,
}

impl GitState {
    /// A stable name for the state, for the row's test selector.
    pub fn key(&self) -> &'static str {
        match self {
            Self::NotChecked => "not-checked",
            Self::Present { .. } => "present",
            Self::Missing => "missing",
            Self::TooOld { .. } => "too-old",
            Self::Unknown { .. } | Self::Unreported => "unknown",
        }
    }
}

/// A row's words: the state word its control shows, and the sentence under its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitRow {
    pub state: GitState,
    pub word: String,
    pub detail: String,
}

/// The row for the answer the store holds; `None` until a connection's read has answered.
pub fn git_row(answer: Option<&Result<Option<ServiceGitStatus>, Refusal>>) -> GitRow {
    let state = match answer {
        None => GitState::NotChecked,
        Some(Ok(None)) => GitState::Unreported,
        Some(Ok(Some(status))) => state_of(status),
        Some(Err(refusal)) => GitState::Unknown {
            reason: refusal.sentence().trim_end_matches('.').to_owned(),
        },
    };
    let (word, detail) = words_of(&state);
    GitRow {
        state,
        word: word.to_owned(),
        detail,
    }
}

fn state_of(status: &ServiceGitStatus) -> GitState {
    match status {
        ServiceGitStatus::Present { version } => GitState::Present {
            version: version.clone(),
        },
        ServiceGitStatus::Missing => GitState::Missing,
        ServiceGitStatus::TooOld { version, required } => GitState::TooOld {
            version: version.clone(),
            required: required.clone(),
        },
        ServiceGitStatus::Unknown { reason } => GitState::Unknown {
            reason: reason.trim_end_matches('.').to_owned(),
        },
    }
}

/// The state word and the sentence under the row's name, for a state.
fn words_of(state: &GitState) -> (&'static str, String) {
    match state {
        GitState::NotChecked => (
            "Not checked",
            "MassOrca checks Git when it connects to this computer's service.".to_owned(),
        ),
        GitState::Present { version } => (
            "Installed",
            format!("Git {version} is installed. Worktrees, branches and Source control can use it."),
        ),
        GitState::Missing => (
            "Not installed",
            "Worktrees, branches and Source control need Git. Install Git, then restart MassOrca to check again.".to_owned(),
        ),
        GitState::TooOld { version, required } => (
            "Too old",
            format!(
                "Git {version} is older than the {required} that MassOrca needs, so worktrees, branches and Source control stay off. Upgrade Git, then restart MassOrca to check again."
            ),
        ),
        GitState::Unknown { reason } => (
            "Unknown",
            format!("MassOrca could not check Git: {reason}. Restart MassOrca to check again."),
        ),
        GitState::Unreported => (
            "Unknown",
            "This service does not report Git. Update MassOrca on this computer, then restart it to check again.".to_owned(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_of(answer: Result<Option<ServiceGitStatus>, Refusal>) -> GitRow {
        git_row(Some(&answer))
    }

    #[test]
    fn before_any_answer_git_is_not_checked() {
        let row = git_row(None);
        assert_eq!(row.state, GitState::NotChecked);
        assert_eq!(row.word, "Not checked");
        assert_eq!(
            row.detail,
            "MassOrca checks Git when it connects to this computer's service."
        );
    }

    #[test]
    fn a_present_git_is_installed_and_its_version_is_in_the_sentence() {
        let row = row_of(Ok(Some(ServiceGitStatus::Present {
            version: "2.49.0".into(),
        })));
        assert_eq!(
            row.state,
            GitState::Present {
                version: "2.49.0".into()
            }
        );
        assert_eq!(row.word, "Installed");
        assert_eq!(
            row.detail,
            "Git 2.49.0 is installed. Worktrees, branches and Source control can use it."
        );
    }

    #[test]
    fn a_missing_git_says_what_stops_and_what_to_do() {
        let row = row_of(Ok(Some(ServiceGitStatus::Missing)));
        assert_eq!(row.state, GitState::Missing);
        assert_eq!(row.word, "Not installed");
        assert_eq!(
            row.detail,
            "Worktrees, branches and Source control need Git. Install Git, then restart MassOrca to check again."
        );
    }

    #[test]
    fn a_git_below_the_baseline_names_both_versions_and_the_consequence() {
        let row = row_of(Ok(Some(ServiceGitStatus::TooOld {
            version: "2.20.1".into(),
            required: "2.25.0".into(),
        })));
        assert_eq!(
            row.state,
            GitState::TooOld {
                version: "2.20.1".into(),
                required: "2.25.0".into()
            }
        );
        assert_eq!(row.word, "Too old");
        assert_eq!(
            row.detail,
            "Git 2.20.1 is older than the 2.25.0 that MassOrca needs, so worktrees, branches and Source control stay off. Upgrade Git, then restart MassOrca to check again."
        );
    }

    #[test]
    fn a_failed_check_is_unknown_and_names_the_reason_once() {
        let row = row_of(Ok(Some(ServiceGitStatus::Unknown {
            reason: "git --version timed out.".into(),
        })));
        assert_eq!(
            row.state,
            GitState::Unknown {
                reason: "git --version timed out".into()
            }
        );
        assert_eq!(row.word, "Unknown");
        assert_eq!(
            row.detail,
            "MassOrca could not check Git: git --version timed out. Restart MassOrca to check again."
        );
    }

    #[test]
    fn a_service_that_predates_the_report_is_unknown_until_it_is_updated() {
        let row = git_row(Some(&Ok(None)));
        assert_eq!(row.state, GitState::Unreported);
        assert_eq!(row.word, "Unknown");
        assert_eq!(
            row.detail,
            "This service does not report Git. Update MassOrca on this computer, then restart it to check again."
        );
    }

    #[test]
    fn a_refused_read_is_unknown_and_carries_the_refusal() {
        let refusal = Refusal::unavailable("the service went away");
        let sentence = refusal.sentence();
        let row = row_of(Err(refusal));
        assert_eq!(row.word, "Unknown");
        let reason = sentence.trim_end_matches('.');
        assert_eq!(
            row.detail,
            format!("MassOrca could not check Git: {reason}. Restart MassOrca to check again.")
        );
    }

    #[test]
    fn each_state_has_its_own_key_for_the_row_selector() {
        let keys = [
            GitState::NotChecked.key(),
            GitState::Present {
                version: String::new(),
            }
            .key(),
            GitState::Missing.key(),
            GitState::TooOld {
                version: String::new(),
                required: String::new(),
            }
            .key(),
            GitState::Unknown {
                reason: String::new(),
            }
            .key(),
        ];
        let distinct: std::collections::BTreeSet<&str> = keys.into_iter().collect();
        assert_eq!(distinct.len(), 5);
        assert_eq!(
            GitState::Unreported.key(),
            GitState::Unknown {
                reason: String::new()
            }
            .key()
        );
    }
}
