//! Bot delegation links (decision 0128): a child task names the parent that delegated it, and the parent lists its children.

use massorca_core_store::{Store, TaskOrigin, TaskRow, TeamRow, new_id, now_ms};

fn team(store: &Store) -> String {
    let row = TeamRow {
        id: new_id(),
        name: "Parser".to_owned(),
        purpose: "Ships the parser".to_owned(),
        character: "triangle".to_owned(),
        access: "full".to_owned(),
        workspaces: false,
        created_at: now_ms(),
    };
    store.insert_team(&row).expect("team");
    row.id
}

fn task(store: &Store, team_id: &str, title: &str) -> String {
    let now = now_ms();
    let row = TaskRow {
        id: new_id(),
        team_id: team_id.to_owned(),
        project_id: None,
        bot_id: None,
        title: title.to_owned(),
        body: String::new(),
        origin: TaskOrigin::Cli,
        session_id: None,
        outcome: None,
        error: None,
        error_code: None,
        workspace_id: None,
        branch: None,
        created_at: now,
        updated_at: now,
    };
    store.insert_task(&row).expect("task");
    row.id
}

#[test]
fn a_child_names_its_parent_and_the_parent_lists_its_children_oldest_first() {
    let store = Store::open_in_memory().expect("store");
    let team = team(&store);
    let parent = task(&store, &team, "Lead the parser");
    let first = task(&store, &team, "Write the lexer");
    let second = task(&store, &team, "Write the tests");
    store
        .insert_task_delegation(&first, &parent, false, now_ms())
        .expect("link");
    store
        .insert_task_delegation(&second, &parent, true, now_ms())
        .expect("link");

    assert_eq!(
        store.task_delegation_parent(&first).expect("read"),
        Some((parent.clone(), false))
    );
    assert_eq!(
        store.task_delegation_parent(&second).expect("read"),
        Some((parent.clone(), true))
    );
    assert_eq!(store.task_delegation_parent(&parent).expect("read"), None);
    assert_eq!(
        store.task_delegation_children(&parent).expect("children"),
        vec![first, second]
    );
}

#[test]
fn a_removed_child_leaves_the_tree_and_a_child_has_one_parent() {
    let store = Store::open_in_memory().expect("store");
    let team = team(&store);
    let parent = task(&store, &team, "Lead the parser");
    let child = task(&store, &team, "Write the lexer");
    store
        .insert_task_delegation(&child, &parent, false, now_ms())
        .expect("link");
    assert!(
        store
            .insert_task_delegation(&child, &parent, false, now_ms())
            .is_err()
    );

    store.delete_task(&child).expect("remove");
    assert!(
        store
            .task_delegation_children(&parent)
            .expect("children")
            .is_empty()
    );
}
