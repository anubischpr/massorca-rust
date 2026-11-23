//! Team memory: the lines a team and its bots keep, their newest-first order, the 40-per-scope start read, scopes kept
//! apart, and their removal with the bot or the team.

use massorca_core_store::{BotRow, Store, TeamMemoryRow, TeamRow, new_id};

fn store() -> Store {
    Store::open_in_memory().expect("in-memory store opens")
}

fn team(store: &Store, name: &str) -> TeamRow {
    let row = TeamRow {
        id: new_id(),
        name: name.to_owned(),
        purpose: "Ships the parser".to_owned(),
        character: "triangle".to_owned(),
        access: "full".to_owned(),
        workspaces: false,
        created_at: 1,
    };
    store.insert_team(&row).unwrap();
    row
}

fn bot(store: &Store, team_id: &str, name: &str) -> BotRow {
    let row = BotRow {
        id: new_id(),
        team_id: team_id.to_owned(),
        name: name.to_owned(),
        job: "Writes the parser".to_owned(),
        kind: "claude".to_owned(),
        model: None,
        effort: None,
        lead: false,
        created_at: 1,
    };
    store.insert_bot(&row).unwrap();
    row
}

/// A person's line in the team scope or one bot's scope, created at `created_at`.
fn line(team_id: &str, bot_id: Option<&str>, text: &str, created_at: i64) -> TeamMemoryRow {
    TeamMemoryRow {
        id: new_id(),
        team_id: team_id.to_owned(),
        bot_id: bot_id.map(str::to_owned),
        text: text.to_owned(),
        source: "person".to_owned(),
        task_id: None,
        created_at,
    }
}

fn texts(rows: Vec<TeamMemoryRow>) -> Vec<String> {
    rows.into_iter().map(|row| row.text).collect()
}

#[test]
fn a_line_round_trips_through_each_read_and_is_removed_once() {
    let store = store();
    let alpha = team(&store, "alpha");
    let mara = bot(&store, &alpha.id, "mara");
    let mut row = line(&alpha.id, Some(&mara.id), "Use the 2024 parser.", 10);
    row.source = "task".to_owned();
    row.task_id = Some("task-1".to_owned());
    store.insert_team_memory(&row).unwrap();

    assert_eq!(store.get_team_memory(&row.id).unwrap(), Some(row.clone()));
    assert_eq!(
        store.list_team_memory(&alpha.id, None, None).unwrap(),
        vec![row.clone()]
    );
    assert_eq!(
        store
            .list_team_memory(&alpha.id, Some(&mara.id), None)
            .unwrap(),
        vec![row.clone()]
    );
    assert!(store.delete_team_memory(&row.id).unwrap());
    assert!(!store.delete_team_memory(&row.id).unwrap());
    assert_eq!(store.get_team_memory(&row.id).unwrap(), None);
}

#[test]
fn lines_list_newest_first_and_a_limit_keeps_the_newest() {
    let store = store();
    let alpha = team(&store, "alpha");
    for (text, at) in [("first", 1), ("third", 3), ("second", 2)] {
        store
            .insert_team_memory(&line(&alpha.id, None, text, at))
            .unwrap();
    }

    assert_eq!(
        texts(store.list_team_memory(&alpha.id, None, None).unwrap()),
        ["third", "second", "first"]
    );
    assert_eq!(
        texts(store.list_team_memory(&alpha.id, None, Some(2)).unwrap()),
        ["third", "second"]
    );
}

#[test]
fn a_task_start_reads_the_40_newest_lines_of_each_scope() {
    let store = store();
    let alpha = team(&store, "alpha");
    let mara = bot(&store, &alpha.id, "mara");
    let bruno = bot(&store, &alpha.id, "bruno");
    for at in 1..=45 {
        store
            .insert_team_memory(&line(&alpha.id, None, &format!("team {at}"), at))
            .unwrap();
        store
            .insert_team_memory(&line(&alpha.id, Some(&mara.id), &format!("mara {at}"), at))
            .unwrap();
    }
    store
        .insert_team_memory(&line(&alpha.id, Some(&bruno.id), "bruno 1", 1))
        .unwrap();

    let (team_lines, bot_lines) = store.memory_for_start(&alpha.id, &mara.id).unwrap();
    let team_texts = texts(team_lines);
    assert_eq!(team_texts.len(), 40);
    assert_eq!(team_texts.first().map(String::as_str), Some("team 45"));
    assert_eq!(team_texts.last().map(String::as_str), Some("team 6"));
    let bot_texts = texts(bot_lines);
    assert_eq!(bot_texts.len(), 40);
    assert_eq!(bot_texts.first().map(String::as_str), Some("mara 45"));
    assert_eq!(bot_texts.last().map(String::as_str), Some("mara 6"));
}

#[test]
fn the_team_scope_and_each_bot_scope_are_kept_apart() {
    let store = store();
    let alpha = team(&store, "alpha");
    let mara = bot(&store, &alpha.id, "mara");
    let bruno = bot(&store, &alpha.id, "bruno");
    store
        .insert_team_memory(&line(&alpha.id, None, "team rule", 1))
        .unwrap();
    store
        .insert_team_memory(&line(&alpha.id, Some(&mara.id), "mara note", 2))
        .unwrap();
    store
        .insert_team_memory(&line(&alpha.id, Some(&bruno.id), "bruno note", 3))
        .unwrap();

    let (team_lines, bot_lines) = store.memory_for_start(&alpha.id, &mara.id).unwrap();
    assert_eq!(texts(team_lines), ["team rule"]);
    assert_eq!(texts(bot_lines), ["mara note"]);
    assert_eq!(
        texts(
            store
                .list_team_memory(&alpha.id, Some(&bruno.id), None)
                .unwrap()
        ),
        ["bruno note"]
    );
    assert_eq!(
        texts(store.list_team_memory(&alpha.id, None, None).unwrap()),
        ["bruno note", "mara note", "team rule"]
    );
}

#[test]
fn a_bots_lines_leave_with_the_bot_and_a_teams_lines_leave_with_the_team() {
    let store = store();
    let alpha = team(&store, "alpha");
    let mara = bot(&store, &alpha.id, "mara");
    store
        .insert_team_memory(&line(&alpha.id, None, "team rule", 1))
        .unwrap();
    store
        .insert_team_memory(&line(&alpha.id, Some(&mara.id), "mara note", 2))
        .unwrap();

    assert!(store.delete_bot(&mara.id).unwrap());
    assert_eq!(
        texts(store.list_team_memory(&alpha.id, None, None).unwrap()),
        ["team rule"]
    );

    let beta = team(&store, "beta");
    let bruno = bot(&store, &beta.id, "bruno");
    store
        .insert_team_memory(&line(&beta.id, Some(&bruno.id), "bruno note", 3))
        .unwrap();
    assert!(store.delete_team(&beta.id).unwrap());
    assert!(
        store
            .list_team_memory(&beta.id, None, None)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        texts(store.list_team_memory(&alpha.id, None, None).unwrap()),
        ["team rule"]
    );
}
