//! `extensions.*`: the wire shapes, the consent rules, the events published after each change, and the remove guards
//! other features register.

use std::path::Path;
use std::time::Duration;

use base64::Engine as _;
use massorca_core_extensions::HookError;
use massorca_core_store::{
    AgentSessionRow, ProjectKind, ProjectRow, RunExit, RunExitReason, RunMode, Store,
    WorkspaceKind, WorkspaceRow, WorkspaceState, new_id, now_ms,
};
use massorca_protocol::codes;
use massorca_protocol::methods::events::{self, ChangeEvent};
use massorca_protocol::methods::extensions::{
    CliPlanState, ExtensionAssetParams, ExtensionInstallParams, ExtensionRemoveParams,
    ExtensionSetEnabledParams, ExtensionState, ExtensionUpdateParams, RiskLevel,
};
use serde_json::json;
use tokio::time::timeout;

use super::common::{connect, profile, remote_code, start, stop, temp_home};

/// An agent package `acme.kit` with no CLI block, so installing it runs nothing.
fn write_package(dir: &Path) {
    let manifest = json!({
        "id": "acme.kit", "name": "Kit", "version": "1.2.0", "publisher": "acme",
        "description": "A coding agent.", "api": "1.0", "requires": { "massorca": ">=0.1.0" },
        "surfaces": ["desktop"], "modes": ["workspaces"],
        "permissions": [
            { "kind": "process", "scope": { "programs": ["kit"] }, "reason": "Runs the agent." }
        ],
        "contributes": { "agentAdapters": [{ "id": "kit", "kind": "agents/kit.json" }] },
        "listing": { "logo": "media/logo.svg", "mention": "kit" }
    });
    std::fs::create_dir_all(dir.join("agents")).expect("agents folder");
    std::fs::create_dir_all(dir.join("media")).expect("media folder");
    std::fs::write(dir.join("extension.json"), manifest.to_string()).expect("manifest");
    std::fs::write(dir.join("agents/kit.json"), r#"{"id":"kit"}"#).expect("declaration");
    std::fs::write(dir.join("media/logo.svg"), "<svg/>").expect("logo");
}

async fn next_event(subscription: &mut massorca_client::Subscription) -> ChangeEvent {
    timeout(
        Duration::from_secs(5),
        subscription.next_as::<ChangeEvent>(),
    )
    .await
    .expect("an event arrives")
    .expect("the stream is open")
    .expect("the event decodes")
}

#[tokio::test]
async fn the_catalogue_lists_the_four_agent_packages_and_each_decodes() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let catalogue = client.extension_catalogue().await.expect("the catalogue");

    let ids: Vec<&str> = catalogue
        .extensions
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "massorca.claude",
            "massorca.codex",
            "massorca.antigravity",
            "massorca.gentle-shell"
        ]
    );
    assert!(
        catalogue
            .extensions
            .iter()
            .all(|item| item.state == ExtensionState::Available)
    );
    let claude = client
        .extension_get("massorca.claude")
        .await
        .expect("a catalogue item");
    assert_eq!(claude.agents.len(), 1);
    assert!(claude.agents[0].cli.is_some());
    assert!(
        client
            .extension_list()
            .await
            .expect("list")
            .extensions
            .is_empty()
    );
    stop(running).await;
}

