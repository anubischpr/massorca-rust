//! Core methods: identity, dispatch errors, parameter decoding, registration rules and the command table.

use std::collections::HashSet;
use std::time::Duration;

use massorca_protocol::methods::config::ConfigList;
use massorca_protocol::methods::{config, events, projects, service, workspaces};
use massorca_protocol::{ErrorBody, PROTOCOL_VERSION, capabilities, codes};
use massorca_server::{
    MethodError, MethodInfo, Owner, Permission, RegistrationError, RequestCtx, core_commands,
};
use serde_json::{Value, json};
use tokio::time::timeout;

use super::common::{SERVER_VERSION, connect, echo, remote_code, start, stop, temp_home};

/// Every method of the Phase 1 surface.
const PHASE1_METHODS: [&str; 15] = [
    service::INFO,
    service::SHUTDOWN,
    events::SUBSCRIBE,
    config::GET,
    config::SET,
    config::LIST,
    config::RESET,
    projects::ADD,
    projects::LIST,
    projects::SHOW,
    projects::REMOVE,
    workspaces::CREATE,
    workspaces::LIST,
    workspaces::SHOW,
    workspaces::REMOVE,
];

/// Phase 2 terminal methods (decision 0035).
const DEVICE_METHODS: [&str; 5] = [
    "device.list",
    "device.revoke",
    "device.whoami",
    "pairing.create",
    "network.status",
];

const TERMINAL_METHODS: [&str; 8] = [
    "terminal.create",
    "terminal.list",
    "terminal.show",
    "terminal.attach",
    "terminal.input",
    "terminal.resize",
    "terminal.kill",
    "terminal.close",
];

/// The agent methods this build registers (capability `agents/1`).
const AGENT_METHODS: [&str; 14] = [
    "agent.kinds",
    "agent.start",
    "agent.list",
    "agent.show",
    "agent.stop",
    "agent.forget",
    "agent.hook",
    "agent.watch",
    "agent.delegate",
    "agent.tree",
    "agent.bring_back",
    "agent.credentials.import",
    "agent.credentials.show",
    "agent.credentials.remove",
];

/// The Bot Teams methods this build registers (capability `teams/1`).
const TEAM_METHODS: [&str; 19] = [
    "team.create",
    "team.list",
    "team.show",
    "team.update",
    "team.rm",
    "team.bot_add",
    "team.bot_update",
    "team.bot_rm",
    "team.project_add",
    "team.project_update",
    "team.project_rm",
    "team.board",
    "task.create",
    "task.list",
    "task.show",
    "task.update",
    "task.done",
    "task.reopen",
    "task.rm",
];

/// The CLI command paths the command table must offer (every subcommand except `service run` and `ext`).
const CLI_COMMAND_IDS: [&str; 26] = [
    "service.info",
    "service.status",
    "service.stop",
    "service.restart",
    "settings.get",
    "settings.set",
    "settings.list",
    "settings.reset",
    "project.add",
    "project.list",
    "project.show",
    "project.rm",
    "workspace.create",
    "workspace.list",
    "workspace.show",
    "workspace.rm",
    "secret.status",
    "secret.list",
    "secret.set",
    "secret.link",
    "secret.unlink",
    "secret.remove",
    "secret.request",
    "secret.requests",
    "secret.decline",
    "secret.reset-key",
];

#[tokio::test]
async fn service_info_reports_the_profile_the_process_and_the_capabilities() {
    let home = temp_home();
    let running = start(home.path(), "alpha").await;
    let client = connect(home.path(), "alpha").await;

    let info = client.service_info().await.expect("service.info answers");
    assert_eq!(info.version, SERVER_VERSION);
    assert_eq!(info.protocol, PROTOCOL_VERSION);
    assert_eq!(info.profile, "alpha");
    assert_eq!(info.pid, std::process::id());
    assert!(info.started_at_ms > 0);
    assert_eq!(info.capabilities, client.welcome().capabilities);
    assert_eq!(running.info().profile, "alpha");
    stop(running).await;
}

#[tokio::test]
async fn an_unknown_method_is_refused_with_method_not_found() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let error = client
        .call::<_, Value>("nothing.here", &Value::Null)
        .await
        .expect_err("there is no such method");
    assert_eq!(remote_code(&error), codes::METHOD_NOT_FOUND);
    stop(running).await;
}

#[tokio::test]
async fn params_that_do_not_decode_are_refused_with_invalid_params() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let error = client
        .call::<_, Value>(config::GET, &json!({"owner": 5}))
        .await
        .expect_err("owner must be text, and key is missing");
    assert_eq!(remote_code(&error), codes::REQUEST_INVALID_PARAMS);
    let error = client
        .call::<_, Value>(config::SET, &json!({"key": "workspaces.default_root"}))
        .await
        .expect_err("set needs a value");
    assert_eq!(remote_code(&error), codes::REQUEST_INVALID_PARAMS);
    stop(running).await;
}

