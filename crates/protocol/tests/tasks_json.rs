//! Wire shapes of the task methods: the state and column names, the origin, the task document and the list filters.

use massorca_protocol::methods::agents::Outcome;
use massorca_protocol::methods::tasks::{CardState, Column, Task, TaskListParams, TaskOrigin};
use serde_json::json;

#[test]
fn card_states_and_columns_use_wire_names_and_the_words_of_the_board() {
    assert_eq!(
        serde_json::to_value(CardState::NotStarted).unwrap(),
        json!("not_started")
    );
    assert_eq!(
        serde_json::to_value(CardState::Stuck).unwrap(),
        json!("stuck")
    );
    assert_eq!(
        serde_json::to_value(Column::Waiting).unwrap(),
        json!("waiting")
    );
    assert_eq!(CardState::NotStarted.label(), "Not started");
    assert_eq!(CardState::Waiting.label(), "Waiting for you");
    assert_eq!(Column::Waiting.label(), "Waiting for you");
    assert_eq!(
        Column::ALL,
        [Column::Open, Column::Running, Column::Waiting, Column::Done]
    );
}

#[test]
fn values_from_a_newer_peer_decode_to_unknown() {
    let state: CardState = serde_json::from_value(json!("archived")).unwrap();
    let column: Column = serde_json::from_value(json!("archived")).unwrap();
    let origin: TaskOrigin = serde_json::from_value(json!("email")).unwrap();

    assert_eq!(
        (state, column, origin),
        (CardState::Unknown, Column::Unknown, TaskOrigin::Unknown)
    );
}

#[test]
fn a_task_names_its_origin_outcome_and_column_and_leaves_out_unset_fields() {
    let task = Task {
        id: "k1".to_owned(),
        team_id: "t1".to_owned(),
        project_id: None,
        bot_id: None,
        title: "parse".to_owned(),
        body: String::new(),
        origin: TaskOrigin::Cli,
        session_id: None,
        outcome: Some(Outcome::Failure),
        error: None,
        column: Column::Waiting,
        state: CardState::Stuck,
        last_known: false,
        created_at_ms: 1,
        updated_at_ms: 2,
    };

    let value = serde_json::to_value(&task).unwrap();

    assert_eq!(value["origin"], json!("cli"));
    assert_eq!(value["outcome"], json!("failure"));
    assert_eq!(value["column"], json!("waiting"));
    assert_eq!(value["state"], json!("stuck"));
    assert!(value.get("last_known").is_none());
    assert!(value.get("project_id").is_none());
}

#[test]
fn the_filters_of_task_list_are_all_optional() {
    let filters: TaskListParams = serde_json::from_value(json!({})).unwrap();

    assert_eq!(filters, TaskListParams::default());
}
