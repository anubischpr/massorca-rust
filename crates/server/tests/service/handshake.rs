//! The handshake: the welcome a client receives, and how the service refuses a handshake it cannot accept.

use std::time::{Duration, Instant};

use massorca_protocol::{ErrorBody, Message};
use massorca_protocol::{
    MIN_PROTOCOL_VERSION, PROTOCOL_VERSION, ProtocolRange, capabilities, codes, methods::service,
};
use massorca_server::{Owner, Permission, RequestCtx};
use serde_json::{Value, json};

use super::common::{
    REPLY_WAIT, Raw, SERVER_NAME, SERVER_VERSION, connect, hello_offering, hello_with,
    refusal_code, refusal_range, request, start, stop, temp_home,
};

#[tokio::test]
async fn welcome_offers_the_registered_capabilities() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let welcome = client.welcome();
    assert_eq!(welcome.protocol, PROTOCOL_VERSION);
    assert_eq!(welcome.server.name, SERVER_NAME);
    assert_eq!(welcome.server.version, SERVER_VERSION);
    assert_eq!(
        welcome.capabilities,
        [
            capabilities::AGENT_ARCHIVE,
            capabilities::AGENT_CHAT,
            capabilities::AGENT_FAST,
            capabilities::AGENT_FORK,
            capabilities::AGENT_IMPORT_DETECT,
            capabilities::AGENT_MODELS,
            capabilities::AGENT_MODES,
            capabilities::AGENT_NO_PROJECT,
            capabilities::AGENT_OPTIONS_LIVE,
            capabilities::AGENT_OPTIONS,
            capabilities::AGENT_PROMPT_ATTACHMENTS,
            capabilities::AGENT_QUEUE,
            capabilities::AGENT_RELEASE,
            capabilities::AGENT_SEEN,
            capabilities::AGENT_TITLE,
            capabilities::AGENTS,
            capabilities::AUTOMATIONS,
            capabilities::BINARY_FRAMES,
            capabilities::CALENDAR,
            capabilities::COMMANDS,
            capabilities::CONFIG,
            capabilities::DEVICES,
            capabilities::EVENTS,
            capabilities::EXTENSION_POINTS,
            capabilities::EXTENSIONS_LOG,
            capabilities::EXTENSIONS_PROGRAMS,
            capabilities::EXTENSIONS_ROLLBACK,
            capabilities::EXTENSIONS,
            capabilities::GIT_PROJECT_BRANCHES,
            capabilities::GIT_REBASE_BASE,
            capabilities::GIT_REMOTE_IDENTITY,
            capabilities::GIT,
            capabilities::HOST,
            capabilities::NETWORK,
            capabilities::PAIRING_ADDRESS,
            capabilities::PAIRING_CANCEL,
            capabilities::PORT_FORWARD,
            capabilities::PROJECT_ICON_COLOR,
            capabilities::PROJECT_INIT_GIT,
            capabilities::PROJECT_UPDATE,
            capabilities::PROJECTS,
            capabilities::SCRIPT_TRUST,
            capabilities::SECRETS,
            capabilities::SERVICE_TOOLS,
            capabilities::SERVICE,
            capabilities::SERVICES,
            capabilities::SKILLS_PLACE,
            capabilities::SKILLS_PLACE_SCOPES,
            capabilities::SKILLS,
            capabilities::TEAM_ROOM,
            capabilities::TEAM_ROOM_CARDS,
            capabilities::TEAM_ROOM_REPLIES,
            capabilities::TEAMS,
            capabilities::TERMINAL_WAIT,
            capabilities::TERMINALS,
            capabilities::USAGE,
            capabilities::WORKSPACE_CHECK,
            capabilities::WORKSPACE_CURRENT,
            capabilities::WORKSPACE_IN_PLACE,
            capabilities::WORKSPACE_RENAME,
            capabilities::WORKSPACE_STOP_RUNS,
            capabilities::WORKSPACE_UNIQUE_NAME,
            capabilities::WORKSPACE_UPDATE,
            capabilities::WORKSPACES,
        ]
    );
    assert!(!welcome.connection_id.is_empty());
    assert!(client.has_capability(capabilities::SERVICE));
    stop(running).await;
}

