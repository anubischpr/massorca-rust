//! The bridge against a real service in a temporary profile, driven headlessly: no window opens. The
//! agent sessions and the agent kinds the service lists reach the store on connect, the way workspaces and
//! terminals do.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use massorca_client::Client;
use massorca_desktop::model::agent_models::{DeclaredModel, declared_models};
use massorca_desktop::model::agent_sessions::bring_back_refusal;
use massorca_desktop::model::home_start::folder_start_workspace;
use massorca_desktop::model::session_ended::refusal_says_conversation_gone;
use massorca_desktop::model::start_page::start_again_params;
use massorca_desktop::service::{
    AttachRole, ForgetSessionParams, Refusal, ServiceBridge, ServiceHandle, StoreState,
    StoreUpdate, TerminalStream, TerminalStreamItem, connect_options, feed_events,
};
use massorca_profile::Profile;
use massorca_protocol::codes;
use massorca_protocol::methods::agents::{
    AgentBringBackParams, AgentDelegateParams, AgentPromptParams, AgentSession, AgentStartParams,
    PackageState,
};
use massorca_protocol::methods::config::ConfigSet;
use massorca_protocol::methods::projects::{ProjectAddParams, ProjectKind};
use massorca_protocol::methods::terminals::TerminalState;
use massorca_protocol::methods::workspaces::{
    Workspace, WorkspaceCreateParams, WorkspaceListParams,
};
use massorca_server::{RunningService, Service, ServiceConfig};
use massorca_ui::components::terminal_surface::feed::TerminalFeed;
use tempfile::TempDir;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;

/// A private home below a temporary folder; the temporary folder itself may be shared with other users (ADR 0073).
struct TempHome {
    _temp: TempDir,
    path: PathBuf,
}

impl TempHome {
    fn path(&self) -> &Path {
        &self.path
    }
}

fn temp_home() -> TempHome {
    let temp = tempfile::tempdir().expect("a temporary folder");
    let path = temp.path().join("home");
    massorca_profile::create_home(&path).expect("a private home");
    TempHome { _temp: temp, path }
}

const WAIT: Duration = Duration::from_secs(30);

fn profile(home: &Path) -> Profile {
    Profile::resolve_with(Some(home.to_path_buf()), Some("default".to_owned()), |_| {
        None
    })
    .expect("the profile resolves")
}

async fn start_service(home: &Path) -> RunningService {
    Service::start(ServiceConfig {
        profile: profile(home),
        server_name: "massorca-test".to_owned(),
        server_version: "0.0.0-test".to_owned(),
    })
    .await
    .expect("the service starts")
}

async fn stop_service(running: RunningService) {
    running.shutdown_handle().shutdown();
    timeout(WAIT, running.wait())
        .await
        .expect("the service stops in time")
        .expect("the service stops cleanly");
}

