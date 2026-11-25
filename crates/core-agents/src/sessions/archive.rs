//! Archive (hide) and delete of a session (decision 0163).
//!
//! Archiving changes nothing but the list: the record, the terminal (pinned, so `max_exited` eviction skips it as it skips
//! a retained session's) and the workspace stay, and only `agent.list` leaves the session out unless asked. No sweep
//! removes an archived session's terminal; an explicit `agent.release` still archives its output and closes it, and an
//! explicit `agent.forget` deletes it. Deleting a session with its workspace runs every check first (the forget's, then
//! that nothing else uses the workspace: no other session, team, task or running terminal, then the workspace removal's
//! own refusals), so a refusal leaves everything as it was. The service then forgets the session, and
//! [`Agents::begin_workspace_removal`] checks the other users again under the start gate and the workspace's mutation
//! lock as it marks the workspace `removing`, after which no start, resume or terminal may begin in it. The preview reads
//! the same checks and changes nothing.

use massorca_core_store::now_ms;
use massorca_core_terminal::TerminalError;
use massorca_core_workspace::{RemovalPreview, RemovalWaivers};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    AgentForgetPreview, AgentSession, ForgetPreviewWorkspace, MAX_TITLE_CHARS,
};
use massorca_protocol::methods::terminals::TerminalState;
use massorca_protocol::methods::workspaces::Workspace;

use super::{AgentError, Agents, Change, lock};

/// What deleting a session with its workspace would meet, read without changing anything.
struct DeletionReading {
    session: AgentSession,
    /// Why the session cannot be forgotten now.
    forget_refusal: Option<AgentError>,
    users: Users,
    workspace: RemovalPreview,
}

impl DeletionReading {
    /// The refusal of the workspace removal: another user first, then the removal's own refusal.
    fn workspace_refusal(&self) -> Option<AgentError> {
        self.users
            .refusal(&self.workspace.workspace.id)
            .or_else(|| self.workspace.refusal.clone().map(AgentError::from))
    }
}

/// What else uses a workspace besides the session being deleted (decision 0163).
#[derive(Debug, Default)]
struct Users {
    /// Other sessions: live, exited or archived.
    sessions: Vec<String>,
    /// Teams and tasks whose `workspace_id` it is.
    teams: Vec<String>,
    tasks: Vec<String>,
    /// Terminals running in it.
    terminals: Vec<String>,
}

impl Users {
    fn refusal(&self, workspace: &str) -> Option<AgentError> {
        if !self.sessions.is_empty() {
            return Some(AgentError::new(
                codes::AGENT_BUSY,
                format!(
                    "workspace {workspace} is used by other sessions ({}); forget them first, or forget this session without removing the workspace",
                    self.sessions.join(", ")
                ),
            ));
        }
        let mut named = Vec::new();
        for (what, ids) in [
            ("teams", &self.teams),
            ("tasks", &self.tasks),
            ("running terminals", &self.terminals),
        ] {
            if !ids.is_empty() {
                named.push(format!("{what} {}", ids.join(", ")));
            }
        }
        (!named.is_empty()).then(|| {
            AgentError::new(
                codes::WORKSPACE_BUSY,
                format!(
                    "workspace {workspace} is still used by {}; end or move them first, or forget this session without removing the workspace",
                    named.join("; ")
                ),
            )
        })
    }
}

impl Agents {
    /// Archives an exited session, or clears its archive (decision 0163). A live run is refused with `agent.busy`. An
    /// archived session's terminal is pinned, so eviction keeps its output; clearing the archive unpins it unless the
    /// session is retained. Archiving an archived session keeps its first archive time.
    pub fn set_archived(&self, selector: &str, archived: bool) -> Result<AgentSession, AgentError> {
        let id = self.resolve(selector)?.id;
        let mut live = lock(&self.inner.status);
        let row = self.reload(&id)?;
        let current = self.view_with(&mut live, row.clone())?;
        if archived && current.execution.state != TerminalState::Exited {
            return Err(still_running(&id));
        }
        // Pinned before the flag is saved, so a failed pin leaves the session as it was.
        if let Some(terminal) = &row.terminal_id {
            match self
                .inner
                .config
                .terminals
                .pin(terminal, archived || row.retained)
            {
                // A terminal the manager no longer has has nothing left to keep.
                Ok(()) | Err(TerminalError::NotFound) => {}
                Err(error) => return Err(error.into()),
            }
        }
        if archived != current.archived {
            self.store()
                .set_agent_session_archived(&id, archived.then(now_ms))?;
        }
        let row = self.reload(&id)?;
        let session = self.view_with(&mut live, row)?;
        self.notify(Change::Updated(Box::new(session.clone())));
        Ok(session)
    }

