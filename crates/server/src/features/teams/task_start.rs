//! `task.start` (capability `teams/1`, decision 0072 and its amendment): makes the team's shared folder workspace, starts
//! the bot's agent there with the task as its prompt, and links the session only once the agent started. A failed start keeps the task Open
//! with the reason in `error`, and a session spawned but not linked is stopped.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};

use massorca_core_agents::{Agents, SecretGrant};
use massorca_core_store::{BotRow, Store, TaskRow, TeamRow, new_id, now_ms};
use massorca_core_workspace::{DomainError, Workspaces};
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
use crate::features::agents::secret_grant;
use crate::features::workspace_domain::{for_mutation, publish};
use crate::owner::Owner;
use crate::permission::Permission;
use crate::registry::RegistrationHandle;

use super::task::{removed, view_of};
use super::task_state::{self, Session};
use super::{blocking, selectors, store_error};

/// The folder project that holds every team's folder, under the workspaces default root.
const TEAMS_FOLDER: &str = "bot-teams";
const TEAMS_PROJECT_NAME: &str = "Bot teams";

static START_GATE: Mutex<()> = Mutex::new(());

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
struct Changes {
    project: Option<String>,
    workspace: Option<String>,
    task: Option<String>,
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
    let workspace = match prepare(store, agents, workspaces, &team, &bot, &row, changes) {
        Ok(workspace) => workspace,
        Err(error) => {
            // Nothing was spawned, so the attempt is settled.
            store
                .end_task_start_attempt(&row.id, &attempt)
                .map_err(store_error)?;
            return Err(record_failure(store, &row, error, changes)?);
        }
    };
    let request = request_id(&row.id, &attempt);
    let session = match spawn(agents, &workspace, &team, &bot, &row, &request, grant) {
        Ok(session) => session,
        Err(error) => {
            return Err(after_failed_start(
                store, stop, &row, &attempt, &request, error, changes,
            )?);
        }
    };
    settle(store, stop, &row, &attempt, &session, changes)?;
    // Read after the link: a failure here is an error to the caller, never a reason to stop the linked run.
    let current = selectors::task(store, &row.id)?;
    Ok(view_of(&current, &task_state::load(agents, &current)?))
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

/// A free bot and the team's folder workspace.
fn prepare(
    store: &Store,
    agents: &Agents,
    workspaces: &Workspaces,
    team: &TeamRow,
    bot: &BotRow,
    task: &TaskRow,
    changes: &mut Changes,
) -> Result<Workspace, ErrorBody> {
    refuse_busy_bot(store, agents, bot, task)?;
    team_folder(store, workspaces, team, changes)
}

/// Starts the bot's agent. A failure here is a start that spawned nothing.
fn spawn(
    agents: &Agents,
    workspace: &Workspace,
    team: &TeamRow,
    bot: &BotRow,
    task: &TaskRow,
    request: &str,
    grant: SecretGrant,
) -> Result<String, ErrorBody> {
    let session = agents.start(
        AgentStartParams {
            workspace: workspace.id.clone(),
            kind: bot.kind.clone(),
            prompt: Some(prompt(team, bot, task)),
            args: Vec::new(),
            env: BTreeMap::new(),
            read_only: false,
            title: Some(task.title.clone()),
            cols: None,
            rows: None,
            // A retry of an attempt that left a live run finds that run instead of starting a second one.
            request_id: Some(request.to_owned()),
        },
        grant,
    )?;
    Ok(session.id)
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
/// stop keeps it, so a retry names the same run and the bot stays busy while that run lives.
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
    if store
        .set_task_start_error(&row.id, &error.code, &error.message, now_ms())
        .map_err(store_error)?
    {
        changes.task = Some(row.id.clone());
    }
    Ok(error)
}

/// A bot works on one task at a time: another of its tasks whose session still runs (or cannot be verified) makes it busy.
fn refuse_busy_bot(
    store: &Store,
    agents: &Agents,
    bot: &BotRow,
    task: &TaskRow,
) -> Result<(), ErrorBody> {
    for other in store.list_tasks_of_bot(&bot.id).map_err(store_error)? {
        if other.id == task.id {
            continue;
        }
        let session = match other.session_id {
            Some(_) => task_state::load(agents, &other)?,
            None => pending_run(store, agents, &other)?,
        };
        if task_state::is_running(&session) {
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

/// The run of a start attempt that was not settled: a session a failed start could not stop, until its stop is confirmed.
fn pending_run(store: &Store, agents: &Agents, task: &TaskRow) -> Result<Session, ErrorBody> {
    let Some(attempt) = store.task_start_attempt(&task.id).map_err(store_error)? else {
        return Ok(Session::Unstarted);
    };
    match store
        .find_agent_session_by_request_id(&request_id(&task.id, &attempt))
        .map_err(store_error)?
    {
        Some(session) => task_state::load_id(agents, &session.id),
        None => Ok(Session::Unstarted),
    }
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
        // The attempt stays, so the busy check finds the run and a retry names the same request.
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
}