#[tokio::test]
async fn the_agent_sessions_the_service_lists_reach_the_store_on_connect() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let mut updates = bridge.take_updates().expect("the updates are there");

    let listed = timeout(WAIT, async {
        loop {
            let update = updates.recv().await.expect("the bridge is alive");
            if let StoreUpdate::AgentsResync(sessions) = update {
                return sessions;
            }
        }
    })
    .await
    .expect("the agent sessions arrive after the connect");
    assert!(
        listed.is_empty(),
        "a new profile has no agent sessions: {listed:?}"
    );
    // An empty list also comes from a service that predates agents, so ask the service directly too.
    let client = Client::connect(&profile(home.path()), connect_options(None))
        .await
        .expect("a client connects");
    let direct = client
        .agent_list(None)
        .await
        .expect("the service answers agent.list");
    assert!(direct.sessions.is_empty());

    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn the_agent_kinds_the_service_declares_reach_the_store_on_connect() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let mut updates = bridge.take_updates().expect("the updates are there");

    let declared = timeout(WAIT, async {
        loop {
            let update = updates.recv().await.expect("the bridge is alive");
            if let StoreUpdate::AgentKindsResync(kinds) = update {
                return kinds;
            }
        }
    })
    .await
    .expect("the agent kinds arrive after the connect");
    // The four built-in kinds, in the order the service lists them; a new profile has installed none of their packages.
    let ids: Vec<&str> = declared.iter().map(|kind| kind.id.as_str()).collect();
    assert_eq!(ids, ["claude", "codex", "antigravity", "gentle-shell"]);
    assert!(
        declared
            .iter()
            .all(|kind| kind.package.state == PackageState::Available)
    );

    let mut store = StoreState::default();
    store.apply(
        StoreUpdate::AgentKindsResync(declared.clone()),
        Instant::now(),
    );
    assert_eq!(store.agent_kinds, declared);

    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn the_claude_kind_declares_its_models_and_levels_to_the_store_and_the_picker_on_a_real_service()
 {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let mut updates = bridge.take_updates().expect("the updates are there");

    let declared = timeout(WAIT, async {
        loop {
            let update = updates.recv().await.expect("the bridge is alive");
            if let StoreUpdate::AgentKindsResync(kinds) = update {
                return kinds;
            }
        }
    })
    .await
    .expect("the agent kinds arrive after the connect");
    let mut store = StoreState::default();
    store.apply(StoreUpdate::AgentKindsResync(declared), Instant::now());
    let claude = store
        .agent_kinds
        .iter()
        .find(|kind| kind.id == "claude")
        .expect("the service declares claude");
    // What `agent.kinds` reported for claude, as the store holds it: the models it declares, then its levels.
    let model_ids: Vec<&str> = claude.model.as_ref().map_or_else(Vec::new, |model| {
        model
            .choices
            .iter()
            .map(|choice| choice.id.as_str())
            .collect()
    });
    assert_eq!(
        model_ids,
        ["fable", "opus", "sonnet", "haiku"],
        "the real service declares claude's models"
    );
    // The picker's rows, built from that declaration: each model with the levels it accepts and its preselected level.
    let rows = declared_models(claude);
    fn level_ids(model: &DeclaredModel) -> Vec<&str> {
        model
            .efforts
            .iter()
            .map(|effort| effort.id.as_str())
            .collect()
    }
    assert_eq!(
        rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        ["fable", "opus", "sonnet", "haiku"]
    );
    assert_eq!(
        level_ids(&rows[1]),
        ["low", "medium", "high", "xhigh", "max"]
    );
    assert!(level_ids(&rows[3]).is_empty(), "haiku takes no level");
    assert!(rows[2].default, "sonnet is the declared default model");
    assert_eq!(
        rows[2].efforts[rows[2].default_effort].id, "high",
        "the declared default level is preselected"
    );

    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn the_service_offers_agent_chat_and_the_bridge_reports_it() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let mut updates = bridge.take_updates().expect("the updates are there");

    let offered = timeout(WAIT, async {
        loop {
            let update = updates.recv().await.expect("the bridge is alive");
            if let StoreUpdate::AgentChatOffered(flag) = update {
                return flag;
            }
        }
    })
    .await
    .expect("the offer arrives after the connect");
    // The service runs chat sessions (decision 0101), so the start page can offer Chat where a kind supports it.
    assert!(offered);

    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn a_delegate_from_a_session_the_service_does_not_have_is_refused_and_its_tree_is_read_again()
{
    let home = temp_home();
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let handle = bridge.handle();
    let mut updates = bridge.take_updates().expect("the updates are there");
    // The first agent list comes once the bridge is connected, so the call below reaches the service.
    timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentsResync(_) = updates.recv().await.expect("the bridge is alive")
            {
                return;
            }
        }
    })
    .await
    .expect("the bridge connects");

    let parent = "0196a6f0-0000-7000-8000-000000000000".to_owned();
    let refusal = handle
        .agent_delegate(AgentDelegateParams {
            parent: parent.clone(),
            request_id: Some("0196a6f0-0000-7000-8000-000000000001".to_owned()),
            ..base_agent_delegate_params()
        })
        .await
        .expect_err("a parent the service does not have is refused");
    assert_eq!(refusal.code(), Some(codes::AGENT_NOT_FOUND), "{refusal:?}");
    // The refetch after the refused call asks for the parent's tree, which the service no longer has.
    let gone = timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentGone(id) = updates.recv().await.expect("the bridge is alive") {
                return id;
            }
        }
    })
    .await
    .expect("the refetch follows the refused call");
    assert_eq!(gone, parent);

    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn a_bring_back_the_service_refuses_arrives_with_its_code_and_a_plain_sentence() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let handle = bridge.handle();
    let mut updates = bridge.take_updates().expect("the updates are there");
    // The first agent list comes once the bridge is connected, so the call below reaches the service.
    timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentsResync(_) = updates.recv().await.expect("the bridge is alive")
            {
                return;
            }
        }
    })
    .await
    .expect("the bridge connects");

    let child = "0196a6f0-0000-7000-8000-000000000000".to_owned();
    let refusal = handle
        .agent_bring_back(AgentBringBackParams {
            child: child.clone(),
            ..base_agent_bring_back_params()
        })
        .await
        .expect_err("a session the service does not have is refused");
    assert_eq!(refusal.code(), Some(codes::AGENT_NOT_FOUND), "{refusal:?}");
    assert_eq!(
        bring_back_refusal(&refusal),
        "This session no longer exists."
    );
    // The refetch after the write finds the session gone, whatever the answer was.
    let gone = timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentGone(id) = updates.recv().await.expect("the bridge is alive") {
                return id;
            }
        }
    })
    .await
    .expect("the refetch after the write reports the session gone");
    assert_eq!(gone, child);

    drop(bridge);
    stop_service(service).await;
}

