//! A run holding vault secrets keeps its agent-produced text from callers below host-admin scope (decision 0071): a
//! standard-scope device sees the run, its state and its mark, never its result, by call and by watch item. No agent is
//! started: the sessions are written to the store directly, as ended runs.

use massorca_core_store::{
    AgentSessionRow, ProjectKind, ProjectRow, RunExit, RunExitReason, Store, WorkspaceKind,
    WorkspaceRow, WorkspaceState, new_id, now_ms,
};
use massorca_protocol::methods::agents::{
    self, AgentSession, AgentSessionList, AgentSessionRef, AgentWatchEvent, AgentWatchParams,
};

use super::common::{connect, profile, stop, temp_home};
use super::network::{client_side, offer, pair, start_listening};

const RESULT: &str = "agent-result-placeholder-6a3f";

/// A project, a workspace and two ended sessions with a result: `marked` holds secrets, `plain` does not.
fn seed(store: &Store) -> (String, String) {
    let project = ProjectRow {
        id: new_id(),
        kind: ProjectKind::Folder,
        path: "/work/app".to_owned(),
        path_key: "/work/app".to_owned(),
        name: "app".to_owned(),
        workspaces_root: "/work/app-workspaces".to_owned(),
        added_at: now_ms(),
        base_ref: None,
    };
    store.insert_project(&project).expect("project");
    let workspace = WorkspaceRow {
        id: new_id(),
        project_id: project.id,
        kind: WorkspaceKind::Folder,
        name: "main".to_owned(),
        path: "/work/app".to_owned(),
        path_key: "/work/app".to_owned(),
        branch: None,
        base_ref: None,
        created_branch: false,
        state: WorkspaceState::Ready,
        request_id: None,
        created_at: now_ms(),
    };
    store.insert_workspace(&workspace).expect("workspace");
    let row = |holds_secrets: bool| AgentSessionRow {
        id: new_id(),
        kind: "fake".to_owned(),
        workspace_id: workspace.id.clone(),
        parent_id: None,
        title: None,
        read_only: true,
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
        result: Some(RESULT.to_owned()),
        brought_back_at: None,
        holds_secrets,
    };
    let (marked, plain) = (row(true), row(false));
    store.insert_agent_session(&marked).expect("marked session");
    store.insert_agent_session(&plain).expect("plain session");
    (marked.id, plain.id)
}

fn find<'a>(sessions: &'a [AgentSession], id: &str) -> &'a AgentSession {
    sessions
        .iter()
        .find(|session| session.id == id)
        .expect("the session is listed")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_standard_device_sees_a_marked_run_but_not_its_result() {
    let home = temp_home();
    let running = start_listening(home.path()).await;
    let store = Store::open(profile(home.path(), "server").store_path()).expect("the store");
    let (marked, plain) = seed(&store);
    let (_client_home, hosts) = client_side();
    let (device, _) = pair(&hosts, &offer(home.path(), false, None).await, "plain")
        .await
        .expect("the device pairs");
    let owner = connect(home.path(), "server").await;

    for (client, admin) in [(&device, false), (&owner, true)] {
        let shown: AgentSession = client
            .call(
                agents::SHOW,
                &AgentSessionRef {
                    session: marked.clone(),
                },
            )
            .await
            .expect("agent.status answers");
        assert!(shown.holds_secrets, "the mark is visible");
        assert_eq!(shown.result.is_some(), admin, "status, admin={admin}");

        let listed: AgentSessionList = client
            .call(agents::LIST, &serde_json::json!({}))
            .await
            .expect("agent.list answers");
        let listed_marked = find(&listed.sessions, &marked);
        assert!(listed_marked.holds_secrets);
        assert_eq!(listed_marked.result.is_some(), admin, "list, admin={admin}");
        assert_eq!(
            find(&listed.sessions, &plain).result.as_deref(),
            Some(RESULT),
            "an unmarked run keeps its result"
        );

        let mut watch = client
            .agent_watch(&AgentWatchParams {
                workspace: None,
                session: None,
            })
            .await
            .expect("agent.watch opens");
        let first = watch
            .next()
            .await
            .expect("a first item")
            .expect("not an error");
        let AgentWatchEvent::Snapshot { sessions } =
            serde_json::from_value(first.clone()).expect("a watch item")
        else {
            panic!("expected a snapshot, got {first}");
        };
        assert_eq!(
            find(&sessions, &marked).result.is_some(),
            admin,
            "watch, admin={admin}"
        );
    }

    // agent.changed names the session only: no subscriber receives agent-produced text through it.
    let mut events = device.subscribe_events().await.expect("events open");
    let _: AgentSession = owner
        .call(
            agents::FORGET,
            &AgentSessionRef {
                session: marked.clone(),
            },
        )
        .await
        .expect("the owner forgets the marked run");
    let event = loop {
        let item = events
            .next()
            .await
            .expect("an event")
            .expect("not an error");
        if item["topic"] == "agent.changed" {
            break item;
        }
    };
    assert_eq!(event["id"], marked.as_str());
    assert!(!event.to_string().contains(RESULT), "{event}");
    drop((device, owner));
    stop(running).await;
}
