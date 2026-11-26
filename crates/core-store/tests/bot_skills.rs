//! A bot's skills: kept in order, replaced as a whole, and removed with the bot.

use massorca_core_store::{BotRow, Store, TeamRow, new_id, now_ms};

fn store_with_bot() -> (Store, BotRow) {
    let store = Store::open_in_memory().expect("in-memory store opens");
    let team = TeamRow {
        id: new_id(),
        name: "alpha".to_owned(),
        purpose: String::new(),
        character: "triangle".to_owned(),
        access: "full".to_owned(),
        workspaces: false,
        created_at: now_ms(),
    };
    store.insert_team(&team).unwrap();
    let bot = BotRow {
        id: new_id(),
        team_id: team.id,
        name: "mara".to_owned(),
        job: "Writes the parser".to_owned(),
        kind: "claude".to_owned(),
        model: None,
        effort: None,
        lead: false,
        created_at: now_ms(),
    };
    store.insert_bot(&bot).unwrap();
    (store, bot)
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|name| (*name).to_owned()).collect()
}

#[test]
fn a_bot_has_no_skills_until_they_are_set_and_keeps_them_in_order() {
    let (store, bot) = store_with_bot();
    assert!(store.bot_skills(&bot.id).unwrap().is_empty());

    store
        .set_bot_skills(&bot.id, &names(&["review", "deploy"]))
        .unwrap();

    assert_eq!(
        store.bot_skills(&bot.id).unwrap(),
        names(&["review", "deploy"])
    );
    let of_team = store.team_bot_skills(&bot.team_id).unwrap();
    assert_eq!(of_team.get(&bot.id), Some(&names(&["review", "deploy"])));
}

#[test]
fn setting_skills_replaces_them_and_an_empty_list_clears_them() {
    let (store, bot) = store_with_bot();
    store
        .set_bot_skills(&bot.id, &names(&["review", "deploy"]))
        .unwrap();

    store.set_bot_skills(&bot.id, &names(&["notes"])).unwrap();
    assert_eq!(store.bot_skills(&bot.id).unwrap(), names(&["notes"]));

    store.set_bot_skills(&bot.id, &[]).unwrap();
    assert!(store.bot_skills(&bot.id).unwrap().is_empty());
    assert!(store.team_bot_skills(&bot.team_id).unwrap().is_empty());
}

#[test]
fn removing_a_bot_removes_its_skills() {
    let (store, bot) = store_with_bot();
    store.set_bot_skills(&bot.id, &names(&["review"])).unwrap();

    assert!(store.delete_bot(&bot.id).unwrap());

    assert!(store.bot_skills(&bot.id).unwrap().is_empty());
}