#[tokio::test]
async fn install_needs_the_accepted_review_and_publishes_events_after_each_change() {
    let home = temp_home();
    let package = temp_home();
    write_package(package.path());
    let source = format!("dir:{}", package.path().display());
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let mut subscription = client.subscribe_events().await.expect("subscribed");

    let review = client.extension_inspect(&source).await.expect("a review");
    assert_eq!(review.id, "acme.kit");
    assert_eq!(review.risk.level, RiskLevel::High);
    assert!(!review.runs_installer);
    assert_eq!(review.cli.len(), 1);
    assert_eq!(review.cli[0].state, CliPlanState::Unsupported);

    let refused = client
        .extension_install(&ExtensionInstallParams {
            source: source.clone(),
            consent: None,
            run_installer: false,
            sha256: None,
            agent_token: None,
        })
        .await
        .expect_err("no consent");
    assert_eq!(remote_code(&refused), codes::EXTENSION_CONSENT_REQUIRED);
    let wrong = client
        .extension_install(&ExtensionInstallParams {
            source: source.clone(),
            consent: Some("not the fingerprint".to_owned()),
            run_installer: false,
            sha256: None,
            agent_token: None,
        })
        .await
        .expect_err("a stale consent");
    assert_eq!(remote_code(&wrong), codes::EXTENSION_CONSENT_MISMATCH);
    assert!(
        client
            .extension_list()
            .await
            .expect("list")
            .extensions
            .is_empty()
    );

    let installed = client
        .extension_install(&ExtensionInstallParams {
            source: source.clone(),
            consent: Some(review.consent.clone()),
            run_installer: false,
            sha256: None,
            agent_token: None,
        })
        .await
        .expect("the install");
    assert_eq!(installed.id, "acme.kit");
    assert!(installed.enabled);
    assert_eq!(
        next_event(&mut subscription).await,
        ChangeEvent {
            topic: events::TOPIC_EXTENSION_INSTALLED.to_owned(),
            id: "acme.kit".to_owned()
        }
    );
    let again = client
        .extension_install(&ExtensionInstallParams {
            source,
            consent: Some(review.consent),
            run_installer: false,
            sha256: None,
            agent_token: None,
        })
        .await
        .expect_err("already installed");
    assert_eq!(remote_code(&again), codes::EXTENSION_ALREADY_INSTALLED);

    let off = client
        .extension_set_enabled(&ExtensionSetEnabledParams {
            id: "acme.kit".to_owned(),
            enabled: false,
        })
        .await
        .expect("disable");
    assert!(off.changed && !off.enabled);
    assert_eq!(
        next_event(&mut subscription).await.topic,
        events::TOPIC_EXTENSION_DISABLED
    );
    let same = client
        .extension_set_enabled(&ExtensionSetEnabledParams {
            id: "acme.kit".to_owned(),
            enabled: false,
        })
        .await
        .expect("disable again");
    assert!(!same.changed, "an unchanged state publishes nothing");
    client
        .extension_set_enabled(&ExtensionSetEnabledParams {
            id: "acme.kit".to_owned(),
            enabled: true,
        })
        .await
        .expect("enable");
    assert_eq!(
        next_event(&mut subscription).await.topic,
        events::TOPIC_EXTENSION_ENABLED
    );

    let item = client.extension_get("acme.kit").await.expect("get");
    assert_eq!(item.state, ExtensionState::Enabled);
    let asset = client
        .extension_asset(&ExtensionAssetParams {
            id: "acme.kit".to_owned(),
            path: "media/logo.svg".to_owned(),
        })
        .await
        .expect("a listed image");
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(asset.data)
            .expect("base64"),
        b"<svg/>"
    );
    let unlisted = client
        .extension_asset(&ExtensionAssetParams {
            id: "acme.kit".to_owned(),
            path: "agents/kit.json".to_owned(),
        })
        .await
        .expect_err("only listed files");
    assert_eq!(remote_code(&unlisted), codes::EXTENSION_ASSET_NOT_LISTED);

    let removed = client
        .extension_remove(&ExtensionRemoveParams {
            id: "acme.kit".to_owned(),
            ..ExtensionRemoveParams::default()
        })
        .await
        .expect("remove");
    assert_eq!(removed.version, "1.2.0");
    assert_eq!(
        next_event(&mut subscription).await,
        ChangeEvent {
            topic: events::TOPIC_EXTENSION_REMOVED.to_owned(),
            id: "acme.kit".to_owned()
        }
    );
    assert!(
        client
            .extension_list()
            .await
            .expect("list")
            .extensions
            .is_empty()
    );
    stop(running).await;
}

