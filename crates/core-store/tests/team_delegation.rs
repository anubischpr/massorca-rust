//! Bot delegation links (decision 0128): a child is stored with its link in one step, a repeated request returns the
//! first child, a child cannot be linked back to an ancestor, and the depth counts the links above a task.

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

fn task_row(team_id: &str, title: &str) -> TaskRow {
    let now = now_ms();
    TaskRow {
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
    }
}

fn task(store: &Store, team_id: &str, title: &str) -> String {
    let row = task_row(team_id, title);
    store.insert_task(&row).expect("task");
    row.id
}

#[test]
fn a_child_names_its_parent_and_the_parent_lists_its_children_oldest_first() {
    let store = Store::open_in_memory().expect("store");
    let team = team(&store);
    let parent = task(&store, &team, "Lead the parser");
    let first = task_row(&team, "Write the lexer");
    let second = task_row(&team, "Write the tests");
    let (first_id, second_id) = (first.id.clone(), second.id.clone());
    store
        .insert_delegated_task(&first, &parent, false, None, now_ms())
        .expect("link");
    store
        .insert_delegated_task(&second, &parent, true, None, now_ms())
        .expect("link");

    assert_eq!(
        store.task_delegation_parent(&first_id).expect("read"),
        Some((parent.clone(), false))
    );
    assert_eq!(
        store.task_delegation_parent(&second_id).expect("read"),
        Some((parent.clone(), true))
    );
    assert_eq!(store.task_delegation_parent(&parent).expect("read"), None);
    assert_eq!(
        store.task_delegation_children(&parent).expect("children"),
        vec![first_id, second_id]
    );
}

#[test]
fn a_removed_child_leaves_the_tree() {
    let store = Store::open_in_memory().expect("store");
    let team = team(&store);
    let parent = task(&store, &team, "Lead the parser");
    let child = task_row(&team, "Write the lexer");
    let child_id = child.id.clone();
    store
        .insert_delegated_task(&child, &parent, false, None, now_ms())
        .expect("link");

    store.delete_task(&child_id).expect("remove");
    assert!(
        store
            .task_delegation_children(&parent)
            .expect("children")
            .is_empty()
    );
}

#[test]
fn a_repeated_request_from_the_same_parent_returns_the_first_child_and_stores_nothing() {
    let store = Store::open_in_memory().expect("store");
    let team = team(&store);
    let parent = task(&store, &team, "Lead the parser");
    let first = task_row(&team, "Write the lexer");
    let first_id = first.id.clone();
    let created = store
        .insert_delegated_task(&first, &parent, false, Some("req-1"), now_ms())
        .expect("first");
    assert_eq!(created, None);

    let again = task_row(&team, "Write the lexer");
    let repeat = store
        .insert_delegated_task(&again, &parent, false, Some("req-1"), now_ms())
        .expect("repeat");
    assert_eq!(repeat, Some(first_id.clone()));
    assert_eq!(store.get_task(&again.id).expect("read"), None);
    assert_eq!(
        store.task_delegation_children(&parent).expect("children"),
        vec![first_id]
    );

    // The same request id under another parent is a different call.
    let other_parent = task(&store, &team, "Lead the tests");
    let other = task_row(&team, "Write the tests");
    assert_eq!(
        store
            .insert_delegated_task(&other, &other_parent, false, Some("req-1"), now_ms())
            .expect("other parent"),
        None
    );
}

#[test]
fn a_child_cannot_be_linked_to_an_ancestor_so_no_cycle_forms() {
    let store = Store::open_in_memory().expect("store");
    let team = team(&store);
    let root = task(&store, &team, "Lead the parser");
    let child = task_row(&team, "Write the lexer");
    let child_id = child.id.clone();
    store
        .insert_delegated_task(&child, &root, false, None, now_ms())
        .expect("link");

    // Linking the root as a child of its own child would re-store an existing task: refused, and nothing is linked.
    let mut as_child = task_row(&team, "unused");
    as_child.id = root.clone();
    assert!(
        store
            .insert_delegated_task(&as_child, &child_id, false, None, now_ms())
            .is_err()
    );
    assert_eq!(store.task_delegation_parent(&root).expect("read"), None);
    assert!(
        store
            .task_delegation_children(&child_id)
            .expect("children")
            .is_empty()
    );
}

#[test]
fn the_depth_counts_the_links_above_a_task() {
    let store = Store::open_in_memory().expect("store");
    let team = team(&store);
    let root = task(&store, &team, "Lead the parser");
    let child = task_row(&team, "Write the lexer");
    let child_id = child.id.clone();
    store
        .insert_delegated_task(&child, &root, false, None, now_ms())
        .expect("link");
    let grandchild = task_row(&team, "Write the tokens");
    let grandchild_id = grandchild.id.clone();
    store
        .insert_delegated_task(&grandchild, &child_id, false, None, now_ms())
        .expect("link");

    assert_eq!(store.task_delegation_depth(&root).expect("depth"), 0);
    assert_eq!(store.task_delegation_depth(&child_id).expect("depth"), 1);
    assert_eq!(
        store.task_delegation_depth(&grandchild_id).expect("depth"),
        2
    );
}
