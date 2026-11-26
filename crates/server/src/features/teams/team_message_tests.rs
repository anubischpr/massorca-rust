//! Unit tests of `team_message`: the delegation edges a message may travel, the one settlement of a `worker_done`, the
//! inbox and replies.

use std::cell::RefCell;

use massorca_core_store::{TaskOrigin, TeamRow};

use super::*;

struct Tree {
    store: Store,
    lead: BotRow,
    worker: BotRow,
    parent: TaskRow,
    child: TaskRow,
    sibling: TaskRow,
}

fn bot(team: &TeamRow, name: &str) -> BotRow {
    BotRow {
        id: new_id(),
        team_id: team.id.clone(),
        name: name.to_owned(),
        job: String::new(),
        kind: "claude".to_owned(),
        model: None,
        effort: None,
        lead: false,
        created_at: 1,
    }
}

fn task(team: &TeamRow, bot: &BotRow, title: &str, session: &str) -> TaskRow {
    TaskRow {
        id: new_id(),
        team_id: team.id.clone(),
        project_id: None,
        bot_id: Some(bot.id.clone()),
        title: title.to_owned(),
        body: String::new(),
        origin: TaskOrigin::Cli,
        session_id: Some(session.to_owned()),
        outcome: None,
        error: None,
        error_code: None,
        workspace_id: None,
        branch: None,
        created_at: 1,
        updated_at: 1,
    }
}

/// Lead Mara's task with two delegated children, both Jun's: `child` (run `child-run`) and `sibling`.
fn tree() -> Tree {
    let store = Store::open_in_memory().expect("store");
    let team = TeamRow {
        id: new_id(),
        name: "alpha".to_owned(),
        purpose: String::new(),
        character: "triangle".to_owned(),
        access: "full".to_owned(),
        workspaces: false,
        created_at: 1,
    };
    store.insert_team(&team).expect("team");
    let (lead, worker) = (bot(&team, "Mara"), bot(&team, "Jun"));
    store.insert_bot(&lead).expect("lead");
    store.insert_bot(&worker).expect("worker");
    let parent = task(&team, &lead, "Lead the parser", "lead-run");
    let child = task(&team, &worker, "Write the lexer", "child-run");
    let sibling = task(&team, &worker, "Write the tests", "sibling-run");
    store.insert_task(&parent).expect("parent");
    for linked in [&child, &sibling] {
        store
            .insert_delegated_task(linked, &parent.id, false, None, 1)
            .expect("link");
    }
    Tree {
        store,
        lead,
        worker,
        parent,
        child,
        sibling,
    }
}

fn draft(to: &str, kind: MessageType) -> Draft {
    Draft {
        to: to.to_owned(),
        kind,
        priority: MessagePriority::Normal,
        subject: "Lexer".to_owned(),
        body: "The lexer reads numbers now.".to_owned(),
        outcome: None,
        task: None,
        thread: None,
        reply_to: None,
    }
}

fn done(outcome: WorkerOutcome) -> Draft {
    Draft {
        outcome: Some(outcome),
        ..draft(TO_PARENT, MessageType::WorkerDone)
    }
}

/// Sends `draft` as `task`'s run. Waking comes with the next change; `_woken` and `_wakes` keep the calls stable.
fn send_as(
    tree: &Tree,
    task: &TaskRow,
    draft: Draft,
    _woken: bool,
    _wakes: &RefCell<Vec<String>>,
) -> Result<TeamMessageSent, ErrorBody> {
    let bot = if task.id == tree.parent.id {
        &tree.lead
    } else {
        &tree.worker
    };
    let from = SenderRun {
        task,
        bot,
        session: task.session_id.as_deref().unwrap_or_default(),
    };
    deliver(&tree.store, &from, draft, &mut Touched::default())
}

fn code(result: Result<TeamMessageSent, ErrorBody>) -> String {
    result.expect_err("refused").code.to_owned()
}

