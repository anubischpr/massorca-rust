//! Fork of a session (decision 0168): a new session that continues a copy of an exited session's conversation, started
//! with the kind's fork form in the source's workspace. The source keeps its own conversation and record.

use std::collections::BTreeMap;

use massorca_protocol::codes;
use massorca_protocol::methods::agents::{AgentForkParams, AgentSession, AgentStartParams};
use massorca_protocol::methods::chat::RunType;
use massorca_protocol::methods::terminals::TerminalState;

use super::{AgentError, Agents, RunStart, SecretGrant};
use crate::resume_state::{self, ResumeInputs};

/// The source of a fork, as the start that makes the new session needs it.
pub(crate) struct ForkFrom {
    pub source_id: String,
    /// The source's agent ref, in place of `{ref}` in the fork form.
    pub source_ref: String,
    /// The source holds vault secrets (decision 0071), so the copy of its conversation may too.
    pub holds_secrets: bool,
    /// The notes of the stored options the fork left out, which its run records (decision 0168).
    pub notes: Vec<String>,
}

impl Agents {
    /// Forks `params.session` (decision 0168) into a new session in the same workspace: a new id, the kind's fork form
    /// with the source's ref, and a new ref learned from the run's reports as a first start learns it. The new session
    /// keeps the source's read-only mark and the model and effort its kind still takes: a stored one the kind no longer
    /// takes is left out and noted, as on a resume. It takes `params.title` or the source's title, and records
    /// `forked_from`. Its first prompt and permission mode are those of a start.
    ///
    /// Refused with `agent.busy` while the source runs: its transcript may be mid-turn, and the fork would write in the
    /// same workspace. `agent.fork_unavailable` when the source's kind declares no fork form, its agent ref is not known,
    /// or its CLI keeps no conversation for the ref; `agent.chat_unsupported` for a chat session. Every check of a start
    /// follows, the one-writer rule among them.
    pub fn fork(
        &self,
        params: AgentForkParams,
        grant: SecretGrant,
    ) -> Result<AgentSession, AgentError> {
        let source = self.resolve(&params.session)?;
        if self.run_type_of(&source.id)? == RunType::Chat {
            return Err(AgentError::new(
                codes::AGENT_CHAT_UNSUPPORTED,
                format!(
                    "session {} is a chat, and chat sessions cannot be forked",
                    source.id
                ),
            ));
        }
        if self.view(source.clone())?.execution.state != TerminalState::Exited {
            return Err(AgentError::new(
                codes::AGENT_BUSY,
                format!("session {} is still running; stop it first", source.id),
            ));
        }
        let source = self.reload(&source.id)?;
        let (decl, _) = self.startable_kind(&source.kind)?;
        if !decl.forkable() {
            return Err(fork_unavailable(
                &source.id,
                &format!("kind {} declares no fork form", source.kind),
            ));
        }
        let source_ref = source
            .agent_ref
            .clone()
            .ok_or_else(|| fork_unavailable(&source.id, "its agent ref is not known"))?;
        // A CLI that saved no conversation for the ref has nothing to copy (decision 0160).
        let state = resume_state::resume_state(&ResumeInputs {
            decl: Ok(&decl),
            agent_ref: Some(&source_ref),
            exited: true,
            profile_dir: self.profile_dir(),
            session_id: &source.id,
            vendor_env: self.inner.config.vendor_env.as_ref(),
            found: Some(&self.inner.found_transcripts),
        });
        if state.code.as_deref() == Some(codes::AGENT_RESUME_NOT_FOUND) {
            return Err(fork_unavailable(
                &source.id,
                state
                    .reason
                    .as_deref()
                    .unwrap_or(resume_state::NOT_FOUND_REASON),
            ));
        }
        let origin = self
            .store()
            .agent_session_origin(&source.id)?
            .unwrap_or_default();
        // The model and effort the source's kind still takes, as a resume passes them (decision 0168).
        let kept = super::kept_options(&decl, &origin);
        let start = AgentStartParams {
            workspace: Some(source.workspace_id.clone()),
            kind: source.kind.clone(),
            prompt: params.prompt,
            args: Vec::new(),
            env: BTreeMap::new(),
            read_only: source.read_only,
            title: params.title.or_else(|| source.title.clone()),
            cols: None,
            rows: None,
            request_id: None,
            permission_mode: params.permission_mode,
            model: kept.model,
            effort: kept.effort,
        };
        let from = ForkFrom {
            source_id: source.id.clone(),
            source_ref,
            holds_secrets: source.holds_secrets,
            notes: kept.notes,
        };
        self.start_session_as(start, None, grant, RunStart::Terminal, Some(&from))
    }
}

fn fork_unavailable(session: &str, reason: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_FORK_UNAVAILABLE,
        format!("session {session} cannot be forked: {reason}"),
    )
}
