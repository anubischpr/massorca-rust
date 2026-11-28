//! The `skills.place` wire shapes: the method, capability and code names, the paths a caller without host administration
//! does not see, and a value a newer peer adds decoding as `Unknown`.

use massorca_protocol::capabilities;
use massorca_protocol::codes;
use massorca_protocol::methods::skill_placement::{
    self as wire, AgentPlacement, PlacementAgent, PlacementState, SkillPlaceParams,
    SkillPlacementResult,
};
use serde_json::{Value, json};

#[test]
fn the_method_capability_and_code_names_are_stable() {
    assert_eq!(wire::PREVIEW, "skills.place-preview");
    assert_eq!(wire::PLACE, "skills.place");
    assert_eq!(wire::UNPLACE, "skills.unplace");
    assert_eq!(wire::MANAGED, "skills.managed");
    assert_eq!(capabilities::SKILLS_PLACE, "skills-place/1");
    assert_eq!(codes::SKILL_NAME_TAKEN, "skill.name_taken");
    assert_eq!(codes::SKILL_CONTENT_CHANGED, "skill.content_changed");
    assert_eq!(codes::SKILL_NOT_PLACED, "skill.not_placed");
    assert_eq!(codes::SKILL_NOT_FOUND, "skill.not_found");
    assert_eq!(codes::SKILL_NOT_PLACEABLE, "skill.not_placeable");
    assert_eq!(codes::SKILL_RECORD_INVALID, "skill.record_invalid");
}

#[test]
fn a_result_leaves_out_the_paths_a_caller_may_not_see() {
    let result = SkillPlacementResult {
        name: "notes".to_owned(),
        canonical_path: None,
        agents: vec![AgentPlacement {
            agent: PlacementAgent::Cursor,
            state: PlacementState::Placed,
            path: None,
            canonical: false,
            topology: None,
        }],
    };

    let value: Value = serde_json::to_value(&result).expect("encodes");

    assert!(value.get("canonical_path").is_none());
    assert!(value["agents"][0].get("path").is_none());
    assert_eq!(value["agents"][0]["agent"], "cursor");
    assert_eq!(value["agents"][0]["state"], "placed");
}

#[test]
fn a_skipped_state_keeps_its_reason_in_the_name() {
    let state: PlacementState = serde_json::from_value(json!("skipped_modified")).expect("decodes");
    assert_eq!(state, PlacementState::SkippedModified);
    let created: PlacementState = serde_json::from_value(json!("create")).expect("decodes");
    assert_eq!(created, PlacementState::Create);
}

#[test]
fn a_newer_agent_or_state_decodes_as_unknown() {
    let agent: PlacementAgent = serde_json::from_value(json!("future-tool")).expect("decodes");
    assert_eq!(agent, PlacementAgent::Unknown);
    let state: PlacementState = serde_json::from_value(json!("moved_elsewhere")).expect("decodes");
    assert_eq!(state, PlacementState::Unknown);
}

#[test]
fn the_place_params_take_agent_ids_and_an_unplace_without_agents_means_all() {
    let params: SkillPlaceParams =
        serde_json::from_value(json!({"skill_id": "abc", "agents": ["codex", "cursor"]}))
            .expect("decodes");
    assert_eq!(
        params.agents,
        [PlacementAgent::Codex, PlacementAgent::Cursor]
    );

    let unplace: wire::SkillUnplaceParams =
        serde_json::from_value(json!({"name": "notes"})).expect("decodes");
    assert!(unplace.agents.is_empty());
}

#[test]
fn the_scope_is_global_when_a_client_sends_none() {
    let params: SkillPlaceParams =
        serde_json::from_value(json!({"skill_id": "abc", "agents": ["codex"]})).expect("decodes");

    assert_eq!(params.target.scope, wire::PlacementScope::Global);
    assert_eq!(params.target.workspace_id, None);
}

#[test]
fn a_workspace_scope_names_its_workspace_and_a_newer_scope_decodes_as_unknown() {
    let params: SkillPlaceParams = serde_json::from_value(json!({
        "skill_id": "abc",
        "agents": ["cursor"],
        "scope": "workspace",
        "workspace_id": "ws-1"
    }))
    .expect("decodes");
    assert_eq!(params.target.scope, wire::PlacementScope::Workspace);
    assert_eq!(params.target.workspace_id.as_deref(), Some("ws-1"));

    let newer: SkillPlaceParams = serde_json::from_value(json!({
        "skill_id": "abc",
        "agents": ["cursor"],
        "scope": "folder"
    }))
    .expect("decodes");
    assert_eq!(newer.target.scope, wire::PlacementScope::Unknown);
}

#[test]
fn the_agent_switch_names_its_agents_to_add_and_to_drop_and_its_method_and_capability() {
    let params: wire::SkillAgentsParams =
        serde_json::from_value(json!({"name": "notes", "add": ["cursor"], "drop": ["claude"]}))
            .expect("decodes");

    assert_eq!(params.add, [PlacementAgent::Cursor]);
    assert_eq!(params.drop, [PlacementAgent::Claude]);
    assert_eq!(params.target.scope, wire::PlacementScope::Global);
    assert_eq!(wire::AGENTS, "skills.agents");
    assert_eq!(capabilities::SKILLS_PLACE_SCOPES, "skills-place/2");
}

#[test]
fn a_managed_skill_names_its_workspace_only_when_it_has_one() {
    let workspace: wire::ManagedSkill =
        serde_json::from_value(json!({"name": "notes", "agents": [], "workspace_id": "ws-1"}))
            .expect("decodes");
    assert_eq!(workspace.workspace_id.as_deref(), Some("ws-1"));

    let global: wire::ManagedSkill =
        serde_json::from_value(json!({"name": "notes", "agents": []})).expect("decodes");
    assert_eq!(global.workspace_id, None);
    let value: Value = serde_json::to_value(&global).expect("encodes");
    assert!(value.get("workspace_id").is_none());
}
