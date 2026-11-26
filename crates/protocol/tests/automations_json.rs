//! The `automation.*` wire shapes (decision 0123): documented method names, unset options left out, timestamps in
//! `*_ms` fields, and a cadence or weekday this build does not know decoding as `Unknown`.

use massorca_protocol::capabilities;
use massorca_protocol::codes;
use massorca_protocol::methods::automations::{
    self, Automation, AutomationCadence, AutomationCreateParams, AutomationDestination,
    AutomationDestinationKind, AutomationList, AutomationRef, AutomationUpdateParams,
    AutomationWeekday,
};
use serde_json::{Value, json};

/// The create params the tests start from; a test changes only the fields it checks.
fn create_params() -> AutomationCreateParams {
    AutomationCreateParams {
        project: "app".to_owned(),
        name: "Morning PRs".to_owned(),
        prompt: "Summarise the open pull requests".to_owned(),
        agent: "claude".to_owned(),
        cadence: AutomationCadence::Daily,
        time: "09:00".to_owned(),
        weekday: None,
        cron: None,
        zone: None,
        grace_minutes: None,
        creation_key: None,
        destination: None,
    }
}

/// Update params that change nothing until a field is set.
fn update_params(automation: &str) -> AutomationUpdateParams {
    AutomationUpdateParams {
        automation: automation.to_owned(),
        project: None,
        name: None,
        prompt: None,
        agent: None,
        cadence: None,
        time: None,
        weekday: None,
        cron: None,
        zone: None,
        grace_minutes: None,
        destination: None,
    }
}

fn reference(automation: &str) -> AutomationRef {
    AutomationRef {
        automation: automation.to_owned(),
    }
}

/// An automation as the service returns it.
fn sample_automation() -> Automation {
    Automation {
        id: "a1".to_owned(),
        project_id: None,
        project_name: None,
        name: "Morning PRs".to_owned(),
        prompt: "Summarise the open pull requests".to_owned(),
        agent: "claude".to_owned(),
        cadence: AutomationCadence::Weekdays,
        time: "08:00".to_owned(),
        weekday: None,
        cron: None,
        zone: None,
        zone_used: Some("Europe/Madrid".to_owned()),
        grace_minutes: automations::DEFAULT_GRACE_MINUTES,
        enabled: false,
        destination: AutomationDestination::default(),
        next_run_at_ms: None,
        last_run_at_ms: None,
        created_at_ms: 10,
        updated_at_ms: 20,
    }
}

#[test]
fn automation_method_names_follow_the_contract() {
    assert_eq!(automations::CREATE, "automation.create");
    assert_eq!(automations::LIST, "automation.list");
    assert_eq!(automations::SHOW, "automation.show");
    assert_eq!(automations::UPDATE, "automation.update");
    assert_eq!(automations::REMOVE, "automation.rm");
    assert_eq!(automations::ENABLE, "automation.enable");
    assert_eq!(automations::DISABLE, "automation.disable");
    assert_eq!(capabilities::AUTOMATIONS, "automations/1");
}

#[test]
fn the_grace_defaults_are_the_ones_the_decision_records() {
    assert_eq!(automations::DEFAULT_GRACE_MINUTES, 720);
    assert_eq!(automations::MAX_GRACE_MINUTES, 10_080);
}

#[test]
fn the_refusal_codes_have_their_wire_values() {
    assert_eq!(codes::AUTOMATION_NOT_FOUND, "automation.not_found");
    assert_eq!(
        codes::AUTOMATION_SCHEDULE_INVALID,
        "automation.schedule_invalid"
    );
    assert_eq!(codes::AUTOMATION_ZONE_INVALID, "automation.zone_invalid");
}

#[test]
fn create_params_use_the_documented_names_and_omit_unset_options() {
    let params = create_params();
    assert_eq!(
        serde_json::to_value(&params).unwrap(),
        json!({
            "project": "app",
            "name": "Morning PRs",
            "prompt": "Summarise the open pull requests",
            "agent": "claude",
            "cadence": "daily",
            "time": "09:00",
        })
    );
}

#[test]
fn a_cron_create_carries_its_expression_and_zone() {
    let params: AutomationCreateParams = serde_json::from_value(json!({
        "project": "app",
        "name": "Weekday audit",
        "prompt": "Audit the lockfile",
        "agent": "codex",
        "cadence": "cron",
        "time": "00:00",
        "cron": "0 9 * * 1-5",
        "zone": "Europe/Madrid",
        "grace_minutes": 30,
        "creation_key": "client-1",
    }))
    .unwrap();
    assert_eq!(params.cadence, AutomationCadence::Cron);
    assert_eq!(params.cron.as_deref(), Some("0 9 * * 1-5"));
    assert_eq!(params.zone.as_deref(), Some("Europe/Madrid"));
    assert_eq!(params.grace_minutes, Some(30));
    assert_eq!(params.creation_key.as_deref(), Some("client-1"));
}

