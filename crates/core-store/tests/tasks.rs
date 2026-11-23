//! Tasks: the rows, their links to a team, bot and project, and what a removal of those does to them.

use massorca_core_store::{
    BotRow, Store, StoreError, TaskOrigin, TaskOutcome, TaskRow, TeamProjectRow, TeamRow, new_id,
    now_ms,
};

fn store() -> Store {
    Store::open_in_memory().expect("in-memory store opens")
}

fn team_row(name: &str) -> TeamRow {
    TeamRow {
        id: new_id(),
        name: name.to_owned(),
        purpose: String::new(),
        character: "triangle".to_owned(),
        access: "full".to_owned(),
        workspaces: false,
        created_at: now_ms(),
    }
}

fn bot_row(team_id: &str, name: &str) -> BotRow {
    BotRow {
        id: new_id(),
        team_id: team_id.to_owned(),
        name: name.to_owned(),
        job: "Writes the parser".to_owned(),
        kind: "claude".to_owned(),
        model: None,
        effort: None,
        lead: false,
        created_at: now_ms(),
    }
}

fn project_row(team_id: &str, name: &str) -> TeamProjectRow {
    TeamProjectRow {
        source_project_id: None,
        id: new_id(),
        team_id: team_id.to_owned(),
        name: name.to_owned(),
        goal: String::new(),
        brief: String::new(),
        created_at: now_ms(),
    }
}

fn task_row(team_id: &str, title: &str, created_at: i64) -> TaskRow {
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
        created_at,
        updated_at: created_at,
    }
}

#[test]
fn task_round_trips_with_its_origin_outcome_and_error() {
    let store = store();
    let team = team_row("alpha");
    store.insert_team(&team).unwrap();
    let row = TaskRow {
        body: "Parse the header first.".to_owned(),
        origin: TaskOrigin::Room,
        session_id: Some("session-1".to_owned()),
        outcome: Some(TaskOutcome::Failure),
        error: Some("the agent refused".to_owned()),
        ..task_row(&team.id, "parse", 10)
    };
    store.insert_task(&row).unwrap();

    assert_eq!(store.get_task(&row.id).unwrap(), Some(row.clone()));
    assert_eq!(store.get_task("missing").unwrap(), None);
    assert_eq!(store.list_tasks(Some(&team.id)).unwrap(), vec![row]);
}

#[test]
fn list_tasks_filters_by_team_and_orders_by_creation() {
    let store = store();
    let alpha = team_row("alpha");
    let beta = team_row("beta");
    store.insert_team(&alpha).unwrap();
    store.insert_team(&beta).unwrap();
    let second = task_row(&alpha.id, "second", 20);
    let first = task_row(&alpha.id, "first", 10);
    let elsewhere = task_row(&beta.id, "elsewhere", 5);
    store.insert_task(&second).unwrap();
    store.insert_task(&first).unwrap();
    store.insert_task(&elsewhere).unwrap();

    assert_eq!(
        store.list_tasks(Some(&alpha.id)).unwrap(),
        vec![first.clone(), second.clone()]
    );
    assert_eq!(store.list_tasks(None).unwrap().len(), 3);
}

#[test]
fn list_tasks_of_a_bot_or_a_project_names_only_its_links() {
    let store = store();
    let team = team_row("alpha");
    store.insert_team(&team).unwrap();
    let mara = bot_row(&team.id, "mara");
    store.insert_bot(&mara).unwrap();
    let parser = project_row(&team.id, "parser");
    store.insert_team_project(&parser).unwrap();
    let linked = TaskRow {
        bot_id: Some(mara.id.clone()),
        project_id: Some(parser.id.clone()),
        ..task_row(&team.id, "linked", 10)
    };
    store.insert_task(&linked).unwrap();
    store.insert_task(&task_row(&team.id, "loose", 20)).unwrap();

    assert_eq!(
        store.list_tasks_of_bot(&mara.id).unwrap(),
        vec![linked.clone()]
    );
    assert_eq!(
        store.list_tasks_of_project(&parser.id).unwrap(),
        vec![linked]
    );
}

