//! Starts an automation run's agent (decision 0125): the automation's prompt and agent, through the same agent start a
//! task uses, in the place its destination names (decision 0127). By default a git project runs in one worktree kept for
//! the automation, made on its first run, and a folder project runs in its folder. The run gets no vault secrets and the
//! agent's default permission mode.

use std::collections::BTreeMap;

use massorca_core_agents::{Agents, SecretGrant};
use massorca_core_store::{AutomationRow, AutomationRunRow, Store, destination, now_ms};
use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::agents::AgentStartParams;
use massorca_protocol::methods::projects::{Project, ProjectKind};
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};
use massorca_protocol::{ErrorBody, codes};

use super::completion::settle_session;
use crate::features::teams::worktree_name;

/// Why a run receives no vault secrets: an automation records no host-admin creator.
const WITHHELD: &str =
    "vault secrets do not go to automation runs: an automation records no host-admin creator";
/// The request id of a run's agent start is this and the run id, so a retry, or the sweep at start, finds its session.
pub(super) const RUN_REQUEST_PREFIX: &str = "automation.run:";
/// The name of a folder project's workspace when the project has none at its own path.
const FOLDER_WORKSPACE: &str = "automations";

/// Starts the agent of a stored dispatching run and records what happened on it: dispatched with its session, or failed
/// with the reason. It never fails its caller. Returns the workspace it made, if any, so the caller can announce it.
pub(super) fn dispatch(
    store: &Store,
    agents: &Agents,
    workspaces: &Workspaces,
    automation: &AutomationRow,
    run: &AutomationRunRow,
) -> Option<String> {
    let (started, made) = match start(agents, workspaces, automation, run) {
        Ok((session, workspace, made)) => (Ok((session, workspace)), made),
        Err(error) => (Err(error), None),
    };
    match started {
        Ok((session, workspace)) => {
            record(|| {
                store.mark_automation_run_dispatched(&run.id, &session, &workspace, now_ms())
            });
            // A Done that landed before the run named its session would never reach the listener otherwise.
            if let Ok(view) = agents.show(&session)
                && let Err(error) = settle_session(store, &view)
            {
                tracing::warn!(code = %error.code, "a run that ended at once could not be settled: {}", error.message);
            }
        }
        Err(error) => {
            record(|| store.fail_automation_run(&run.id, &error.code, &error.message, now_ms()));
        }
    }
    made
}

/// Writes a run's state, trying once more when the write fails. A run still dispatching is failed at the next service
/// start, so its state never lives only in the log.
fn record(mut write: impl FnMut() -> Result<bool, massorca_core_store::StoreError>) {
    for attempt in 1..=2 {
        match write() {
            Ok(_) => return,
            Err(error) => {
                tracing::warn!(
                    attempt,
                    "an automation run's state could not be saved: {error}"
                );
            }
        }
    }
}

/// The session and workspace of the started agent, and the workspace made for it when one was made.
fn start(
    agents: &Agents,
    workspaces: &Workspaces,
    automation: &AutomationRow,
    run: &AutomationRunRow,
) -> Result<(String, String, Option<String>), ErrorBody> {
    let project_id = automation.project_id.as_deref().ok_or_else(|| {
        ErrorBody::new(
            codes::AUTOMATION_NO_PROJECT,
            "the automation has no project",
        )
    })?;
    let project = workspaces.show_project(project_id)?;
    let (workspace, made) = place(workspaces, automation, &project, run)?;
    let session = agents.start(
        AgentStartParams {
            workspace: Some(workspace.id.clone()),
            kind: automation.agent_kind.clone(),
            prompt: Some(automation.prompt.clone()),
            args: Vec::new(),
            env: BTreeMap::new(),
            read_only: false,
            title: Some(automation.name.clone()),
            cols: None,
            rows: None,
            // A retry of this run finds its session instead of starting a second one.
            request_id: Some(format!("{RUN_REQUEST_PREFIX}{}", run.id)),
            permission_mode: None,
            model: None,
            fast: None,
            effort: None,
        },
        SecretGrant::Withhold(WITHHELD),
    )?;
    let made = made.then(|| workspace.id.clone());
    Ok((session.id, workspace.id, made))
}