/// An ended session of kind `kind` that can resume (a recorded end and a known ref), in a new folder workspace.
fn resumable_session(store: &Store, kind: &str) -> String {
    let project = ProjectRow {
        id: new_id(),
        kind: ProjectKind::Folder,
        path: "/work/app".to_owned(),
        path_key: "/work/app".to_owned(),
        name: "app".to_owned(),
        workspaces_root: "/work/app-workspaces".to_owned(),
        added_at: now_ms(),
        base_ref: None,
        icon: None,
    };
    store.insert_project(&project).expect("project");
    let workspace = WorkspaceRow {
        id: new_id(),
        project_id: project.id,
        kind: WorkspaceKind::Folder,
        name: "main".to_owned(),
        path: "/work/app".to_owned(),
        path_key: "/work/app".to_owned(),
        branch: None,
        base_ref: None,
        created_branch: false,
        state: WorkspaceState::Ready,
        request_id: None,
        created_at: now_ms(),
        comment: None,
    };
    store.insert_workspace(&workspace).expect("workspace");
    let session = AgentSessionRow {
        id: new_id(),
        kind: kind.to_owned(),
        workspace_id: workspace.id,
        parent_id: None,
        title: None,
        read_only: true,
        agent_ref: Some("ref-1".to_owned()),
        terminal_id: None,
        request_id: None,
        created_at: now_ms(),
        exit: Some(RunExit {
            code: Some(0),
            signal: None,
            reason: RunExitReason::Exited,
            at: now_ms(),
        }),
        activity: None,
        result: None,
        brought_back_at: None,
        holds_secrets: false,
        mode: RunMode::default(),
        pending_permission_mode: None,
    };
    store.insert_agent_session(&session).expect("session");
    session.id
}

/// The service registers the agents' remove guard at start: a package whose kind a resumable session uses is neither
/// disabled nor removed, with `agent.busy` naming the session, until the session is forgotten.
#[tokio::test(flavor = "multi_thread")]
async fn a_package_whose_kind_a_session_uses_is_not_disabled_or_removed() {
    let home = temp_home();
    let package = temp_home();
    write_package(package.path());
    let source = format!("dir:{}", package.path().display());
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let review = client.extension_inspect(&source).await.expect("a review");
    client
        .extension_install(&ExtensionInstallParams {
            source,
            consent: Some(review.consent),
            run_installer: false,
            sha256: None,
            agent_token: None,
        })
        .await
        .expect("the install");
    let store = Store::open(profile(home.path(), "default").store_path()).expect("the store");
    let session = resumable_session(&store, "kit");

    let disable = client
        .extension_set_enabled(&ExtensionSetEnabledParams {
            id: "acme.kit".to_owned(),
            enabled: false,
        })
        .await
        .expect_err("a used kind keeps its package enabled");
    let remove = client
        .extension_remove(&ExtensionRemoveParams {
            id: "acme.kit".to_owned(),
            ..ExtensionRemoveParams::default()
        })
        .await
        .expect_err("a used kind keeps its package");
    for refused in [&disable, &remove] {
        assert_eq!(remote_code(refused), codes::AGENT_BUSY);
        assert!(refused.to_string().contains(&session), "{refused}");
    }

    client
        .agent_forget(&session)
        .await
        .expect("the session is forgotten");
    client
        .extension_remove(&ExtensionRemoveParams {
            id: "acme.kit".to_owned(),
            ..ExtensionRemoveParams::default()
        })
        .await
        .expect("removed once no session uses the kind");
    stop(running).await;
}

