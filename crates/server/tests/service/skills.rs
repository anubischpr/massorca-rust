//! `skills.discover` through the service: the skills of the injected home folder and of the registered projects, the
//! ten-second shared scan, and the refusal of a kind this build does not know.

use std::fs;
use std::path::Path;

use massorca_core_extensions::skills::SkillRootEnv;
use massorca_protocol::codes;
use massorca_protocol::methods::projects::{self, ProjectAddParams};
use massorca_protocol::methods::skills::{self, SkillDiscovery, SkillsDiscoverParams};
use massorca_server::DiscoveryOptions;
use serde_json::{Value, json};
use tempfile::TempDir;

use super::common::{connect, remote_code, start_with_discovery, stop, temp_home};

/// A `SKILL.md` named `name` in `root/relative`.
fn skill_at(root: &Path, relative: &str, name: &str) {
    let folder = root.join(relative);
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

fn names(discovery: &SkillDiscovery) -> Vec<&str> {
    discovery
        .skills
        .iter()
        .map(|skill| skill.name.as_str())
        .collect()
}

#[tokio::test]
async fn discovery_lists_the_home_skills_and_the_registered_projects_skills() {
    let profile_home = temp_home();
    let fake = TempDir::new().expect("a temporary folder");
    let user_home = fake.path().join("user-home");
    let project = fake.path().join("proj");
    skill_at(&user_home, ".codex/skills", "alpha");
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

    assert_eq!(names(&discovery), ["alpha", "beta"]);
    let beta = discovery
        .skills
        .iter()
        .find(|skill| skill.name == "beta")
        .expect("beta");
    assert_eq!(beta.source_label, "Repo proj .agents");
    // The owner is a host administrator, so the paths of the host are shown to it.
    assert!(beta.root_path.is_some());
    drop(client);
    stop(running).await;
}

#[tokio::test]
async fn a_scan_is_shared_for_ten_seconds_unless_the_caller_asks_for_a_fresh_one() {
    let profile_home = temp_home();
    let fake = TempDir::new().expect("a temporary folder");
    let user_home = fake.path().join("user-home");
    skill_at(&user_home, ".codex/skills", "first");
    let running =
        start_with_discovery(profile_home.path(), "default", discovery_of(&user_home)).await;
    let client = connect(profile_home.path(), "default").await;
    let first: SkillDiscovery = client
        .call(skills::DISCOVER, &SkillsDiscoverParams::default())
        .await
        .expect("the first scan");
    skill_at(&user_home, ".agents/skills", "second");

    let shared: SkillDiscovery = client
        .call(skills::DISCOVER, &SkillsDiscoverParams::default())
        .await
        .expect("the shared scan");
    let fresh: SkillDiscovery = client
        .call(
            skills::DISCOVER,
            &SkillsDiscoverParams {
                refresh: true,
                source_kinds: Vec::new(),
            },
        )
        .await
        .expect("the fresh scan");

    assert_eq!(names(&first), ["first"]);
    assert_eq!(shared, first);
    assert_eq!(names(&fresh), ["first", "second"]);
    drop(client);
    stop(running).await;
}

#[tokio::test]
async fn a_source_kind_this_build_does_not_know_is_refused() {
    let profile_home = temp_home();
    let fake = TempDir::new().expect("a temporary folder");
    let running = start_with_discovery(
        profile_home.path(),
        "default",
        discovery_of(&fake.path().join("user-home")),
    )
    .await;
    let client = connect(profile_home.path(), "default").await;

    let error = client
        .call::<_, Value>(skills::DISCOVER, &json!({"source_kinds": ["mystery"]}))
        .await
        .expect_err("an unknown kind is refused");

    assert_eq!(remote_code(&error), codes::REQUEST_INVALID_PARAMS);
    drop(client);
    stop(running).await;
}