#[test]
fn update_task_changes_its_mutable_fields_and_keeps_its_team_and_origin() {
    let store = store();
    let team = team_row("alpha");
    store.insert_team(&team).unwrap();
    let mara = bot_row(&team.id, "mara");
    store.insert_bot(&mara).unwrap();
    let mut row = TaskRow {
        bot_id: Some(mara.id.clone()),
        origin: TaskOrigin::Room,
        error: Some("start failed".to_owned()),
        ..task_row(&team.id, "parse", 10)
    };
    store.insert_task(&row).unwrap();

    row.title = "parse the body".to_owned();
    row.bot_id = None;
    row.session_id = Some("session-2".to_owned());
    row.outcome = Some(TaskOutcome::Success);
    row.error = None;
    row.updated_at = 30;
    assert!(store.update_task(&row).unwrap());

    let found = store.get_task(&row.id).unwrap().unwrap();
    assert_eq!(found, row);
    assert_eq!(found.origin, TaskOrigin::Room);
    assert!(!store.update_task(&task_row(&team.id, "gone", 1)).unwrap());
}

#[test]
fn delete_task_removes_only_that_task() {
    let store = store();
    let team = team_row("alpha");
    store.insert_team(&team).unwrap();
    let gone = task_row(&team.id, "gone", 10);
    let kept = task_row(&team.id, "kept", 20);
    store.insert_task(&gone).unwrap();
    store.insert_task(&kept).unwrap();

    assert!(store.delete_task(&gone.id).unwrap());
    assert!(!store.delete_task(&gone.id).unwrap());
    assert_eq!(store.list_tasks(Some(&team.id)).unwrap(), vec![kept]);
}

#[test]
fn removing_a_bot_or_a_project_leaves_its_tasks_without_that_link() {
    let store = store();
    let team = team_row("alpha");
    store.insert_team(&team).unwrap();
    let mara = bot_row(&team.id, "mara");
    store.insert_bot(&mara).unwrap();
    let parser = project_row(&team.id, "parser");
    store.insert_team_project(&parser).unwrap();
    let row = TaskRow {
        bot_id: Some(mara.id.clone()),
        project_id: Some(parser.id.clone()),
        ..task_row(&team.id, "parse", 10)
    };
    store.insert_task(&row).unwrap();

    assert!(store.delete_bot(&mara.id).unwrap());
    assert!(store.delete_team_project(&parser.id).unwrap());

    let found = store.get_task(&row.id).unwrap().unwrap();
    assert_eq!((found.bot_id, found.project_id), (None, None));
    assert_eq!(found.title, "parse");
}

#[test]
fn deleting_a_team_removes_its_bots_projects_and_tasks_and_keeps_other_teams() {
    let store = store();
    let team = team_row("alpha");
    let other = team_row("beta");
    store.insert_team(&team).unwrap();
    store.insert_team(&other).unwrap();
    let mara = bot_row(&team.id, "mara");
    store.insert_bot(&mara).unwrap();
    let parser = project_row(&team.id, "parser");
    store.insert_team_project(&parser).unwrap();
    let task = TaskRow {
        bot_id: Some(mara.id.clone()),
        project_id: Some(parser.id.clone()),
        ..task_row(&team.id, "parse", 10)
    };
    store.insert_task(&task).unwrap();
    let kept = task_row(&other.id, "kept", 20);
    store.insert_task(&kept).unwrap();

    assert!(store.delete_team(&team.id).unwrap());

    assert_eq!(store.get_bot(&mara.id).unwrap(), None);
    assert_eq!(store.get_team_project(&parser.id).unwrap(), None);
    assert_eq!(store.get_task(&task.id).unwrap(), None);
    assert_eq!(store.get_task(&kept.id).unwrap(), Some(kept));
}

#[test]
fn a_task_must_name_an_existing_team_bot_and_project() {
    let store = store();
    let team = team_row("alpha");
    store.insert_team(&team).unwrap();

    let orphan_team = task_row("missing-team", "orphan", 10);
    assert!(matches!(
        store.insert_task(&orphan_team),
        Err(StoreError::Constraint { .. })
    ));

    let missing_bot = TaskRow {
        bot_id: Some("missing-bot".to_owned()),
        ..task_row(&team.id, "bot", 20)
    };
    assert!(matches!(
        store.insert_task(&missing_bot),
        Err(StoreError::Constraint { .. })
    ));

    let missing_project = TaskRow {
        project_id: Some("missing-project".to_owned()),
        ..task_row(&team.id, "project", 30)
    };
    assert!(matches!(
        store.insert_task(&missing_project),
        Err(StoreError::Constraint { .. })
    ));
    assert_eq!(store.list_tasks(None).unwrap(), Vec::<TaskRow>::new());
}
