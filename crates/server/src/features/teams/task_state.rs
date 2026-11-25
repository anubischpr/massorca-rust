//! The state a task shows, derived each time from the task's own record and the agent session it names (decision 0072).
//! A task stores its lifecycle only; the session's activity and execution are read from the agents domain.

use massorca_core_agents::Agents;
use massorca_core_store::{TaskOutcome, TaskRow};
use massorca_protocol::methods::agents::{ActivityState, AgentSession, Outcome};
use massorca_protocol::methods::tasks::{CardState, Column};
use massorca_protocol::methods::terminals::TerminalState;
use massorca_protocol::{ErrorBody, codes};

/// What the agents domain says about a task's session.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Session {
    /// The task never started; it names no session.
    Unstarted,
    /// The task names a session the agents domain no longer has.
    Forgotten,
    /// The session as the agents domain reports it now.
    Known(Box<AgentSession>),
}

/// Where a task stands: its card state, its Board column, and whether the state is the last one the agent reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Standing {
    pub(crate) state: CardState,
    pub(crate) column: Column,
    pub(crate) last_known: bool,
}

/// Reads the session a task names. A session the agents domain has forgotten is `Forgotten`, not an error.
pub(crate) fn load(agents: &Agents, task: &TaskRow) -> Result<Session, ErrorBody> {
    match task.session_id.as_deref() {
        Some(session_id) => load_id(agents, session_id),
        None => Ok(Session::Unstarted),
    }
}

/// Reads one agent session. A session the agents domain has forgotten is `Forgotten`, not an error.
pub(crate) fn load_id(agents: &Agents, session_id: &str) -> Result<Session, ErrorBody> {
    match agents.show(session_id) {
        Ok(session) => Ok(Session::Known(Box::new(session))),
        Err(error) if error.code == codes::AGENT_NOT_FOUND => Ok(Session::Forgotten),
        Err(error) => Err(ErrorBody::from(error)),
    }
}

/// Whether the task's agent session still runs. An unverifiable run counts as running: it cannot be told from a live one.
pub(crate) fn is_running(session: &Session) -> bool {
    matches!(session, Session::Known(session) if session.execution.state != TerminalState::Exited)
}

/// The standing of a task: its own latched outcome decides first, then its session.
pub(crate) fn standing(task: &TaskRow, session: &Session) -> Standing {
    if let Some(outcome) = task.outcome {
        return of_outcome(outcome_of(outcome));
    }
    match session {
        Session::Unstarted => Standing {
            state: CardState::NotStarted,
            column: Column::Open,
            last_known: false,
        },
        Session::Forgotten => stopped(false),
        Session::Known(session) => of_session(session),
    }
}

/// The protocol name of a latched outcome.
pub(crate) fn outcome_of(outcome: TaskOutcome) -> Outcome {
    match outcome {
        TaskOutcome::Success => Outcome::Success,
        TaskOutcome::Failure => Outcome::Failure,
        TaskOutcome::Cancelled => Outcome::Cancelled,
        TaskOutcome::Interrupted => Outcome::Interrupted,
    }
}

/// The latched form of a protocol outcome. `Unknown` latches nothing: an outcome never observed is no result.
pub(crate) fn latched_outcome(outcome: Outcome) -> Option<TaskOutcome> {
    match outcome {
        Outcome::Success => Some(TaskOutcome::Success),
        Outcome::Failure => Some(TaskOutcome::Failure),
        Outcome::Cancelled => Some(TaskOutcome::Cancelled),
        Outcome::Interrupted => Some(TaskOutcome::Interrupted),
        Outcome::Unknown => None,
    }
}

/// The outcome a session latches into its task: a Done with a known outcome, from a run the host can see. Loss of
/// contact latches nothing, because an unverifiable run is not observed.
pub(crate) fn observed_outcome(session: &AgentSession) -> Option<TaskOutcome> {
    if !matches!(
        session.execution.state,
        TerminalState::Live | TerminalState::Exited
    ) {
        return None;
    }
    let activity = session.activity.as_ref()?;
    if activity.state != ActivityState::Done {
        return None;
    }
    latched_outcome(activity.outcome?)
}

