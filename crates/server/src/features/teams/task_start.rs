//! `task.start` (capability `teams/1`, decision 0072 and its amendment): makes the team's shared folder workspace, starts
//! the bot's agent there with the task as its prompt, and links the session only once the agent started. A failed start keeps the task Open
//! with the reason in `error`, and a session spawned but not linked is stopped.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use massorca_core_agents::{Agents, SecretGrant};
use massorca_core_store::{BotRow, Store, TaskRow, TeamProjectRow, TeamRow, new_id, now_ms};
use massorca_core_workspace::{DomainError, Workspaces};
use massorca_protocol::methods::agents::AgentStartParams;
use massorca_protocol::methods::events::{
    ChangeEvent, TOPIC_PROJECT_ADDED, TOPIC_TASK_CHANGED, TOPIC_WORKSPACE_CREATED,
};
use massorca_protocol::methods::projects::{ProjectAddParams, ProjectKind};
use massorca_protocol::methods::tasks::{self, Task, TaskRef};
use massorca_protocol::methods::workspaces::{Workspace, WorkspaceCreateParams};
use massorca_protocol::{ErrorBody, capabilities, codes};

use crate::context::{AppState, Registries, RequestCtx};
use crate::error::ServiceError;
use crate::features::agents::secret_grant;
use crate::features::workspace_domain::for_mutation;
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

use super::task::{removed, view_of};
use super::task_outcome::latch_at_link;
use super::task_state;
use super::team::ACCESS_FULL;
use super::{blocking, selectors, store_error};

/// The folder project that holds every team's folder, under the workspaces default root.
const TEAMS_FOLDER: &str = "bot-teams";
const TEAMS_PROJECT_NAME: &str = "Bot teams";

static START_GATE: Mutex<()> = Mutex::new(());

/// Failed starts in a row after which a task is refused until it is reopened (AUTO-16; pv96 trips at three).
const CIRCUIT_BREAK_FAILURES: u32 = 3;

/// Serialises task starts with the calls that read whether a task runs and then write: removal, done, reopen, a bot
/// change, and an agent resume, which can revive a task's session. One gate for every team: starts are short.
pub(crate) fn start_gate() -> MutexGuard<'static, ()> {
    START_GATE.lock().unwrap_or_else(PoisonError::into_inner)
}

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
pub(super) struct Changes {
    pub(super) project: Option<String>,
    pub(super) workspace: Option<String>,
    pub(super) task: Option<String>,
}

/// Publishes what a start or a routine run changed. Called only after the change is committed.
pub(super) fn publish_changes(state: &AppState, changes: &Changes) {
    let changed = [
        (TOPIC_PROJECT_ADDED, &changes.project),
        (TOPIC_WORKSPACE_CREATED, &changes.workspace),
        (TOPIC_TASK_CHANGED, &changes.task),
    ];
    for (topic, id) in changed {
        if let Some(id) = id {
            state.events.publish(ChangeEvent {
                topic: topic.to_owned(),
                id: id.clone(),
            });
        }
    }
}

async fn start(ctx: RequestCtx, params: TaskRef) -> Result<Task, ErrorBody> {
    let workspaces = for_mutation(&ctx).await?;
    // The run gets the vault's variables only when this caller could start an agent with them itself.
    let grant = secret_grant(&ctx);
    let (changes, result) = blocking(&ctx, move |store, agents| {
        let mut changes = Changes::default();
        let result = start_task(
            store,
            agents,
            &workspaces,
            &params.task,
            grant,
            &mut changes,
        );
        Ok((changes, result))
    })
    .await?;
    publish_changes(&ctx.state, &changes);
    result
}

