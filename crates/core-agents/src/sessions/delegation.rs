//! Delegation and bring-back (decision 0055): a child session for a parent session, in a new child workspace or, when
//! read-only, in the parent's; the delegation tree; and bringing a finished child back into its parent.
//!
//! The git work of a bring-back (the clean check and the `--no-ff` merge) goes through [`RepoOps`], which the service
//! implements with `core-git`, so this crate keeps no git dependency of its own.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::path::Path;

use massorca_core_store::now_ms;
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    ActivityState, AgentBringBackParams, AgentDelegateParams, AgentDelegated, AgentSession,
    AgentStartParams, AgentTree, BringBackResult,
};
use massorca_protocol::methods::terminals::TerminalState;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams, WorkspaceKind};
use serde_json::json;

use super::{AgentError, Agents, Change, lock, tokens_match};
use crate::launch;

/// What a merge into the parent's worktree did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeResult {
    /// The child's branch is merged; `commit` is the parent's new HEAD.
    Merged { commit: String },
    /// The merge stopped on conflicts in `paths` and was aborted, so the parent is as it was.
    Conflict { paths: Vec<String> },
}

/// The git operations a bring-back runs in the parent's worktree. None of them ever pushes.
pub trait RepoOps {
    /// Whether the work tree at `dir` has staged, unstaged or untracked changes.
    fn is_dirty(&self, dir: &Path) -> Result<bool, AgentError>;
    /// `git merge --no-ff --no-edit <branch>` in `dir`, aborted when it stops on conflicts.
    fn merge_no_ff(&self, dir: &Path, branch: &str) -> Result<MergeResult, AgentError>;
}

/// The highest `n` tried for a child workspace named `<parent workspace>-<n>`.
const MAX_CHILD_SUFFIX: u32 = 1000;
/// Most bytes of the child's result quoted in the parent's bring-back prompt, so the prompt stays under its limit.
const MAX_QUOTED_RESULT: usize = 4 * 1024;
/// What the result of a writing child's delegation says (0055).
const NOT_CARRIED_OVER: &str = "the child starts from the parent branch's last commit; uncommitted changes in the parent's worktree are not carried over";

impl Agents {
    /// Starts a child session of `params.parent` (0055). A writing child gets a new worktree workspace in the parent's
    /// project, based on the parent workspace's branch; a read-only child runs in the parent's workspace. A child
    /// cannot delegate (`agent.depth_exceeded`), a writing child of a folder workspace is refused
    /// (`agent.delegate_needs_git`), and a token that does not match the parent's live run is refused
    /// (`agent.hook_refused`). A repeated `request_id` returns the first child.
    pub fn delegate(&self, params: AgentDelegateParams) -> Result<AgentDelegated, AgentError> {
        launch::check_prompt(&params.prompt)?;
        if let Some(request_id) = &params.request_id
            && let Some(row) = self.store().find_agent_session_by_request_id(request_id)?
        {
            return Ok(AgentDelegated {
                session: self.view(row)?,
                note: None,
            });
        }
        let parent = self.resolve(&params.parent)?;
        if let Some(token) = &params.token {
            self.check_run_token(&parent.id, token)?;
        }
        if parent.parent_id.is_some() {
            return Err(AgentError::new(
                codes::AGENT_DEPTH_EXCEEDED,
                format!(
                    "session {} is itself delegated, and a delegated session cannot delegate",
                    parent.id
                ),
            ));
        }
        let kind = params.kind.unwrap_or_else(|| parent.kind.clone());
        // Checked before a child workspace exists, so an unknown or missing kind leaves nothing behind.
        self.startable_kind(&kind)?;
        let parent_workspace = self
            .inner
            .config
            .workspaces
            .show_workspace(&parent.workspace_id)?;
        let (workspace, note) = if params.read_only {
            (parent_workspace.id, None)
        } else {
            let child = self.child_workspace(
                &parent_workspace,
                params.name.as_deref(),
                params.request_id.as_deref(),
            )?;
            (child.id, Some(NOT_CARRIED_OVER.to_owned()))
        };
        let session = self.start_session(
            AgentStartParams {
                workspace,
                kind,
                prompt: Some(params.prompt),
                args: Vec::new(),
                env: BTreeMap::new(),
                read_only: params.read_only,
                title: None,
                cols: None,
                rows: None,
                request_id: params.request_id,
            },
            Some(parent.id),
        )?;
        Ok(AgentDelegated { session, note })
    }