    /// Sets a live or exited session's title (decision 0163): trimmed, 1 to [`MAX_TITLE_CHARS`] characters without
    /// control characters, else `request.invalid_params`. Every reader gets the change.
    pub fn rename(&self, selector: &str, title: &str) -> Result<AgentSession, AgentError> {
        let title = title.trim();
        let count = title.chars().count();
        if count == 0 || count > MAX_TITLE_CHARS || title.chars().any(char::is_control) {
            return Err(AgentError::new(
                codes::REQUEST_INVALID_PARAMS,
                format!(
                    "a session title is 1 to {MAX_TITLE_CHARS} characters without control characters"
                ),
            ));
        }
        let id = self.resolve(selector)?.id;
        let mut live = lock(&self.inner.status);
        self.store().set_agent_session_title(&id, title)?;
        let row = self.reload(&id)?;
        let session = self.view_with(&mut live, row)?;
        self.notify(Change::Updated(Box::new(session.clone())));
        Ok(session)
    }

    /// Sessions newest first, optionally only those of one workspace (a selector), leaving archived sessions out unless
    /// `include_archived` (decision 0163). `agent.list` reads this; the service's own sweeps read [`Agents::list`].
    pub fn list_visible(
        &self,
        workspace: Option<&str>,
        include_archived: bool,
    ) -> Result<Vec<AgentSession>, AgentError> {
        let sessions = self.list(workspace)?;
        Ok(if include_archived {
            sessions
        } else {
            sessions
                .into_iter()
                .filter(|session| !session.archived)
                .collect()
        })
    }

    /// What deleting a session with its workspace would meet (decision 0163), from the checks `agent.forget` with
    /// `remove_workspace` runs, without `force`. Nothing changes. The path is the service's to show or withhold.
    pub fn forget_preview(&self, selector: &str) -> Result<AgentForgetPreview, AgentError> {
        let reading = self.deletion_reading(selector, RemovalWaivers::NONE)?;
        let blocker = reading.workspace_refusal();
        let workspace = &reading.workspace.workspace;
        Ok(AgentForgetPreview {
            session: reading.session.id.clone(),
            can_forget: reading.forget_refusal.is_none(),
            forget_blocker: reading
                .forget_refusal
                .as_ref()
                .map(|error| error.message.clone()),
            workspace: ForgetPreviewWorkspace {
                id: workspace.id.clone(),
                kind: workspace.kind,
                path: Some(workspace.path.clone()),
                branch: workspace.branch.clone(),
                removable: blocker.is_none(),
                blocker: blocker.as_ref().map(|error| error.message.clone()),
                blocker_code: blocker.as_ref().map(|error| error.code.to_owned()),
                dirty: reading.workspace.dirty,
                untracked: reading.workspace.untracked,
                ignored_files: reading.workspace.ignored,
                other_sessions: reading.users.sessions,
                other_teams: reading.users.teams,
                other_tasks: reading.users.tasks,
                running_terminals: reading.users.terminals,
            },
        })
    }

    /// Runs every check of forgetting a session with its workspace, with the removal's `waivers` as `workspace.remove`
    /// takes them, and returns the workspace's id. Nothing changes: a refusal here leaves the session and the workspace
    /// as they were. Refusals: the forget's own (`agent.busy` for a live run); `agent.busy` naming the other sessions of
    /// the workspace; `workspace.busy` naming its teams, tasks and running terminals; with `expect_clean`, a worktree
    /// that has changes now (`workspace.dirty`); then the removal's codes (`workspace.dirty`, `workspace.main_worktree`,
    /// `workspace.ignored_files`, ...).
    pub fn check_forget_with_workspace(
        &self,
        selector: &str,
        waivers: RemovalWaivers,
        expect_clean: bool,
    ) -> Result<String, AgentError> {
        let reading = self.deletion_reading(selector, waivers)?;
        if expect_clean && (reading.workspace.dirty || reading.workspace.untracked) {
            return Err(AgentError::new(
                codes::WORKSPACE_DIRTY,
                format!(
                    "workspace {} has changed since it was shown clean; look again before removing it",
                    reading.workspace.workspace.id
                ),
            ));
        }
        if let Some(error) = reading.forget_refusal.clone() {
            return Err(error);
        }
        if let Some(error) = reading.workspace_refusal() {
            return Err(error);
        }
        Ok(reading.workspace.workspace.id)
    }