#[tokio::test]
async fn null_params_decode_as_an_empty_object() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let list: ConfigList = client
        .call(config::LIST, &Value::Null)
        .await
        .expect("config.list takes no params, so null is fine");
    assert!(!list.values.is_empty());
    let error = client
        .call::<_, Value>(config::GET, &Value::Null)
        .await
        .expect_err("config.get needs a key even when params are null");
    assert_eq!(remote_code(&error), codes::REQUEST_INVALID_PARAMS);
    stop(running).await;
}

#[tokio::test]
async fn core_may_not_define_extension_methods_and_extensions_must_use_the_prefix() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let methods = &running.registries().methods;
    let demo = Owner::Extension("demo".to_owned());

    let reserved = methods
        .register(
            Owner::Core,
            "ext.core.thing",
            capabilities::SERVICE,
            Permission::Read,
            echo,
        )
        .expect_err("core may not define ext. methods");
    assert!(
        matches!(reserved, MethodError::ReservedPrefix { .. }),
        "got {reserved:?}"
    );
    let unprefixed = methods
        .register(demo.clone(), "demo.thing", "demo/1", Permission::Read, echo)
        .expect_err("an extension method must use ext.");
    assert!(
        matches!(unprefixed, MethodError::ExtensionPrefixMissing { .. }),
        "got {unprefixed:?}"
    );
    let _accepted = methods
        .register(demo, "ext.demo.thing", "demo/1", Permission::Read, echo)
        .expect("an extension may use its prefix");
    stop(running).await;
}

#[tokio::test]
async fn a_method_name_can_be_registered_only_once() {
    let home = temp_home();
    let running = start(home.path(), "default").await;

    let duplicate = running
        .registries()
        .methods
        .register(
            Owner::Core,
            service::INFO,
            capabilities::SERVICE,
            Permission::Read,
            echo,
        )
        .expect_err("service.info is already registered");
    assert!(
        matches!(
            duplicate,
            MethodError::Registration(RegistrationError::Duplicate { .. })
        ),
        "got {duplicate:?}"
    );
    stop(running).await;
}

#[tokio::test]
async fn every_core_method_has_a_command_descriptor() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let commands = core_commands();

    let methods: Vec<MethodInfo> = running.registries().methods.methods();
    for info in methods.iter().filter(|info| info.owner == Owner::Core) {
        assert!(
            commands.iter().any(|command| command.method == info.method),
            "no command runs {}",
            info.method
        );
    }
    for method in PHASE1_METHODS
        .iter()
        .chain(&TERMINAL_METHODS)
        .chain(&DEVICE_METHODS)
        .chain(&AGENT_METHODS)
        .chain(&TEAM_METHODS)
    {
        assert!(
            commands.iter().any(|command| command.method == *method),
            "no command runs {method}"
        );
    }
    assert!(
        commands.iter().all(|command| methods
            .iter()
            .any(|info| info.owner == Owner::Core && info.method == command.method)),
        "every command runs a registered core method"
    );
    stop(running).await;
}

#[tokio::test]
async fn the_registry_offers_the_core_command_table_under_owner_core() {
    let home = temp_home();
    let running = start(home.path(), "default").await;

    let registered = running.registries().commands.list();
    assert_eq!(registered.len(), core_commands().len());
    assert!(registered.iter().all(|entry| entry.owner == Owner::Core));
    stop(running).await;
}

#[test]
fn command_ids_are_the_cli_paths_and_unique() {
    let commands = core_commands();
    let ids: HashSet<&str> = commands.iter().map(|command| command.id.as_str()).collect();
    assert_eq!(ids.len(), commands.len(), "command ids are unique");
    for expected in CLI_COMMAND_IDS {
        assert!(
            ids.contains(&expected),
            "no command for `massorca {}`",
            expected.replace('.', " ")
        );
    }
}

/// A handler that panics, so the test can check the request is still answered.
async fn panicking(_ctx: RequestCtx, _params: Value) -> Result<Value, ErrorBody> {
    panic!("a handler bug")
}

#[tokio::test]
async fn a_panicking_handler_is_answered_as_internal_and_the_connection_keeps_working() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    // The handle keeps the method registered for the test's duration.
    let _panicking = running
        .registries()
        .methods
        .register(
            Owner::Core,
            "test.panic",
            capabilities::SERVICE,
            Permission::Read,
            panicking,
        )
        .expect("the method is registered");
    let client = connect(home.path(), "default").await;

    let answer = timeout(
        Duration::from_secs(5),
        client.call::<_, Value>("test.panic", &Value::Null),
    )
    .await
    .expect("the request is answered instead of hanging");
    let error = answer.expect_err("the panicking call fails");
    assert_eq!(remote_code(&error), codes::INTERNAL);

    let info = timeout(
        Duration::from_secs(5),
        client.call::<_, Value>(service::INFO, &Value::Null),
    )
    .await
    .expect("the connection still answers");
    assert!(info.is_ok(), "{info:?}");
    stop(running).await;
}