/// Runs git in `dir` with a fixed identity, and fails when git does.
fn git(dir: &Path, args: &[&str]) {
    let output = massorca_process::command("git")
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
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn repo(dir: &Path) {
    std::fs::create_dir_all(dir).expect("the repository directory");
    git(dir, &["init", "-q"]);
    std::fs::write(dir.join("README.md"), "hello\n").expect("a readme");
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "initial"]);
}

/// Declares kind `fake`, which runs the platform shell: the fake agent is a binary of the CLI crate, which
/// this crate's tests cannot start, and the terminal test uses the same shell.
fn declare_fake_kind(home: &Path) {
    let dir = profile(home).dir().join("agents").join("kinds");
    std::fs::create_dir_all(&dir).expect("the kinds folder");
    let program = if cfg!(windows) { "cmd.exe" } else { "/bin/sh" };
    let kind = format!(r#"{{"id":"fake","name":"Fake agent","program":["{program}"]}}"#);
    std::fs::write(dir.join("fake.json"), kind).expect("the kind file");
}

/// Declares kind `fast-fake`: the shell again, with a default model that takes fast mode and one that does not. Fast mode
/// is passed as the `--settings` object a Claude-style kind declares.
fn declare_fast_kind(home: &Path) {
    let dir = profile(home).dir().join("agents").join("kinds");
    std::fs::create_dir_all(&dir).expect("the kinds folder");
    let program = if cfg!(windows) { "cmd.exe" } else { "/bin/sh" };
    let kind = format!(
        r#"{{"id":"fast-fake","name":"Fast fake agent","program":["{program}"],"model":{{"flag":"--model","choices":[{{"id":"fast-model","label":"Fast model","default":true,"fast":true}},{{"id":"plain-model","label":"Plain model"}}]}},"fast":{{"settings":{{"fastMode":true}}}}}}"#
    );
    std::fs::write(dir.join("fast-fake.json"), kind).expect("the kind file");
}

/// Folds the bridge's updates into `state`, as the store does, until `done` holds.
async fn fold_until(
    updates: &mut UnboundedReceiver<StoreUpdate>,
    state: &mut StoreState,
    what: &str,
    done: impl Fn(&StoreState) -> bool,
) {
    let folded = timeout(WAIT, async {
        while !done(state) {
            let update = updates.recv().await.expect("the bridge is alive");
            state.apply(update, Instant::now());
        }
    })
    .await;
    assert!(folded.is_ok(), "timed out waiting for {what}: {state:?}");
}

/// A bridge on `home`'s profile, with its updates folded into a store state until kind `kind` is detected.
async fn bridge_with_detected_kind(
    home: &Path,
    kind: &str,
) -> (ServiceBridge, UnboundedReceiver<StoreUpdate>, StoreState) {
    let mut bridge =
        ServiceBridge::start(profile(home), connect_options(None)).expect("the bridge starts");
    let mut updates = bridge.take_updates().expect("the updates are there");
    let mut state = StoreState::default();
    fold_until(&mut updates, &mut state, "the declared kind", |s| {
        s.agent_kinds
            .iter()
            .any(|declared| declared.id == kind && declared.detected)
    })
    .await;
    (bridge, updates, state)
}

#[tokio::test]
async fn an_agent_session_starts_in_a_workspace_through_the_bridge_and_reports_its_terminal() {
    let home = temp_home();
    let repo_dir = home.path().with_file_name("repo");
    repo(&repo_dir);
    declare_fake_kind(home.path());
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let handle = bridge.handle();
    let mut updates = bridge.take_updates().expect("the updates are there");
    timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentKindsResync(kinds) =
                updates.recv().await.expect("the bridge is alive")
                && kinds.iter().any(|kind| kind.id == "fake" && kind.detected)
            {
                return;
            }
        }
    })
    .await
    .expect("the declared kind is listed once the bridge connects");

    let workspace = add_workspace(&handle, &repo_dir).await;
    let session = start_fake(&handle, &workspace.id).await;
    assert_eq!(session.kind, "fake");
    assert_eq!(session.workspace_id, workspace.id);
    let terminal = session
        .terminal_id
        .clone()
        .expect("the session has its terminal");
    // The started session reaches the store as a change, not only through a later refetch.
    let changed = timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentChanged(changed) =
                updates.recv().await.expect("the bridge is alive")
                && changed.id == session.id
            {
                return changed;
            }
        }
    })
    .await
    .expect("the session reaches the store");
    assert_eq!(changed.terminal_id.as_deref(), Some(terminal.as_str()));

    handle
        .close_terminal(&terminal)
        .await
        .expect("the session's terminal closes");
    drop(bridge);
    stop_service(service).await;
}