    /// Begins removing workspace `workspace` after its session was forgotten (decision 0163): under the start gate, so no
    /// start or resume interleaves, and under the workspace's mutation lock, checks again that no session, team, task or
    /// running terminal uses it, then marks it `removing`, so every later start, resume and terminal in it is refused.
    /// A refusal leaves the workspace as it was. The caller finishes with `Workspaces::finish_remove`.
    pub fn begin_workspace_removal(
        &self,
        workspace: &str,
        waivers: RemovalWaivers,
        expect_clean: bool,
    ) -> Result<Workspace, AgentError> {
        let _gate = lock(&self.inner.start_gate);
        self.inner.config.workspaces.begin_remove_guarded(
            workspace,
            waivers,
            expect_clean,
            |found: &Workspace| match self.users_of(&found.id, None)?.refusal(&found.id) {
                Some(error) => Err(error),
                None => Ok(()),
            },
        )
    }

    /// What uses workspace `workspace` besides session `except`.
    fn users_of(&self, workspace: &str, except: Option<&str>) -> Result<Users, AgentError> {
        let sessions = self
            .store()
            .list_agent_sessions(Some(workspace))?
            .into_iter()
            .map(|row| row.id)
            .filter(|id| Some(id.as_str()) != except)
            .collect();
        let (teams, tasks) = self.store().teams_and_tasks_of_workspace(workspace)?;
        let terminals = self
            .inner
            .config
            .terminals
            .list(Some(workspace))
            .into_iter()
            .filter(|terminal| terminal.exit.is_none())
            .map(|terminal| terminal.id)
            .collect();
        Ok(Users {
            sessions,
            teams,
            tasks,
            terminals,
        })
    }

    fn deletion_reading(
        &self,
        selector: &str,
        waivers: RemovalWaivers,
    ) -> Result<DeletionReading, AgentError> {
        let session = self.show(selector)?;
        let forget_refusal =
            (session.execution.state != TerminalState::Exited).then(|| still_running(&session.id));
        let users = self.users_of(&session.workspace_id, Some(&session.id))?;
        let workspace = self
            .inner
            .config
            .workspaces
            .removal_preview(&session.workspace_id, waivers)?;
        Ok(DeletionReading {
            session,
            forget_refusal,
            users,
            workspace,
        })
    }
}

fn still_running(id: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_BUSY,
        format!("session {id} is still running; stop it first"),
    )
}

#[cfg(test)]
mod tests {
    use massorca_core_store::{AgentSessionRow, RunExit, RunExitReason, RunMode, new_id};
    use massorca_protocol::methods::workspaces::WorkspaceKind;

    use super::super::tests::service;
    use super::*;

    /// An exited session of `workspace` whose terminal the manager does not know.
    fn exited_row(workspace: &str) -> AgentSessionRow {
        AgentSessionRow {
            id: new_id(),
            kind: "fake".to_owned(),
            workspace_id: workspace.to_owned(),
            parent_id: None,
            title: None,
            read_only: false,
            agent_ref: None,
            terminal_id: Some("gone".to_owned()),
            request_id: None,
            created_at: now_ms(),
            exit: Some(RunExit {
                code: Some(0),
                signal: None,
                reason: RunExitReason::Exited,
                at: now_ms(),
            }),
            activity: None,
            result: None,
            brought_back_at: None,
            holds_secrets: false,
            mode: RunMode::default(),
            pending_permission_mode: None,
            retained: false,
            released_at: None,
        }
    }

