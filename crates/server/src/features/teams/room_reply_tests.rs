//! Unit tests of `room_reply`: what makes a conversation, who answers, and how the answer is posted once.

use massorca_core_store::new_id;
use massorca_protocol::codes;

use super::super::room_quote::context;
use super::super::room_reply_end::{NO_ANSWER, ReplyEnd, bot_answer};
use super::*;

fn bot(team: &TeamRow, name: &str, lead: bool) -> BotRow {
    BotRow {
        id: name.to_lowercase(),
        team_id: team.id.clone(),
        name: name.to_owned(),
        job: "Writes the parser".to_owned(),
        kind: "claude".to_owned(),
        model: None,
        effort: None,
        lead,
        created_at: 1,
    }
}

/// Team alpha with Mara and Bruno, a line of Mara's, and a person's question to Mara answered by run `session-1`.
fn room() -> (Store, TeamRow, Vec<BotRow>, TeamRoomMessageRow) {
    let store = Store::open_in_memory().expect("in-memory store opens");
    let team = TeamRow {
        id: new_id(),
        name: "alpha".to_owned(),
        purpose: String::new(),
        character: "triangle".to_owned(),
        access: "default".to_owned(),
        workspaces: false,
        created_at: 1,
    };
    store.insert_team(&team).expect("team");
    let bots = vec![bot(&team, "Mara", false), bot(&team, "Bruno", false)];
    for row in &bots {
        store.insert_bot(row).expect("bot");
    }
    let said = new_line(&team, Some(&bots[0]), "Done: parse".to_owned(), None);
    insert_row(&store, &said).expect("line");
    let mut question = new_line(&team, None, "Why was it slow?".to_owned(), None);
    question.reply_to = Some(said.id.clone());
    question.reply_bot_id = Some("mara".to_owned());
    question.reply_state = Some(OPEN.to_owned());
    question.reply_session = Some("session-1".to_owned());
    insert_row(&store, &question).expect("question");
    (store, team, bots, question)
}

fn texts(store: &Store, team: &TeamRow) -> Vec<String> {
    store
        .list_team_room(&team.id, None)
        .unwrap()
        .into_iter()
        .map(|line| line.text)
        .collect()
}

#[test]
fn a_line_is_a_conversation_when_it_asks_or_answers_a_bot_line_and_does_not_ask_for_a_task() {
    let (_, team, bots, question) = room();
    let said = new_line(&team, Some(&bots[0]), "Done".to_owned(), None);
    let mut card = new_line(&team, None, "Who should take this?".to_owned(), None);
    card.author = "bot".to_owned();
    card.card = Some("{}".to_owned());
    assert!(!is_conversation(None, None), "a plain line is a task");
    assert!(is_conversation(Some(true), None), "asked");
    assert!(is_conversation(None, Some(&said)), "a reply to a bot");
    assert!(
        !is_conversation(Some(false), Some(&said)),
        "asked for a task"
    );
    assert!(
        !is_conversation(None, Some(&question)),
        "a reply to a person"
    );
    assert!(!is_conversation(None, Some(&card)), "a reply to a card");
}

#[test]
fn the_answering_bot_is_the_named_one_then_the_replied_one_then_to_then_the_lead_then_the_first() {
    let (_, team, mut bots, _) = room();
    let said = new_line(&team, Some(&bots[1]), "Done".to_owned(), None);
    let name = |bot: &BotRow| bot.name.clone();
    assert_eq!(
        name(answering_bot(&bots, Some(&bots[0]), Some(&said), None)),
        "Mara"
    );
    assert_eq!(
        name(answering_bot(&bots, None, Some(&said), Some(&bots[0]))),
        "Bruno"
    );
    assert_eq!(
        name(answering_bot(&bots, None, None, Some(&bots[1]))),
        "Bruno"
    );
    assert_eq!(
        name(answering_bot(&bots, None, None, None)),
        "Mara",
        "the first"
    );
    bots[1].lead = true;
    assert_eq!(
        name(answering_bot(&bots, None, None, None)),
        "Bruno",
        "the lead"
    );
}