    /// The delegation tree of a session (0055): its root (the session itself, or its parent) and the root's children,
    /// oldest first.
    pub fn tree(&self, selector: &str) -> Result<AgentTree, AgentError> {
        let row = self.resolve(selector)?;
        let root = match &row.parent_id {
            Some(parent) => self.reload(parent)?,
            None => row,
        };
        let children = self
            .store()
            .list_agent_children(&root.id)?
            .into_iter()
            .map(|child| self.view(child))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AgentTree {
            root: self.view(root)?,
            children,
        })
    }

    /// Brings a finished child back into its parent (0055, steps 1 to 4): merges the child's branch into the parent's
    /// worktree unless `merge` is off or the child ran in the parent's workspace, tells a live parent run about it
    /// unless `prompt` is off, and marks the child brought back. A refused prompt does not fail the call; the result
    /// says the prompt was not delivered.
    pub fn bring_back(
        &self,
        params: &AgentBringBackParams,
        repo: &dyn RepoOps,
    ) -> Result<BringBackResult, AgentError> {
        let _gate = lock(&self.inner.bring_back_gate);
        let row = self.resolve(&params.child)?;
        let Some(parent_id) = row.parent_id.clone() else {
            return Err(AgentError::new(
                codes::REQUEST_INVALID_PARAMS,
                format!(
                    "session {} was not delegated, so it has no parent to come back to",
                    row.id
                ),
            ));
        };
        if let Some(token) = &params.token {
            self.check_run_token(&parent_id, token)?;
        }
        let child = self.view(row)?;
        if child.brought_back_at_ms.is_some() {
            return Err(already_brought_back(&child.id));
        }
        if !self.finished(&child) {
            return Err(AgentError::new(
                codes::AGENT_BUSY,
                format!(
                    "session {} has not finished: wait until it is done or exited",
                    child.id
                ),
            ));
        }
        let parent = self.view(self.reload(&parent_id)?)?;

        let workspaces = &self.inner.config.workspaces;
        let child_workspace = workspaces.show_workspace(&child.workspace_id)?;
        let mut merge_commit = None;
        if params.merge && child.workspace_id != parent.workspace_id {
            merge_commit = Some(self.merge_child(&parent, &child_workspace, repo)?);
        }

        if !self
            .store()
            .set_agent_session_brought_back(&child.id, now_ms())?
        {
            return Err(already_brought_back(&child.id));
        }
        let child = {
            let mut live = lock(&self.inner.status);
            let session = self.view_with(&mut live, self.reload(&child.id)?)?;
            self.notify(Change::Updated(Box::new(session.clone())));
            session
        };

        let (prompt_delivered, note) = if !params.prompt {
            (false, None)
        } else if parent.execution.state == TerminalState::Exited {
            (
                false,
                Some("the parent's run is not live, so no prompt was sent".to_owned()),
            )
        } else {
            let text = bring_back_prompt(&child, &child_workspace.name, merge_commit.as_deref());
            match self.prompt(&parent.id, &text, false) {
                Ok(_) => (true, None),
                Err(error) => (
                    false,
                    Some(format!(
                        "the prompt was not delivered to the parent ({}); the bring-back is done",
                        error.code
                    )),
                ),
            }
        };
        Ok(BringBackResult {
            merged: merge_commit.is_some(),
            merge_commit,
            child,
            prompt_delivered,
            note,
        })
    }

    /// Merges the child workspace's branch into the parent's worktree (0055, step 2). The parent must not be working
    /// and its worktree must be clean; a conflict is aborted and reported with its paths. Returns the merge commit.
    fn merge_child(
        &self,
        parent: &AgentSession,
        child_workspace: &Workspace,
        repo: &dyn RepoOps,
    ) -> Result<String, AgentError> {
        if parent
            .activity
            .as_ref()
            .is_some_and(|activity| activity.state == ActivityState::Working)
        {
            return Err(AgentError::new(
                codes::AGENT_PARENT_BUSY,
                format!(
                    "the parent session {} is working; bring the child back once it is not",
                    parent.id
                ),
            ));
        }
        let branch = child_workspace.branch.as_deref().ok_or_else(|| {
            AgentError::new(
                codes::AGENT_DELEGATE_NEEDS_GIT,
                format!(
                    "the child workspace {} has no branch to merge",
                    child_workspace.name
                ),
            )
        })?;
        let parent_workspace = self
            .inner
            .config
            .workspaces
            .show_workspace(&parent.workspace_id)?;
        if parent_workspace.missing {
            return Err(AgentError::new(
                codes::PATH_NOT_FOUND,
                format!(
                    "the workspace directory is missing: {}",
                    parent_workspace.path
                ),
            ));
        }
        let dir = Path::new(&parent_workspace.path);
        if repo.is_dirty(dir)? {
            return Err(AgentError::new(
                codes::AGENT_PARENT_DIRTY,
                format!(
                    "the parent workspace {} has uncommitted changes; commit or discard them first",
                    parent_workspace.name
                ),
            ));
        }
        match repo.merge_no_ff(dir, branch)? {
            MergeResult::Merged { commit } => Ok(commit),
            MergeResult::Conflict { paths } => {
                let mut error = AgentError::new(
                    codes::AGENT_MERGE_CONFLICT,
                    format!(
                        "merging {branch} into {} stopped on conflicts; the merge was aborted",
                        parent_workspace.name
                    ),
                );
                error.data = Some(json!({ "paths": paths }));
                Err(error)
            }
        }
    }

    /// Whether a child has finished: its run exited, or its last turn is done and no prompt was typed since (0058).
    fn finished(&self, child: &AgentSession) -> bool {
        if child.execution.state == TerminalState::Exited {
            return true;
        }
        let done = child
            .activity
            .as_ref()
            .is_some_and(|activity| activity.state == ActivityState::Done);
        let typed = lock(&self.inner.status)
            .get(&child.id)
            .is_some_and(|run| run.typed_since_report);
        done && !typed
    }

    /// The new workspace of a writing child: `name`, or `<parent workspace>-<n>` with the first free `n`, on a branch of
    /// the same name based on the parent workspace's branch.
    fn child_workspace(
        &self,
        parent: &Workspace,
        name: Option<&str>,
        request_id: Option<&str>,
    ) -> Result<Workspace, AgentError> {
        let needs_git = |reason: &str| {
            AgentError::new(
                codes::AGENT_DELEGATE_NEEDS_GIT,
                format!(
                    "workspace {} {reason}, so a writing child cannot get a worktree of its own; delegate it read-only",
                    parent.name
                ),
            )
        };
        if parent.kind != WorkspaceKind::Worktree {
            return Err(needs_git("is not a git worktree"));
        }
        let base = parent
            .branch
            .clone()
            .ok_or_else(|| needs_git("has no branch"))?;
        let workspaces = &self.inner.config.workspaces;
        let create = |name: String| {
            workspaces.create_workspace(WorkspaceCreateParams {
                project: parent.project_id.clone(),
                name,
                branch: None,
                base: Some(base.clone()),
                existing_branch: false,
                path: None,
                request_id: request_id.map(|id| format!("agent.delegate:{id}")),
            })
        };
        if let Some(name) = name {
            return Ok(create(name.to_owned())?);
        }
        let taken: HashSet<String> = workspaces
            .list_workspaces(Some(&parent.project_id))?
            .into_iter()
            .map(|workspace| workspace.name)
            .collect();
        for n in 1..=MAX_CHILD_SUFFIX {
            let name = format!("{}-{n}", parent.name);
            if taken.contains(&name) {
                continue;
            }
            match create(name) {
                Ok(workspace) => return Ok(workspace),
                // A branch or directory of that name left from earlier work: try the next number.
                Err(error)
                    if matches!(
                        error.code(),
                        codes::BRANCH_EXISTS
                            | codes::WORKSPACE_NAME_TAKEN
                            | codes::WORKSPACE_PATH_TAKEN
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err(AgentError::new(
            codes::WORKSPACE_NAME_TAKEN,
            format!(
                "no free child workspace name {}-1 to {}-{MAX_CHILD_SUFFIX}; give one with name",
                parent.name, parent.name
            ),
        ))
    }

    /// Refuses a call that carries a token unless it is the token of `session`'s live run (0055).
    fn check_run_token(&self, session: &str, token: &str) -> Result<(), AgentError> {
        let matches = lock(&self.inner.status)
            .get(session)
            .is_some_and(|run| tokens_match(&run.token, token));
        if matches {
            Ok(())
        } else {
            Err(AgentError::new(
                codes::AGENT_HOOK_REFUSED,
                format!("the token does not match a live run of session {session}"),
            ))
        }
    }
}

fn already_brought_back(session: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_ALREADY_BROUGHT_BACK,
        format!("session {session} was already brought back"),
    )
}

/// The prompt a parent run receives on a bring-back: the child, its workspace, the merge commit and the child's result.
/// It is one line, so a plain paste submits it once: every control character in the result, line breaks included,
/// becomes a space, and the result is cut to [`MAX_QUOTED_RESULT`] bytes.
fn bring_back_prompt(child: &AgentSession, workspace: &str, merge_commit: Option<&str>) -> String {
    let merge = match merge_commit {
        Some(commit) => format!("Its branch is merged into this worktree as commit {commit}."),
        None => "Nothing was merged.".to_owned(),
    };
    let result = child.result.as_deref().map_or_else(
        || "none".to_owned(),
        |text| one_line(text, MAX_QUOTED_RESULT),
    );
    format!(
        "MassOrca: delegated session {} in workspace {workspace} is back. {merge} Its result: {result}",
        child.id
    )
}

/// `text` with every control character replaced by a space, cut on a character boundary to at most `max` bytes.
fn one_line(text: &str, max: usize) -> String {
    let mut line = String::with_capacity(text.len().min(max));
    for c in text.chars() {
        let c = if c.is_control() { ' ' } else { c };
        if line.len() + c.len_utf8() > max {
            break;
        }
        line.push(c);
    }
    line.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::one_line;
    use crate::launch::check_prompt;

    #[test]
    fn a_quoted_result_is_one_line_without_control_characters_and_within_its_limit() {
        let text = "line one\r\nline\ttwo\u{1b}[31m red\u{7f}\u{85} é";
        let line = one_line(text, 1024);
        assert_eq!(line, "line one  line two [31m red   é");
        assert!(check_prompt(&line).is_ok());
        assert_eq!(one_line("ééé", 5), "éé");
    }
}
