//! The setup checklist as plain data (D-OB-12): the eight steps of first run, each with the state the
//! core can show for it. A step the core cannot satisfy yet says so rather than claim a result.
//! Free of GPUI, so the rules test without a window.

use super::settings_sections::CONVERSATIONS_FOLDER;
use crate::service::StoreState;

/// What a step shows: a result, a warning, a step still to do, or a step that waits for a layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Done,
    NotStarted,
    Warning,
    NotAvailable,
}

/// One row of the checklist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetupRow {
    pub title: &'static str,
    pub detail: String,
    pub step: Step,
}

/// The checklist for the state the store holds and the theme in use (`Dark` or `Light`).
pub fn checklist(state: &StoreState, theme: &'static str) -> Vec<SetupRow> {
    let connected = state.connection.is_connected();
    let projects = state.projects.len();
    let row = |title, detail: String, step| SetupRow {
        title,
        detail,
        step,
    };
    vec![
        row(
            "Agent accounts",
            "Agent sign-in is not available yet.".to_owned(),
            Step::NotAvailable,
        ),
        row(
            "Add a host",
            if connected {
                "This PC is connected.".to_owned()
            } else {
                "The service is not connected.".to_owned()
            },
            if connected { Step::Done } else { Step::Warning },
        ),
        row(
            "Create a workspace",
            match projects {
                0 => "No project yet.".to_owned(),
                1 => "1 project on This PC.".to_owned(),
                n => format!("{n} projects on This PC."),
            },
            if projects == 0 {
                Step::NotStarted
            } else {
                Step::Done
            },
        ),
        row(
            "Conversations without a project",
            format!(
                "Saved in {CONVERSATIONS_FOLDER} on This PC. Changing it is not available yet."
            ),
            Step::NotAvailable,
        ),
        row(
            "Import sessions",
            "Session import is not available yet.".to_owned(),
            Step::NotAvailable,
        ),
        row("Theme", format!("{theme} is in use."), Step::Done),
        row(
            "Notifications",
            "Notification settings are not available yet.".to_owned(),
            Step::NotAvailable,
        ),
        row(
            "Usage data",
            "Usage data settings are not available yet.".to_owned(),
            Step::NotAvailable,
        ),
    ]
}

/// How many steps are done, out of the whole checklist.
pub fn done_count(rows: &[SetupRow]) -> usize {
    rows.iter().filter(|row| row.step == Step::Done).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::ConnectionState;
    use crate::service::fixtures::project;

    fn connected() -> ConnectionState {
        ConnectionState::Connected {
            server: "massorca".into(),
            version: "0.1.0".into(),
        }
    }

    #[test]
    fn the_checklist_has_eight_steps_in_the_spec_order() {
        let rows = checklist(&StoreState::default(), "Dark");
        let titles: Vec<&str> = rows.iter().map(|row| row.title).collect();
        assert_eq!(titles.len(), 8);
        assert_eq!(titles[0], "Agent accounts");
        assert_eq!(titles[7], "Usage data");
    }

    #[test]
    fn a_connected_service_with_a_project_completes_those_steps() {
        let state = StoreState {
            connection: connected(),
            projects: vec![project("p1", "alpha"), project("p2", "beta")],
            ..StoreState::default()
        };
        let rows = checklist(&state, "Dark");
        assert_eq!(rows[1].step, Step::Done);
        assert_eq!(rows[1].detail, "This PC is connected.");
        assert_eq!(rows[2].step, Step::Done);
        assert_eq!(rows[2].detail, "2 projects on This PC.");
        assert_eq!(done_count(&rows), 3, "the host, the project and the theme");
    }

    #[test]
    fn without_a_connection_or_a_project_the_steps_say_what_is_missing() {
        let rows = checklist(&StoreState::default(), "Light");
        assert_eq!(rows[1].step, Step::Warning);
        assert_eq!(rows[1].detail, "The service is not connected.");
        assert_eq!(rows[2].step, Step::NotStarted);
        assert_eq!(rows[2].detail, "No project yet.");
        assert_eq!(rows[5].detail, "Light is in use.");
        assert_eq!(done_count(&rows), 1, "only the theme");
    }

    #[test]
    fn steps_the_core_cannot_satisfy_are_never_done() {
        let rows = checklist(&StoreState::default(), "Dark");
        for index in [0, 3, 4, 6, 7] {
            assert_eq!(
                rows[index].step,
                Step::NotAvailable,
                "{}",
                rows[index].title
            );
        }
        assert!(rows[3].detail.contains("~/MassOrca/Conversations"));
    }
}
