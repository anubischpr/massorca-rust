//! Wire shapes of the team methods: the team, bot and project documents, the memory lines, the defaults of the params,
//! and the Board.

use massorca_protocol::methods::projects::ProjectKind;
use massorca_protocol::methods::tasks::{CardState, Column};
use massorca_protocol::methods::teams::{
    Board, BoardCard, BoardColumn, BoardParams, Bot, BotUpdateParams, CodeSource, MemorySource,
    Team, TeamAccess, TeamCharacter, TeamCreateParams, TeamMemoryAddParams, TeamMemoryLine,
    TeamProject, TeamProjectAddParams, TeamProjectShowParams, TeamProjectUpdateParams,
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
        skills: Vec::new(),
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
        workspace_id: None,
        branch: None,
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

#[test]
fn a_memory_line_leaves_out_its_bot_and_task_when_they_are_unset() {
    let line = TeamMemoryLine {
        id: "line-1".to_owned(),
        team_id: "alpha".to_owned(),
        bot_id: None,
        text: "Keep it small.".to_owned(),
        source: MemorySource::Person,
        task_id: None,
        created_at_ms: 1_700_000_000_000,
    };

    let document = serde_json::to_value(&line).unwrap();

    assert_eq!(
        document,
        json!({
            "id": "line-1",
            "team_id": "alpha",
            "text": "Keep it small.",
            "source": "person",
            "created_at_ms": 1_700_000_000_000_i64
        })
    );
}

#[test]
fn a_memory_line_of_a_task_names_its_bot_and_task_and_a_newer_source_stays_unknown() {
    let line: TeamMemoryLine = serde_json::from_value(json!({
        "id": "line-2",
        "team_id": "alpha",
        "bot_id": "bot-1",
        "text": "Use the 2024 parser.",
        "source": "task",
        "task_id": "task-1",
        "created_at_ms": 5
    }))
    .unwrap();
    assert_eq!(line.source, MemorySource::Task);
    assert_eq!(line.bot_id.as_deref(), Some("bot-1"));
    assert_eq!(line.task_id.as_deref(), Some("task-1"));

    let newer: TeamMemoryLine = serde_json::from_value(json!({
        "id": "line-3",
        "team_id": "alpha",
        "text": "Written by a newer build.",
        "source": "webhook",
        "created_at_ms": 6
    }))
    .unwrap();
    assert_eq!(newer.source, MemorySource::Unknown);
}

#[test]
fn the_memory_add_params_take_the_team_and_text_and_leave_the_scope_unset() {
    let params: TeamMemoryAddParams =
        serde_json::from_value(json!({ "team": "alpha", "text": "Keep it small." })).unwrap();

    assert_eq!(params.bot, None);
    assert_eq!(params.task, None);
}

fn project(source: Option<(&str, &str, ProjectKind)>) -> TeamProject {
    let (source_project, source_project_name, source_kind) = match source {
        Some((id, name, kind)) => (Some(id.to_owned()), Some(name.to_owned()), Some(kind)),
        None => (None, None, None),
    };
    TeamProject {
        id: "p1".to_owned(),
        team_id: "t1".to_owned(),
        name: "parser".to_owned(),
        goal: String::new(),
        brief: String::new(),
        created_at_ms: 3,
        source_project,
        source_project_name,
        source_kind,
        code_source: None,
    }
}

#[test]
fn an_unset_source_project_is_left_out_of_a_team_project_document() {
    assert_eq!(
        serde_json::to_value(project(None)).unwrap(),
        json!({
            "id": "p1",
            "team_id": "t1",
            "name": "parser",
            "goal": "",
            "brief": "",
            "created_at_ms": 3
        })
    );
}

#[test]
fn a_linked_team_project_names_its_source_project_and_its_kind() {
    assert_eq!(
        serde_json::to_value(project(Some(("w1", "notes", ProjectKind::Folder)))).unwrap(),
        json!({
            "id": "p1",
            "team_id": "t1",
            "name": "parser",
            "goal": "",
            "brief": "",
            "created_at_ms": 3,
            "source_project": "w1",
            "source_project_name": "notes",
            "source_kind": "folder"
        })
    );
}

#[test]
fn an_older_team_project_document_decodes_without_a_source_project() {
    let older: TeamProject = serde_json::from_value(json!({
        "id": "p1",
        "team_id": "t1",
        "name": "parser",
        "goal": "",
        "brief": "",
        "created_at_ms": 3
    }))
    .unwrap();

    assert_eq!(older, project(None));
}

#[test]
fn unset_source_project_params_are_left_out_and_an_empty_one_is_kept_for_clearing() {
    let add = TeamProjectAddParams {
        team: "alpha".to_owned(),
        name: "parser".to_owned(),
        goal: None,
        brief: None,
        source_project: None,
        code_source: None,
    };
    assert_eq!(
        serde_json::to_value(&add).unwrap(),
        json!({ "team": "alpha", "name": "parser" })
    );

    let untouched: TeamProjectUpdateParams =
        serde_json::from_value(json!({ "team": "alpha", "project": "parser" })).unwrap();
    assert_eq!(untouched.source_project, None);

    let cleared: TeamProjectUpdateParams = serde_json::from_value(
        json!({ "team": "alpha", "project": "parser", "source_project": "" }),
    )
    .unwrap();
    assert_eq!(cleared.source_project, Some(String::new()));
}

#[test]
fn a_cleared_code_source_is_sent_as_none_and_read_back() {
    let clear = TeamProjectUpdateParams {
        team: "alpha".to_owned(),
        project: "parser".to_owned(),
        name: None,
        goal: None,
        brief: None,
        source_project: None,
        code_source: Some(CodeSource::None),
    };
    assert_eq!(
        serde_json::to_value(&clear).unwrap(),
        json!({ "team": "alpha", "project": "parser", "code_source": "none" })
    );

    let read: TeamProjectUpdateParams = serde_json::from_value(
        json!({ "team": "alpha", "project": "parser", "code_source": "none" }),
    )
    .unwrap();
    assert_eq!(read.code_source, Some(CodeSource::None));
    assert_eq!(CodeSource::None.label(), "None");
}

#[test]
fn project_show_names_its_team_and_project_under_team_project_show() {
    assert_eq!(
        massorca_protocol::methods::teams::PROJECT_SHOW,
        "team.project_show"
    );
    let params = TeamProjectShowParams {
        team: "alpha".to_owned(),
        project: "parser".to_owned(),
    };
    assert_eq!(
        serde_json::to_value(&params).unwrap(),
        json!({ "team": "alpha", "project": "parser" })
    );
}

#[test]
fn a_bots_skills_are_sent_in_order_and_an_older_document_without_them_reads_as_none() {
    let old: Bot = serde_json::from_value(json!({
        "id": "b1",
        "team_id": "t1",
        "name": "mara",
        "job": "Writes the parser",
        "kind": "claude",
        "lead": false,
        "created_at_ms": 2
    }))
    .unwrap();
    assert!(old.skills.is_empty());

    let bot = Bot {
        skills: vec!["review".to_owned(), "release-notes".to_owned()],
        ..old
    };
    assert_eq!(
        serde_json::to_value(&bot).unwrap()["skills"],
        json!(["review", "release-notes"])
    );
}

#[test]
fn a_bot_update_sends_skills_only_when_given_and_an_empty_list_to_clear_them() {
    let update = |skills: Option<Vec<String>>| BotUpdateParams {
        team: "alpha".to_owned(),
        bot: "mara".to_owned(),
        name: None,
        job: None,
        kind: None,
        model: None,
        effort: None,
        lead: None,
        skills,
    };
    assert!(
        serde_json::to_value(update(None))
            .unwrap()
            .get("skills")
            .is_none()
    );
    assert_eq!(
        serde_json::to_value(update(Some(Vec::new()))).unwrap()["skills"],
        json!([])
    );
}
