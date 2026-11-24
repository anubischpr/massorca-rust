//! Delegation and bring-back (decision 0055): a child session for a parent session, in a new child workspace or, when
//! read-only, in the parent's; the delegation tree; and bringing a finished child back into its parent.
//!
//! The git work of a bring-back (the clean check and the `--no-ff` merge) goes through [`RepoOps`], which the service
//! implements with `core-git`, so this crate keeps no git dependency of its own.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

use massorca_core_store::{AgentSessionRow, now_ms};
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    ActivityState, AgentBringBackParams, AgentDelegateParams, AgentDelegated, AgentSession,
    AgentStartParams, AgentTree, BringBackResult,
};
use massorca_protocol::methods::terminals::TerminalState;
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams, WorkspaceKind};
use serde_json::{Value, json};

use super::{AgentError, Agents, Change, lock, tokens_match};
use crate::launch;
use crate::modes;
use crate::provided_env::SecretGrant;

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
    /// The operation git stopped in the middle of in the work tree at `dir` (`a rebase`, `a merge`...), if any.
    fn operation_in_progress(&self, dir: &Path) -> Result<Option<String>, AgentError>;
    /// `git merge --no-ff --no-edit <branch>` in `dir`, aborted when it stops on conflicts.
    fn merge_no_ff(&self, dir: &Path, branch: &str) -> Result<MergeResult, AgentError>;
}

/// The highest `n` tried for a child workspace named `<parent workspace>-<n>`.
const MAX_CHILD_SUFFIX: u32 = 1000;
/// Most bytes of the child's result quoted in the parent's bring-back prompt, so the prompt stays under its limit.
const MAX_QUOTED_RESULT: usize = 4 * 1024;
/// What the result of a writing child's delegation says (0055).
const NOT_CARRIED_OVER: &str = "the child starts from the parent branch's last commit; uncommitted changes in the parent's worktree are not carried over";
/// The setting that says how many levels deep agents may delegate (decision 0070).
pub const DELEGATION_DEPTH_SETTING: &str = "agents.delegation_depth";
/// The owner core settings are stored under.
const DELEGATION_DEPTH_OWNER: &str = "core";
/// The smallest and the largest limit the setting accepts (decision 0070).
pub const DELEGATION_DEPTH_MIN: i64 = 1;
pub const DELEGATION_DEPTH_MAX: i64 = 5;
/// The limit when the setting is unset: a child may not delegate. pv96 ships the same default
/// (`NESTED_WORKER_MAX_DEPTH_DEFAULT = 1`, commit d3c6311a531, `src/shared/nested-worker-depth.ts`).
pub const DELEGATION_DEPTH_DEFAULT: i64 = 1;
/// Most links walked up a delegation chain before the chain counts as broken.
const MAX_CHAIN_WALK: i64 = 64;

/// The limit a stored value gives. Unset gives the default. A whole number is clamped to the range, and a value
/// outside it is logged. Anything that is not a whole number gives the default, also logged.
pub fn delegation_depth_from_stored(stored: Option<&str>) -> i64 {
    let Some(text) = stored else {
        return DELEGATION_DEPTH_DEFAULT;
    };
    match serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|value| value.as_i64())
    {
        Some(depth) => {
            let clamped = depth.clamp(DELEGATION_DEPTH_MIN, DELEGATION_DEPTH_MAX);
            if clamped != depth {
                tracing::warn!(
                    setting = DELEGATION_DEPTH_SETTING,
                    stored = depth,
                    used = clamped,
                    "a stored delegation depth is outside its range; it is clamped to the range"
                );
            }
            clamped
        }
        None => {
            tracing::warn!(
                setting = DELEGATION_DEPTH_SETTING,
                "a stored delegation depth is not a whole number; the default applies"
            );
            DELEGATION_DEPTH_DEFAULT
        }
    }
}

