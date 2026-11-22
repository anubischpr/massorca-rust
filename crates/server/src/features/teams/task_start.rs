//! `task.start` (capability `teams/1`, decision 0072 and its amendment): makes the team's shared folder workspace, starts
//! the bot's agent there with the task as its prompt, and links the session only once the agent started. A failed start keeps the task Open
//! with the reason in `error`.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

use massorca_core_agents::Agents;
use massorca_core_store::{BotRow, Store, TaskRow, TeamRow, now_ms};
use massorca_core_workspace::Workspaces;
use massorca_protocol::methods::agents::AgentStartParams;
use massorca_protocol::methods::events::{
    TOPIC_PROJECT_ADDED, TOPIC_TASK_CHANGED, TOPIC_WORKSPACE_CREATED,
};
use massorca_protocol::methods::projects::{ProjectAddParams, ProjectKind};
use massorca_protocol::methods::tasks::{self, Task, TaskRef};
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::workspace_domain::{for_mutation, publish};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

use super::task::view_of;
use super::task_state;
use super::{blocking, selectors, store_error};

/// The folder project that holds every team's folder, under the workspaces default root.
const TEAMS_FOLDER: &str = "bot-teams";
const TEAMS_PROJECT_NAME: &str = "Bot teams";

static START_GATE: Mutex<()> = Mutex::new(());

pub(crate) fn register(registries: &Registries) -> Result<Vec<RegistrationHandle>, ServiceError> {
    Ok(vec![registries.methods.register(
        Owner::Core,
        tasks::START,
        capabilities::TEAMS,
        Permission::Workspace,
        start,
    )?])
}

/// What a start attempt changed, so the handler publishes it whether the agent started or not.
#[derive(Default)]
struct Changes {
    project: Option<String>,
    workspace: Option<String>,
    task: Option<String>,
}

async fn start(ctx: RequestCtx, params: TaskRef) -> Result<Task, ErrorBody> {
    let workspaces = for_mutation(&ctx).await?;
    let (changes, result) = blocking(&ctx, move |store, agents| {
        let mut changes = Changes::default();
        let result = start_task(store, agents, &workspaces, &params.task, &mut changes);
        Ok((changes, result))
    })
    .await?;
    if let Some(project) = &changes.project {
        publish(&ctx, TOPIC_PROJECT_ADDED, project);
    }
    if let Some(workspace) = &changes.workspace {
        publish(&ctx, TOPIC_WORKSPACE_CREATED, workspace);
    }
    if let Some(task) = &changes.task {
        publish(&ctx, TOPIC_TASK_CHANGED, task);
    }
    result
}

fn start_task(
    store: &Store,
    agents: &Agents,
    workspaces: &Workspaces,
    selector: &str,
    changes: &mut Changes,
) -> Result<Task, ErrorBody> {
    let row = selectors::task(store, selector)?;
    if row.session_id.is_some() || row.outcome.is_some() {
        return Err(ErrorBody::new(
            codes::TASK_STARTED,
            format!(
                "task {} was already started; reopen it to start it again",
                row.id
            ),
        ));
    }
    let team = selectors::team(store, &row.team_id)?;
    let bot = match row.bot_id.as_deref() {
        Some(bot) => selectors::bot(store, &team, bot)?,
        None => {
            return Err(ErrorBody::new(
                codes::TASK_NO_BOT,
                format!("task {} has no bot; give it one before starting it", row.id),
            ));
        }
    };
    // One start at a time, so two starts for one bot cannot both find it free.
    let _gate = START_GATE.lock().unwrap_or_else(PoisonError::into_inner);
    match launch(store, agents, workspaces, &team, &bot, &row, changes) {
        Ok(session) => link(store, agents, &row, &session, changes),
        Err(error) => {
            if store
                .set_task_start_error(&row.id, &error.message, now_ms())
                .map_err(store_error)?
            {
                changes.task = Some(row.id.clone());
            }
            Err(error)
        }
    }
}

/// The team's folder workspace, then the bot's agent in it. Every failure here is a start that did not happen.
fn launch(
    store: &Store,
    agents: &Agents,
    workspaces: &Workspaces,
    team: &TeamRow,
    bot: &BotRow,
    task: &TaskRow,
    changes: &mut Changes,
) -> Result<String, ErrorBody> {
    refuse_busy_bot(store, agents, bot, task)?;
    let workspace = team_folder(store, workspaces, team, changes)?;
    let session = agents.start(AgentStartParams {
        workspace: workspace.id,
        kind: bot.kind.clone(),
        prompt: Some(prompt(team, bot, task)),
        args: Vec::new(),
        env: BTreeMap::new(),
        read_only: false,
        title: Some(task.title.clone()),
        cols: None,
        rows: None,
        // Concurrent starts of one task read the same update time, so they share one session.
        request_id: Some(format!("task.start:{}:{}", task.id, task.updated_at)),
    })?;
    Ok(session.id)
}