#[test]
fn a_turns_end_posts_its_result_once_as_the_bot_cut_to_1200_characters() {
    let (store, team, _, question) = room();
    let long = "x".repeat(MAX_REPLY_CHARS + 50);
    let settled = settle_line(&store, &question, &ReplyEnd::Finished(Some(long))).unwrap();
    let posted = settled.posted.expect("the answer is posted");
    assert_eq!(posted.bot_id.as_deref(), Some("mara"));
    assert_eq!(posted.reply_to.as_deref(), Some(question.id.as_str()));
    assert_eq!(posted.text.chars().count(), MAX_REPLY_CHARS);
    assert!(posted.text.ends_with('…'));
    assert_eq!(
        settled.stop.as_deref(),
        Some("session-1"),
        "the run is stopped"
    );

    let more = ReplyEnd::Finished(Some("more".to_owned()));
    let again = settle_line(&store, &question, &more).unwrap();
    assert!(again.posted.is_none() && !again.changed && again.stop.is_none());
    assert_eq!(texts(&store, &team).len(), 3, "one answer");
}

#[test]
fn a_bot_answers_once_as_itself_and_its_turns_end_then_only_closes_the_reply() {
    let (store, team, _, question) = room();
    let other = bot_answer(&store, &team, "session-1", Some("Bruno"), "I am Bruno")
        .expect_err("another bot is refused");
    assert_eq!(other.code, codes::PERMISSION_DENIED);
    let posted = bot_answer(&store, &team, "session-1", None, "Network was slow")
        .unwrap()
        .expect("the run answers its conversation");
    assert_eq!(posted.bot_id.as_deref(), Some("mara"));
    let second =
        bot_answer(&store, &team, "session-1", Some("Mara"), "And also").expect_err("one answer");
    assert_eq!(second.code, codes::PERMISSION_DENIED);
    assert!(
        bot_answer(&store, &team, "session-9", None, "hi")
            .unwrap()
            .is_none(),
        "a run with no conversation"
    );

    let end = ReplyEnd::Finished(Some("Network was slow".to_owned()));
    let settled = settle_line(&store, &question, &end).unwrap();
    assert!(settled.posted.is_none());
    assert!(settled.changed);
    assert_eq!(settled.stop.as_deref(), Some("session-1"));
    assert_eq!(
        texts(&store, &team),
        ["Done: parse", "Why was it slow?", "Network was slow"]
    );
}

#[test]
fn a_turn_without_a_result_or_a_forgotten_run_says_it_could_not_answer() {
    let (store, _, _, question) = room();
    let settled = settle_line(&store, &question, &ReplyEnd::Forgotten).unwrap();
    assert_eq!(
        settled.posted.map(|line| line.text).as_deref(),
        Some(NO_ANSWER)
    );
    assert_eq!(settled.stop, None, "a forgotten run is not stopped");

    let (store, _, _, question) = room();
    let blank = ReplyEnd::Finished(Some("  ".to_owned()));
    let settled = settle_line(&store, &question, &blank).unwrap();
    assert_eq!(
        settled.posted.map(|line| line.text).as_deref(),
        Some(NO_ANSWER)
    );
}

#[test]
fn the_prompt_names_the_bot_the_room_the_quoted_line_and_the_cap() {
    let (store, team, bots, question) = room();
    let history = store.list_team_room(&team.id, None).unwrap();
    let said = store
        .get_team_room_message(question.reply_to.as_deref().unwrap())
        .unwrap()
        .unwrap();
    let text = prompt(&team, &bots[0], &bots, &history[..1], Some(&said), "Why?");
    assert!(text.starts_with("You are Mara, a bot in the team alpha."));
    assert!(text.contains("in at most 1200 characters"));
    assert!(text.contains("Recent room lines, oldest first:\nMara: Done: parse\n"));
    assert!(text.contains("They reply to this line:\nMara: Done: parse\n"));
    assert!(text.ends_with("Their message:\nWhy?\n"));
    assert_eq!(context(&said, &bots), "In reply to Mara: Done: parse");
}
