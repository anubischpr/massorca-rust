//! The startup preflight: a tool the service reports missing or too old, named once after connect (PLAT-08). The service
//! reports git only (`service.info`), so git is the one tool checked. The GitHub, GitLab, Bitbucket, Azure DevOps and
//! Gitea CLIs, the shells and the agent CLIs are not reported, so they are not checked here.

use massorca_protocol::methods::service::GitStatus;

/// The git baseline the service enforces, as its missing-git message names it (core-git's `GitVersion::BASELINE`).
const GIT_BASELINE: &str = "2.25.0";

/// A notice above the Canvas about a tool that needs attention: what is wrong and what to do about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreflightNotice {
    pub title: String,
    pub message: String,
}

/// The notice for the host's git while connected. None when git is present, when the probe could not run it (nothing
/// is known to be wrong), and when the service reports no git at all.
pub fn preflight_notice(connected: bool, git: Option<&GitStatus>) -> Option<PreflightNotice> {
    if !connected {
        return None;
    }
    match git? {
        GitStatus::Missing => Some(PreflightNotice {
            title: "Git is not installed".to_owned(),
            message: format!(
                "MassOrca needs Git {GIT_BASELINE} or newer. Install Git, or set MASSORCA_GIT to its path, then restart the service to check again."
            ),
        }),
        GitStatus::TooOld { version, required } => Some(PreflightNotice {
            title: format!("Git {version} is too old"),
            message: format!(
                "MassOrca needs Git {required} or newer. Update Git, then restart the service to check again."
            ),
        }),
        GitStatus::Present { .. } | GitStatus::Unknown { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{DataSource, SampleDataSource};
    use crate::model::shell_state::ShellModel;

    fn missing() -> PreflightNotice {
        PreflightNotice {
            title: "Git is not installed".to_owned(),
            message: "install".to_owned(),
        }
    }

    fn too_old() -> PreflightNotice {
        PreflightNotice {
            title: "Git 2.20.1 is too old".to_owned(),
            message: "update".to_owned(),
        }
    }

    fn with_notice(notice: Option<PreflightNotice>) -> ShellModel {
        let mut snapshot = SampleDataSource.snapshot();
        snapshot.preflight = notice;
        ShellModel::new(snapshot, 1440.0)
    }

    #[test]
    fn a_missing_git_is_named_with_what_to_do() {
        let notice = preflight_notice(true, Some(&GitStatus::Missing)).expect("a notice");
        assert_eq!(notice.title, "Git is not installed");
        assert!(
            notice.message.contains("2.25.0 or newer"),
            "{}",
            notice.message
        );
        assert!(
            notice.message.contains("MASSORCA_GIT"),
            "{}",
            notice.message
        );
    }

    #[test]
    fn a_too_old_git_names_its_version_and_the_one_required() {
        let git = GitStatus::TooOld {
            version: "2.20.1".to_owned(),
            required: "2.25.0".to_owned(),
        };
        let notice = preflight_notice(true, Some(&git)).expect("a notice");
        assert_eq!(notice.title, "Git 2.20.1 is too old");
        assert!(
            notice.message.contains("2.25.0 or newer"),
            "{}",
            notice.message
        );
    }

    #[test]
    fn present_unknown_and_unreported_git_draw_no_notice() {
        let present = GitStatus::Present {
            version: "2.49.0".to_owned(),
        };
        let unknown = GitStatus::Unknown {
            reason: "timed out".to_owned(),
        };
        assert_eq!(preflight_notice(true, Some(&present)), None);
        assert_eq!(preflight_notice(true, Some(&unknown)), None);
        assert_eq!(
            preflight_notice(true, None),
            None,
            "a service that predates git"
        );
    }

    #[test]
    fn nothing_is_named_while_the_link_is_down() {
        assert_eq!(preflight_notice(false, Some(&GitStatus::Missing)), None);
    }

    #[test]
    fn a_dismissed_notice_stays_hidden_until_the_service_reports_a_different_problem() {
        let mut model = with_notice(Some(missing()));
        assert_eq!(model.preflight(), Some(&missing()));
        model.dismiss_preflight();
        assert_eq!(model.preflight(), None, "dismissed");

        let mut snapshot = SampleDataSource.snapshot();
        snapshot.preflight = Some(missing());
        model.set_snapshot(snapshot);
        assert_eq!(
            model.preflight(),
            None,
            "the same problem on a later snapshot"
        );

        let mut snapshot = SampleDataSource.snapshot();
        snapshot.preflight = Some(too_old());
        model.set_snapshot(snapshot);
        assert_eq!(model.preflight(), Some(&too_old()), "a different problem");
    }

    #[test]
    fn a_fixed_git_clears_the_notice_and_a_later_problem_shows_again() {
        let mut model = with_notice(Some(missing()));
        model.dismiss_preflight();
        model.set_snapshot(SampleDataSource.snapshot());
        assert_eq!(model.preflight(), None, "fixed");

        let mut snapshot = SampleDataSource.snapshot();
        snapshot.preflight = Some(missing());
        model.set_snapshot(snapshot);
        assert_eq!(model.preflight(), Some(&missing()), "broken again");
    }
}
