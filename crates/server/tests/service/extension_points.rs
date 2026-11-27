//! `extensions.points` (decision 0153): a package contributing to every point registers under its owner, its skill is
//! placed through skill placement, and disabling or removing it empties every point.

use std::fs;
use std::path::Path;

use massorca_core_extensions::skills::SkillRootEnv;
use massorca_protocol::methods::extension_points::{ExtensionPointOwner, ExtensionPointsList};
use massorca_protocol::methods::extensions::{
    ExtensionInstallParams, ExtensionRemoveParams, ExtensionSetEnabledParams,
};
use massorca_protocol::methods::skill_placement::{
    self as placement, PlacementAgent, PlacementState, SkillPlaceParams, SkillPlacementResult,
};
use massorca_server::DiscoveryOptions;
use serde_json::json;
use tempfile::TempDir;

use super::common::{connect, start_with_discovery, stop, temp_home};

/// A package `acme.all` with one contribution on every point, and every file its manifest names.
fn write_every_point_package(dir: &Path) {
    let manifest = json!({
        "id": "acme.all", "name": "All", "version": "1.0.0", "publisher": "acme",
        "description": "Everything.", "api": "1.0", "requires": { "massorca": ">=0.1.0" },
        "views": 1, "surfaces": ["desktop"], "modes": ["workspaces"], "main": "main.wasm",
        "permissions": [
            { "kind": "process", "scope": { "programs": ["kit"] }, "reason": "Runs kit." }
        ],
        "contributes": {
            "tools": [{ "id": "search", "description": "Searches.", "input": "tools/search.json" }],
            "hostPrograms": [{ "id": "kit", "cli": { "binary": "kit", "source": "https://vendor.example" } }],
            "skills": [{ "id": "review", "title": "Review", "path": "skills/review" }],
            "views": [{ "id": "wall", "slot": "status-item", "title": "Wall", "document": "views/wall.json" }],
            "themes": [{ "id": "dusk", "title": "Dusk", "document": "themes/dusk.json" }],
            "agentRules": [{ "id": "waiting", "document": "rules/waiting.json" }]
        }
    });
    let files = [
        ("extension.json", manifest.to_string()),
        ("main.wasm", String::new()),
        ("tools/search.json", "{}".to_owned()),
        (
            "skills/review/SKILL.md",
            "---\nname: review\ndescription: Reviews.\n---\n".to_owned(),
        ),
        ("views/wall.json", "{}".to_owned()),
        ("themes/dusk.json", "{}".to_owned()),
        ("rules/waiting.json", "{}".to_owned()),
    ];
    for (path, text) in files {
        let target = dir.join(path);
        fs::create_dir_all(target.parent().expect("a parent")).expect("the folder");
        fs::write(target, text).expect("the file");
    }
}

fn install_params(source: &str, consent: &str) -> ExtensionInstallParams {
    ExtensionInstallParams {
        source: source.to_owned(),
        consent: Some(consent.to_owned()),
        run_installer: false,
        sha256: None,
        agent_token: None,
    }
}

fn keys(points: &[&str]) -> Vec<String> {
    points.iter().map(|key| (*key).to_owned()).collect()
}

fn every_point_owner() -> ExtensionPointOwner {
    ExtensionPointOwner {
        owner: "ext:acme.all".to_owned(),
        tools: keys(&["acme.all/kit", "acme.all/search"]),
        skills: keys(&["acme.all/review"]),
        views: keys(&["acme.all/wall"]),
        themes: keys(&["acme.all/dusk"]),
        status_rules: keys(&["acme.all/waiting"]),
    }
}

#[tokio::test]
async fn a_package_on_every_point_registers_under_its_owner_and_leaves_none_once_removed() {
    let profile_home = temp_home();
    let fake = TempDir::new().expect("a temporary folder");
    let package = fake.path().join("package");
    write_every_point_package(&package);
    let discovery = DiscoveryOptions {
        home: Some(fake.path().join("user-home")),
        env: SkillRootEnv::default(),
    };
    let running = start_with_discovery(profile_home.path(), "default", discovery).await;
    let client = connect(profile_home.path(), "default").await;
    let source = format!("dir:{}", package.display());

    assert_eq!(
        client.extension_points().await.expect("points"),
        ExtensionPointsList::default()
    );
    let review = client.extension_inspect(&source).await.expect("a review");
    client
        .extension_install(&install_params(&source, &review.consent))
        .await
        .expect("installs");

    let points = client.extension_points().await.expect("points");
    assert_eq!(points.owners, [every_point_owner()]);
    assert!(points.problems.is_empty());

    // The contributed skill is placed from its package folder through skill placement (SKL-02).
    let preview: SkillPlacementResult = client
        .call(
            placement::PREVIEW,
            &SkillPlaceParams {
                skill_id: "acme.all/review".to_owned(),
                agents: vec![PlacementAgent::Codex],
            },
        )
        .await
        .expect("previewed");
    assert_eq!(preview.name, "review");
    assert_eq!(preview.agents[0].state, PlacementState::Canonical);

    client
        .extension_set_enabled(&ExtensionSetEnabledParams {
            id: "acme.all".to_owned(),
            enabled: false,
        })
        .await
        .expect("disables");
    assert!(
        client
            .extension_points()
            .await
            .expect("points")
            .owners
            .is_empty()
    );
    client
        .extension_set_enabled(&ExtensionSetEnabledParams {
            id: "acme.all".to_owned(),
            enabled: true,
        })
        .await
        .expect("enables");
    assert_eq!(
        client.extension_points().await.expect("points").owners,
        [every_point_owner()]
    );

    client
        .extension_remove(&ExtensionRemoveParams {
            id: "acme.all".to_owned(),
            ..ExtensionRemoveParams::default()
        })
        .await
        .expect("removes");
    assert_eq!(
        client.extension_points().await.expect("points"),
        ExtensionPointsList::default()
    );
    stop(running).await;
}