#[tokio::test]
async fn a_hello_with_no_shared_version_is_refused_and_the_connection_closed() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let mut raw = Raw::open(home.path(), "default").await;
    raw.send(&hello_with(ProtocolRange { min: 5, max: 9 }))
        .await;

    let refusal = raw
        .recv_step("the refusal")
        .await
        .expect("the service explains the refusal before closing");
    assert_eq!(
        refusal_code(&refusal).as_deref(),
        Some(codes::PROTOCOL_UNSUPPORTED_VERSION)
    );
    assert_eq!(
        refusal_range(&refusal),
        Some(ProtocolRange {
            min: MIN_PROTOCOL_VERSION,
            max: PROTOCOL_VERSION,
        })
    );
    assert!(
        raw.recv_step("the close after the refusal").await.is_none(),
        "the connection closes after the refusal"
    );
    stop(running).await;
}

#[tokio::test]
async fn a_first_frame_other_than_hello_is_refused() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let mut raw = Raw::open(home.path(), "default").await;
    raw.send(&request(1, service::INFO, Value::Null)).await;

    let refusal = raw
        .recv_step("the refusal")
        .await
        .expect("the service explains the refusal before closing");
    assert_eq!(
        refusal_code(&refusal).as_deref(),
        Some(codes::PROTOCOL_HANDSHAKE)
    );
    assert!(
        raw.recv_step("the close after the refusal").await.is_none(),
        "the connection closes after the refusal"
    );
    stop(running).await;
}

/// A refusal the client reads only after the service has stopped, its end of the pipe closed, still arrives.
#[tokio::test]
async fn a_refusal_read_after_the_service_closed_still_arrives() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let mut raw = Raw::open(home.path(), "default").await;
    raw.send(&request(1, service::INFO, Value::Null)).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    // A stopped service has dropped every connection, so the refusal is read from a pipe the service closed.
    stop(running).await;

    let refusal = raw
        .recv_step("the refusal")
        .await
        .expect("the refusal survives the close");
    assert_eq!(
        refusal_code(&refusal).as_deref(),
        Some(codes::PROTOCOL_HANDSHAKE)
    );
    assert!(
        raw.recv_step("the close after the refusal").await.is_none(),
        "the connection closes after the refusal"
    );
}

// The exact window is measured on a paused clock in `handshake.rs`; this checks a real connection honours it.
#[tokio::test]
async fn hello_must_arrive_within_ten_seconds() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let mut raw = Raw::open(home.path(), "default").await;
    let started = Instant::now();
    // The service may explain itself before it closes; only the moment of the close matters here.
    // A loaded runner can only delay the close, so the limit is the window plus the usual reply wait.
    let waited = loop {
        if raw
            .recv_within(Duration::from_secs(10) + REPLY_WAIT)
            .await
            .is_none()
        {
            break started.elapsed();
        }
    };
    assert!(
        waited >= Duration::from_secs(9),
        "the connection closed after {waited:?}, before the 10 second hello window"
    );
    stop(running).await;
}

/// A handler sees who is calling and what the client listed in its `hello`, even capabilities the service does not know.
#[tokio::test]
async fn handlers_see_who_is_calling_and_what_the_client_offered() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let demo = Owner::Extension("demo".to_owned());
    let _capability = running
        .registries()
        .capabilities
        .register("demo/1", demo.clone())
        .expect("the capability registers");
    let _who = running
        .registries()
        .methods
        .register(
            demo,
            "ext.demo.who",
            "demo/1",
            Permission::Read,
            |ctx: RequestCtx, _params: Value| async move {
                let mut offered: Vec<String> = ctx.client_capabilities.iter().cloned().collect();
                offered.sort();
                Ok::<Value, ErrorBody>(json!({
                    "principal": format!("{:?}", ctx.principal),
                    "offered": offered,
                }))
            },
        )
        .expect("the method registers");

    let mut raw = Raw::open(home.path(), "default").await;
    raw.send(&hello_offering(&["service/1", "demo/1", "future/9"]))
        .await;
    assert!(
        matches!(raw.recv().await, Some(Message::Welcome { .. })),
        "the welcome comes first"
    );
    raw.send(&request(1, "ext.demo.who", Value::Null)).await;
    let answer = loop {
        match raw.recv().await {
            Some(Message::Response {
                id: 1,
                result: Some(value),
                ..
            }) => break value,
            Some(_) => {}
            None => panic!("the connection closed before the answer"),
        }
    };
    assert_eq!(answer["principal"], "Owner");
    assert_eq!(
        answer["offered"],
        json!(["demo/1", "future/9", "service/1"])
    );
    stop(running).await;
}
