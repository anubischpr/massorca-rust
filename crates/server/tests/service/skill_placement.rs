//! `skills.place-preview`, `skills.place`, `skills.unplace` and `skills.managed` through the service (SKL-02): a home skill is
//! copied into the chosen agents' folders under an injected home folder, listed as managed, and removed again. Only the
//! copies MassOrca placed go; a folder the user owns is never touched.

use std::fs;
use std::path::Path;

use massorca_core_extensions::skills::SkillRootEnv;
use massorca_protocol::capabilities;
use massorca_protocol::codes;
use massorca_protocol::methods::projects::{self, ProjectAddParams};
use massorca_protocol::methods::skill_placement::{
    self as placement, AgentPlacement, PlacementAgent, PlacementState, SkillPlaceParams,
    SkillPlacementResult, SkillUnplaceParams, SkillsManagedParams, SkillsManagedResult,
};
use massorca_protocol::methods::skills::{self, SkillDiscovery, SkillsDiscoverParams};
use massorca_server::DiscoveryOptions;
use serde_json::{Value, json};
use tempfile::TempDir;

use super::common::{connect, remote_code, start_with_discovery, stop, temp_home};

/// A `SKILL.md` named `name` in `root/relative`.
fn skill_at(root: &Path, relative: &str, name: &str) {
    let folder = root.join(relative).join(name);
    fs::create_dir_all(&folder).expect("the skill folder");
    fs::write(
        folder.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {name} skill.\n---\n"),
    )
    .expect("the skill file");
}

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
        .find(|row: &&AgentPlacement| row.agent == agent)
        .expect("a row for the agent")
        .state
}

#[tokio::test]
async fn placing_copies_a_home_skill_for_the_chosen_agents_and_unplacing_removes_only_those_copies()
{
    let profile_home = temp_home();
    let fake = TempDir::new().expect("a temporary folder");
    let user_home = fake.path().join("user-home");
    skill_at(&user_home, ".claude/skills", "notes");
    let running =
        start_with_discovery(profile_home.path(), "default", discovery_of(&user_home)).await;
    let client = connect(profile_home.path(), "default").await;
    let discovery: SkillDiscovery = client
        .call(skills::DISCOVER, &SkillsDiscoverParams::default())
        .await
        .expect("discovery answers");
    let skill_id = discovery
        .skills
        .iter()
        .find(|skill| skill.name == "notes")
        .expect("the home skill")
        .id
        .clone();
    let params = SkillPlaceParams {
        skill_id,
        agents: vec![PlacementAgent::Codex, PlacementAgent::Cursor],
    };

    let preview: SkillPlacementResult = client
        .call(placement::PREVIEW, &params)
        .await
        .expect("previewed");
    assert_eq!(
        state_of(&preview, PlacementAgent::Cursor),
        PlacementState::Create
    );
    assert!(
        !user_home.join(".agents").exists(),
        "a preview writes nothing"
    );

    let placed: SkillPlacementResult = client
        .call(placement::PLACE, &params)
        .await
        .expect("placed");
    assert_eq!(
        state_of(&placed, PlacementAgent::Cursor),
        PlacementState::Placed
    );
    assert_eq!(
        state_of(&placed, PlacementAgent::Codex),
        PlacementState::Canonical
    );
    assert!(user_home.join(".agents/skills/notes/SKILL.md").is_file());
    assert!(user_home.join(".cursor/skills/notes/SKILL.md").is_file());
    assert!(!user_home.join(".codex").exists());

    let managed: SkillsManagedResult = client
        .call(placement::MANAGED, &SkillsManagedParams::default())
        .await
        .expect("listed");
    assert_eq!(managed.skills.len(), 1);
    assert_eq!(managed.skills[0].name, "notes");
    assert!(
        managed.skills[0].canonical_path.is_some(),
        "the owner is a host administrator, so the host path is shown"
    );

    let again: SkillPlacementResult = client
        .call(placement::PLACE, &params)
        .await
        .expect("placed again");
    assert_eq!(
        state_of(&again, PlacementAgent::Cursor),
        PlacementState::AlreadyPlaced
    );

    let removed: SkillPlacementResult = client
        .call(
            placement::UNPLACE,
            &SkillUnplaceParams {
                name: "notes".to_owned(),
                agents: Vec::new(),
            },
        )
        .await
        .expect("removed");
    assert_eq!(
        state_of(&removed, PlacementAgent::Cursor),
        PlacementState::Removed
    );
    assert!(!user_home.join(".cursor/skills/notes").exists());
    assert!(!user_home.join(".agents/skills/notes").exists());
    assert!(
        user_home.join(".claude/skills/notes/SKILL.md").is_file(),
        "the skill the user had stays"
    );
    let left: SkillsManagedResult = client
        .call(placement::MANAGED, &SkillsManagedParams::default())
        .await
        .expect("listed");
    assert!(left.skills.is_empty());
    drop(client);
    stop(running).await;
}

