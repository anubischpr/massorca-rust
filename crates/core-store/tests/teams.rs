//! Teams and their bots: the rows, their uniqueness rules, and the bot lead rule.

use massorca_core_store::{BotRow, Store, StoreError, TeamRow, new_id, now_ms};

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

fn bot_row(team_id: &str, name: &str, lead: bool) -> BotRow {
    BotRow {
        id: new_id(),
        team_id: team_id.to_owned(),
        name: name.to_owned(),
        job: "Writes the parser".to_owned(),
        kind: "claude".to_owned(),
        model: None,
        effort: None,
        lead,
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
fn team_round_trips_through_each_lookup() {
    let store = store();
    let row = team_row("desarrollo");
    store.insert_team(&row).unwrap();

    assert_eq!(store.get_team(&row.id).unwrap(), Some(row.clone()));
    assert_eq!(
        store.find_team_by_name("desarrollo").unwrap(),
        Some(row.clone())
    );
    assert_eq!(store.get_team("missing").unwrap(), None);
    assert_eq!(store.find_team_by_name("missing").unwrap(), None);
}

#[test]
fn team_names_and_ids_are_unique() {
    let store = store();
    let first = team_row("alpha");
    store.insert_team(&first).unwrap();

    assert_conflict(store.insert_team(&team_row("alpha")), "teams.name");

    let mut same_id = team_row("beta");
    same_id.id = first.id.clone();
    assert_conflict(store.insert_team(&same_id), "teams.id");

    assert_eq!(store.list_teams().unwrap().len(), 1);
}

#[test]
fn list_teams_orders_by_name() {
    let store = store();
    for name in ["gamma", "alpha", "beta"] {
        store.insert_team(&team_row(name)).unwrap();
    }

    let names: Vec<String> = store
        .list_teams()
        .unwrap()
        .into_iter()
        .map(|team| team.name)
        .collect();
    assert_eq!(names, ["alpha", "beta", "gamma"]);
}

#[test]
fn update_team_changes_its_fields_and_keeps_its_creation_time() {
    let store = store();
    let row = team_row("alpha");
    store.insert_team(&row).unwrap();

    let changed = TeamRow {
        name: "renamed".to_owned(),
        purpose: "Owns the release".to_owned(),
        character: "square".to_owned(),
        access: "full".to_owned(),
        workspaces: true,
        ..row.clone()
    };
    assert!(store.update_team(&changed).unwrap());

    let found = store.get_team(&row.id).unwrap().unwrap();
    assert_eq!(found, changed);
    assert_eq!(found.created_at, row.created_at);
    assert!(!store.update_team(&team_row("missing")).unwrap());
}

#[test]
fn bot_round_trips_and_its_name_is_unique_within_its_team() {
    let store = store();
    let team = team_row("alpha");
    let other = team_row("beta");
    store.insert_team(&team).unwrap();
    store.insert_team(&other).unwrap();

    let mara = bot_row(&team.id, "mara", false);
    store.insert_bot(&mara).unwrap();
    assert_eq!(store.get_bot(&mara.id).unwrap(), Some(mara.clone()));
    assert_eq!(
        store.find_bot_by_name(&team.id, "mara").unwrap(),
        Some(mara.clone())
    );
    assert_eq!(store.find_bot_by_name(&other.id, "mara").unwrap(), None);

    // Another team may already use the name.
    store
        .insert_bot(&bot_row(&other.id, "mara", false))
        .unwrap();
    assert_conflict(
        store.insert_bot(&bot_row(&team.id, "mara", false)),
        "bots.name",
    );
    assert_eq!(store.list_bots(&team.id).unwrap(), vec![mara]);
}

#[test]
fn a_team_has_at_most_one_lead_bot() {
    let store = store();
    let team = team_row("alpha");
    store.insert_team(&team).unwrap();
    let lead = bot_row(&team.id, "mara", true);
    store.insert_bot(&lead).unwrap();

    assert_conflict(
        store.insert_bot(&bot_row(&team.id, "bruno", true)),
        "bots.team_id",
    );
    store
        .insert_bot(&bot_row(&team.id, "bruno", false))
        .unwrap();
    assert_eq!(store.find_lead_bot(&team.id).unwrap(), Some(lead.clone()));

    let mut demoted = lead.clone();
    demoted.lead = false;
    assert!(store.update_bot(&demoted).unwrap());
    assert_eq!(store.find_lead_bot(&team.id).unwrap(), None);
}

#[test]
fn update_bot_sets_and_clears_its_model_and_effort() {
    let store = store();
    let team = team_row("alpha");
    store.insert_team(&team).unwrap();
    let mut bot = bot_row(&team.id, "mara", false);
    store.insert_bot(&bot).unwrap();

    bot.model = Some("opus".to_owned());
    bot.effort = Some("high".to_owned());
    assert!(store.update_bot(&bot).unwrap());
    assert_eq!(store.get_bot(&bot.id).unwrap(), Some(bot.clone()));

    bot.model = None;
    bot.effort = None;
    assert!(store.update_bot(&bot).unwrap());
    let found = store.get_bot(&bot.id).unwrap().unwrap();
    assert_eq!((found.model, found.effort), (None, None));
}

#[test]
fn deleting_a_bot_removes_only_that_bot() {
    let store = store();
    let team = team_row("alpha");
    store.insert_team(&team).unwrap();
    let mara = bot_row(&team.id, "mara", false);
    let bruno = bot_row(&team.id, "bruno", false);
    store.insert_bot(&mara).unwrap();
    store.insert_bot(&bruno).unwrap();

    assert!(store.delete_bot(&mara.id).unwrap());
    assert!(!store.delete_bot(&mara.id).unwrap());
    assert_eq!(store.get_bot(&mara.id).unwrap(), None);
    assert_eq!(store.get_bot(&bruno.id).unwrap(), Some(bruno));
}

#[test]
fn deleting_a_team_removes_its_bots_and_keeps_other_teams() {
    let store = store();
    let team = team_row("alpha");
    let other = team_row("beta");
    store.insert_team(&team).unwrap();
    store.insert_team(&other).unwrap();
    let mara = bot_row(&team.id, "mara", true);
    store.insert_bot(&mara).unwrap();
    let bruno = bot_row(&other.id, "bruno", false);
    store.insert_bot(&bruno).unwrap();

    assert!(store.delete_team(&team.id).unwrap());
    assert!(!store.delete_team(&team.id).unwrap());
    assert_eq!(store.get_team(&team.id).unwrap(), None);
    assert_eq!(store.get_bot(&mara.id).unwrap(), None);
    assert_eq!(store.get_bot(&bruno.id).unwrap(), Some(bruno));
    assert_eq!(store.get_team(&other.id).unwrap(), Some(other));
}

#[test]
fn a_teams_folder_workspace_is_recorded_once_set() {
    let store = store();
    let team = team_row("alpha");
    store.insert_team(&team).unwrap();
    assert_eq!(store.team_workspace(&team.id).unwrap(), None);
    assert!(store.set_team_workspace(&team.id, "workspace-1").unwrap());
    assert_eq!(
        store.team_workspace(&team.id).unwrap().as_deref(),
        Some("workspace-1")
    );
    assert_eq!(store.team_workspace("missing").unwrap(), None);
    assert!(!store.set_team_workspace("missing", "workspace-2").unwrap());
}