#[test]
fn a_child_messages_its_parent_and_a_parent_its_child_as_themselves() {
    let tree = tree();
    let wakes = RefCell::new(Vec::new());
    let up = send_as(
        &tree,
        &tree.child,
        draft(TO_PARENT, MessageType::Status),
        false,
        &wakes,
    )
    .expect("up");
    assert_eq!(up.message.to_task, tree.parent.id);
    assert_eq!(up.message.from_bot.as_deref(), Some("Jun"));
    assert_eq!(up.message.to_bot.as_deref(), Some("Mara"));
    assert_eq!(up.message.thread, up.message.id);
    let down = send_as(
        &tree,
        &tree.parent,
        draft(&tree.child.id, MessageType::Question),
        false,
        &wakes,
    )
    .expect("down");
    assert_eq!(down.message.to_task, tree.child.id);
    assert_eq!(down.message.from_bot.as_deref(), Some("Mara"));
}

#[test]
fn a_message_off_the_delegation_edges_is_refused() {
    let tree = tree();
    let wakes = RefCell::new(Vec::new());
    let sideways = send_as(
        &tree,
        &tree.child,
        draft(&tree.sibling.id, MessageType::Status),
        false,
        &wakes,
    );
    assert_eq!(code(sideways), codes::TEAM_MESSAGE_NO_EDGE);
    let no_parent = send_as(
        &tree,
        &tree.parent,
        draft(TO_PARENT, MessageType::Status),
        false,
        &wakes,
    );
    assert_eq!(code(no_parent), codes::TEAM_MESSAGE_NO_EDGE);
    let unknown = send_as(
        &tree,
        &tree.parent,
        draft("no-such-task", MessageType::Status),
        false,
        &wakes,
    );
    assert_eq!(code(unknown), codes::TEAM_MESSAGE_NO_EDGE);
}

#[test]
fn an_unknown_type_or_a_misplaced_outcome_is_invalid() {
    let tree = tree();
    let wakes = RefCell::new(Vec::new());
    let unknown = send_as(
        &tree,
        &tree.child,
        draft(TO_PARENT, MessageType::Unknown),
        false,
        &wakes,
    );
    assert_eq!(code(unknown), codes::REQUEST_INVALID_PARAMS);
    let bare_done = send_as(
        &tree,
        &tree.child,
        draft(TO_PARENT, MessageType::WorkerDone),
        false,
        &wakes,
    );
    assert_eq!(code(bare_done), codes::REQUEST_INVALID_PARAMS);
    let status_with_outcome = Draft {
        outcome: Some(WorkerOutcome::Failed),
        ..draft(TO_PARENT, MessageType::Status)
    };
    assert_eq!(
        code(send_as(
            &tree,
            &tree.child,
            status_with_outcome,
            false,
            &wakes
        )),
        codes::REQUEST_INVALID_PARAMS
    );
    let long = Draft {
        body: "x".repeat(MAX_MESSAGE_BODY_BYTES + 1),
        ..draft(TO_PARENT, MessageType::Status)
    };
    assert_eq!(
        code(send_as(&tree, &tree.child, long, false, &wakes)),
        codes::REQUEST_INVALID_PARAMS
    );
    let escape = Draft {
        body: "colour \u{1b}[31m".to_owned(),
        ..draft(TO_PARENT, MessageType::Status)
    };
    assert_eq!(
        code(send_as(&tree, &tree.child, escape, false, &wakes)),
        codes::REQUEST_INVALID_PARAMS
    );
}

#[test]
fn a_worker_done_settles_its_task_once_and_a_repeat_is_a_duplicate() {
    let tree = tree();
    let wakes = RefCell::new(Vec::new());
    let first = send_as(
        &tree,
        &tree.child,
        done(WorkerOutcome::Succeeded),
        false,
        &wakes,
    )
    .expect("done");
    assert!(!first.duplicate);
    assert_eq!(
        first.message.settles.as_deref(),
        Some(tree.child.id.as_str())
    );
    let settled = tree
        .store
        .get_task(&tree.child.id)
        .expect("read")
        .expect("task");
    assert_eq!(settled.outcome, Some(TaskOutcome::Success));

    let again = send_as(
        &tree,
        &tree.child,
        done(WorkerOutcome::Succeeded),
        false,
        &wakes,
    )
    .expect("repeat");
    assert!(again.duplicate);
    assert_eq!(again.message.id, first.message.id);
    assert_eq!(
        tree.store
            .team_inbox(&tree.parent.id, false)
            .expect("inbox")
            .len(),
        1
    );
}