#[tokio::test]
async fn a_registered_guard_stops_a_disable_and_a_remove_with_its_own_code() {
    let home = temp_home();
    let package = temp_home();
    write_package(package.path());
    let source = format!("dir:{}", package.path().display());
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;
    let review = client.extension_inspect(&source).await.expect("a review");
    client
        .extension_install(&ExtensionInstallParams {
            source,
            consent: Some(review.consent),
            run_installer: false,
            sha256: None,
            agent_token: None,
        })
        .await
        .expect("the install");
    let guard = running.extensions().register_remove_guard("agents", |_id| {
        Err(HookError::new(codes::AGENT_BUSY, "a session is live"))
    });

    let disable = client
        .extension_set_enabled(&ExtensionSetEnabledParams {
            id: "acme.kit".to_owned(),
            enabled: false,
        })
        .await
        .expect_err("the guard refuses");
    let remove = client
        .extension_remove(&ExtensionRemoveParams {
            id: "acme.kit".to_owned(),
            ..ExtensionRemoveParams::default()
        })
        .await
        .expect_err("the guard refuses");

    assert_eq!(remote_code(&disable), codes::AGENT_BUSY);
    assert_eq!(remote_code(&remove), codes::AGENT_BUSY);
    assert_eq!(
        client
            .extension_list()
            .await
            .expect("list")
            .extensions
            .len(),
        1
    );
    drop(guard);
    client
        .extension_remove(&ExtensionRemoveParams {
            id: "acme.kit".to_owned(),
            ..ExtensionRemoveParams::default()
        })
        .await
        .expect("remove once the guard is gone");
    stop(running).await;
}

#[tokio::test]
async fn bad_sources_and_unknown_ids_have_stable_codes() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let source = client
        .extension_inspect("somewhere")
        .await
        .expect_err("no scheme");
    let missing = client
        .extension_get("acme.nothing")
        .await
        .expect_err("no such item");
    let archive = client
        .extension_install(&ExtensionInstallParams {
            source: "file:/no/such/package.zip".to_owned(),
            consent: None,
            run_installer: false,
            sha256: None,
            agent_token: None,
        })
        .await
        .expect_err("an archive needs its hash");

    assert_eq!(remote_code(&source), codes::EXTENSION_INVALID_SOURCE);
    assert_eq!(remote_code(&missing), codes::EXTENSION_NOT_FOUND);
    assert_eq!(remote_code(&archive), codes::EXTENSION_HASH_REQUIRED);
    stop(running).await;
}

#[tokio::test]
async fn a_request_carrying_an_agent_run_token_cannot_run_a_vendor_installer_or_uninstaller() {
    let home = temp_home();
    let package = temp_home();
    write_package(package.path());
    let source = format!("dir:{}", package.path().display());
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let install = client
        .extension_install(&ExtensionInstallParams {
            source,
            consent: Some("unused".to_owned()),
            run_installer: true,
            sha256: None,
            agent_token: Some("a-run-token".to_owned()),
        })
        .await
        .expect_err("an agent run may not run the installer");
    assert_eq!(remote_code(&install), codes::EXTENSION_NEEDS_PERSON);

    let removal = client
        .extension_remove(&ExtensionRemoveParams {
            id: "acme.kit".to_owned(),
            uninstall_cli: true,
            agent_token: Some("a-run-token".to_owned()),
            ..Default::default()
        })
        .await
        .expect_err("an agent run may not run the uninstaller");
    assert_eq!(remote_code(&removal), codes::EXTENSION_NEEDS_PERSON);
    assert!(
        client
            .extension_list()
            .await
            .expect("the list")
            .extensions
            .is_empty(),
        "nothing was installed"
    );
    stop(running).await;
}

#[tokio::test]
async fn update_refuses_an_agent_run_and_names_a_package_that_is_not_installed() {
    let home = temp_home();
    let running = start(home.path(), "default").await;
    let client = connect(home.path(), "default").await;

    let refused = client
        .extension_update(&ExtensionUpdateParams {
            id: "acme.kit".to_owned(),
            consent: None,
            sha256: None,
            agent_token: Some("a-run-token".to_owned()),
        })
        .await
        .expect_err("an agent run may not update a package");
    assert_eq!(remote_code(&refused), codes::EXTENSION_NEEDS_PERSON);

    let missing = client
        .extension_update(&ExtensionUpdateParams {
            id: "acme.kit".to_owned(),
            consent: None,
            sha256: None,
            agent_token: None,
        })
        .await
        .expect_err("nothing is installed");
    assert_eq!(remote_code(&missing), codes::EXTENSION_NOT_INSTALLED);
    stop(running).await;
}
