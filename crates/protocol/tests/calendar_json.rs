//! The `calendar.occurrences` wire shapes (decision 0124): the method and capability names, the params and result
//! fields, unset options left out, `include_done` false unless asked, and a kind this build does not know decoding as
//! `Unknown`.

use massorca_protocol::capabilities;
use massorca_protocol::codes;
use massorca_protocol::methods::calendar::{
    self, CalendarKind, CalendarOccurrence, CalendarOccurrenceList, CalendarOccurrencesParams,
};
use serde_json::{Value, json};

fn window() -> CalendarOccurrencesParams {
    CalendarOccurrencesParams {
        from_ms: 1_795_478_400_000,
        to_ms: 1_796_083_200_000,
        kind: None,
        include_done: false,
    }
}

#[test]
fn the_method_capability_and_code_have_their_documented_names() {
    assert_eq!(calendar::OCCURRENCES, "calendar.occurrences");
    assert_eq!(capabilities::CALENDAR, "calendar/1");
    assert_eq!(codes::CALENDAR_WINDOW_INVALID, "calendar.window_invalid");
}

#[test]
fn unset_kind_and_false_include_done_are_left_out_of_the_params() {
    let wire = serde_json::to_value(window()).expect("params encode");
    assert_eq!(
        wire,
        json!({ "from_ms": 1_795_478_400_000_i64, "to_ms": 1_796_083_200_000_i64 })
    );
}

#[test]
fn a_kind_is_named_in_snake_case_and_a_params_document_without_include_done_decodes() {
    let params: CalendarOccurrencesParams = serde_json::from_value(json!({
        "from_ms": 1, "to_ms": 2, "kind": "automation"
    }))
    .expect("params decode");
    assert_eq!(params.kind, Some(CalendarKind::Automation));
    assert!(!params.include_done);

    let routine: CalendarKind = serde_json::from_value(json!("routine")).expect("kind decodes");
    assert_eq!(routine, CalendarKind::Routine);
}

#[test]
fn a_kind_from_a_newer_peer_decodes_as_unknown() {
    let kind: CalendarKind = serde_json::from_value(json!("meeting")).expect("kind decodes");
    assert_eq!(kind, CalendarKind::Unknown);
}

#[test]
fn an_occurrence_carries_its_instant_and_its_wall_clock_on_its_zone() {
    let occurrence = CalendarOccurrence {
        kind: CalendarKind::Routine,
        source_id: "routine-1".to_owned(),
        name: "Weekly backup".to_owned(),
        team_id: Some("team-1".to_owned()),
        project_id: None,
        at_ms: 1_795_478_400_000,
        local_date: "2026-11-25".to_owned(),
        local_time: "09:00".to_owned(),
        zone: Some("Europe/Madrid".to_owned()),
        done: false,
    };
    let wire: Value = serde_json::to_value(&occurrence).expect("occurrence encodes");
    assert_eq!(
        wire,
        json!({
            "kind": "routine",
            "source_id": "routine-1",
            "name": "Weekly backup",
            "team_id": "team-1",
            "at_ms": 1_795_478_400_000_i64,
            "local_date": "2026-11-25",
            "local_time": "09:00",
            "zone": "Europe/Madrid",
            "done": false
        })
    );
    let back: CalendarOccurrence = serde_json::from_value(wire).expect("occurrence decodes");
    assert_eq!(back, occurrence);
}

#[test]
fn a_list_without_truncation_says_so_by_default() {
    let list: CalendarOccurrenceList =
        serde_json::from_value(json!({ "occurrences": [] })).expect("list decodes");
    assert_eq!(
        list,
        CalendarOccurrenceList {
            occurrences: Vec::new(),
            truncated: false,
        }
    );
}
