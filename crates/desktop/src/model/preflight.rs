//! The startup preflight notice (PLAT-08): this computer's git when it is missing or too old, shown above the Canvas
//! after connect. The state and the words come from the Git row of Settings (GIT-01, `git_availability`), so the notice
//! and the row name the same thing the same way. This file only picks which states reach the Canvas.

use massorca_protocol::methods::service::GitStatus as ServiceGitStatus;

use super::git_availability::{GitState, git_row};
use crate::service::Refusal;

/// A notice above the Canvas: the Git row's state word as its title, and its sentence as the message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreflightNotice {
    pub title: String,
    pub message: String,
}

/// The notice for this computer's git while connected. Only a git that is missing or below the baseline reaches the
/// Canvas; present, unknown, unreported, refused and not yet answered git stay in Settings.
pub fn preflight_notice(
    connected: bool,
    answer: Option<&Result<Option<ServiceGitStatus>, Refusal>>,
) -> Option<PreflightNotice> {
    if !connected {
        return None;
    }
    let row = git_row(answer);
    match row.state {
        GitState::Missing | GitState::TooOld { .. } => Some(PreflightNotice {
            title: row.word,
            message: row.detail,
        }),
        GitState::NotChecked
        | GitState::Present { .. }
        | GitState::Unknown { .. }
        | GitState::Unreported => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{DataSource, SampleDataSource};
    use crate::model::shell_state::ShellModel;

    fn missing() -> PreflightNotice {
        PreflightNotice {
            title: "Not installed".to_owned(),
            message: "Install Git, then restart MassOrca to check again.".to_owned(),
        }
    }

    fn too_old() -> PreflightNotice {
        PreflightNotice {
            title: "Too old".to_owned(),
            message: "Upgrade Git, then restart MassOrca to check again.".to_owned(),
        }
    }

    fn with_notice(notice: Option<PreflightNotice>) -> ShellModel {
        let mut snapshot = SampleDataSource.snapshot();
        snapshot.preflight = notice;
        ShellModel::new(snapshot, 1440.0)
    }

    #[test]
    fn a_missing_git_reaches_the_canvas_with_the_settings_row_words() {
        let answer = Ok(Some(ServiceGitStatus::Missing));
        let notice = preflight_notice(true, Some(&answer)).expect("a notice");
        let row = git_row(Some(&answer));
        assert_eq!(notice.title, row.word);
        assert_eq!(notice.message, row.detail);
        assert_eq!(notice.title, "Not installed");
        assert!(
            notice.message.contains("restart MassOrca"),
            "{}",
            notice.message
        );
    }

    #[test]
    fn a_too_old_git_reaches_the_canvas_and_names_both_versions() {
        let answer = Ok(Some(ServiceGitStatus::TooOld {
            version: "2.20.1".to_owned(),
            required: "2.25.0".to_owned(),
        }));
        let notice = preflight_notice(true, Some(&answer)).expect("a notice");
        assert_eq!(notice.title, "Too old");
        assert!(notice.message.contains("2.20.1"), "{}", notice.message);
        assert!(notice.message.contains("2.25.0"), "{}", notice.message);
        assert!(
            notice.message.contains("restart MassOrca"),
            "{}",
            notice.message
        );
    }

    #[test]
    fn present_unknown_unreported_refused_and_unanswered_git_draw_no_notice() {
        let present = Ok(Some(ServiceGitStatus::Present {
            version: "2.49.0".to_owned(),
        }));
        let unknown = Ok(Some(ServiceGitStatus::Unknown {
            reason: "timed out".to_owned(),
        }));
        let refused = Err(Refusal::unavailable("the service went away"));
        assert_eq!(preflight_notice(true, Some(&present)), None);
        assert_eq!(preflight_notice(true, Some(&unknown)), None);
        assert_eq!(preflight_notice(true, Some(&Ok(None))), None, "unreported");
        assert_eq!(preflight_notice(true, Some(&refused)), None, "refused");
        assert_eq!(preflight_notice(true, None), None, "not checked yet");
    }

    #[test]
    fn nothing_is_named_while_the_link_is_down() {
        let answer = Ok(Some(ServiceGitStatus::Missing));
        assert_eq!(preflight_notice(false, Some(&answer)), None);
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
