//! Wire shapes of the team methods: the team, bot and project documents, the defaults of the params, and the Board.

use massorca_protocol::methods::tasks::{CardState, Column};
use massorca_protocol::methods::teams::{
    Board, BoardCard, BoardColumn, BoardParams, Bot, Team, TeamAccess, TeamCharacter,
    TeamCreateParams,
};
use serde_json::{Value, json};

#[test]
fn create_params_take_only_the_name_and_leave_the_rest_unset() {
    let params: TeamCreateParams = serde_json::from_value(json!({ "name": "alpha" })).unwrap();

    assert_eq!(params.name, "alpha");
    assert_eq!(
        (
            params.purpose,
            params.character,
            params.access,
            params.workspaces
        ),
        (None, None, None, None)
    );
}

#[test]
fn a_team_names_its_character_and_access_in_snake_case() {
    let team = Team {
        id: "t1".to_owned(),
        name: "alpha".to_owned(),
        purpose: String::new(),
        character: TeamCharacter::Triangle,
        access: TeamAccess::Full,
        workspaces: false,
        created_at_ms: 1,
    };

    assert_eq!(
        serde_json::to_value(&team).unwrap(),
        json!({
            "id": "t1",
            "name": "alpha",
            "purpose": "",
            "character": "triangle",
            "access": "full",
            "workspaces": false,
            "created_at_ms": 1
        })
    );
}

#[test]
fn an_unset_bot_model_is_left_out_of_the_document() {
    let bot = Bot {
        id: "b1".to_owned(),
        team_id: "t1".to_owned(),
        name: "mara".to_owned(),
        job: "Writes the parser".to_owned(),
        kind: "claude".to_owned(),
        model: None,
        effort: Some("high".to_owned()),
        lead: true,
        created_at_ms: 2,
    };

    assert_eq!(
        serde_json::to_value(&bot).unwrap(),
        json!({
            "id": "b1",
            "team_id": "t1",
            "name": "mara",
            "job": "Writes the parser",
            "kind": "claude",
            "effort": "high",
            "lead": true,
            "created_at_ms": 2
        })
    );
}

#[test]
fn a_board_lists_four_columns_in_order_and_each_card_names_its_bot_and_time() {
    let board = Board {
        team_id: "t1".to_owned(),
        project_id: None,
        columns: Column::ALL
            .iter()
            .map(|&column| BoardColumn {
                column,
                cards: Vec::new(),
            })
            .collect(),
    };
    let value = serde_json::to_value(&board).unwrap();
    let columns: Vec<&Value> = value["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|column| &column["column"])
        .collect();
    assert_eq!(
        columns,
        [
            &json!("open"),
            &json!("running"),
            &json!("waiting"),
            &json!("done")
        ]
    );

    let card = BoardCard {
        id: "k1".to_owned(),
        title: "parse".to_owned(),
        bot_id: Some("b1".to_owned()),
        bot: Some("mara".to_owned()),
        state: CardState::Working,
        last_known: false,
        error: None,
        error_code: None,
        at_ms: 9,
    };
    assert_eq!(
        serde_json::to_value(&card).unwrap(),
        json!({
            "id": "k1",
            "title": "parse",
            "bot_id": "b1",
            "bot": "mara",
            "state": "working",
            "at_ms": 9
        })
    );
}

#[test]
fn the_project_of_a_board_is_optional() {
    let board: BoardParams = serde_json::from_value(json!({ "team": "alpha" })).unwrap();

    assert_eq!(board.project, None);
}