#[test]
fn a_worker_done_that_contradicts_an_ended_task_is_refused_with_its_outcome() {
    let tree = tree();
    let wakes = RefCell::new(Vec::new());
    assert!(
        tree.store
            .latch_task_outcome(&tree.child.id, "child-run", TaskOutcome::Failure, 2)
            .expect("latch")
    );
    let refused = send_as(
        &tree,
        &tree.child,
        done(WorkerOutcome::Succeeded),
        false,
        &wakes,
    )
    .expect_err("settled");
    assert_eq!(refused.code, codes::TEAM_MESSAGE_ALREADY_SETTLED);
    assert_eq!(refused.data.expect("data")["outcome"], "failure");
}

#[test]
fn a_worker_done_settles_only_the_senders_own_task_under_its_parent() {
    let tree = tree();
    let wakes = RefCell::new(Vec::new());
    let unknown = Draft {
        task: Some("no-such-task".to_owned()),
        ..done(WorkerOutcome::Succeeded)
    };
    assert_eq!(
        code(send_as(&tree, &tree.child, unknown, false, &wakes)),
        codes::TEAM_MESSAGE_UNKNOWN_TASK
    );
    let siblings = Draft {
        task: Some(tree.sibling.id.clone()),
        ..done(WorkerOutcome::Succeeded)
    };
    assert_eq!(
        code(send_as(&tree, &tree.child, siblings, false, &wakes)),
        codes::TEAM_MESSAGE_NOT_YOUR_CHILD
    );
    let downward = Draft {
        to: tree.child.id.clone(),
        ..done(WorkerOutcome::Succeeded)
    };
    assert_eq!(
        code(send_as(&tree, &tree.parent, downward, false, &wakes)),
        codes::TEAM_MESSAGE_NOT_YOUR_CHILD
    );
    let untouched = tree
        .store
        .get_task(&tree.sibling.id)
        .expect("read")
        .expect("task");
    assert_eq!(untouched.outcome, None);
}

#[test]
fn the_inbox_lists_unread_messages_and_mark_read_empties_it() {
    let tree = tree();
    let wakes = RefCell::new(Vec::new());
    send_as(
        &tree,
        &tree.child,
        draft(TO_PARENT, MessageType::Status),
        false,
        &wakes,
    )
    .expect("first");
    send_as(
        &tree,
        &tree.sibling,
        draft(TO_PARENT, MessageType::Question),
        false,
        &wakes,
    )
    .expect("second");

    let listed = read_inbox(&tree.store, &tree.parent, false).expect("inbox");
    assert_eq!(listed.messages.len(), 2);
    assert_eq!(listed.messages[0].kind, MessageType::Status);
    let marked = read_inbox(&tree.store, &tree.parent, true).expect("inbox");
    assert_eq!(marked.messages.len(), 2);
    assert!(
        read_inbox(&tree.store, &tree.parent, false)
            .expect("inbox")
            .messages
            .is_empty()
    );
    assert!(
        read_inbox(&tree.store, &tree.child, false)
            .expect("inbox")
            .messages
            .is_empty()
    );
}

#[test]
fn a_reply_goes_back_to_the_sender_on_its_thread_and_only_from_its_recipient() {
    let tree = tree();
    let wakes = RefCell::new(Vec::new());
    let asked = send_as(
        &tree,
        &tree.child,
        draft(TO_PARENT, MessageType::Question),
        false,
        &wakes,
    )
    .expect("question");
    let answer = reply_draft(
        &tree.store,
        &tree.parent,
        asked.message.id.clone(),
        "Use the new token table.".to_owned(),
        None,
        MessagePriority::Normal,
    )
    .expect("reply");
    let replied = send_as(&tree, &tree.parent, answer, false, &wakes).expect("sent");
    assert_eq!(replied.message.to_task, tree.child.id);
    assert_eq!(replied.message.thread, asked.message.thread);
    assert_eq!(
        replied.message.reply_to.as_deref(),
        Some(asked.message.id.as_str())
    );
    assert_eq!(replied.message.subject, "Re: Lexer");
    assert_eq!(replied.message.kind, MessageType::Status);

    let not_mine = reply_draft(
        &tree.store,
        &tree.sibling,
        asked.message.id.clone(),
        "Hijack".to_owned(),
        None,
        MessagePriority::Normal,
    )
    .expect_err("not in its inbox");
    assert_eq!(not_mine.code, codes::TEAM_MESSAGE_UNKNOWN);
}
