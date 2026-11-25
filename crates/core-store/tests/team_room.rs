//! A team's Conference room: lines oldest first, the newest kept by a limit, unknown teams and bots refused, a line
//! about a task found once, and the room removed with its team while a removed bot's lines stay.

use massorca_core_store::{BotRow, Store, TeamRoomMessageRow, TeamRow, new_id};

fn store() -> Store {
    Store::open_in_memory().expect("in-memory store opens")
}

fn team(store: &Store, name: &str) -> TeamRow {
    let row = TeamRow {
        id: new_id(),
        name: name.to_owned(),
        purpose: "Ships the parser".to_owned(),
        character: "triangle".to_owned(),
        access: "default".to_owned(),
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

fn line(team_id: &str, bot_id: Option<&str>, text: &str, created_at: i64) -> TeamRoomMessageRow {
    TeamRoomMessageRow {
        id: new_id(),
        team_id: team_id.to_owned(),
        author: if bot_id.is_some() { "bot" } else { "person" }.to_owned(),
        bot_id: bot_id.map(str::to_owned),
        text: text.to_owned(),
        task_id: None,
        created_at,
    }
}

fn texts(rows: Vec<TeamRoomMessageRow>) -> Vec<String> {
    rows.into_iter().map(|row| row.text).collect()
}

#[test]
fn a_room_lists_oldest_first_and_a_limit_keeps_the_newest() {
    let store = store();
    let alpha = team(&store, "alpha");
    for (text, at) in [("one", 1), ("two", 2), ("three", 3)] {
        store
            .insert_team_room_message(&line(&alpha.id, None, text, at))
            .unwrap();
    }
    assert_eq!(
        texts(store.list_team_room(&alpha.id, None).unwrap()),
        ["one", "two", "three"]
    );
    assert_eq!(
        texts(store.list_team_room(&alpha.id, Some(2)).unwrap()),
        ["two", "three"]
    );
}

#[test]
fn a_line_of_an_unknown_team_or_bot_is_refused() {
    let store = store();
    let alpha = team(&store, "alpha");
    assert!(
        store
            .insert_team_room_message(&line("no-team", None, "hi", 1))
            .is_err()
    );
    assert!(
        store
            .insert_team_room_message(&line(&alpha.id, Some("no-bot"), "hi", 1))
            .is_err()
    );
}

#[test]
fn a_removed_bot_keeps_its_lines_and_a_removed_team_takes_its_room() {
    let store = store();
    let alpha = team(&store, "alpha");
    let mara = bot(&store, &alpha.id, "Mara");
    store
        .insert_team_room_message(&line(&alpha.id, Some(&mara.id), "On it", 1))
        .unwrap();
    store.delete_bot(&mara.id).unwrap();
    let kept = store.list_team_room(&alpha.id, None).unwrap();
    assert_eq!(texts(kept.clone()), ["On it"]);
    assert_eq!(kept[0].bot_id, None);
    store.delete_team(&alpha.id).unwrap();
    assert!(store.list_team_room(&alpha.id, None).unwrap().is_empty());
}