#[test]
fn a_cadence_or_weekday_this_build_does_not_know_decodes_as_unknown() {
    let params: AutomationCreateParams = serde_json::from_value(json!({
        "project": "app",
        "name": "Future",
        "prompt": "Later",
        "agent": "claude",
        "cadence": "monthly",
        "time": "09:00",
        "weekday": "someday",
    }))
    .unwrap();
    assert_eq!(params.cadence, AutomationCadence::Unknown);
    assert_eq!(params.weekday, Some(AutomationWeekday::Unknown));
}

#[test]
fn update_params_carry_only_the_fields_given() {
    let params = AutomationUpdateParams {
        name: Some("Nightly audit".to_owned()),
        zone: Some(String::new()),
        ..update_params("nightly")
    };
    assert_eq!(
        serde_json::to_value(&params).unwrap(),
        json!({"automation": "nightly", "name": "Nightly audit", "zone": ""})
    );
}

#[test]
fn a_selector_is_a_plain_automation_field() {
    let value: Value = serde_json::to_value(reference("nightly")).unwrap();
    assert_eq!(value, json!({"automation": "nightly"}));
}

#[test]
fn an_automation_uses_the_documented_names_and_omits_absent_options() {
    let automation = sample_automation();
    assert_eq!(
        serde_json::to_value(&automation).unwrap(),
        json!({
            "id": "a1",
            "name": "Morning PRs",
            "prompt": "Summarise the open pull requests",
            "agent": "claude",
            "cadence": "weekdays",
            "time": "08:00",
            "zone_used": "Europe/Madrid",
            "grace_minutes": 720,
            "enabled": false,
            "destination": { "kind": "project_default" },
            "created_at_ms": 10,
            "updated_at_ms": 20,
        })
    );
}

#[test]
fn the_list_result_names_its_automations_and_round_trips() {
    let list: AutomationList = serde_json::from_value(json!({
        "automations": [{
            "id": "a1",
            "project_id": "p1",
            "project_name": "app",
            "name": "Hourly sync",
            "prompt": "Sync",
            "agent": "claude",
            "cadence": "hourly",
            "time": "00:15",
            "grace_minutes": 60,
            "enabled": true,
            "next_run_at_ms": 1000,
            "created_at_ms": 1,
            "updated_at_ms": 2,
        }]
    }))
    .unwrap();
    assert_eq!(list.automations.len(), 1);
    let first = &list.automations[0];
    assert_eq!(first.cadence, AutomationCadence::Hourly);
    assert_eq!(first.project_id.as_deref(), Some("p1"));
    assert_eq!(first.next_run_at_ms, Some(1000));
    assert_eq!(first.last_run_at_ms, None);
    let again: AutomationList =
        serde_json::from_value(serde_json::to_value(&list).unwrap()).unwrap();
    assert_eq!(again, list);
}

#[test]
fn a_destination_uses_the_documented_names_and_omits_absent_options() {
    let params = AutomationCreateParams {
        destination: Some(AutomationDestination {
            kind: AutomationDestinationKind::ExistingWorkspace,
            workspace: Some("ws-1".to_owned()),
            ..AutomationDestination::default()
        }),
        ..create_params()
    };
    let value = serde_json::to_value(&params).unwrap();
    assert_eq!(
        value["destination"],
        json!({ "kind": "existing_workspace", "workspace": "ws-1" })
    );
}

#[test]
fn a_worktree_per_run_destination_carries_its_base_branch() {
    let params: AutomationCreateParams = serde_json::from_value(json!({
        "project": "app",
        "name": "Morning PRs",
        "prompt": "Summarise",
        "agent": "claude",
        "cadence": "daily",
        "time": "09:00",
        "destination": { "kind": "project_worktree_per_run", "base_branch": "main" }
    }))
    .unwrap();
    let destination = params.destination.expect("the destination is read");
    assert_eq!(
        destination.kind,
        AutomationDestinationKind::ProjectWorktreePerRun
    );
    assert_eq!(destination.base_branch.as_deref(), Some("main"));
    assert_eq!(destination.workspace, None);
}

#[test]
fn an_automation_without_a_destination_is_the_project_default_and_an_unknown_kind_decodes_as_unknown()
 {
    let mut value = serde_json::to_value(sample_automation()).unwrap();
    value.as_object_mut().unwrap().remove("destination");
    let old: Automation = serde_json::from_value(value).unwrap();
    assert_eq!(
        old.destination.kind,
        AutomationDestinationKind::ProjectDefault
    );

    let newer: AutomationDestination =
        serde_json::from_value(json!({ "kind": "somewhere_new" })).unwrap();
    assert_eq!(newer.kind, AutomationDestinationKind::Unknown);
}

#[test]
fn the_destination_capability_and_refusal_codes_have_their_wire_values() {
    assert_eq!(
        capabilities::AUTOMATION_DESTINATIONS,
        "automation-destinations/1"
    );
    assert_eq!(
        codes::AUTOMATION_DESTINATION_REMOVED,
        "automation.destination_removed"
    );
    assert_eq!(
        codes::AUTOMATION_REPOSITORY_DETACHED,
        "automation.repository_detached"
    );
    assert_eq!(codes::AUTOMATION_PATH_CHANGED, "automation.path_changed");
    assert_eq!(
        codes::AUTOMATION_DESTINATION_INVALID,
        "automation.destination_invalid"
    );
}
