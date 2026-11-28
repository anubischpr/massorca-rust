//! The bundled-skill wire shapes (SKL-04): method names, the capability, and the JSON of the state and the install params.

use massorca_protocol::capabilities;
use massorca_protocol::methods::skill_bundled::{
    self as wire, BundledSkillState, SkillsBundledInstallParams,
};
use massorca_protocol::methods::skill_placement::PlacementAgent;
use serde_json::{Value, json};

#[test]
fn the_methods_and_the_capability_have_their_names() {
    assert_eq!(wire::LIST, "skills.bundled.list");
    assert_eq!(wire::INSTALL, "skills.bundled.install");
    assert_eq!(wire::REFRESH, "skills.bundled.refresh");
    assert_eq!(capabilities::SKILLS_BUNDLED, "skills-bundled/1");
}

#[test]
fn a_state_is_its_snake_case_name_and_a_newer_one_reads_as_unknown() {
    assert_eq!(
        serde_json::to_value(BundledSkillState::NotPlaced).expect("json"),
        json!("not_placed")
    );
    assert_eq!(
        serde_json::to_value(BundledSkillState::Outdated).expect("json"),
        json!("outdated")
    );
    let newer: BundledSkillState = serde_json::from_value(json!("pending_review")).expect("read");
    assert_eq!(newer, BundledSkillState::Unknown);
}

#[test]
fn install_without_agents_names_only_the_skill() {
    let params: SkillsBundledInstallParams =
        serde_json::from_value(json!({ "name": "massorca" })).expect("read");
    assert_eq!(params.name, "massorca");
    assert!(params.agents.is_empty());

    let with_agent: Value = serde_json::to_value(SkillsBundledInstallParams {
        name: "massorca".to_owned(),
        agents: vec![PlacementAgent::Claude],
    })
    .expect("json");
    assert_eq!(
        with_agent,
        json!({ "name": "massorca", "agents": ["claude"] })
    );
}
