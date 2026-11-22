//! Human-readable lines for agent kinds and sessions.

use massorca_protocol::methods::agents::{
    Activity, ActivityState, AgentKind, AgentSession, Execution, Outcome,
};

use crate::terminal;
use crate::workspace_text;

/// The kinds as a table, then one line per problem of a kind that has any.
pub fn kinds_lines(kinds: &[AgentKind]) -> Vec<String> {
    if kinds.is_empty() {
        return vec!["no agent kinds".to_owned()];
    }
    let rows = kinds
        .iter()
        .map(|kind| {
            vec![
                kind.id.clone(),
                kind.name.clone(),
                kind.owner.clone(),
                installed_text(kind),
            ]
        })
        .collect();
    let mut lines = workspace_text::table(&["ID", "NAME", "OWNER", "INSTALLED"], rows);
    for kind in kinds {
        for problem in &kind.problems {
            lines.push(format!("problem: {}: {problem}", kind.id));
        }
    }
    lines
}

/// The resolved program, `invalid` for a declaration with problems, or `no`.
fn installed_text(kind: &AgentKind) -> String {
    match (&kind.program_path, kind.problems.is_empty()) {
        (Some(path), _) if kind.installed => path.clone(),
        (_, false) => "invalid".to_owned(),
        _ => "no".to_owned(),
    }
}

/// The fields of one session, one per line. The result text is not printed; `--json` carries it.
pub fn session_details(session: &AgentSession) -> Vec<String> {
    let mut lines = vec![
        format!("id: {}", session.id),
        format!("kind: {}", session.kind),
        format!("workspace: {}", session.workspace_id),
        format!("execution: {}", execution_text(&session.execution)),
        format!("activity: {}", activity_text(session.activity.as_ref())),
    ];
    if let Some(title) = &session.title {
        lines.push(format!("title: {title}"));
    }
    if session.read_only {
        lines.push("read only: yes".to_owned());
    }
    if let Some(terminal) = &session.terminal_id {
        lines.push(format!("terminal: {terminal}"));
    }
    if let Some(agent_ref) = &session.agent_ref {
        lines.push(format!("agent ref: {agent_ref}"));
    }
    if let Some(parent) = &session.parent_id {
        lines.push(format!("parent: {parent}"));
    }
    lines
}

/// The sessions as a table, newest first.
pub fn sessions_lines(sessions: &[AgentSession]) -> Vec<String> {
    if sessions.is_empty() {
        return vec!["no agent sessions".to_owned()];
    }
    let rows = sessions
        .iter()
        .map(|session| {
            vec![
                session.id.clone(),
                session.kind.clone(),
                execution_text(&session.execution),
                activity_text(session.activity.as_ref()),
                session.title.clone().unwrap_or_default(),
            ]
        })
        .collect();
    workspace_text::table(&["ID", "KIND", "EXECUTION", "ACTIVITY", "TITLE"], rows)
}

/// `live`, `unverifiable`, or `exited (killed)` with how the run ended.
pub fn execution_text(execution: &Execution) -> String {
    let state = terminal::state_text(execution.state);
    match &execution.exit {
        Some(exit) => format!("{state} ({})", terminal::exit_text(exit)),
        None => state.to_owned(),
    }
}

/// The reported activity, or `no status` when the agent has reported nothing.
pub fn activity_text(activity: Option<&Activity>) -> String {
    let Some(activity) = activity else {
        return "no status".to_owned();
    };
    let state = match activity.state {
        ActivityState::Working => "working",
        ActivityState::Waiting => "waiting",
        ActivityState::Idle => "idle",
        ActivityState::Done => "done",
        ActivityState::Unknown => "unknown",
    };
    match activity.outcome {
        Some(outcome) if activity.state == ActivityState::Done => {
            let outcome = match outcome {
                Outcome::Success => "success",
                Outcome::Failure => "failure",
                Outcome::Cancelled => "cancelled",
                Outcome::Interrupted => "interrupted",
                Outcome::Unknown => "unknown",
            };
            format!("{state} ({outcome})")
        }
        _ => state.to_owned(),
    }
}