pub(super) fn start_task(
    store: &Store,
    agents: &Agents,
    workspaces: &Workspaces,
    selector: &str,
    grant: SecretGrant,
    changes: &mut Changes,
) -> Result<Task, ErrorBody> {
    // One start at a time, so two starts for one bot cannot both find it free.
    let _gate = start_gate();
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
    // The circuit breaker (AUTO-16): after three failed starts in a row, only a reopen lets the task start again.
    if store.task_start_failures(&row.id).map_err(store_error)? >= CIRCUIT_BREAK_FAILURES {
        return Err(ErrorBody::new(
            codes::TASK_CIRCUIT_BROKEN,
            row.error.clone().unwrap_or_else(|| {
                format!("Circuit broken: task {} failed to start too often", row.id)
            }),
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
    let attempt = store
        .begin_task_start_attempt(&row.id, &new_id())
        .map_err(store_error)?
        .ok_or_else(removed)?;
    let stop = |session: &str| {
        agents
            .stop(session, true)
            .map(drop)
            .map_err(ErrorBody::from)
    };
    let place = match prepare(store, workspaces, &team, &row, &attempt, changes) {
        Ok(place) => place,
        Err(error) => {
            // Nothing was spawned, so the attempt is settled.
            store
                .end_task_start_attempt(&row.id, &attempt)
                .map_err(store_error)?;
            return Err(record_failure(store, &row, error, changes)?);
        }
    };
    store
        .set_task_place(
            &row.id,
            &place.workspace.id,
            place.branch().map(String::as_str),
            now_ms(),
        )
        .map_err(store_error)?;
    changes.task = Some(row.id.clone());
    let request = request_id(&row.id, &attempt);
    let project = match row.project_id.as_deref() {
        Some(id) => store.get_team_project(id).map_err(store_error)?,
        None => None,
    };
    let text = prompt(
        &team,
        &bot,
        &row,
        project.as_ref(),
        place.branch().map(String::as_str),
    );
    let session = match spawn(agents, &place, &team, &bot, &row, text, &request, grant) {
        Ok(session) => session,
        Err(error) => {
            return Err(after_failed_start(
                store, stop, &row, &attempt, &request, error, changes,
            )?);
        }
    };
    settle(store, stop, &row, &attempt, &session, changes)?;
    // Read after the link: a failure here is an error to the caller, never a reason to stop the linked run.
    let mut current = selectors::task(store, &row.id)?;
    let shown = task_state::load(agents, &current)?;
    // The agent may have finished before the link, when the listener could not find the task yet.
    if latch_at_link(store, &current.id, &shown).map_err(store_error)? {
        current = selectors::task(store, &current.id)?;
    }
    Ok(view_of(&current, &shown))
}

/// The agent request a start attempt is keyed by.
fn request_id(task: &str, attempt: &str) -> String {
    format!("task.start:{task}:{attempt}")
}

/// A start that failed after its spawn point may have left a session; it is found by its request id and abandoned. When
/// none exists the attempt is settled. When the lookup fails the attempt is kept, so a retry names the same request.
fn after_failed_start(
    store: &Store,
    stop: impl FnOnce(&str) -> Result<(), ErrorBody>,
    row: &TaskRow,
    attempt: &str,
    request: &str,
    error: ErrorBody,
    changes: &mut Changes,
) -> Result<ErrorBody, ErrorBody> {
    match store.find_agent_session_by_request_id(request) {
        Ok(Some(spawned)) => abandon(store, stop, row, attempt, &spawned.id, error, changes),
        Ok(None) => {
            store
                .end_task_start_attempt(&row.id, attempt)
                .map_err(store_error)?;
            record_failure(store, row, error, changes)
        }
        Err(_) => record_failure(store, row, error, changes),
    }
}

/// Where a start runs a task.
struct Place {
    workspace: Workspace,
    /// The task has its own git worktree, so its result is that worktree's branch.
    own_worktree: bool,
}

impl Place {
    fn branch(&self) -> Option<&String> {
        self.workspace.branch.as_ref().filter(|_| self.own_worktree)
    }
}

/// The workspace the task runs in: its own worktree of a linked git project, a linked folder, or the team's folder when
/// the task's project links no source (or its source was removed).
fn prepare(
    store: &Store,
    workspaces: &Workspaces,
    team: &TeamRow,
    task: &TaskRow,
    attempt: &str,
    changes: &mut Changes,
) -> Result<Place, ErrorBody> {
    if let Some(place) = linked_place(store, workspaces, task, attempt, changes)? {
        return Ok(place);
    }
    Ok(Place {
        workspace: team_folder(store, workspaces, team, changes)?,
        own_worktree: false,
    })
}

/// The workspace of the task's linked project source, or `None` when the task has no project or it links no source.
fn linked_place(
    store: &Store,
    workspaces: &Workspaces,
    task: &TaskRow,
    attempt: &str,
    changes: &mut Changes,
) -> Result<Option<Place>, ErrorBody> {
    let Some(project_id) = task.project_id.as_deref() else {
        return Ok(None);
    };
    let Some(source) = store
        .get_team_project(project_id)
        .map_err(store_error)?
        .and_then(|project| project.source_project_id)
    else {
        return Ok(None);
    };
    let Some(project) = unless_missing(workspaces.show_project(&source), codes::PROJECT_NOT_FOUND)?
    else {
        return Ok(None);
    };
    let place = match project.kind {
        ProjectKind::Git => Place {
            workspace: task_worktree(workspaces, &project.id, task, attempt, changes)?,
            own_worktree: true,
        },
        ProjectKind::Folder => Place {
            workspace: linked_folder(workspaces, &project.id, &project.path, changes)?,
            own_worktree: false,
        },
        // A kind this build does not know runs in the team folder rather than guess how to share it.
        ProjectKind::Unknown => return Ok(None),
    };
    Ok(Some(place))
}

/// The task's own worktree, made the way delegation makes a child's: through the Workspaces entry, which picks the
/// project's base. Named after the task title and the attempt, so a retry of one attempt finds its worktree and a
/// later start makes a new one.
fn task_worktree(
    workspaces: &Workspaces,
    project: &str,
    task: &TaskRow,
    attempt: &str,
    changes: &mut Changes,
) -> Result<Workspace, ErrorBody> {
    let workspace = workspaces.create_workspace(WorkspaceCreateParams {
        project: project.to_owned(),
        name: worktree_name(&task.title, attempt),
        branch: None,
        base: None,
        existing_branch: false,
        refresh_base: false,
        path: None,
        request_id: Some(format!("task.worktree:{}:{attempt}", task.id)),
        no_setup: false,
    })?;
    changes.workspace = Some(workspace.id.clone());
    Ok(workspace)
}

/// A folder project's workspace at the project's own path, made when it has none; shared by several writers.
fn linked_folder(
    workspaces: &Workspaces,
    project: &str,
    path: &str,
    changes: &mut Changes,
) -> Result<Workspace, ErrorBody> {
    let found = workspaces
        .list_workspaces(Some(project))?
        .into_iter()
        .find(|workspace| workspace.path == path);
    let workspace = match found {
        Some(workspace) => workspace,
        None => {
            let workspace = workspaces.create_workspace(WorkspaceCreateParams {
                project: project.to_owned(),
                name: TEAMS_FOLDER.to_owned(),
                branch: None,
                base: None,
                existing_branch: false,
                refresh_base: false,
                path: None,
                request_id: Some(format!("team.linked_folder:{project}")),
                no_setup: false,
            })?;
            changes.workspace = Some(workspace.id.clone());
            workspace
        }
    };
    workspaces.share_writers(&workspace.id)?;
    Ok(workspace)
}

/// Longest title part of a task worktree's name.
const WORKTREE_SLUG_MAX_CHARS: usize = 40;

/// `<title slug>-<6 characters of the attempt>`, or `child-<6>` when the title has no letters or digits, as a delegated
/// child's worktree is named.
pub(crate) fn worktree_name(title: &str, attempt: &str) -> String {
    let mut slug = String::new();
    for ch in title.chars().map(fold_accent) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug: String = slug
        .trim_matches('-')
        .chars()
        .take(WORKTREE_SLUG_MAX_CHARS)
        .collect();
    let slug = slug.trim_end_matches('-');
    let suffix: String = attempt
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(6)
        .collect::<String>()
        .to_ascii_lowercase();
    if slug.is_empty() {
        format!("child-{suffix}")
    } else {
        format!("{slug}-{suffix}")
    }
}

/// The base letter of a common accented Latin letter; any other character as it is.
fn fold_accent(ch: char) -> char {
    match ch {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
        'ñ' | 'Ñ' => 'n',
        'ç' | 'Ç' => 'c',
        'ý' | 'ÿ' | 'Ý' => 'y',
        other => other,
    }
}

/// Starts the bot's agent. A failure here is a start that spawned nothing.
#[allow(clippy::too_many_arguments)]
fn spawn(
    agents: &Agents,
    place: &Place,
    team: &TeamRow,
    bot: &BotRow,
    task: &TaskRow,
    prompt: String,
    request: &str,
    grant: SecretGrant,
) -> Result<String, ErrorBody> {
    let session = agents.start(
        AgentStartParams {
            workspace: place.workspace.id.clone(),
            kind: bot.kind.clone(),
            prompt: Some(prompt),
            args: Vec::new(),
            env: BTreeMap::new(),
            read_only: false,
            title: Some(task.title.clone()),
            cols: None,
            rows: None,
            // A retry of an attempt that left a live run finds that run instead of starting a second one.
            request_id: Some(request.to_owned()),
            permission_mode: team_mode(agents, team, bot),
        },
        grant,
    )?;
    Ok(session.id)
}

/// The mode a team's bot runs in (decision 0072, amendment "team access"; decision 0083): a full-access team names the
/// first bypass mode its kind declares that a start can apply; a team with default access, or a kind with no such mode,
/// passes none, so the vendor CLI starts in its own default.
fn team_mode(agents: &Agents, team: &TeamRow, bot: &BotRow) -> Option<String> {
    if team.access != ACCESS_FULL {
        return None;
    }
    let mode = agents.startable_bypass_mode(&bot.kind);
    if mode.is_none() {
        tracing::info!(
            kind = %bot.kind,
            "a full-access team's bot runs in its agent's default mode: the kind has no bypass mode a start can apply"
        );
    }
    mode
}

/// Links the spawned session to the task. When that fails, the session is abandoned so no unlinked process survives.
fn settle(
    store: &Store,
    stop: impl FnOnce(&str) -> Result<(), ErrorBody>,
    row: &TaskRow,
    attempt: &str,
    session: &str,
    changes: &mut Changes,
) -> Result<(), ErrorBody> {
    let error = match store.link_task_session(&row.id, session, now_ms()) {
        Ok(true) => {
            changes.task = Some(row.id.clone());
            return Ok(());
        }
        Ok(false) => unlinked(store, &row.id),
        Err(error) => store_error(error),
    };
    Err(abandon(store, stop, row, attempt, session, error, changes)?)
}

/// Why a link wrote no row: the task is gone, was marked finished during the start, or another start linked it.
fn unlinked(store: &Store, id: &str) -> ErrorBody {
    match store.get_task(id) {
        Ok(None) => removed(),
        Ok(Some(current)) if current.session_id.is_none() && current.outcome.is_some() => {
            ErrorBody::new(
                codes::TASK_CHANGED,
                format!("task {id} was marked finished while it started"),
            )
        }
        Ok(Some(_)) | Err(_) => ErrorBody::new(
            codes::TASK_STARTED,
            format!("task {id} was started by another call"),
        ),
    }
}

/// Stops a session its task does not name and records `error`. The attempt is settled once the stop succeeded; a failed
/// stop keeps it, so a retry names the same run instead of starting a second one.
fn abandon(
    store: &Store,
    stop: impl FnOnce(&str) -> Result<(), ErrorBody>,
    row: &TaskRow,
    attempt: &str,
    session: &str,
    error: ErrorBody,
    changes: &mut Changes,
) -> Result<ErrorBody, ErrorBody> {
    let error = match stop(session) {
        Ok(()) => {
            store
                .end_task_start_attempt(&row.id, attempt)
                .map_err(store_error)?;
            error
        }
        Err(stop_error) => ErrorBody::new(
            &error.code,
            format!(
                "{}; its agent session {session} could not be stopped: {}",
                error.message, stop_error.message
            ),
        ),
    };
    record_failure(store, row, error, changes)
}

/// Records a failed start on the task when it still exists without a session, and gives the error back.
fn record_failure(
    store: &Store,
    row: &TaskRow,
    error: ErrorBody,
    changes: &mut Changes,
) -> Result<ErrorBody, ErrorBody> {
    // The failure that trips the breaker is recorded as such, with the reason it failed, as pv96 words it.
    let failures = store.task_start_failures(&row.id).map_err(store_error)?;
    let error = if failures + 1 >= CIRCUIT_BREAK_FAILURES {
        ErrorBody::new(
            codes::TASK_CIRCUIT_BROKEN,
            format!("Circuit broken: {}", error.message),
        )
    } else {
        error
    };
    if store
        .set_task_start_error(&row.id, &error.code, &error.message, now_ms())
        .map_err(store_error)?
    {
        changes.task = Some(row.id.clone());
    }
    Ok(error)
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
        && let Some(workspace) =
            unless_missing(workspaces.show_workspace(&id), codes::WORKSPACE_NOT_FOUND)?
    {
        // Idempotent; covers a start that stopped between making the workspace and sharing it.
        workspaces.share_writers(&workspace.id)?;
        return Ok(workspace);
    }
    let root = workspaces.default_root().join(TEAMS_FOLDER);
    let folder = root.join(&team.id);
    // Checked before the folder is made, so nothing is written inside a project that is not ours.
    let found = unless_missing(
        workspaces.show_project(&path_text(&root)?),
        codes::PROJECT_NOT_FOUND,
    )?;
    if let Some(project) = found
        .as_ref()
        .filter(|project| project.kind != ProjectKind::Folder)
    {
        return Err(ErrorBody::new(
            codes::TEAM_FOLDER_TAKEN,
            format!(
                "{} is already added as project {}, which is not a folder project; remove that project to run team tasks",
                root.display(),
                project.name
            ),
        ));
    }
    std::fs::create_dir_all(&folder).map_err(|error| {
        ErrorBody::new(
            codes::INTERNAL,
            format!(
                "cannot create the team folder {}: {error}",
                folder.display()
            ),
        )
    })?;
    let project = match found {
        Some(project) => project,
        None => {
            let project = workspaces.add_project(ProjectAddParams {
                icon: None,
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
        refresh_base: false,
        path: Some(path_text(&folder)?),
        request_id: Some(format!("team.folder:{}", team.id)),
        no_setup: false,
    })?;
    workspaces.share_writers(&workspace.id)?;
    store
        .set_team_workspace(&team.id, &workspace.id)
        .map_err(store_error)?;
    changes.workspace = Some(workspace.id.clone());
    Ok(workspace)
}

/// `None` for a lookup refused with `missing`; any other failure refuses the start.
fn unless_missing<T>(found: Result<T, DomainError>, missing: &str) -> Result<Option<T>, ErrorBody> {
    match found {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.code() == missing => Ok(None),
        Err(error) => Err(ErrorBody::from(error)),
    }
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

/// The bot's first prompt: who it is, its team and job, the task, then the project's goal and brief when the task is in a
/// project (as pv96 gives them), and the branch when it works in its own worktree.
pub(crate) fn prompt(
    team: &TeamRow,
    bot: &BotRow,
    task: &TaskRow,
    project: Option<&TeamProjectRow>,
    branch: Option<&str>,
) -> String {
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
    if let Some(project) = project {
        let brief = project.brief.trim();
        let read = if brief.is_empty() {
            ""
        } else {
            " Read its brief before you start."
        };
        text.push_str(&format!(
            "\nThis task is part of the project \"{}\".{read}\n",
            project.name
        ));
        if !project.goal.trim().is_empty() {
            text.push_str(&format!("Goal: {}\n", project.goal.trim()));
        }
        if !brief.is_empty() {
            text.push_str(&format!("\nBrief:\n{brief}\n"));
        }
    }
    if let Some(branch) = branch {
        text.push_str(&format!(
            "\nYou work in your own git worktree on the branch {branch}. It was made from the project's base, without \
             the uncommitted changes of the main checkout. Commit your work on this branch; the owner merges it.\n"
        ));
    }
    text
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use massorca_core_store::{
        AgentSessionRow, ProjectKind as StoredProjectKind, ProjectRow, TaskOrigin, TaskOutcome,
        WorkspaceKind, WorkspaceRow, WorkspaceState,
    };

    use super::*;

    fn task_in(store: &Store) -> TaskRow {
        let id = new_id();
        let team = TeamRow {
            name: id.clone(),
            id,
            purpose: String::new(),
            character: "triangle".to_owned(),
            access: "full".to_owned(),
            workspaces: false,
            created_at: 1,
        };
        store.insert_team(&team).expect("team");
        let task = TaskRow {
            id: new_id(),
            team_id: team.id,
            project_id: None,
            bot_id: None,
            title: "parse".to_owned(),
            body: String::new(),
            origin: TaskOrigin::Cli,
            session_id: None,
            outcome: None,
            error: None,
            error_code: None,
            workspace_id: None,
            branch: None,
            created_at: 1,
            updated_at: 1,
        };
        store.insert_task(&task).expect("task");
        task
    }

    fn attempt_of(store: &Store, task: &TaskRow) -> Option<String> {
        store
            .begin_task_start_attempt(&task.id, "probe")
            .expect("attempt")
            .filter(|attempt| attempt != "probe")
    }

    #[test]
    fn a_linked_session_is_kept_and_settles_the_attempt() {
        let store = Store::open_in_memory().expect("store");
        let task = task_in(&store);
        let attempt = store
            .begin_task_start_attempt(&task.id, "attempt-1")
            .expect("attempt")
            .expect("task");
        let stopped = RefCell::new(Vec::new());
        let stop = |session: &str| {
            stopped.borrow_mut().push(session.to_owned());
            Ok(())
        };

        settle(
            &store,
            stop,
            &task,
            &attempt,
            "session-1",
            &mut Changes::default(),
        )
        .expect("linked");

        let current = store.get_task(&task.id).expect("read").expect("task");
        assert_eq!(current.session_id.as_deref(), Some("session-1"));
        assert!(stopped.borrow().is_empty());
        assert_eq!(attempt_of(&store, &task), None);
    }

    #[test]
    fn a_session_that_loses_the_link_to_another_is_stopped() {
        let store = Store::open_in_memory().expect("store");
        let task = task_in(&store);
        let attempt = store
            .begin_task_start_attempt(&task.id, "attempt-1")
            .expect("attempt")
            .expect("task");
        // Another session was linked first.
        assert!(store.link_task_session(&task.id, "other", 2).expect("link"));
        let stopped = RefCell::new(Vec::new());
        let stop = |session: &str| {
            stopped.borrow_mut().push(session.to_owned());
            Ok(())
        };

        let error = settle(
            &store,
            stop,
            &task,
            &attempt,
            "session-1",
            &mut Changes::default(),
        )
        .expect_err("not linked");

        assert_eq!(error.code, codes::TASK_STARTED);
        assert_eq!(*stopped.borrow(), vec!["session-1".to_owned()]);
        let kept = store.get_task(&task.id).expect("read").expect("task");
        assert_eq!(kept.session_id.as_deref(), Some("other"));
    }

    #[test]
    fn a_removed_task_stops_its_session_and_names_a_failed_stop() {
        let store = Store::open_in_memory().expect("store");
        let task = task_in(&store);
        let attempt = store
            .begin_task_start_attempt(&task.id, "attempt-1")
            .expect("attempt")
            .expect("task");
        assert!(store.delete_task(&task.id).expect("delete"));
        let stopped = RefCell::new(Vec::new());
        let stop = |session: &str| {
            stopped.borrow_mut().push(session.to_owned());
            Ok(())
        };
        let error = settle(
            &store,
            stop,
            &task,
            &attempt,
            "session-1",
            &mut Changes::default(),
        )
        .expect_err("task gone");
        assert_eq!(error.code, codes::TASK_NOT_FOUND);
        assert_eq!(*stopped.borrow(), vec!["session-1".to_owned()]);

        // A stop that fails is named in the error.
        let task = task_in(&store);
        let attempt = store
            .begin_task_start_attempt(&task.id, "attempt-2")
            .expect("attempt")
            .expect("task");
        assert!(store.delete_task(&task.id).expect("delete"));
        let failing = |_: &str| {
            Err(ErrorBody::new(
                codes::INTERNAL,
                "the terminal did not answer",
            ))
        };
        let error = settle(
            &store,
            failing,
            &task,
            &attempt,
            "session-2",
            &mut Changes::default(),
        )
        .expect_err("task gone");
        assert_eq!(error.code, codes::TASK_NOT_FOUND);
        assert!(error.message.contains("could not be stopped"));
    }

    /// An agent session row keyed by `request`, as a start that failed after its spawn point leaves it.
    fn spawned_session(store: &Store, request: &str) -> String {
        let project = ProjectRow {
            id: new_id(),
            kind: StoredProjectKind::Folder,
            path: format!("/work/{}", new_id()),
            path_key: new_id(),
            name: new_id(),
            workspaces_root: "/work/root".to_owned(),
            base_ref: None,
            icon: None,
            added_at: 1,
        };
        store.insert_project(&project).expect("project");
        let workspace = WorkspaceRow {
            id: new_id(),
            project_id: project.id,
            kind: WorkspaceKind::Folder,
            name: "main".to_owned(),
            path: "/work/team".to_owned(),
            path_key: new_id(),
            branch: None,
            base_ref: None,
            created_branch: false,
            state: WorkspaceState::Ready,
            request_id: None,
            created_at: 1,
        };
        store.insert_workspace(&workspace).expect("workspace");
        let session = AgentSessionRow {
            id: new_id(),
            kind: "fake".to_owned(),
            workspace_id: workspace.id,
            parent_id: None,
            title: None,
            read_only: false,
            agent_ref: None,
            terminal_id: Some("terminal-1".to_owned()),
            request_id: Some(request.to_owned()),
            created_at: 1,
            exit: None,
            activity: None,
            result: None,
            brought_back_at: None,
            holds_secrets: false,
            mode: massorca_core_store::RunMode::default(),
            pending_permission_mode: None,
        };
        store.insert_agent_session(&session).expect("session");
        session.id
    }

    #[test]
    fn a_task_marked_finished_during_its_start_stops_the_run_and_records_why() {
        let store = Store::open_in_memory().expect("store");
        let task = task_in(&store);
        let attempt = store
            .begin_task_start_attempt(&task.id, "attempt-1")
            .expect("attempt")
            .expect("task");
        // `task.done` landed between the spawn and the link.
        assert!(
            store
                .set_task_outcome(&task.id, TaskOutcome::Success, None, 2)
                .expect("done")
        );
        let stopped = RefCell::new(Vec::new());
        let stop = |session: &str| {
            stopped.borrow_mut().push(session.to_owned());
            Ok(())
        };

        let error = settle(
            &store,
            stop,
            &task,
            &attempt,
            "session-1",
            &mut Changes::default(),
        )
        .expect_err("not linked");

        assert_eq!(error.code, codes::TASK_CHANGED);
        assert_eq!(*stopped.borrow(), vec!["session-1".to_owned()]);
        let kept = store.get_task(&task.id).expect("read").expect("task");
        assert_eq!(kept.session_id, None);
        assert_eq!(kept.error_code.as_deref(), Some(codes::TASK_CHANGED));
        assert_eq!(attempt_of(&store, &task), None);
    }

    #[test]
    fn a_start_that_failed_after_its_spawn_stops_the_session_it_left() {
        let store = Store::open_in_memory().expect("store");
        let task = task_in(&store);
        let attempt = store
            .begin_task_start_attempt(&task.id, "attempt-1")
            .expect("attempt")
            .expect("task");
        let request = request_id(&task.id, &attempt);
        let session = spawned_session(&store, &request);
        let stopped = RefCell::new(Vec::new());
        let stop = |session: &str| {
            stopped.borrow_mut().push(session.to_owned());
            Ok(())
        };

        let error = after_failed_start(
            &store,
            stop,
            &task,
            &attempt,
            &request,
            ErrorBody::new(codes::INTERNAL, "the view failed"),
            &mut Changes::default(),
        )
        .expect("recorded");

        assert_eq!(error.code, codes::INTERNAL);
        assert_eq!(*stopped.borrow(), vec![session]);
        assert_eq!(attempt_of(&store, &task), None);
        let kept = store.get_task(&task.id).expect("read").expect("task");
        assert_eq!(kept.error_code.as_deref(), Some(codes::INTERNAL));
    }

    #[test]
    fn a_session_left_by_a_failed_start_keeps_its_attempt_until_a_stop_succeeds() {
        let store = Store::open_in_memory().expect("store");
        let task = task_in(&store);
        let attempt = store
            .begin_task_start_attempt(&task.id, "attempt-1")
            .expect("attempt")
            .expect("task");
        let request = request_id(&task.id, &attempt);
        spawned_session(&store, &request);
        let failing = |_: &str| {
            Err(ErrorBody::new(
                codes::INTERNAL,
                "the terminal did not answer",
            ))
        };

        let error = after_failed_start(
            &store,
            failing,
            &task,
            &attempt,
            &request,
            ErrorBody::new(codes::INTERNAL, "the view failed"),
            &mut Changes::default(),
        )
        .expect("recorded");

        assert!(error.message.contains("could not be stopped"));
        // The attempt stays, so a retry names the same request and finds that run.
        assert_eq!(
            store.task_start_attempt(&task.id).expect("read").as_deref(),
            Some(attempt.as_str())
        );
    }

    #[test]
    fn a_start_that_failed_before_any_spawn_settles_its_attempt_and_stops_nothing() {
        let store = Store::open_in_memory().expect("store");
        let task = task_in(&store);
        let attempt = store
            .begin_task_start_attempt(&task.id, "attempt-1")
            .expect("attempt")
            .expect("task");
        let stopped = RefCell::new(Vec::new());
        let stop = |session: &str| {
            stopped.borrow_mut().push(session.to_owned());
            Ok(())
        };

        after_failed_start(
            &store,
            stop,
            &task,
            &attempt,
            &request_id(&task.id, &attempt),
            ErrorBody::new(codes::AGENT_NOT_INSTALLED, "no such program"),
            &mut Changes::default(),
        )
        .expect("recorded");

        assert!(stopped.borrow().is_empty());
        assert_eq!(attempt_of(&store, &task), None);
    }

    #[test]
    fn a_task_worktree_is_named_after_its_title_and_attempt() {
        assert_eq!(
            worktree_name("Parse the Headers!", "AbC123xyz"),
            "parse-the-headers-abc123"
        );
        assert_eq!(
            worktree_name("Añadir créditos", "a1b2c3d4"),
            "anadir-creditos-a1b2c3"
        );
        assert_eq!(worktree_name("¿?", "a1b2c3d4"), "child-a1b2c3");
        let long = worktree_name(&"word ".repeat(20), "a1b2c3d4");
        let slug = long.strip_suffix("-a1b2c3").expect("suffix");
        assert!(slug.len() <= WORKTREE_SLUG_MAX_CHARS, "{long}");
        assert!(!slug.ends_with('-'), "{long}");
    }

    #[test]
    fn a_task_in_its_own_worktree_is_told_its_branch_and_what_it_does_not_carry() {
        let store = Store::open_in_memory().expect("store");
        let task = task_in(&store);
        let team = store.get_team(&task.team_id).expect("read").expect("team");
        let bot = BotRow {
            id: new_id(),
            team_id: team.id.clone(),
            name: "mara".to_owned(),
            job: "Writes the parser".to_owned(),
            kind: "fake".to_owned(),
            model: None,
            effort: None,
            lead: false,
            created_at: 1,
        };

        let own = prompt(&team, &bot, &task, None, Some("parse-a1b2c3"));
        assert!(own.contains("branch parse-a1b2c3"), "{own}");
        assert!(
            own.contains("uncommitted changes of the main checkout"),
            "{own}"
        );
        assert!(!prompt(&team, &bot, &task, None, None).contains("worktree"));
    }

    #[test]
    fn a_task_in_a_project_reads_the_projects_goal_and_brief() {
        let store = Store::open_in_memory().expect("store");
        let task = task_in(&store);
        let team = store.get_team(&task.team_id).expect("read").expect("team");
        let bot = BotRow {
            id: new_id(),
            team_id: team.id.clone(),
            name: "mara".to_owned(),
            job: "Writes the parser".to_owned(),
            kind: "fake".to_owned(),
            model: None,
            effort: None,
            lead: false,
            created_at: 1,
        };
        let mut project = TeamProjectRow {
            id: new_id(),
            team_id: team.id.clone(),
            name: "Parser".to_owned(),
            goal: "Parse every header".to_owned(),
            brief: "Keep the public API.".to_owned(),
            created_at: 1,
            source_project_id: None,
            code_source: None,
        };

        let text = prompt(&team, &bot, &task, Some(&project), None);
        assert!(
            text.contains(
                "This task is part of the project \"Parser\". Read its brief before you start.\nGoal: Parse every header\n\nBrief:\nKeep the public API.\n"
            ),
            "{text}"
        );

        // A project without a goal or brief only names itself.
        project.goal = String::new();
        project.brief = "  ".to_owned();
        let bare = prompt(&team, &bot, &task, Some(&project), None);
        assert!(
            bare.ends_with("This task is part of the project \"Parser\".\n"),
            "{bare}"
        );
        assert!(!prompt(&team, &bot, &task, None, None).contains("project \""));
    }
}