fn of_session(session: &AgentSession) -> Standing {
    if let Some(activity) = &session.activity
        && activity.state == ActivityState::Done
    {
        return of_outcome(activity.outcome.unwrap_or(Outcome::Unknown));
    }
    if session.execution.state == TerminalState::Exited {
        return stopped(false);
    }
    let last_known = session.execution.state == TerminalState::Unverifiable;
    let (state, column) = match session.activity.as_ref().map(|activity| activity.state) {
        None | Some(ActivityState::Working) => (CardState::Working, Column::Running),
        Some(ActivityState::Waiting | ActivityState::Idle) => (CardState::Waiting, Column::Waiting),
        Some(ActivityState::Done | ActivityState::Unknown) => (CardState::Unknown, Column::Waiting),
    };
    Standing {
        state,
        column,
        last_known,
    }
}

fn of_outcome(outcome: Outcome) -> Standing {
    let (state, column) = match outcome {
        Outcome::Success => (CardState::Done, Column::Done),
        Outcome::Failure => (CardState::Stuck, Column::Waiting),
        Outcome::Cancelled | Outcome::Interrupted => (CardState::Stopped, Column::Waiting),
        Outcome::Unknown => (CardState::Unknown, Column::Waiting),
    };
    Standing {
        state,
        column,
        last_known: false,
    }
}

fn stopped(last_known: bool) -> Standing {
    Standing {
        state: CardState::Stopped,
        column: Column::Waiting,
        last_known,
    }
}

#[cfg(test)]
mod tests {
    use massorca_core_store::{TaskOrigin, TaskOutcome, TaskRow};
    use massorca_protocol::methods::agents::{
        Activity, ActivitySource, ActivityState, AgentSession, Execution, Outcome,
    };
    use massorca_protocol::methods::tasks::{CardState, Column};
    use massorca_protocol::methods::terminals::TerminalState;

    use super::{Session, Standing, is_running, observed_outcome, standing};

    fn task(outcome: Option<TaskOutcome>) -> TaskRow {
        TaskRow {
            id: "k1".to_owned(),
            team_id: "t1".to_owned(),
            project_id: None,
            bot_id: None,
            title: "parse".to_owned(),
            body: String::new(),
            origin: TaskOrigin::Cli,
            session_id: Some("s1".to_owned()),
            outcome,
            error: None,
            error_code: None,
            workspace_id: None,
            branch: None,
            created_at: 1,
            updated_at: 2,
        }
    }

    fn session(
        execution: TerminalState,
        activity: Option<(ActivityState, Option<Outcome>)>,
    ) -> Session {
        Session::Known(Box::new(record(execution, activity)))
    }

    fn record(
        execution: TerminalState,
        activity: Option<(ActivityState, Option<Outcome>)>,
    ) -> AgentSession {
        AgentSession {
            id: "s1".to_owned(),
            kind: "claude".to_owned(),
            workspace_id: "w1".to_owned(),
            title: None,
            read_only: false,
            parent_id: None,
            brought_back_at_ms: None,
            agent_ref: None,
            terminal_id: None,
            run: Default::default(),
            created_at_ms: 1,
            execution: Execution {
                state: execution,
                exit: None,
            },
            activity: activity.map(|(state, outcome)| Activity {
                state,
                outcome,
                source: ActivitySource::Hook,
                at_ms: 2,
                seen_at_ms: None,
            }),
            result: None,
            holds_secrets: false,
            secrets_withheld: None,
            permission_mode: None,
            mode_confirmed: false,
            mode_source: None,
            pending_permission_mode: None,
            retained: false,
            released: false,
            resume: None,
            delegation: None,
            notes: Vec::new(),
        }
    }

    fn at(task: &TaskRow, session: &Session) -> (CardState, Column, bool) {
        let Standing {
            state,
            column,
            last_known,
        } = standing(task, session);
        (state, column, last_known)
    }

    #[test]
    fn a_task_that_never_started_is_not_started_in_the_open_column() {
        assert_eq!(
            at(&task(None), &Session::Unstarted),
            (CardState::NotStarted, Column::Open, false)
        );
    }

    #[test]
    fn a_task_whose_session_is_forgotten_without_an_outcome_is_stopped() {
        assert_eq!(
            at(&task(None), &Session::Forgotten),
            (CardState::Stopped, Column::Waiting, false)
        );
    }

    #[test]
    fn a_latched_outcome_decides_before_the_session_does() {
        let working = session(TerminalState::Live, Some((ActivityState::Working, None)));
        assert_eq!(
            at(&task(Some(TaskOutcome::Failure)), &working),
            (CardState::Stuck, Column::Waiting, false)
        );
        assert_eq!(
            at(&task(Some(TaskOutcome::Success)), &working),
            (CardState::Done, Column::Done, false)
        );
        assert_eq!(
            at(&task(Some(TaskOutcome::Cancelled)), &Session::Forgotten),
            (CardState::Stopped, Column::Waiting, false)
        );
    }

