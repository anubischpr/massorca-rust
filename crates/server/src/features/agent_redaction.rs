//! Agent-produced text of a run that holds vault secrets is for host-admin callers only (decision 0071): the agent may
//! print a secret, and setting one needs host-admin scope. Lower scopes still see the run, its state and its mark.
//! `agent.changed` carries a session id only, so it needs no redaction.

use massorca_protocol::methods::agents::{AgentSession, AgentWatchEvent};

use crate::context::RequestCtx;
use crate::permission::{Permission, authorize};

pub(super) fn is_host_admin(ctx: &RequestCtx) -> bool {
    authorize(&ctx.principal, Permission::HostAdmin).is_ok()
}

/// The session as this caller may see it.
pub(super) fn for_caller(ctx: &RequestCtx, session: AgentSession) -> AgentSession {
    redacted(is_host_admin(ctx), session)
}

fn redacted(admin: bool, mut session: AgentSession) -> AgentSession {
    if session.holds_secrets && !admin {
        session.result = None;
        // A preview is agent-produced text too (decision 0168).
        session.meta.preview = None;
    }
    session
}

/// An `agent.watch` item as its subscriber may see it, by the subscriber's scope.
pub(super) fn watch_item_for(admin: bool, item: AgentWatchEvent) -> AgentWatchEvent {
    match item {
        AgentWatchEvent::Snapshot { sessions } => AgentWatchEvent::Snapshot {
            sessions: sessions
                .into_iter()
                .map(|session| redacted(admin, session))
                .collect(),
        },
        AgentWatchEvent::Changed { session } => AgentWatchEvent::Changed {
            session: Box::new(redacted(admin, *session)),
        },
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use massorca_protocol::methods::agents::{
        AgentSession, AgentWatchEvent, Execution, SessionMeta,
    };
    use massorca_protocol::methods::terminals::TerminalState;

    use super::watch_item_for;

    fn session(holds_secrets: bool) -> AgentSession {
        AgentSession {
            id: "s1".to_owned(),
            kind: "fake".to_owned(),
            workspace_id: "w1".to_owned(),
            created_at_ms: 1,
            execution: Execution {
                state: TerminalState::Exited,
                exit: None,
            },
            result: Some("agent-output-placeholder".to_owned()),
            holds_secrets,
            meta: SessionMeta {
                preview: Some("agent-output-placeholder".to_owned()),
                ..SessionMeta::default()
            },
            ..Default::default()
        }
    }

    fn changed(item: AgentWatchEvent) -> AgentSession {
        match item {
            AgentWatchEvent::Changed { session } => *session,
            other => panic!("expected a change, got {other:?}"),
        }
    }

    #[test]
    fn a_changed_item_drops_the_text_of_a_marked_run_for_a_lower_scope_only() {
        let marked = || AgentWatchEvent::Changed {
            session: Box::new(session(true)),
        };
        let lower = changed(watch_item_for(false, marked()));
        assert_eq!(
            (lower.result, lower.meta.preview, lower.holds_secrets),
            (None, None, true)
        );
        assert_eq!(changed(watch_item_for(true, marked())), session(true));
        let unmarked = AgentWatchEvent::Changed {
            session: Box::new(session(false)),
        };
        assert_eq!(changed(watch_item_for(false, unmarked)), session(false));
    }
}
