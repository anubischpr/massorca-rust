//! The `team.routine_*` wire shapes (decision 0072 amendment): documented field names, unset options left out, and a
//! cadence or weekday this build does not know decoding as `Unknown` rather than failing the whole reply.

use massorca_protocol::methods::teams::{
    self, Routine, RoutineAddParams, RoutineCadence, RoutineRun, RoutineRunsParams,
    RoutineUpdateParams, RoutineWeekday,
};
use serde_json::{Value, json};

#[test]
fn routine_method_names_follow_the_contract() {
    assert_eq!(teams::ROUTINE_LIST, "team.routine_list");
    assert_eq!(teams::ROUTINE_ADD, "team.routine_add");
    assert_eq!(teams::ROUTINE_UPDATE, "team.routine_update");
    assert_eq!(teams::ROUTINE_REMOVE, "team.routine_rm");
    assert_eq!(teams::ROUTINE_PAUSE, "team.routine_pause");
    assert_eq!(teams::ROUTINE_RESUME, "team.routine_resume");
    assert_eq!(teams::ROUTINE_RUNS, "team.routine_runs");
    assert_eq!(teams::ROUTINE_RUN, "team.routine_run");
}

#[test]
fn add_params_use_the_documented_names_and_omit_unset_options() {
    let params = RoutineAddParams {
        team: "desarrollo".to_owned(),
        bot: "mara".to_owned(),
        name: "Morning check".to_owned(),
        prompt: "Read the open issues.".to_owned(),
        cadence: RoutineCadence::Weekly,
        time: "09:00".to_owned(),
        weekday: Some(RoutineWeekday::Monday),
        month_day: None,
        run_date: None,
        zone: None,
    };
    assert_eq!(
        serde_json::to_value(&params).unwrap(),
        json!({
            "team": "desarrollo",
            "bot": "mara",
            "name": "Morning check",
            "prompt": "Read the open issues.",
            "cadence": "weekly",
            "time": "09:00",
            "weekday": "monday"
        })
    );
}

#[test]
fn update_params_send_only_the_fields_given_and_an_empty_zone_clears_it() {
    let params = RoutineUpdateParams {
        team: "desarrollo".to_owned(),
        routine: "morning".to_owned(),
        bot: None,
        name: Some("Standup".to_owned()),
        prompt: None,
        cadence: None,
        time: None,
        weekday: None,
        month_day: None,
        run_date: None,
        zone: Some(String::new()),
    };
    assert_eq!(
        serde_json::to_value(&params).unwrap(),
        json!({"team": "desarrollo", "routine": "morning", "name": "Standup", "zone": ""})
    );
}

#[test]
fn runs_params_take_an_optional_limit() {
    let params = RoutineRunsParams {
        team: "desarrollo".to_owned(),
        routine: "morning".to_owned(),
        limit: None,
    };
    assert_eq!(
        serde_json::to_value(&params).unwrap(),
        json!({"team": "desarrollo", "routine": "morning"})
    );
}

#[test]
fn routine_has_the_documented_shape_and_omits_an_absent_bot_and_next_run() {
    let routine = Routine {
        id: "r1".to_owned(),
        team_id: "t1".to_owned(),
        bot_id: None,
        name: "Morning check".to_owned(),
        prompt: "Read the open issues.".to_owned(),
        cadence: RoutineCadence::Daily,
        time: "09:00".to_owned(),
        weekday: None,
        month_day: None,
        run_date: None,
        zone: Some("Europe/Madrid".to_owned()),
        paused: false,
        starts_as_admin: true,
        next_run_at_ms: None,
        created_at_ms: 1,
    };
    assert_eq!(
        serde_json::to_value(&routine).unwrap(),
        json!({
            "id": "r1",
            "team_id": "t1",
            "name": "Morning check",
            "prompt": "Read the open issues.",
            "cadence": "daily",
            "time": "09:00",
            "zone": "Europe/Madrid",
            "paused": false,
            "starts_as_admin": true,
            "created_at_ms": 1
        })
    );
}

#[test]
fn a_routine_from_an_older_peer_without_the_new_fields_still_decodes() {
    let routine: Routine = serde_json::from_value(json!({
        "id": "r1",
        "team_id": "t1",
        "name": "Morning check",
        "prompt": "Read the open issues.",
        "cadence": "once",
        "time": "09:00",
        "run_date": "2026-11-24",
        "paused": true,
        "starts_as_admin": false,
        "created_at_ms": 1
    }))
    .unwrap();
    assert_eq!(routine.cadence, RoutineCadence::Once);
    assert_eq!(routine.run_date.as_deref(), Some("2026-11-24"));
    assert_eq!(routine.bot_id, None);
    assert_eq!(routine.next_run_at_ms, None);
}

#[test]
fn an_unknown_cadence_or_weekday_decodes_as_unknown() {
    let cadence: RoutineCadence = serde_json::from_value(json!("hourly")).unwrap();
    assert_eq!(cadence, RoutineCadence::Unknown);
    let weekday: RoutineWeekday = serde_json::from_value(json!("someday")).unwrap();
    assert_eq!(weekday, RoutineWeekday::Unknown);
}

#[test]
fn a_run_has_the_documented_shape() {
    let run = RoutineRun {
        id: "run1".to_owned(),
        routine_id: "r1".to_owned(),
        scheduled_for_ms: 100,
        task_id: Some("task1".to_owned()),
        error: None,
        created_at_ms: 100,
    };
    let value: Value = serde_json::to_value(&run).unwrap();
    assert_eq!(
        value,
        json!({
            "id": "run1",
            "routine_id": "r1",
            "scheduled_for_ms": 100,
            "task_id": "task1",
            "created_at_ms": 100
        })
    );
}