    #[test]
    fn a_live_session_shows_working_until_it_waits_or_is_idle() {
        let no_activity = session(TerminalState::Live, None);
        let working = session(TerminalState::Live, Some((ActivityState::Working, None)));
        let waiting = session(TerminalState::Live, Some((ActivityState::Waiting, None)));
        let idle = session(TerminalState::Live, Some((ActivityState::Idle, None)));

        assert_eq!(
            at(&task(None), &no_activity),
            (CardState::Working, Column::Running, false)
        );
        assert_eq!(
            at(&task(None), &working),
            (CardState::Working, Column::Running, false)
        );
        assert_eq!(
            at(&task(None), &waiting),
            (CardState::Waiting, Column::Waiting, false)
        );
        assert_eq!(
            at(&task(None), &idle),
            (CardState::Waiting, Column::Waiting, false)
        );
    }

    #[test]
    fn a_done_session_shows_its_outcome() {
        let done = |outcome| {
            session(
                TerminalState::Live,
                Some((ActivityState::Done, Some(outcome))),
            )
        };

        assert_eq!(
            at(&task(None), &done(Outcome::Success)),
            (CardState::Done, Column::Done, false)
        );
        assert_eq!(
            at(&task(None), &done(Outcome::Failure)),
            (CardState::Stuck, Column::Waiting, false)
        );
        assert_eq!(
            at(&task(None), &done(Outcome::Interrupted)),
            (CardState::Stopped, Column::Waiting, false)
        );
    }

    #[test]
    fn an_exited_run_without_a_done_turn_is_stopped() {
        let exited = session(TerminalState::Exited, Some((ActivityState::Working, None)));

        assert_eq!(
            at(&task(None), &exited),
            (CardState::Stopped, Column::Waiting, false)
        );
    }

    #[test]
    fn an_unverifiable_run_keeps_its_last_state_marked_as_known() {
        let unverifiable = session(
            TerminalState::Unverifiable,
            Some((ActivityState::Waiting, None)),
        );

        assert_eq!(
            at(&task(None), &unverifiable),
            (CardState::Waiting, Column::Waiting, true)
        );
    }

    #[test]
    fn only_a_run_that_has_not_exited_counts_as_running() {
        let live = session(TerminalState::Live, None);
        let unverifiable = session(TerminalState::Unverifiable, None);
        let exited = session(TerminalState::Exited, None);

        assert!(is_running(&live));
        assert!(is_running(&unverifiable));
        assert!(!is_running(&exited));
        assert!(!is_running(&Session::Forgotten));
        assert!(!is_running(&Session::Unstarted));
    }

    #[test]
    fn a_done_with_an_outcome_from_a_run_the_host_can_see_latches_that_outcome() {
        let done = |execution: TerminalState, outcome: Outcome| {
            observed_outcome(&record(
                execution,
                Some((ActivityState::Done, Some(outcome))),
            ))
        };
        assert_eq!(
            done(TerminalState::Live, Outcome::Success),
            Some(TaskOutcome::Success)
        );
        assert_eq!(
            done(TerminalState::Live, Outcome::Failure),
            Some(TaskOutcome::Failure)
        );
        assert_eq!(
            done(TerminalState::Live, Outcome::Cancelled),
            Some(TaskOutcome::Cancelled)
        );
        assert_eq!(
            done(TerminalState::Exited, Outcome::Interrupted),
            Some(TaskOutcome::Interrupted)
        );
    }

    #[test]
    fn loss_of_contact_latches_nothing_even_when_the_last_report_was_done() {
        let unverifiable = record(
            TerminalState::Unverifiable,
            Some((ActivityState::Done, Some(Outcome::Failure))),
        );
        assert_eq!(observed_outcome(&unverifiable), None);
        let working = record(
            TerminalState::Unverifiable,
            Some((ActivityState::Working, None)),
        );
        assert_eq!(observed_outcome(&working), None);
    }

    #[test]
    fn a_done_without_a_known_outcome_or_a_turn_that_is_not_done_latches_nothing() {
        let live = |activity| observed_outcome(&record(TerminalState::Live, activity));
        assert_eq!(live(Some((ActivityState::Done, None))), None);
        assert_eq!(
            live(Some((ActivityState::Done, Some(Outcome::Unknown)))),
            None
        );
        assert_eq!(live(Some((ActivityState::Working, None))), None);
        assert_eq!(live(None), None);
    }
}