#[tokio::test]
async fn a_name_the_user_already_uses_is_refused_with_its_code_and_nothing_is_written() {
    let profile_home = temp_home();
    let fake = TempDir::new().expect("a temporary folder");
    let user_home = fake.path().join("user-home");
    skill_at(&user_home, ".claude/skills", "notes");
    skill_at(&user_home, ".agents/skills", "notes");
    fs::write(
        user_home.join(".agents/skills/notes/SKILL.md"),
        "the user's own copy",
    )
    .expect("the user's file");
    let running =
        start_with_discovery(profile_home.path(), "default", discovery_of(&user_home)).await;
    let client = connect(profile_home.path(), "default").await;
    let discovery: SkillDiscovery = client
        .call(skills::DISCOVER, &SkillsDiscoverParams::default())
        .await
        .expect("discovery answers");
    let claude_copy = discovery
        .skills
        .iter()
        .find(|skill| skill.source_label == "Claude home")
        .expect("the Claude copy")
        .id
        .clone();

    let err = client
        .call::<_, SkillPlacementResult>(
            placement::PLACE,
            &SkillPlaceParams {
                skill_id: claude_copy,
                agents: vec![PlacementAgent::Cursor],
            },
        )
        .await
        .expect_err("refused");

    assert_eq!(remote_code(&err), codes::SKILL_NAME_TAKEN);
    assert_eq!(
        fs::read_to_string(user_home.join(".agents/skills/notes/SKILL.md")).expect("the file"),
        "the user's own copy"
    );
    assert!(!user_home.join(".cursor").exists());
    drop(client);
    stop(running).await;
}

#[tokio::test]
async fn an_unknown_skill_or_agent_is_refused_with_its_code() {
    let profile_home = temp_home();
    let fake = TempDir::new().expect("a temporary folder");
    let user_home = fake.path().join("user-home");
    skill_at(&user_home, ".claude/skills", "notes");
    let running =
        start_with_discovery(profile_home.path(), "default", discovery_of(&user_home)).await;
    let client = connect(profile_home.path(), "default").await;

    let missing = client
        .call::<_, SkillPlacementResult>(
            placement::PREVIEW,
            &SkillPlaceParams {
                skill_id: "0000000000000000".to_owned(),
                agents: vec![PlacementAgent::Cursor],
            },
        )
        .await
        .expect_err("refused");
    assert_eq!(remote_code(&missing), codes::SKILL_NOT_FOUND);

    let unknown_agent = client
        .call::<_, Value>(
            placement::PLACE,
            &json!({"skill_id": "0000000000000000", "agents": ["future-tool"]}),
        )
        .await
        .expect_err("refused");
    assert_eq!(remote_code(&unknown_agent), codes::REQUEST_INVALID_PARAMS);
    drop(client);
    stop(running).await;
}

#[tokio::test]
async fn a_skill_in_a_registered_folder_is_not_copied_and_the_capability_is_offered() {
    let profile_home = temp_home();
    let fake = TempDir::new().expect("a temporary folder");
    let user_home = fake.path().join("user-home");
    let project = fake.path().join("proj");
    skill_at(&project, ".agents/skills", "beta");
    let running =
        start_with_discovery(profile_home.path(), "default", discovery_of(&user_home)).await;
    let client = connect(profile_home.path(), "default").await;
    let added = ProjectAddParams {
        path: project.to_str().expect("utf-8 path").to_owned(),
        name: None,
        kind: None,
        workspaces_root: None,
        icon: None,
    };
    client
        .call::<_, Value>(projects::ADD, &added)
        .await
        .expect("the project is added");
    let discovery: SkillDiscovery = client
        .call(skills::DISCOVER, &SkillsDiscoverParams::default())
        .await
        .expect("discovery answers");
    let beta = discovery
        .skills
        .iter()
        .find(|skill| skill.name == "beta")
        .expect("beta")
        .id
        .clone();

    let err = client
        .call::<_, SkillPlacementResult>(
            placement::PLACE,
            &SkillPlaceParams {
                skill_id: beta,
                agents: vec![PlacementAgent::Cursor],
            },
        )
        .await
        .expect_err("refused");

    assert_eq!(remote_code(&err), codes::SKILL_NOT_PLACEABLE);
    assert!(
        client
            .welcome()
            .capabilities
            .iter()
            .any(|offered| offered == capabilities::SKILLS_PLACE),
        "the service offers skills-place/1"
    );
    drop(client);
    stop(running).await;
}

#[tokio::test]
async fn a_preview_reuses_the_recent_scan_instead_of_forcing_a_new_one() {
    let profile_home = temp_home();
    let fake = TempDir::new().expect("a temporary folder");
    let user_home = fake.path().join("user-home");
    skill_at(&user_home, ".claude/skills", "notes");
    let running =
        start_with_discovery(profile_home.path(), "default", discovery_of(&user_home)).await;
    let client = connect(profile_home.path(), "default").await;
    let discovery: SkillDiscovery = client
        .call(skills::DISCOVER, &SkillsDiscoverParams::default())
        .await
        .expect("discovery answers");
    let skill_id = discovery
        .skills
        .iter()
        .find(|skill| skill.name == "notes")
        .expect("the home skill")
        .id
        .clone();
    // A skill added after the scan shows only once something scans again.
    skill_at(&user_home, ".claude/skills", "later");

    let _: SkillPlacementResult = client
        .call(
            placement::PREVIEW,
            &SkillPlaceParams {
                skill_id,
                agents: vec![PlacementAgent::Cursor],
            },
        )
        .await
        .expect("previewed");

    let cached: SkillDiscovery = client
        .call(skills::DISCOVER, &SkillsDiscoverParams::default())
        .await
        .expect("discovery answers");
    assert!(
        !cached.skills.iter().any(|skill| skill.name == "later"),
        "the preview did not rescan, so the shared scan is still the first one"
    );
    drop(client);
    stop(running).await;
}