/// Links the session. A concurrent start of the same request linked the same session first, which is the same start.
fn link(
    store: &Store,
    agents: &Agents,
    row: &TaskRow,
    session: &str,
    changes: &mut Changes,
) -> Result<Task, ErrorBody> {
    let linked = store
        .link_task_session(&row.id, session, now_ms())
        .map_err(store_error)?;
    let current = selectors::task(store, &row.id)?;
    if !linked && current.session_id.as_deref() != Some(session) {
        return Err(ErrorBody::new(
            codes::TASK_STARTED,
            format!("task {} was started by another call", row.id),
        ));
    }
    changes.task = Some(row.id.clone());
    Ok(view_of(&current, &task_state::load(agents, &current)?))
}

/// A bot works on one task at a time: another of its tasks whose session still runs (or cannot be verified) makes it busy.
fn refuse_busy_bot(
    store: &Store,
    agents: &Agents,
    bot: &BotRow,
    task: &TaskRow,
) -> Result<(), ErrorBody> {
    for other in store.list_tasks_of_bot(&bot.id).map_err(store_error)? {
        if other.id != task.id
            && other.session_id.is_some()
            && task_state::is_running(&task_state::load(agents, &other)?)
        {
            return Err(ErrorBody::new(
                codes::BOT_BUSY,
                format!(
                    "bot {} is working on task {}; start this one when it is done",
                    bot.name, other.title
                ),
            ));
        }
    }
    Ok(())
}

/// The team's folder workspace: `<workspaces root>/bot-teams/<team id>`, in the "Bot teams" folder project, shared by
/// the team's bots (several writers). Made on the first start; made again if the workspace was removed.
fn team_folder(
    store: &Store,
    workspaces: &Workspaces,
    team: &TeamRow,
    changes: &mut Changes,
) -> Result<Workspace, ErrorBody> {
    if let Some(id) = store.team_workspace(&team.id).map_err(store_error)?
        && let Ok(workspace) = workspaces.show_workspace(&id)
    {
        // Idempotent; covers a start that stopped between making the workspace and sharing it.
        workspaces.share_writers(&workspace.id)?;
        return Ok(workspace);
    }
    let root = workspaces.default_root().join(TEAMS_FOLDER);
    let folder = root.join(&team.id);
    std::fs::create_dir_all(&folder).map_err(|error| {
        ErrorBody::new(
            codes::INTERNAL,
            format!(
                "cannot create the team folder {}: {error}",
                folder.display()
            ),
        )
    })?;
    let project = match workspaces.show_project(&path_text(&root)?) {
        Ok(project) => project,
        Err(_) => {
            let project = workspaces.add_project(ProjectAddParams {
                path: path_text(&root)?,
                name: Some(TEAMS_PROJECT_NAME.to_owned()),
                kind: Some(ProjectKind::Folder),
                workspaces_root: None,
            })?;
            changes.project = Some(project.id.clone());
            project
        }
    };
    let workspace = workspaces.create_workspace(WorkspaceCreateParams {
        project: project.id,
        name: team.id.clone(),
        branch: None,
        base: None,
        existing_branch: false,
        path: Some(path_text(&folder)?),
        request_id: Some(format!("team.folder:{}", team.id)),
    })?;
    workspaces.share_writers(&workspace.id)?;
    store
        .set_team_workspace(&team.id, &workspace.id)
        .map_err(store_error)?;
    changes.workspace = Some(workspace.id.clone());
    Ok(workspace)
}

fn path_text(path: &Path) -> Result<String, ErrorBody> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        ErrorBody::new(
            codes::INTERNAL,
            format!(
                "the team folder path is not valid Unicode: {}",
                path.display()
            ),
        )
    })
}

/// The bot's first prompt: who it is, its team and job, then the task.
pub(crate) fn prompt(team: &TeamRow, bot: &BotRow, task: &TaskRow) -> String {
    let mut text = format!(
        "You are {}, a bot in the team {}.\nYour job: {}\n",
        bot.name, team.name, bot.job
    );
    if !team.purpose.is_empty() {
        text.push_str(&format!("The team's purpose: {}\n", team.purpose));
    }
    text.push_str(&format!("\nTask: {}\n", task.title));
    if !task.body.is_empty() {
        text.push_str(&format!("\n{}\n", task.body));
    }
    text
}
