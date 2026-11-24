//! `commands.describe`: the command table, with each command's method, capability and permission.

use massorca_protocol::capabilities::{COMMANDS, SERVICE};
use massorca_protocol::methods::commands::{CommandEntry, CommandTable, DESCRIBE};
use massorca_server::core_commands;

use super::common::{connect, start, stop, temp_home};

/// The command table of a fresh service, which is stopped before the table is returned.
async fn command_table() -> CommandTable {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let table = client
        .commands_describe()
        .await
        .expect("the command table is answered");
    drop(client);
    stop(running).await;
    table
}

fn entry<'a>(table: &'a CommandTable, id: &str) -> &'a CommandEntry {
    table
        .commands
        .iter()
        .find(|entry| entry.id == id)
        .unwrap_or_else(|| panic!("no entry for `{id}`"))
}

#[tokio::test]
async fn the_service_offers_the_command_capability_and_its_table_matches_the_descriptors() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    assert!(
        client.has_capability(COMMANDS),
        "the welcome offers {COMMANDS}"
    );
    let table = client
        .commands_describe()
        .await
        .expect("the command table is answered");
    drop(client);
    stop(running).await;

    let descriptors = core_commands();
    assert_eq!(
        table.commands.len(),
        descriptors.len(),
        "one entry per core command"
    );
    for descriptor in &descriptors {
        let listed = entry(&table, &descriptor.id);
        assert_eq!(listed.method, descriptor.method, "{}", descriptor.id);
        assert_eq!(listed.title, descriptor.title, "{}", descriptor.id);
        assert_eq!(
            listed.args_schema, descriptor.args_schema,
            "{}",
            descriptor.id
        );
    }
}

#[tokio::test]
async fn entries_come_in_id_order() {
    let table = command_table().await;
    let ids: Vec<&str> = table
        .commands
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted);
}

#[tokio::test]
async fn each_entry_names_the_capability_and_permission_of_its_method() {
    let table = command_table().await;

    let info = entry(&table, "service.info");
    assert_eq!(info.capability.as_deref(), Some(SERVICE));
    assert_eq!(info.permission.as_deref(), Some("read"));

    // `service.stop` issues `service.shutdown`, which needs the host-admin permission.
    let stopping = entry(&table, "service.stop");
    assert_eq!(stopping.method, "service.shutdown");
    assert_eq!(stopping.permission.as_deref(), Some("host-admin"));

    for listed in &table.commands {
        assert!(
            listed.capability.is_some() && listed.permission.is_some(),
            "`{}` names the capability and permission of its method",
            listed.id
        );
    }
}

#[tokio::test]
async fn the_table_lists_its_own_command_as_a_read() {
    let table = command_table().await;
    let own = entry(&table, "agent-context");
    assert_eq!(own.method, DESCRIBE);
    assert_eq!(own.capability.as_deref(), Some(COMMANDS));
    assert_eq!(own.permission.as_deref(), Some("read"));
}
