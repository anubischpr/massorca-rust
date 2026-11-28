//! A person's room line waiting for its routing query (AUTO-13): found by its run's session, settled once, kept with
//! whether its task starts at once and whether its author held host-admin scope, removed with its line; and a line made
//! a conversation after it was posted, opened once.

use massorca_core_store::{Store, TeamRoomMessageRow, TeamRoomRouteRow, TeamRow, new_id};

fn store() -> Store {
    Store::open_in_memory().expect("in-memory store opens")
}

fn team(store: &Store) -> TeamRow {
    let row = TeamRow {
        id: new_id(),
        name: "alpha".to_owned(),
        purpose: String::new(),
        character: "triangle".to_owned(),
        access: "default".to_owned(),
        workspaces: false,
        created_at: 1,
    };
    store.insert_team(&row).unwrap();
    row
}

fn person_line(store: &Store, team_id: &str) -> TeamRoomMessageRow {
    let row = TeamRoomMessageRow {
        id: new_id(),
        team_id: team_id.to_owned(),
        author: "person".to_owned(),
        bot_id: None,
        text: "Fix the parser".to_owned(),
        task_id: None,
        routine_id: None,
        card: None,
        card_answer: None,
        reply_to: None,
        reply_bot_id: None,
        reply_session: None,
        reply_state: None,
        created_at: 1,
    };
    store.insert_team_room_message(&row).unwrap();
    row
}

#[test]
fn a_route_is_found_by_its_session_and_settles_once() {
    let store = store();
    let alpha = team(&store);
    let line = person_line(&store, &alpha.id);
    let route = TeamRoomRouteRow::open(&line.id, &alpha.id, false, true, 5);
    store.insert_team_room_route(&route).unwrap();
    assert_eq!(store.find_team_room_route("s-1").unwrap(), None);

    store.set_team_room_route_session(&line.id, "s-1").unwrap();
    let found = store.find_team_room_route("s-1").unwrap().expect("found");
    assert_eq!(found.message_id, line.id);
    assert_eq!(found.session.as_deref(), Some("s-1"));
    assert_eq!((found.start, found.admin), (false, true));
    assert_eq!(found.state, "open");

    assert!(store.settle_team_room_route(&line.id).unwrap());
    assert!(
        !store.settle_team_room_route(&line.id).unwrap(),
        "a route settles once"
    );
    let settled = store.get_team_room_route(&line.id).unwrap().expect("kept");
    assert_eq!(settled.state, "settled");
}

#[test]
fn a_route_goes_with_its_line() {
    let store = store();
    let alpha = team(&store);
    let line = person_line(&store, &alpha.id);
    store
        .insert_team_room_route(&TeamRoomRouteRow::open(&line.id, &alpha.id, true, false, 5))
        .unwrap();
    store.delete_team(&alpha.id).unwrap();
    assert_eq!(store.get_team_room_route(&line.id).unwrap(), None);
}

#[test]
fn a_posted_line_opens_as_a_conversation_once() {
    let store = store();
    let alpha = team(&store);
    let line = person_line(&store, &alpha.id);
    assert!(store.open_team_room_reply(&line.id, None).unwrap());
    assert!(
        !store.open_team_room_reply(&line.id, None).unwrap(),
        "a line already a conversation stays as it is"
    );
    let row = store.get_team_room_message(&line.id).unwrap().unwrap();
    assert_eq!(row.reply_state.as_deref(), Some("open"));
}