/// Adds `repo_dir` as a project and creates its workspace.
async fn add_workspace(handle: &ServiceHandle, repo_dir: &Path) -> Workspace {
    let project = handle
        .add_project(ProjectAddParams {
            path: repo_dir.display().to_string(),
            name: Some("demo".to_owned()),
            workspaces_root: repo_dir
                .parent()
                .map(|parent| parent.join("demo-worktrees").display().to_string()),
            ..base_project_add_params()
        })
        .await
        .expect("the project is added");
    handle
        .create_workspace(WorkspaceCreateParams {
            project: project.id,
            ..base_workspace_create_params()
        })
        .await
        .expect("the workspace is created")
}

/// The `agent.start` params of a fake session in `workspace`, sized for the feed the test reads it into.
fn fake_start_params(workspace: &str) -> AgentStartParams {
    AgentStartParams {
        workspace: Some(workspace.to_owned()),
        env: BTreeMap::new(),
        cols: Some(100),
        rows: Some(30),
        ..base_agent_start_params()
    }
}

/// Starts a fake session in `workspace`, with a terminal sized for the feed the test reads it into.
async fn start_fake(handle: &ServiceHandle, workspace: &str) -> AgentSession {
    handle
        .agent_start(fake_start_params(workspace))
        .await
        .expect("the session starts")
}

#[tokio::test]
async fn an_agent_session_that_exits_is_ended_and_starts_again_in_its_workspace() {
    let home = temp_home();
    let repo_dir = home.path().with_file_name("repo");
    repo(&repo_dir);
    declare_fake_kind(home.path());
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let handle = bridge.handle();
    let mut updates = bridge.take_updates().expect("the updates are there");
    timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentKindsResync(kinds) =
                updates.recv().await.expect("the bridge is alive")
                && kinds.iter().any(|kind| kind.id == "fake" && kind.detected)
            {
                return;
            }
        }
    })
    .await
    .expect("the declared kind is listed once the bridge connects");

    let workspace = add_workspace(&handle, &repo_dir).await;
    let session = start_fake(&handle, &workspace.id).await;
    let terminal = session
        .terminal_id
        .clone()
        .expect("the session has its terminal");
    handle.send_terminal_input(&terminal, b"exit\r\n".to_vec());
    let ended = timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentChanged(changed) =
                updates.recv().await.expect("the bridge is alive")
                && changed.id == session.id
                && changed.execution.state == TerminalState::Exited
            {
                return changed;
            }
        }
    })
    .await
    .expect("the exit of the run reaches the store");
    assert_eq!(ended.execution.state, TerminalState::Exited);

    // The fake kind declares no resume form, so the refusal carries a code the desktop reads as a missing conversation.
    let refusal = handle
        .agent_resume(&session.id, None, None)
        .await
        .expect_err("a kind without a resume form does not resume");
    assert_eq!(refusal.code(), Some(codes::AGENT_RESUME_UNAVAILABLE));
    assert!(refusal_says_conversation_gone(refusal.code()));

    // The ended state's other action: the same workspace and agent, started again without the saved conversation.
    let again = handle
        .agent_start(start_again_params(&workspace.id, "fake", (120, 40)))
        .await
        .expect("the session starts again");
    assert_eq!(again.kind, "fake");
    assert_eq!(again.workspace_id, workspace.id);
    assert_ne!(again.id, session.id);

    drop(bridge);
    stop_service(service).await;
}

/// A line the shell runs, and the sum it prints: only the run prints the sum, not the echo of the line.
fn sum_line() -> (&'static str, &'static str) {
    if cfg!(windows) {
        ("set /a 1234567*7", "8641969")
    } else {
        ("echo $((1234567*7))", "8641969")
    }
}