    #[test]
    fn an_archived_session_is_listed_only_when_asked_and_keeps_its_first_archive_time() {
        let (agents, workspace, _profile) = service();
        let row = exited_row(&workspace);
        agents.store().insert_agent_session(&row).unwrap();

        assert!(agents.set_archived(&row.id, true).unwrap().archived);
        let at = agents.store().agent_session_archived_at(&row.id).unwrap();
        assert!(at.is_some());
        assert!(agents.list_visible(None, false).unwrap().is_empty());
        let all = agents.list_visible(None, true).unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].archived);
        assert_eq!(
            agents.list(None).unwrap().len(),
            1,
            "the service's own reads see it"
        );
        assert!(agents.set_archived(&row.id, true).unwrap().archived);
        assert_eq!(
            agents.store().agent_session_archived_at(&row.id).unwrap(),
            at
        );

        let shown = agents.set_archived(&row.id, false).unwrap();
        assert!(!shown.archived);
        assert_eq!(agents.list_visible(None, false).unwrap().len(), 1);
        assert!(
            agents
                .store()
                .archived_agent_session_ids()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_rename_trims_the_title_and_refuses_an_empty_long_or_control_title() {
        let (agents, workspace, _profile) = service();
        let row = exited_row(&workspace);
        agents.store().insert_agent_session(&row).unwrap();

        let renamed = agents.rename(&row.id, "  New name  ").unwrap();
        assert_eq!(renamed.title.as_deref(), Some("New name"));
        let longest = "é".repeat(MAX_TITLE_CHARS);
        assert_eq!(
            agents.rename(&row.id, &longest).unwrap().title.as_deref(),
            Some(longest.as_str()),
            "the limit counts characters, not bytes"
        );
        for bad in [
            "   ",
            &"x".repeat(MAX_TITLE_CHARS + 1),
            "a\tb",
            "line\nbreak",
        ] {
            let error = agents.rename(&row.id, bad).unwrap_err();
            assert_eq!(error.code, codes::REQUEST_INVALID_PARAMS, "{bad:?}");
        }
        assert_eq!(
            agents.show(&row.id).unwrap().title.as_deref(),
            Some(longest.as_str()),
            "a refusal changes nothing"
        );
    }

    #[test]
    fn the_preview_names_other_sessions_and_a_delete_is_refused_while_they_are_left() {
        let (agents, workspace, _profile) = service();
        let mine = exited_row(&workspace);
        agents.store().insert_agent_session(&mine).unwrap();

        let alone = agents.forget_preview(&mine.id).unwrap();
        assert!(alone.can_forget);
        assert_eq!(alone.workspace.kind, WorkspaceKind::Folder);
        assert!(alone.workspace.removable, "{alone:?}");
        assert!(alone.workspace.other_sessions.is_empty());
        assert_eq!(
            agents
                .check_forget_with_workspace(&mine.id, RemovalWaivers::NONE, false)
                .unwrap(),
            workspace
        );

        let other = exited_row(&workspace);
        agents.store().insert_agent_session(&other).unwrap();
        agents.set_archived(&other.id, true).unwrap();
        let shared = agents.forget_preview(&mine.id).unwrap();
        assert!(
            shared.can_forget,
            "the session itself can still be forgotten"
        );
        assert!(!shared.workspace.removable);
        assert_eq!(
            shared.workspace.other_sessions,
            std::slice::from_ref(&other.id)
        );
        assert_eq!(
            shared.workspace.blocker_code.as_deref(),
            Some(codes::AGENT_BUSY)
        );
        let refused = agents
            .check_forget_with_workspace(&mine.id, RemovalWaivers::FORCE, false)
            .unwrap_err();
        assert_eq!(refused.code, codes::AGENT_BUSY);
        assert!(refused.message.contains(&other.id), "{refused}");
        assert_eq!(
            agents.list(None).unwrap().len(),
            2,
            "a check changes nothing"
        );
    }
}

#[cfg(test)]
mod removal_race_tests {
    use massorca_core_store::{AgentSessionRow, RunExit, RunExitReason, RunMode, new_id};
    use massorca_protocol::methods::workspaces::WorkspaceState;

    use super::super::tests::service;
    use super::*;

    fn exited_row(workspace: &str) -> AgentSessionRow {
        AgentSessionRow {
            id: new_id(),
            kind: "fake".to_owned(),
            workspace_id: workspace.to_owned(),
            parent_id: None,
            title: None,
            read_only: false,
            agent_ref: None,
            terminal_id: None,
            request_id: None,
            created_at: now_ms(),
            exit: Some(RunExit {
                code: Some(0),
                signal: None,
                reason: RunExitReason::Exited,
                at: now_ms(),
            }),
            activity: None,
            result: None,
            brought_back_at: None,
            holds_secrets: false,
            mode: RunMode::default(),
            pending_permission_mode: None,
            retained: false,
            released_at: None,
        }
    }

    #[test]
    fn a_session_started_between_the_check_and_the_removal_refuses_it_and_keeps_its_row() {
        let (agents, workspace, _profile) = service();
        let mine = exited_row(&workspace);
        agents.store().insert_agent_session(&mine).unwrap();
        let checked = agents
            .check_forget_with_workspace(&mine.id, RemovalWaivers::NONE, false)
            .unwrap();
        agents.forget(&mine.id).unwrap();
        // A session that starts after the check and before the removal begins.
        let late = exited_row(&workspace);
        agents.store().insert_agent_session(&late).unwrap();

        let refused = agents
            .begin_workspace_removal(&checked, RemovalWaivers::NONE, false)
            .unwrap_err();
        assert_eq!(refused.code, codes::AGENT_BUSY);
        assert!(refused.message.contains(&late.id), "{refused}");
        assert!(
            agents
                .store()
                .get_agent_session(&late.id)
                .unwrap()
                .is_some()
        );
        let kept = agents
            .inner
            .config
            .workspaces
            .show_workspace(&checked)
            .unwrap();
        assert_eq!(
            kept.state,
            WorkspaceState::Ready,
            "the refusal marks nothing"
        );

        agents.forget(&late.id).unwrap();
        let begun = agents
            .begin_workspace_removal(&checked, RemovalWaivers::NONE, false)
            .unwrap();
        assert_eq!(begun.state, WorkspaceState::Removing);
    }
}