impl Agents {
    /// Starts a child session of `params.parent` (0055). A writing child gets a new worktree workspace in the parent's
    /// project, based on the parent workspace's branch; a read-only child runs in the parent's workspace. A child
    /// goes no deeper than `agents.delegation_depth` (`agent.depth_exceeded` past it; the default of 1 means a child cannot
    /// delegate), a writing child of a folder workspace is refused
    /// (`agent.delegate_needs_git`), and a token that does not match the parent's live run is refused
    /// (`agent.hook_refused`). A repeated `request_id` returns the first child.
    pub fn delegate(
        &self,
        params: AgentDelegateParams,
        grant: SecretGrant,
    ) -> Result<AgentDelegated, AgentError> {
        launch::check_prompt(&params.prompt)?;
        let parent = self.resolve(&params.parent)?;
        if let Some(token) = &params.token {
            self.check_run_token(&parent.id, token)?;
        }
        // A retry returns its first child, but only to the same parent: request ids are unique across sessions.
        if let Some(request_id) = &params.request_id
            && let Some(row) = self.store().find_agent_session_by_request_id(request_id)?
        {
            if row.parent_id.as_deref() != Some(parent.id.as_str()) {
                return Err(AgentError::new(
                    codes::REQUEST_INVALID_PARAMS,
                    format!("request id {request_id} belongs to a call for another session"),
                ));
            }
            return Ok(AgentDelegated {
                session: self.view(row)?,
                note: None,
            });
        }
        // The child would sit one level below the parent. The limit is read on every call, so a changed setting applies
        // to the next delegation.
        let limit = self.delegation_depth_limit()?;
        let child_depth = self.delegation_depth_of(&parent)? + 1;
        if child_depth > limit {
            let mut error = AgentError::new(
                codes::AGENT_DEPTH_EXCEEDED,
                format!(
                    "session {} cannot delegate: its child would be at depth {child_depth}, and {DELEGATION_DEPTH_SETTING} allows at most {limit}",
                    parent.id
                ),
            );
            error.data = Some(json!({
                "limit": limit,
                "setting": DELEGATION_DEPTH_SETTING,
                "depth": child_depth,
            }));
            return Err(error);
        }
        let kind = params.kind.unwrap_or_else(|| parent.kind.clone());
        // Checked before a child workspace exists, so an unknown or missing kind, or a mode it does not declare, leaves
        // nothing behind. The child's mode is its own request or its kind's default, never the parent's (decision 0083).
        // An agent asking (its run token is on the call) never picks a bypass mode for its child: that is the person's call
        // to make, through agent.start (decision 0083, R1).
        if params.token.is_some()
            && let Some(mode) = params.permission_mode.as_deref()
            && self
                .declared_kind(&kind)?
                .permission_mode(mode)
                .is_some_and(|declared| declared.bypass)
        {
            return Err(AgentError::new(
                codes::AGENT_MODE_UNAVAILABLE,
                format!(
                    "an agent cannot delegate in bypass mode {mode}; a bypass mode must be chosen by the person, through agent.start"
                ),
            ));
        }
        let (decl, _) = self.startable_kind(&kind)?;
        modes::plan_start(&decl, params.permission_mode.as_deref())?;
        let parent_workspace = self
            .inner
            .config
            .workspaces
            .show_workspace(&parent.workspace_id)?;
        let (workspace, note, kept) = if params.read_only {
            (parent_workspace.id, None, None)
        } else {
            let child = self.child_workspace(
                &parent_workspace,
                params.name.as_deref(),
                params.request_id.as_deref(),
            )?;
            (
                child.id.clone(),
                Some(NOT_CARRIED_OVER.to_owned()),
                Some(child),
            )
        };
        let started = self.start_session(
            AgentStartParams {
                workspace: Some(workspace),
                kind,
                title: child_title(params.title.as_deref(), &params.prompt),
                prompt: Some(params.prompt),
                args: Vec::new(),
                env: BTreeMap::new(),
                read_only: params.read_only,
                cols: None,
                rows: None,
                request_id: params.request_id,
                permission_mode: params.permission_mode,
                model: None,
                effort: None,
            },
            Some(&parent),
            grant,
        );
        let session = match (started, kept) {
            (Ok(session), _) => session,
            (Err(error), None) => return Err(error),
            (Err(error), Some(kept)) => return Err(kept_workspace(error, &kept)),
        };
        Ok(AgentDelegated { session, note })
    }