/// Reads the attach until the feed shows `row` on a line, answering the program's queries as it goes.
async fn read_until_row(
    stream: &mut TerminalStream,
    feed: &mut TerminalFeed,
    handle: &ServiceHandle,
    terminal: &str,
    row: &str,
) {
    let shown = timeout(WAIT, async {
        while !(0..feed.lines()).any(|line| feed.row_text(line).trim_end() == row) {
            match stream.next().await {
                Some(TerminalStreamItem::Update(update)) => {
                    for event in feed_events(&update) {
                        feed.apply(event).expect("the feed accepts the item");
                    }
                    let replies = feed.take_replies();
                    if !replies.is_empty() {
                        handle.send_terminal_input(terminal, replies);
                    }
                }
                other => panic!("the stream ended before {row:?} showed: {other:?}"),
            }
        }
    })
    .await;
    assert!(shown.is_ok(), "no {row:?} within {WAIT:?}");
}

#[tokio::test]
async fn a_prompt_reaches_the_session_terminal_and_stop_ends_the_run() {
    let home = temp_home();
    let repo_dir = home.path().with_file_name("repo");
    repo(&repo_dir);
    declare_fake_kind(home.path());
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let handle = bridge.handle();
    let mut updates = bridge.take_updates().expect("the updates are there");
    timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentKindsResync(kinds) =
                updates.recv().await.expect("the bridge is alive")
                && kinds.iter().any(|kind| kind.id == "fake" && kind.detected)
            {
                return;
            }
        }
    })
    .await
    .expect("the declared kind is listed once the bridge connects");

    let workspace = add_workspace(&handle, &repo_dir).await;
    let session = start_fake(&handle, &workspace.id).await;
    let terminal = session
        .terminal_id
        .clone()
        .expect("the session has its terminal");
    let mut stream = handle
        .attach_terminal(AttachRole::AnswersQueries.attach_params(&terminal, None))
        .await
        .expect("the terminal attaches");
    let mut feed = TerminalFeed::new(100, 30);

    let (line, sum) = sum_line();
    let receipt = handle
        .agent_prompt(AgentPromptParams {
            session: session.id.clone(),
            text: line.to_owned(),
            ..base_agent_prompt_params()
        })
        .await
        .expect("the prompt is typed into the session");
    assert!(
        receipt.bytes > 0,
        "the receipt says the bytes were written: {receipt:?}"
    );
    read_until_row(&mut stream, &mut feed, &handle, &terminal, sum).await;

    let stopped = handle
        .agent_stop(&session.id)
        .await
        .expect("the stop is accepted");
    assert_eq!(stopped.id, session.id);
    timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentChanged(changed) =
                updates.recv().await.expect("the bridge is alive")
                && changed.id == session.id
                && changed.execution.state == TerminalState::Exited
            {
                return;
            }
        }
    })
    .await
    .expect("the session's run ends after the stop");

    drop(stream);
    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn a_session_with_no_project_starts_on_a_real_service_in_the_conversations_folder() {
    let home = temp_home();
    declare_fake_kind(home.path());
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let handle = bridge.handle();
    let mut updates = bridge.take_updates().expect("the updates are there");
    timeout(WAIT, async {
        let (mut offered, mut listed) = (false, false);
        while !(offered && listed) {
            match updates.recv().await.expect("the bridge is alive") {
                StoreUpdate::AgentNoProjectOffered(flag) => offered = flag,
                StoreUpdate::AgentKindsResync(kinds) => {
                    listed = kinds.iter().any(|kind| kind.id == "fake" && kind.detected);
                }
                _ => {}
            }
        }
    })
    .await
    .expect("the service offers agent-no-project/1 and lists the fake kind after the connect");

    // Without this setting the conversations folder is under the user's home, so the test points it into its own home.
    let conversations = home.path().with_file_name("conversations");
    handle
        .config_set(ConfigSet {
            owner: None,
            key: "conversations.folder".to_owned(),
            value: conversations.to_string_lossy().into_owned().into(),
        })
        .await
        .expect("the conversations folder is set");

    let session = handle
        .agent_start(AgentStartParams {
            env: BTreeMap::new(),
            cols: Some(100),
            rows: Some(30),
            ..base_agent_start_params()
        })
        .await
        .expect("the service starts a session with no workspace");
    assert_eq!(session.kind, "fake");
    assert!(
        !session.workspace_id.is_empty(),
        "the session has a folder of its own"
    );
    let folders = std::fs::read_dir(&conversations)
        .expect("the conversations folder is in the throwaway home")
        .count();
    assert_eq!(folders, 1, "one conversation folder, made for this session");

    let client = Client::connect(&profile(home.path()), connect_options(None))
        .await
        .expect("a client connects");
    let projects = client
        .project_list()
        .await
        .expect("the service lists projects")
        .projects;
    assert!(
        projects.is_empty(),
        "a session with no project adds no visible project: {projects:?}"
    );
    let workspaces = client
        .workspace_list(&WorkspaceListParams {
            ..base_workspace_list_params()
        })
        .await
        .expect("the service lists workspaces")
        .workspaces;
    assert!(
        workspaces
            .iter()
            .any(|known| known.id == session.workspace_id),
        "the conversation's folder is listed as a workspace, so the sidebar can group it"
    );

    let terminal = session
        .terminal_id
        .clone()
        .expect("the session has its terminal");
    handle
        .close_terminal(&terminal)
        .await
        .expect("the session's terminal closes");
    drop(bridge);
    stop_service(service).await;
}

