//! `skills.bundled.list`, `skills.bundled.install` and `skills.bundled.refresh` through the real service (SKL-04): the skill
//! MassOrca ships is placed into every enabled agent folder under an injected home, listed as current, and refreshed.

use std::fs;
use std::path::Path;

use massorca_core_extensions::skills::SkillRootEnv;
use massorca_protocol::codes;
use massorca_protocol::methods::skill_bundled::{
    self as bundled, BundledSkillState, SkillsBundledInstallParams, SkillsBundledListParams,
    SkillsBundledListResult, SkillsBundledNameParams,
};
use massorca_protocol::methods::skill_placement::{
    PlacementAgent, PlacementState, SkillPlacementResult, SkillsManagedParams, SkillsManagedResult,
};
use massorca_server::DiscoveryOptions;
use serde_json::json;
use tempfile::TempDir;

use super::common::{connect, remote_code, start_with_discovery, stop, temp_home};

/// The text this build ships: the checked-in file, which a test keeps equal to what the CLI renders.
const SHIPPED: &str = include_str!("../../../../packages/skills/massorca/SKILL.md");

fn discovery_of(home: &Path) -> DiscoveryOptions {
    DiscoveryOptions {
        home: Some(home.to_path_buf()),
        env: SkillRootEnv::default(),
    }
}

fn state_of(result: &SkillPlacementResult, agent: PlacementAgent) -> PlacementState {
    result
        .agents
        .iter()
        .find(|row| row.agent == agent)
        .expect("a row for the agent")
        .state
}

#[tokio::test]
async fn the_bundled_skill_is_listed_and_placed_into_every_enabled_agent_folder() {
    let profile_home = temp_home();
    let fake = TempDir::new().expect("a temporary folder");
    let user_home = fake.path().join("user-home");
    // Why: an agent whose skill folder exists is enabled here; one whose folder is missing is not installed.
    fs::create_dir_all(user_home.join(".claude/skills")).expect("claude");
    fs::create_dir_all(user_home.join(".cursor/skills")).expect("cursor");
    let running =
        start_with_discovery(profile_home.path(), "default", discovery_of(&user_home)).await;
    let client = connect(profile_home.path(), "default").await;

    let listed: SkillsBundledListResult = client
        .call(bundled::LIST, &SkillsBundledListParams::default())
        .await
        .expect("listed");
    let skill = listed
        .skills
        .iter()
        .find(|skill| skill.name == "massorca")
        .expect("MassOrca ships the massorca skill");
    assert_eq!(skill.state, BundledSkillState::NotPlaced);
    assert!(skill.agents.is_empty());
    assert!(!skill.description.is_empty());
    assert!(!user_home.join(".agents").exists(), "a list writes nothing");

    let installed: SkillPlacementResult = client
        .call(
            bundled::INSTALL,
            &SkillsBundledInstallParams {
                name: "massorca".to_owned(),
                agents: Vec::new(),
            },
        )
        .await
        .expect("installed");
    assert_eq!(
        state_of(&installed, PlacementAgent::Claude),
        PlacementState::Placed
    );
    assert_eq!(
        state_of(&installed, PlacementAgent::Cursor),
        PlacementState::Placed
    );
    for folder in [".claude/skills", ".cursor/skills"] {
        assert_eq!(
            fs::read_to_string(user_home.join(folder).join("massorca/SKILL.md"))
                .expect("the agent folder reads the skill"),
            SHIPPED,
            "{folder}"
        );
    }

    let listed: SkillsBundledListResult = client
        .call(bundled::LIST, &SkillsBundledListParams::default())
        .await
        .expect("listed");
    let skill = listed
        .skills
        .iter()
        .find(|skill| skill.name == "massorca")
        .expect("the skill");
    assert_eq!(skill.state, BundledSkillState::Current);
    assert_eq!(
        skill.agents,
        vec![PlacementAgent::Claude, PlacementAgent::Cursor]
    );

    let managed: SkillsManagedResult = client
        .call(
            massorca_protocol::methods::skill_placement::MANAGED,
            &SkillsManagedParams::default(),
        )
        .await
        .expect("managed");
    assert!(managed.skills.iter().any(|skill| skill.name == "massorca"));

    stop(running).await;
}

#[tokio::test]
async fn refresh_and_install_refuse_what_cannot_be_placed() {
    let profile_home = temp_home();
    let fake = TempDir::new().expect("a temporary folder");
    let user_home = fake.path().join("user-home");
    fs::create_dir_all(user_home.join(".claude/skills")).expect("claude");
    let running =
        start_with_discovery(profile_home.path(), "default", discovery_of(&user_home)).await;
    let client = connect(profile_home.path(), "default").await;

    let refresh_before_install: Result<SkillPlacementResult, _> = client
        .call(
            bundled::REFRESH,
            &SkillsBundledNameParams {
                name: "massorca".to_owned(),
            },
        )
        .await;
    assert_eq!(
        remote_code(&refresh_before_install.expect_err("nothing is placed yet")),
        codes::SKILL_NOT_PLACED
    );

    let unknown: Result<SkillPlacementResult, _> = client
        .call(
            bundled::INSTALL,
            &SkillsBundledInstallParams {
                name: "not-shipped".to_owned(),
                agents: Vec::new(),
            },
        )
        .await;
    assert_eq!(
        remote_code(&unknown.expect_err("refused")),
        codes::SKILL_NOT_FOUND
    );

    let bad_agent: Result<SkillPlacementResult, _> = client
        .call(
            bundled::INSTALL,
            &json!({ "name": "massorca", "agents": ["not-an-agent"] }),
        )
        .await;
    assert_eq!(
        remote_code(&bad_agent.expect_err("refused")),
        codes::REQUEST_INVALID_PARAMS
    );

    let installed: SkillPlacementResult = client
        .call(
            bundled::INSTALL,
            &SkillsBundledInstallParams {
                name: "massorca".to_owned(),
                agents: vec![PlacementAgent::Claude],
            },
        )
        .await
        .expect("installed");
    assert_eq!(
        state_of(&installed, PlacementAgent::Claude),
        PlacementState::Placed
    );

    // Why: nothing changed since, so a refresh reconciles the copies and reports them as already placed.
    let refreshed: SkillPlacementResult = client
        .call(
            bundled::REFRESH,
            &SkillsBundledNameParams {
                name: "massorca".to_owned(),
            },
        )
        .await
        .expect("refreshed");
    assert_eq!(
        state_of(&refreshed, PlacementAgent::Claude),
        PlacementState::AlreadyPlaced
    );

    stop(running).await;
}