/// Where the automation's runs work, and whether this call made it: its destination's workspace, a worktree made for
/// this run, or the project's own place.
fn place(
    workspaces: &Workspaces,
    automation: &AutomationRow,
    project: &Project,
    run: &AutomationRunRow,
) -> Result<(Workspace, bool), ErrorBody> {
    match automation.destination.as_str() {
        destination::EXISTING_WORKSPACE => {
            let id = automation
                .destination_workspace_id
                .as_deref()
                .unwrap_or_default();
            Ok((workspaces.show_workspace(id)?, false))
        }
        destination::PROJECT_WORKTREE_PER_RUN => run_worktree(workspaces, automation, project, run),
        _ => project_place(workspaces, automation, project),
    }
}

/// A new worktree for one run, titled with the automation's name and started from its base branch when it has one. Its
/// request id is the run's, so a retry finds the worktree the first try made.
fn run_worktree(
    workspaces: &Workspaces,
    automation: &AutomationRow,
    project: &Project,
    run: &AutomationRunRow,
) -> Result<(Workspace, bool), ErrorBody> {
    let created = workspaces.create_workspace_once(WorkspaceCreateParams {
        project: project.id.clone(),
        name: automation.name.clone(),
        branch: None,
        base: automation.base_branch.clone(),
        existing_branch: false,
        refresh_base: false,
        path: None,
        request_id: Some(format!("automation.run-worktree:{}", run.id)),
        no_setup: false,
        unique_name: true,
        name_generated: false,
        in_place: false,
    })?;
    Ok((created.workspace, !created.repeated))
}

/// The project's own place (decision 0125): one worktree kept for the automation, or the folder's workspace.
fn project_place(
    workspaces: &Workspaces,
    automation: &AutomationRow,
    project: &Project,
) -> Result<(Workspace, bool), ErrorBody> {
    match project.kind {
        ProjectKind::Git => {
            let created = workspaces.create_workspace_once(WorkspaceCreateParams {
                project: project.id.clone(),
                name: worktree_name(&automation.name, &id_tail(&automation.id)),
                branch: None,
                base: None,
                existing_branch: false,
                refresh_base: false,
                path: None,
                request_id: Some(format!("automation.worktree:{}", automation.id)),
                no_setup: false,
                unique_name: false,
                name_generated: false,
                in_place: false,
            })?;
            Ok((created.workspace, !created.repeated))
        }
        ProjectKind::Folder => folder(workspaces, project),
        ProjectKind::Unknown => Err(ErrorBody::new(
            codes::REQUEST_INVALID_PARAMS,
            format!(
                "project {} is of a kind this host cannot run in",
                project.name
            ),
        )),
    }
}

/// The random end of an automation's id: a worktree name keeps six characters of it, so two automations with one
/// name get two worktrees.
fn id_tail(automation_id: &str) -> String {
    let alnum: Vec<char> = automation_id
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    alnum[alnum.len().saturating_sub(6)..].iter().collect()
}

/// A folder project's workspace at the project's own path, made when it has none; shared by several writers.
fn folder(workspaces: &Workspaces, project: &Project) -> Result<(Workspace, bool), ErrorBody> {
    let found = workspaces
        .list_workspaces(Some(&project.id))?
        .into_iter()
        .find(|workspace| workspace.path == project.path);
    let (workspace, made) = match found {
        Some(workspace) => (workspace, false),
        None => {
            let created = workspaces.create_workspace_once(WorkspaceCreateParams {
                project: project.id.clone(),
                name: FOLDER_WORKSPACE.to_owned(),
                branch: None,
                base: None,
                existing_branch: false,
                refresh_base: false,
                path: None,
                request_id: Some(format!("automation.folder:{}", project.id)),
                no_setup: false,
                unique_name: true,
                name_generated: false,
                in_place: false,
            })?;
            (created.workspace, !created.repeated)
        }
    };
    workspaces.share_writers(&workspace.id)?;
    Ok((workspace, made))
}

#[cfg(test)]
mod tests {
    use super::id_tail;
    use crate::features::teams::worktree_name;

    #[test]
    fn the_worktree_is_named_after_the_automation_and_the_end_of_its_id() {
        let tail = id_tail("0192f3a4-7b1c-7e2d-9f00-1a2b3c4d5e6f");
        assert_eq!(tail, "4d5e6f");
        assert_eq!(worktree_name("Morning PRs", &tail), "morning-prs-4d5e6f");
    }
}