/// Until the service offers `agent-archive/1`, a forget with its options is refused in the desktop and never sent.
/// When the service offers it, this test must send the call to a real session instead.
#[tokio::test]
async fn a_forget_with_the_archive_options_reaches_the_service_that_offers_the_capability() {
    let home = temp_home();
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let mut updates = bridge.take_updates().expect("the updates are there");
    let offered = timeout(WAIT, async {
        loop {
            let update = updates.recv().await.expect("the bridge is alive");
            if let StoreUpdate::AgentArchiveOffered(offered) = update {
                return offered;
            }
        }
    })
    .await
    .expect("the capability answer arrives after the connect");
    assert!(offered, "the service offers agent-archive/1");

    // The desktop sends the forget; the service answers for a session it does not have.
    let refused = bridge
        .handle()
        .agent_forget(ForgetSessionParams {
            session: "no-such-session".to_owned(),
            remove_workspace: true,
            force: false,
        })
        .await;
    match refused {
        Err(Refusal::Failed { code, .. }) => assert_ne!(code, "service.capability_missing"),
        other => panic!("the service refuses an unknown session, got {other:?}"),
    }

    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn a_rename_of_a_session_reaches_the_store_through_the_bridge() {
    let home = temp_home();
    let repo_dir = home.path().with_file_name("repo");
    repo(&repo_dir);
    declare_fake_kind(home.path());
    let service = start_service(home.path()).await;
    let (bridge, mut updates, mut state) = bridge_with_detected_kind(home.path(), "fake").await;
    let handle = bridge.handle();
    let workspace = add_workspace(&handle, &repo_dir).await;
    let session = start_fake(&handle, &workspace.id).await;
    let terminal = session
        .terminal_id
        .clone()
        .expect("the session has its terminal");
    fold_until(&mut updates, &mut state, "the started session", |s| {
        s.agents.iter().any(|agent| agent.id == session.id)
    })
    .await;

    let renamed = handle
        .agent_rename(&session.id, "Parser cleanup")
        .await
        .expect("the session is renamed");
    assert_eq!(renamed.title.as_deref(), Some("Parser cleanup"));
    fold_until(&mut updates, &mut state, "the new title", |s| {
        s.agents
            .iter()
            .any(|agent| agent.id == session.id && agent.title.as_deref() == Some("Parser cleanup"))
    })
    .await;

    handle
        .close_terminal(&terminal)
        .await
        .expect("the session's terminal closes");
    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn an_exited_session_archives_and_unarchives_through_the_bridge() {
    let home = temp_home();
    let repo_dir = home.path().with_file_name("repo");
    repo(&repo_dir);
    declare_fake_kind(home.path());
    let service = start_service(home.path()).await;
    let (bridge, mut updates, mut state) = bridge_with_detected_kind(home.path(), "fake").await;
    let handle = bridge.handle();
    let workspace = add_workspace(&handle, &repo_dir).await;
    let session = start_fake(&handle, &workspace.id).await;
    let terminal = session
        .terminal_id
        .clone()
        .expect("the session has its terminal");
    handle.send_terminal_input(&terminal, b"exit\r\n".to_vec());
    fold_until(&mut updates, &mut state, "the exit of the run", |s| {
        s.agents
            .iter()
            .any(|agent| agent.id == session.id && agent.execution.state == TerminalState::Exited)
    })
    .await;

    let archived = handle
        .agent_archive(&session.id, true)
        .await
        .expect("the exited session is archived");
    assert!(archived.archived);
    fold_until(&mut updates, &mut state, "the archive", |s| {
        s.agents
            .iter()
            .any(|agent| agent.id == session.id && agent.archived)
    })
    .await;

    let restored = handle
        .agent_archive(&session.id, false)
        .await
        .expect("the archive is cleared");
    assert!(!restored.archived);
    fold_until(&mut updates, &mut state, "the cleared archive", |s| {
        s.agents
            .iter()
            .any(|agent| agent.id == session.id && !agent.archived)
    })
    .await;

    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn a_fast_start_carries_fast_to_the_session_only_when_the_kind_and_model_take_it() {
    let home = temp_home();
    let repo_dir = home.path().with_file_name("repo");
    repo(&repo_dir);
    declare_fake_kind(home.path());
    declare_fast_kind(home.path());
    let service = start_service(home.path()).await;
    let (bridge, mut updates, mut state) =
        bridge_with_detected_kind(home.path(), "fast-fake").await;
    let handle = bridge.handle();
    let workspace = add_workspace(&handle, &repo_dir).await;

    // The kind's default model takes fast mode, so the start carries it to the session.
    let fast = handle
        .agent_start(AgentStartParams {
            kind: "fast-fake".to_owned(),
            read_only: true,
            fast: Some(true),
            ..fake_start_params(&workspace.id)
        })
        .await
        .expect("the fast start is accepted");
    assert!(fast.fast.as_ref().is_some_and(|speed| speed.on));
    fold_until(&mut updates, &mut state, "the fast session", |s| {
        s.agents
            .iter()
            .any(|agent| agent.id == fast.id && agent.fast.as_ref().is_some_and(|speed| speed.on))
    })
    .await;

    // A start that does not ask for fast mode carries none.
    let plain = handle
        .agent_start(AgentStartParams {
            kind: "fast-fake".to_owned(),
            read_only: true,
            ..fake_start_params(&workspace.id)
        })
        .await
        .expect("the plain start is accepted");
    assert!(plain.fast.is_none());

    // A model that does not take fast mode refuses it, and so does a kind with no fast mode.
    let refused = handle
        .agent_start(AgentStartParams {
            kind: "fast-fake".to_owned(),
            model: Some("plain-model".to_owned()),
            read_only: true,
            fast: Some(true),
            ..fake_start_params(&workspace.id)
        })
        .await
        .expect_err("a model without fast mode refuses it");
    assert_eq!(refused.code(), Some(codes::AGENT_OPTION_UNSUPPORTED));
    let refused = handle
        .agent_start(AgentStartParams {
            read_only: true,
            fast: Some(true),
            ..fake_start_params(&workspace.id)
        })
        .await
        .expect_err("a kind with no fast mode refuses it");
    assert_eq!(refused.code(), Some(codes::AGENT_OPTION_UNSUPPORTED));

    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn a_folder_projects_second_start_runs_in_its_folder_beside_the_first() {
    let home = temp_home();
    let folder = home.path().with_file_name("notes");
    std::fs::create_dir_all(&folder).expect("the folder");
    declare_fake_kind(home.path());
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let handle = bridge.handle();
    let mut updates = bridge.take_updates().expect("the updates are there");
    timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentKindsResync(kinds) =
                updates.recv().await.expect("the bridge is alive")
                && kinds.iter().any(|kind| kind.id == "fake" && kind.detected)
            {
                return;
            }
        }
    })
    .await
    .expect("the bridge connects and lists the fake kind");
    let project = handle
        .add_project(ProjectAddParams {
            path: folder.display().to_string(),
            name: Some("notes".to_owned()),
            kind: Some(ProjectKind::Folder),
            ..base_project_add_params()
        })
        .await
        .expect("the folder project is added");
    let params = WorkspaceCreateParams {
        project: project.id.clone(),
        name: "notes".to_owned(),
        ..base_workspace_create_params()
    };
    let first_workspace = handle
        .create_workspace(params.clone())
        .await
        .expect("the first start creates the folder's workspace");
    assert!(
        handle.create_workspace(params).await.is_err(),
        "the folder is taken, so a second create there is refused"
    );
    let client = Client::connect(&profile(home.path()), connect_options(None))
        .await
        .expect("the client connects");
    let workspaces = client
        .workspace_list(&WorkspaceListParams {
            ..base_workspace_list_params()
        })
        .await
        .expect("the service lists workspaces")
        .workspaces;
    let folder_workspace = folder_start_workspace(&project, &workspaces)
        .expect("the folder workspace is listed for its project");
    assert_eq!(folder_workspace.id, first_workspace.id);
    let first = start_fake(&handle, &folder_workspace.id).await;
    // Several writers may work in a folder workspace (decision 0169): a start beside a live one succeeds.
    let second = handle
        .agent_start(fake_start_params(&folder_workspace.id))
        .await
        .expect("a second writer starts in the folder while the first runs");
    assert_eq!(second.workspace_id, folder_workspace.id);
    assert_ne!(second.id, first.id);
    assert!(
        second.shared_folder_with.contains(&first.id),
        "the second session names the first as sharing its folder: {:?}",
        second.shared_folder_with
    );
    drop(bridge);
    stop_service(service).await;
}

#[tokio::test]
async fn a_worktree_keeps_refusing_a_second_writer_while_the_first_runs() {
    let home = temp_home();
    let repo_dir = home.path().with_file_name("repo");
    repo(&repo_dir);
    declare_fake_kind(home.path());
    let service = start_service(home.path()).await;
    let mut bridge = ServiceBridge::start(profile(home.path()), connect_options(None))
        .expect("the bridge starts");
    let handle = bridge.handle();
    let mut updates = bridge.take_updates().expect("the updates are there");
    timeout(WAIT, async {
        loop {
            if let StoreUpdate::AgentKindsResync(kinds) =
                updates.recv().await.expect("the bridge is alive")
                && kinds.iter().any(|kind| kind.id == "fake" && kind.detected)
            {
                return;
            }
        }
    })
    .await
    .expect("the bridge connects and lists the fake kind");
    let workspace = add_workspace(&handle, &repo_dir).await;
    let _first = start_fake(&handle, &workspace.id).await;
    let beside = handle
        .agent_start(fake_start_params(&workspace.id))
        .await
        .expect_err("a live writer holds the worktree");
    assert_eq!(beside.code(), Some(codes::AGENT_WORKSPACE_BUSY));
    drop(bridge);
    stop_service(service).await;
}

/// Every `AgentBringBackParams` field at its usual value; tests override the fields they vary.
fn base_agent_bring_back_params() -> massorca_protocol::methods::agents::AgentBringBackParams {
    massorca_protocol::methods::agents::AgentBringBackParams {
        child: String::new(),
        merge: true,
        prompt: true,
        token: None,
    }
}

/// Every `AgentDelegateParams` field at its usual value; tests override the fields they vary.
fn base_agent_delegate_params() -> massorca_protocol::methods::agents::AgentDelegateParams {
    massorca_protocol::methods::agents::AgentDelegateParams {
        parent: String::new(),
        prompt: "look".to_owned(),
        kind: None,
        name: None,
        title: None,
        read_only: true,
        same_workspace: false,
        token: None,
        request_id: None,
        permission_mode: None,
    }
}

/// Every `AgentPromptParams` field at its usual value; tests override the fields they vary.
fn base_agent_prompt_params() -> massorca_protocol::methods::agents::AgentPromptParams {
    massorca_protocol::methods::agents::AgentPromptParams {
        session: String::new(),
        text: "again".to_owned(),
        force: false,
    }
}

/// Every `AgentStartParams` field at its usual value; tests override the fields they vary.
fn base_agent_start_params() -> massorca_protocol::methods::agents::AgentStartParams {
    massorca_protocol::methods::agents::AgentStartParams {
        workspace: None,
        kind: "fake".to_owned(),
        prompt: None,
        args: Vec::new(),
        env: Default::default(),
        read_only: false,
        title: None,
        cols: None,
        rows: None,
        request_id: None,
        permission_mode: None,
        model: None,
        effort: None,
        fast: None,
    }
}

/// Every `WorkspaceCreateParams` field at its usual value; tests override the fields they vary.
fn base_workspace_create_params() -> massorca_protocol::methods::workspaces::WorkspaceCreateParams {
    massorca_protocol::methods::workspaces::WorkspaceCreateParams {
        project: String::new(),
        name: "main".to_owned(),
        branch: None,
        base: None,
        existing_branch: false,
        refresh_base: false,
        path: None,
        request_id: None,
        no_setup: false,
        unique_name: false,
        name_generated: false,
    }
}

/// Every `WorkspaceListParams` field at its usual value; tests override the fields they vary.
fn base_workspace_list_params() -> massorca_protocol::methods::workspaces::WorkspaceListParams {
    massorca_protocol::methods::workspaces::WorkspaceListParams { project: None }
}

/// Every `ProjectAddParams` field at its usual value; tests override the fields they vary.
fn base_project_add_params() -> massorca_protocol::methods::projects::ProjectAddParams {
    massorca_protocol::methods::projects::ProjectAddParams {
        path: String::new(),
        name: None,
        kind: None,
        workspaces_root: None,
        icon: None,
    }
}
