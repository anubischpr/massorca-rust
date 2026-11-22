//! Team projects: the rows, their names unique within a team, and their removal with the team.

use massorca_core_store::{Store, StoreError, TeamProjectRow, TeamRow, new_id, now_ms};

fn store() -> Store {
    Store::open_in_memory().expect("in-memory store opens")
}

fn team_row(name: &str) -> TeamRow {
    TeamRow {
        id: new_id(),
        name: name.to_owned(),
        purpose: "Ships the parser".to_owned(),
        character: "triangle".to_owned(),
        access: "full".to_owned(),
        workspaces: false,
        created_at: now_ms(),
    }
}

fn project_row(team_id: &str, name: &str) -> TeamProjectRow {
    TeamProjectRow {
        id: new_id(),
        team_id: team_id.to_owned(),
        name: name.to_owned(),
        goal: "Parse the config".to_owned(),
        brief: String::new(),
        created_at: now_ms(),
    }
}

/// Asserts a unique-constraint conflict whose message names `expected`.
fn assert_conflict(result: Result<(), StoreError>, expected: &str) {
    match result {
        Err(StoreError::Conflict { constraint }) => {
            assert!(
                constraint.contains(expected),
                "expected {expected}, got {constraint}"
            );
        }
        other => panic!("expected a conflict on {expected}, got {other:?}"),
    }
}

#[test]
fn team_project_round_trips_through_each_lookup() {
    let store = store();
    let team = team_row("alpha");
    store.insert_team(&team).unwrap();
    let row = project_row(&team.id, "parser");
    store.insert_team_project(&row).unwrap();

    assert_eq!(store.get_team_project(&row.id).unwrap(), Some(row.clone()));
    assert_eq!(
        store.find_team_project_by_name(&team.id, "parser").unwrap(),
        Some(row.clone())
    );
    assert_eq!(
        store.list_team_projects(&team.id).unwrap(),
        vec![row.clone()]
    );
    assert_eq!(store.get_team_project("missing").unwrap(), None);
}

#[test]
fn team_project_names_are_unique_within_their_team_only() {
    let store = store();
    let team = team_row("alpha");
    let other = team_row("beta");
    store.insert_team(&team).unwrap();
    store.insert_team(&other).unwrap();
    store
        .insert_team_project(&project_row(&team.id, "parser"))
        .unwrap();

    store
        .insert_team_project(&project_row(&other.id, "parser"))
        .unwrap();
    assert_conflict(
        store.insert_team_project(&project_row(&team.id, "parser")),
        "team_projects.name",
    );
    assert_eq!(store.list_team_projects(&team.id).unwrap().len(), 1);
}

#[test]
fn update_team_project_changes_its_name_goal_and_brief() {
    let store = store();
    let team = team_row("alpha");
    store.insert_team(&team).unwrap();
    let mut row = project_row(&team.id, "parser");
    store.insert_team_project(&row).unwrap();

    row.name = "lexer".to_owned();
    row.goal = "Tokenise the config".to_owned();
    row.brief = "Keep it small.".to_owned();
    assert!(store.update_team_project(&row).unwrap());
    assert_eq!(store.get_team_project(&row.id).unwrap(), Some(row.clone()));
    assert!(
        !store
            .update_team_project(&project_row(&team.id, "gone"))
            .unwrap()
    );
}

#[test]
fn deleting_a_team_project_keeps_the_team_and_its_other_projects() {
    let store = store();
    let team = team_row("alpha");
    store.insert_team(&team).unwrap();
    let parser = project_row(&team.id, "parser");
    let lexer = project_row(&team.id, "lexer");
    store.insert_team_project(&parser).unwrap();
    store.insert_team_project(&lexer).unwrap();

    assert!(store.delete_team_project(&parser.id).unwrap());
    assert!(!store.delete_team_project(&parser.id).unwrap());
    assert_eq!(store.list_team_projects(&team.id).unwrap(), vec![lexer]);
    assert_eq!(store.get_team(&team.id).unwrap(), Some(team));
}

#[test]
fn deleting_a_team_removes_its_projects() {
    let store = store();
    let team = team_row("alpha");
    let other = team_row("beta");
    store.insert_team(&team).unwrap();
    store.insert_team(&other).unwrap();
    let parser = project_row(&team.id, "parser");
    store.insert_team_project(&parser).unwrap();
    let kept = project_row(&other.id, "parser");
    store.insert_team_project(&kept).unwrap();

    assert!(store.delete_team(&team.id).unwrap());
    assert_eq!(store.get_team_project(&parser.id).unwrap(), None);
    assert_eq!(store.get_team_project(&kept.id).unwrap(), Some(kept));
}