    /// The delegation tree of a session (0055): its root, the session at the top of its delegation chain, and the root's
    /// children (its direct delegations), oldest first.
    pub fn tree(&self, selector: &str) -> Result<AgentTree, AgentError> {
        let mut root = self.resolve(selector)?;
        let mut walked = 0;
        while let Some(parent) = root.parent_id.clone() {
            walked += 1;
            if walked > MAX_CHAIN_WALK {
                return Err(chain_too_long(&root.id));
            }
            root = self.reload(&parent)?;
        }
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
        // Drained however this call ends: a merge that failed or a refused mark may have held the parent's queue (0084, D3).
        let _drain = DrainParentQueue {
            agents: self,
            parent: parent_id.clone(),
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

        // Merge, then mark: when the mark fails after a merge, the error carries the merge commit, so a refused retry
        // does not hide that the merge happened.
        match self
            .store()
            .set_agent_session_brought_back(&child.id, now_ms())
        {
            Ok(true) => {}
            Ok(false) => {
                return Err(with_merge_commit(
                    already_brought_back(&child.id),
                    merge_commit.as_deref(),
                ));
            }
            Err(error) => {
                return Err(with_merge_commit(error.into(), merge_commit.as_deref()));
            }
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
            match self.send_prompt(&parent.id, &text, false, false) {
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

    /// Merges the child workspace's branch into the parent's worktree (0055, step 2). The parent must not be working,
    /// no other session may write in its workspace, and its worktree must be clean with no rebase or merge stopped in it;
    /// a conflict is aborted and reported with its paths. Returns the merge commit.
    fn merge_child(
        &self,
        parent: &AgentSession,
        child_workspace: &Workspace,
        repo: &dyn RepoOps,
    ) -> Result<String, AgentError> {
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
        // Held until the merge ends: no prompt to the parent and no start in its workspace can land meanwhile.
        let _claim = self.claim_for_merge(&parent.id, &parent_workspace.id)?;
        let dir = Path::new(&parent_workspace.path);
        if let Some(operation) = repo.operation_in_progress(dir)? {
            return Err(AgentError::new(
                codes::AGENT_PARENT_DIRTY,
                format!(
                    "{operation} is in progress in the parent workspace {}; finish or abort it first",
                    parent_workspace.name
                ),
            ));
        }
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

    /// Claims the parent's workspace for a merge (0055). Under the start gate, so no start slips in, it checks that no
    /// session other than the parent writes there (`agent.workspace_busy`). Under the status lock, so no prompt slips
    /// in, it reads the parent's activity again (`agent.parent_busy` while it works or has a prompt it has not answered)
    /// and lists the workspace as merging. The claim ends when it is dropped.
    fn claim_for_merge(
        &self,
        parent_id: &str,
        workspace_id: &str,
    ) -> Result<MergeClaim<'_>, AgentError> {
        let _gate = lock(&self.inner.start_gate);
        for row in self.store().list_agent_sessions(Some(workspace_id))? {
            if row.read_only || row.id == parent_id {
                continue;
            }
            let session = self.view(row)?;
            if session.execution.state != TerminalState::Exited {
                let mut error = AgentError::new(
                    codes::AGENT_WORKSPACE_BUSY,
                    format!(
                        "session {} writes in the parent workspace; bring the child back once it has stopped",
                        session.id
                    ),
                );
                error.data = Some(json!({ "session": session.id }));
                return Err(error);
            }
        }
        let mut live = lock(&self.inner.status);
        let parent = self.view_with(&mut live, self.reload(parent_id)?)?;
        let working = parent
            .activity
            .as_ref()
            .is_some_and(|activity| activity.state == ActivityState::Working)
            || live
                .get(parent_id)
                .is_some_and(|run| run.typed_since_report);
        if working {
            return Err(AgentError::new(
                codes::AGENT_PARENT_BUSY,
                format!(
                    "the parent session {parent_id} is working; bring the child back once it is not"
                ),
            ));
        }
        if !lock(&self.inner.merging).insert(workspace_id.to_owned()) {
            return Err(merging_busy(workspace_id));
        }
        Ok(MergeClaim {
            merging: &self.inner.merging,
            workspace_id: workspace_id.to_owned(),
        })
    }

    /// Refuses a start, a resume or a prompt in a workspace a bring-back is merging into (0055).
    pub(super) fn refuse_while_merging(&self, workspace_id: &str) -> Result<(), AgentError> {
        if lock(&self.inner.merging).contains(workspace_id) {
            Err(merging_busy(workspace_id))
        } else {
            Ok(())
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
        let request_key = request_id.map(|id| format!("agent.delegate:{id}"));
        // A retry reuses the workspace its first call created, whatever its name.
        if let Some(key) = &request_key
            && let Some(row) = self.store().find_workspace_by_request_id(key)?
        {
            if row.project_id != parent.project_id {
                return Err(AgentError::new(
                    codes::REQUEST_INVALID_PARAMS,
                    "the request id belongs to a call in another project",
                ));
            }
            return Ok(workspaces.show_workspace(&row.id)?);
        }
        let create = |name: String| {
            workspaces.create_workspace(WorkspaceCreateParams {
                project: parent.project_id.clone(),
                name,
                branch: None,
                base: Some(base.clone()),
                existing_branch: false,
                refresh_base: false,
                path: None,
                request_id: request_key.clone(),
                no_setup: false,
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

    /// The delegation depth limit now stored under `agents.delegation_depth`, or the default when unset (decision 0070).
    pub(crate) fn delegation_depth_limit(&self) -> Result<i64, AgentError> {
        let stored = self
            .store()
            .get_setting(DELEGATION_DEPTH_OWNER, DELEGATION_DEPTH_SETTING)?;
        Ok(delegation_depth_from_stored(stored.as_deref()))
    }

    /// How many delegations lie above `row`: 0 for a root, 1 for its child, and so on.
    fn delegation_depth_of(&self, row: &AgentSessionRow) -> Result<i64, AgentError> {
        let mut depth = 0;
        let mut parent = row.parent_id.clone();
        while let Some(id) = parent {
            depth += 1;
            if depth > MAX_CHAIN_WALK {
                return Err(chain_too_long(&row.id));
            }
            parent = self
                .store()
                .get_agent_session(&id)?
                .and_then(|link| link.parent_id);
        }
        Ok(depth)
    }

    /// `root` and every session delegated from it, directly or through others, parents before their children (0055).
    pub(super) fn subtree(&self, root: &str) -> Result<Vec<String>, AgentError> {
        let mut ids = vec![root.to_owned()];
        let mut next = 0;
        while next < ids.len() {
            let children = self.store().list_agent_children(&ids[next])?;
            ids.extend(children.into_iter().map(|child| child.id));
            next += 1;
        }
        Ok(ids)
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

/// A workspace listed as merging; the listing ends when the claim is dropped, on every path out of the merge.
struct MergeClaim<'a> {
    merging: &'a Mutex<HashSet<String>>,
    workspace_id: String,
}

impl Drop for MergeClaim<'_> {
    fn drop(&mut self) {
        lock(self.merging).remove(&self.workspace_id);
    }
}

/// The refusal for a delegation chain that loops or is longer than [`MAX_CHAIN_WALK`].
fn chain_too_long(session: &str) -> AgentError {
    AgentError::new(
        codes::INTERNAL,
        format!("the delegation chain of session {session} is too long to check"),
    )
}

fn merging_busy(workspace_id: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_BUSY,
        format!("a bring-back is merging into workspace {workspace_id}; try again once it ends"),
    )
}

/// `error` with the merge commit a bring-back already made, in its data and message.
fn with_merge_commit(mut error: AgentError, merge_commit: Option<&str>) -> AgentError {
    if let Some(commit) = merge_commit {
        error.message = format!(
            "{}; the child was already merged as commit {commit}",
            error.message
        );
        add_data(&mut error, "merge_commit", json!(commit));
    }
    error
}

/// A start that failed after the child workspace was created: the error names the workspace, which is kept.
fn kept_workspace(mut error: AgentError, workspace: &Workspace) -> AgentError {
    error.message = format!(
        "{}; the child workspace {} was kept, and a retry with the same request id reuses it",
        error.message, workspace.name
    );
    add_data(
        &mut error,
        "workspace",
        json!({ "id": workspace.id, "name": workspace.name }),
    );
    error
}

/// Adds `key` to an error's data object, creating the object when there is none.
fn add_data(error: &mut AgentError, key: &str, value: serde_json::Value) {
    match &mut error.data {
        Some(serde_json::Value::Object(map)) => {
            map.insert(key.to_owned(), value);
        }
        _ => error.data = Some(json!({ key: value })),
    }
}

fn already_brought_back(session: &str) -> AgentError {
    AgentError::new(
        codes::AGENT_ALREADY_BROUGHT_BACK,
        format!("session {session} was already brought back"),
    )
}

/// Drains a session's queue when it goes out of scope, so a bring-back drains the parent however it ends.
struct DrainParentQueue<'a> {
    agents: &'a Agents,
    parent: String,
}

impl Drop for DrainParentQueue<'_> {
    fn drop(&mut self) {
        self.agents.drain_queue(&self.parent);
    }
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

/// The most characters a delegated child's title keeps.
const CHILD_TITLE_CHARS: usize = 80;

/// A delegated child's title: the caller's title, else the first non-empty line of its prompt, trimmed, with control
/// characters replaced by spaces and cut to [`CHILD_TITLE_CHARS`] characters. `None` when both are blank.
fn child_title(title: Option<&str>, prompt: &str) -> Option<String> {
    let source = match title.map(str::trim).filter(|title| !title.is_empty()) {
        Some(title) => title,
        None => prompt
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())?,
    };
    let clean: String = source
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(CHILD_TITLE_CHARS)
        .collect();
    let clean = clean.trim();
    (!clean.is_empty()).then(|| clean.to_owned())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_child_is_titled_by_its_title_or_the_first_line_of_its_prompt() {
        use super::{CHILD_TITLE_CHARS, child_title};

        assert_eq!(
            child_title(Some("  Fix the parser  "), "ignored").as_deref(),
            Some("Fix the parser")
        );
        assert_eq!(
            child_title(
                None,
                "

  Review the diff
then report"
            )
            .as_deref(),
            Some("Review the diff")
        );
        assert_eq!(
            child_title(
                Some(" "),
                "  
 "
            )
            .as_deref(),
            None
        );
        assert_eq!(child_title(None, "tab	here").as_deref(), Some("tab here"));
        let long = "x".repeat(200);
        assert_eq!(
            child_title(None, &long).map(|title| title.chars().count()),
            Some(CHILD_TITLE_CHARS)
        );
    }

    use super::one_line;
    use crate::launch::check_prompt;

    use std::collections::BTreeMap;
    use std::path::Path;

    use massorca_core_store::{AgentSessionRow, RunExit, RunExitReason, new_id, now_ms};
    use massorca_protocol::codes;
    use massorca_protocol::methods::agents::{
        AgentBringBackParams, AgentDelegateParams, AgentStartParams,
    };
    use massorca_protocol::methods::projects::ProjectAddParams;
    use massorca_protocol::methods::workspaces::WorkspaceCreateParams;

    use super::super::tests::service;
    use super::super::{LiveRun, PromptRules, lock};
    use super::{AgentError, MergeResult, RepoOps, SecretGrant};
    use crate::kind::PasteMode;
    use crate::status::HookRules;

    /// Git operations a test must never reach.
    struct NoGit;

    impl RepoOps for NoGit {
        fn is_dirty(&self, _dir: &Path) -> Result<bool, AgentError> {
            unreachable!("no git work in this test")
        }
        fn operation_in_progress(&self, _dir: &Path) -> Result<Option<String>, AgentError> {
            unreachable!("no git work in this test")
        }
        fn merge_no_ff(&self, _dir: &Path, _branch: &str) -> Result<MergeResult, AgentError> {
            unreachable!("no git work in this test")
        }
    }

    /// A session row in `workspace_id`, delegated from `parent` when given; its run has ended.
    fn session_row(workspace_id: &str, parent: Option<&str>) -> AgentSessionRow {
        AgentSessionRow {
            id: new_id(),
            kind: "fake".to_owned(),
            workspace_id: workspace_id.to_owned(),
            parent_id: parent.map(str::to_owned),
            title: None,
            read_only: parent.is_some(),
            agent_ref: None,
            terminal_id: Some(new_id()),
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
            mode: massorca_core_store::RunMode::default(),
            pending_permission_mode: None,
            retained: false,
            released_at: None,
        }
    }

    /// Registers a live run with `token` for `session`, as a start does.
    fn live_run(agents: &super::Agents, session: &str, token: &str) {
        lock(&agents.inner.status).insert(
            session.to_owned(),
            LiveRun {
                token: token.to_owned(),
                rules: HookRules::default(),
                prompt: PromptRules {
                    has_status: true,
                    paste: PasteMode::Plain,
                    submit: "\r".to_owned(),
                },
                pending: None,
                pending_switch: None,
                account: None,
                typed_since_report: false,
                switching: false,
                delivering: false,
            },
        );
    }

    fn delegate_params(parent: &str, request_id: &str) -> AgentDelegateParams {
        AgentDelegateParams {
            title: None,
            parent: parent.to_owned(),
            prompt: "look".to_owned(),
            kind: None,
            name: None,
            read_only: true,
            token: None,
            request_id: Some(request_id.to_owned()),
            permission_mode: None,
        }
    }

    #[test]
    fn an_agent_cannot_delegate_into_a_bypass_mode() {
        let (agents, workspace_id, _profile) = service();
        let parent = session_row(&workspace_id, None);
        agents.store().insert_agent_session(&parent).unwrap();
        live_run(&agents, &parent.id, "parent-token");
        let params = |token: Option<&str>, mode: &str| AgentDelegateParams {
            kind: Some("claude".to_owned()),
            token: token.map(str::to_owned),
            permission_mode: Some(mode.to_owned()),
            ..delegate_params(&parent.id, &new_id())
        };

        let refused = agents
            .delegate(
                params(Some("parent-token"), "bypassPermissions"),
                SecretGrant::Provide,
            )
            .unwrap_err();
        assert_eq!(refused.code, codes::AGENT_MODE_UNAVAILABLE);
        assert!(
            refused.message.contains("agent.start"),
            "{}",
            refused.message
        );

        // The person's own call, and an agent's call for a mode that is not a bypass, go on to the kind's program check.
        for (token, mode) in [(None, "bypassPermissions"), (Some("parent-token"), "plan")] {
            let error = agents
                .delegate(params(token, mode), SecretGrant::Provide)
                .unwrap_err();
            assert_ne!(
                error.code,
                codes::AGENT_MODE_UNAVAILABLE,
                "{token:?} {mode}"
            );
        }
    }

    #[test]
    fn only_the_parents_run_token_brings_its_child_back() {
        let (agents, workspace_id, _profile) = service();
        let parent = session_row(&workspace_id, None);
        let child = session_row(&workspace_id, Some(&parent.id));
        agents.store().insert_agent_session(&parent).unwrap();
        agents.store().insert_agent_session(&child).unwrap();
        live_run(&agents, &parent.id, "parent-token");
        let params = |token: &str| AgentBringBackParams {
            child: child.id.clone(),
            merge: false,
            prompt: false,
            token: Some(token.to_owned()),
        };

        let refused = agents
            .bring_back(&params("child-token"), &NoGit)
            .unwrap_err();
        assert_eq!(refused.code, codes::AGENT_HOOK_REFUSED);
        assert!(agents.show(&child.id).unwrap().brought_back_at_ms.is_none());

        let brought = agents.bring_back(&params("parent-token"), &NoGit).unwrap();
        assert!(!brought.merged);
        assert!(brought.child.brought_back_at_ms.is_some());
    }

    #[test]
    fn a_request_id_of_another_parents_child_is_refused() {
        let (agents, workspace_id, _profile) = service();
        let parent = session_row(&workspace_id, None);
        let other = session_row(&workspace_id, None);
        let mut child = session_row(&workspace_id, Some(&parent.id));
        child.request_id = Some("req-1".to_owned());
        for row in [&parent, &other, &child] {
            agents.store().insert_agent_session(row).unwrap();
        }

        let refused = agents
            .delegate(delegate_params(&other.id, "req-1"), SecretGrant::Provide)
            .unwrap_err();
        assert_eq!(refused.code, codes::REQUEST_INVALID_PARAMS);

        let replayed = agents
            .delegate(delegate_params(&parent.id, "req-1"), SecretGrant::Provide)
            .unwrap();
        assert_eq!(replayed.session.id, child.id);
    }

    #[test]
    fn a_workspace_being_merged_into_refuses_starts_and_every_prompt() {
        let (agents, workspace_id, _profile) = service();
        let row = session_row(&workspace_id, None);
        agents.store().insert_agent_session(&row).unwrap();
        lock(&agents.inner.merging).insert(workspace_id.clone());

        let start = agents
            .start(
                AgentStartParams {
                    workspace: Some(workspace_id.clone()),
                    kind: "any".to_owned(),
                    prompt: None,
                    args: Vec::new(),
                    env: BTreeMap::new(),
                    read_only: true,
                    title: None,
                    cols: None,
                    rows: None,
                    request_id: None,
                    permission_mode: None,
                    model: None,
                    effort: None,
                },
                SecretGrant::Provide,
            )
            .unwrap_err();
        assert_eq!(start.code, codes::AGENT_BUSY);
        assert_eq!(
            agents.prompt(&row.id, "hello", false).unwrap_err().code,
            codes::AGENT_BUSY
        );
        // `force` does not override the merge: the agent could write into the worktree while git merges into it.
        assert_eq!(
            agents.prompt(&row.id, "hello", true).unwrap_err().code,
            codes::AGENT_BUSY
        );

        lock(&agents.inner.merging).remove(&workspace_id);
        assert_eq!(
            agents.prompt(&row.id, "hello", false).unwrap_err().code,
            codes::AGENT_NOT_LIVE
        );
    }

    /// Runs git in `dir` with a fixed identity; panics on failure.
    fn git(dir: &Path, args: &[&str]) {
        let status = massorca_process::command("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.invalid",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(dir)
            .status()
            .expect("spawn git");
        assert!(status.success(), "git {args:?} failed");
    }

    /// Declares kind `broken`, whose program resolves (a file with execute permission) but cannot be started, so its
    /// start fails at the spawn on every platform. Windows refuses a file that is not an executable image. On Unix a
    /// script without a `#!` line would run under `/bin/sh` (the exec fallback), so the file names an interpreter that
    /// does not exist, which makes the exec fail with the spawn.
    fn declare_broken_kind(profile: &std::path::Path, dir: &std::path::Path) {
        let program = dir.join("broken.exe");
        std::fs::write(
            &program,
            "#!/nonexistent/massorca-interpreter\nnot a program\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let kinds = profile.join("agents").join("kinds");
        std::fs::create_dir_all(&kinds).unwrap();
        let kind = serde_json::json!({
            "id": "broken",
            "name": "Broken",
            "program": [program.to_string_lossy()],
            "prompt": { "mode": "argv" },
            "status": { "mechanism": "none" }
        });
        std::fs::write(kinds.join("broken.json"), kind.to_string()).unwrap();
    }

    #[test]
    fn a_start_whose_program_cannot_run_fails_and_keeps_no_session() {
        let (agents, workspace_id, profile) = service();
        let root = tempfile::tempdir().unwrap();
        declare_broken_kind(profile.path(), root.path());

        let failed = agents
            .start(
                AgentStartParams {
                    workspace: Some(workspace_id.clone()),
                    kind: "broken".to_owned(),
                    prompt: None,
                    args: Vec::new(),
                    env: BTreeMap::new(),
                    read_only: true,
                    title: None,
                    cols: None,
                    rows: None,
                    request_id: None,
                    permission_mode: None,
                    model: None,
                    effort: None,
                },
                SecretGrant::Provide,
            )
            .unwrap_err();
        assert_ne!(failed.code, codes::AGENT_NOT_INSTALLED, "{failed}");
        assert!(agents.list(None).unwrap().is_empty());
    }

    #[test]
    fn a_child_whose_start_fails_keeps_one_workspace_and_names_it() {
        let (agents, _workspace_id, profile) = service();
        let root = tempfile::tempdir().unwrap();
        let repo = root.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q"]);
        git(&repo, &["commit", "-q", "--allow-empty", "-m", "initial"]);
        let workspaces = &agents.inner.config.workspaces;
        let project = workspaces
            .add_project(ProjectAddParams {
                icon: None,
                path: repo.to_string_lossy().into_owned(),
                name: Some("repo".to_owned()),
                kind: None,
                workspaces_root: Some(root.path().join("ws").to_string_lossy().into_owned()),
            })
            .unwrap();
        let lead = workspaces
            .create_workspace(WorkspaceCreateParams {
                project: project.id.clone(),
                name: "lead".to_owned(),
                branch: None,
                base: None,
                existing_branch: false,
                refresh_base: false,
                path: None,
                request_id: None,
                no_setup: false,
            })
            .unwrap();
        let parent = session_row(&lead.id, None);
        agents.store().insert_agent_session(&parent).unwrap();
        declare_broken_kind(profile.path(), root.path());
        let params = || AgentDelegateParams {
            kind: Some("broken".to_owned()),
            read_only: false,
            ..delegate_params(&parent.id, "req-broken")
        };

        for _ in 0..2 {
            let failed = agents.delegate(params(), SecretGrant::Provide).unwrap_err();
            assert!(failed.message.contains("lead-1"), "{failed}");
            let data = failed.data.expect("data names the kept workspace");
            assert_eq!(data["workspace"]["name"], "lead-1");
        }

        let names: Vec<String> = workspaces
            .list_workspaces(Some(&project.id))
            .unwrap()
            .into_iter()
            .map(|workspace| workspace.name)
            .collect();
        assert_eq!(names, ["lead", "lead-1"]);
    }

    #[test]
    fn a_quoted_result_is_one_line_without_control_characters_and_within_its_limit() {
        let text = "line one\r\nline\ttwo\u{1b}[31m red\u{7f}\u{85} é";
        let line = one_line(text, 1024);
        assert_eq!(line, "line one  line two [31m red   é");
        assert!(check_prompt(&line).is_ok());
        assert_eq!(one_line("ééé", 5), "éé");
    }

    #[test]
    fn the_stored_depth_is_the_default_when_unset_and_clamped_to_the_range_when_out_of_it() {
        use super::{
            DELEGATION_DEPTH_DEFAULT, DELEGATION_DEPTH_MAX, DELEGATION_DEPTH_MIN,
            delegation_depth_from_stored,
        };

        assert_eq!(delegation_depth_from_stored(None), DELEGATION_DEPTH_DEFAULT);
        assert_eq!(delegation_depth_from_stored(Some("2")), 2);
        assert_eq!(
            delegation_depth_from_stored(Some("5")),
            DELEGATION_DEPTH_MAX
        );
        assert_eq!(
            delegation_depth_from_stored(Some("9")),
            DELEGATION_DEPTH_MAX
        );
        assert_eq!(
            delegation_depth_from_stored(Some("0")),
            DELEGATION_DEPTH_MIN
        );
        assert_eq!(
            delegation_depth_from_stored(Some("-4")),
            DELEGATION_DEPTH_MIN
        );
        // Not a whole number at all: the default applies, since there is no nearest value to clamp to.
        assert_eq!(
            delegation_depth_from_stored(Some("\"3\"")),
            DELEGATION_DEPTH_DEFAULT
        );
        assert_eq!(
            delegation_depth_from_stored(Some("2.5")),
            DELEGATION_DEPTH_DEFAULT
        );
        assert_eq!(
            delegation_depth_from_stored(Some("not json")),
            DELEGATION_DEPTH_DEFAULT
        );
    }

    #[test]
    fn the_limit_is_read_from_the_store_on_every_call() {
        use super::{DELEGATION_DEPTH_DEFAULT, DELEGATION_DEPTH_SETTING};

        let (agents, _workspace_id, _profile) = service();
        assert_eq!(
            agents.delegation_depth_limit().unwrap(),
            DELEGATION_DEPTH_DEFAULT
        );
        agents
            .store()
            .replace_setting("core", DELEGATION_DEPTH_SETTING, "2", &[])
            .unwrap();
        assert_eq!(agents.delegation_depth_limit().unwrap(), 2);
        agents
            .store()
            .replace_setting("core", DELEGATION_DEPTH_SETTING, "9", &[])
            .unwrap();
        assert_eq!(agents.delegation_depth_limit().unwrap(), 5);
    }

    #[test]
    fn a_grandchild_refusal_at_the_configured_limit_names_the_limit_and_the_setting() {
        use super::DELEGATION_DEPTH_SETTING;

        let (agents, workspace_id, _profile) = service();
        let root = session_row(&workspace_id, None);
        let child = session_row(&workspace_id, Some(&root.id));
        let grandchild = session_row(&workspace_id, Some(&child.id));
        for row in [&root, &child, &grandchild] {
            agents.store().insert_agent_session(row).unwrap();
        }
        agents
            .store()
            .replace_setting("core", DELEGATION_DEPTH_SETTING, "2", &[])
            .unwrap();

        // The grandchild sits at depth 2; a great-grandchild would be at depth 3, past the limit of 2.
        let error = agents
            .delegate(
                delegate_params(&grandchild.id, &new_id()),
                SecretGrant::Provide,
            )
            .unwrap_err();

        assert_eq!(error.code, codes::AGENT_DEPTH_EXCEEDED);
        assert!(
            error.message.contains(DELEGATION_DEPTH_SETTING),
            "{}",
            error.message
        );
        assert!(error.message.contains("at most 2"), "{}", error.message);
        let data = error.data.expect("the limit is in the error data");
        assert_eq!(data["limit"], 2);
        assert_eq!(data["setting"], DELEGATION_DEPTH_SETTING);
    }

    fn brought_back_at(agents: &super::Agents, session: &str) -> Option<i64> {
        agents
            .store()
            .get_agent_session(session)
            .unwrap()
            .and_then(|row| row.brought_back_at)
    }

    #[test]
    fn each_level_brings_back_only_its_own_children_with_its_own_run_token() {
        let (agents, workspace_id, _profile) = service();
        let root = session_row(&workspace_id, None);
        let child = session_row(&workspace_id, Some(&root.id));
        let grandchild = session_row(&workspace_id, Some(&child.id));
        for row in [&root, &child, &grandchild] {
            agents.store().insert_agent_session(row).unwrap();
        }
        live_run(&agents, &root.id, "root-token");
        live_run(&agents, &child.id, "child-token");
        let params = |session: &str, token: &str| AgentBringBackParams {
            child: session.to_owned(),
            merge: false,
            prompt: false,
            token: Some(token.to_owned()),
        };

        // The grandchild's parent is the child, so the root's token is refused for it.
        let refused = agents
            .bring_back(&params(&grandchild.id, "root-token"), &NoGit)
            .unwrap_err();
        assert_eq!(refused.code, codes::AGENT_HOOK_REFUSED);
        // The store is read here: showing an exited session drops its live run, which the calls below still need.
        assert!(brought_back_at(&agents, &grandchild.id).is_none());
        // The child's parent is the root, so the child's token is refused for the child.
        let refused = agents
            .bring_back(&params(&child.id, "child-token"), &NoGit)
            .unwrap_err();
        assert_eq!(refused.code, codes::AGENT_HOOK_REFUSED);
        assert!(brought_back_at(&agents, &child.id).is_none());

        agents
            .bring_back(&params(&grandchild.id, "child-token"), &NoGit)
            .unwrap();
        agents
            .bring_back(&params(&child.id, "root-token"), &NoGit)
            .unwrap();
        assert!(
            agents
                .show(&grandchild.id)
                .unwrap()
                .brought_back_at_ms
                .is_some()
        );
        assert!(agents.show(&child.id).unwrap().brought_back_at_ms.is_some());
    }
}
