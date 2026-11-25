use massorca_protocol::methods::{
    RESERVED_EXTENSION_METHOD_PREFIX, config, events, projects, service, workspaces,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::fmt::Debug;

/// Checks that `value` serializes to `expected` and that `expected` decodes back to `value`.
fn assert_wire<T>(value: &T, expected: &Value)
where
    T: Serialize + DeserializeOwned + Debug + PartialEq,
{
    assert_eq!(&serde_json::to_value(value).expect("serializes"), expected);
    let decoded: T = serde_json::from_value(expected.clone()).expect("deserializes");
    assert_eq!(&decoded, value);
}

fn sample_project() -> projects::Project {
    projects::Project {
        id: "p-1".to_owned(),
        kind: projects::ProjectKind::Git,
        path: "/repos/app".to_owned(),
        name: "app".to_owned(),
        workspaces_root: "/repos/app-workspaces".to_owned(),
        added_at_ms: 1_767_225_600_000_i64,
        base_ref: None,
        icon: None,
    }
}

fn sample_project_json() -> Value {
    json!({
        "id": "p-1",
        "kind": "git",
        "path": "/repos/app",
        "name": "app",
        "workspaces_root": "/repos/app-workspaces",
        "added_at_ms": 1_767_225_600_000_i64
    })
}

fn sample_workspace() -> workspaces::Workspace {
    workspaces::Workspace {
        id: "w-1".to_owned(),
        project_id: "p-1".to_owned(),
        kind: workspaces::WorkspaceKind::Worktree,
        name: "feat-x".to_owned(),
        path: "/work/app/feat-x".to_owned(),
        branch: Some("feat/x".to_owned()),
        base_ref: Some("main".to_owned()),
        created_branch: true,
        state: workspaces::WorkspaceState::Ready,
        missing: false,
        created_at_ms: 1_767_225_600_500_i64,
        locked: None,
        prunable: None,
        base_refresh: None,
        comment: None,
        setup: None,
        name_generated: false,
    }
}

fn sample_workspace_json() -> Value {
    json!({
        "id": "w-1",
        "project_id": "p-1",
        "kind": "worktree",
        "name": "feat-x",
        "path": "/work/app/feat-x",
        "branch": "feat/x",
        "base_ref": "main",
        "created_branch": true,
        "state": "ready",
        "missing": false,
        "created_at_ms": 1_767_225_600_500_i64
    })
}

fn sample_config_value() -> config::ConfigValue {
    config::ConfigValue {
        owner: "core".to_owned(),
        key: "ui.zoom".to_owned(),
        value: json!(2),
        default: json!(1),
        is_default: false,
        ..Default::default()
    }
}

fn sample_config_value_json() -> Value {
    json!({ "owner": "core", "key": "ui.zoom", "value": 2, "default": 1, "is_default": false })
}

#[test]
fn method_names_follow_the_contract() {
    assert_eq!(service::INFO, "service.info");
    assert_eq!(service::SHUTDOWN, "service.shutdown");
    assert_eq!(projects::ADD, "project.add");
    assert_eq!(projects::LIST, "project.list");
    assert_eq!(projects::SHOW, "project.show");
    assert_eq!(projects::REMOVE, "project.remove");
    assert_eq!(projects::SET_BASE, "project.set_base");
    assert_eq!(workspaces::CREATE, "workspace.create");
    assert_eq!(workspaces::LIST, "workspace.list");
    assert_eq!(workspaces::SHOW, "workspace.show");
    assert_eq!(workspaces::CURRENT, "workspace.current");
    assert_eq!(workspaces::REMOVE, "workspace.remove");
    assert_eq!(workspaces::UPDATE, "workspace.update");
    assert_eq!(workspaces::CHECK, "workspace.check");
    assert_eq!(workspaces::RENAME, "workspace.rename");
    assert_eq!(events::SUBSCRIBE, "events.subscribe");
    assert_eq!(config::GET, "config.get");
    assert_eq!(config::SET, "config.set");
    assert_eq!(config::LIST, "config.list");
}

#[test]
fn no_core_method_uses_the_reserved_extension_prefix() {
    assert_eq!(RESERVED_EXTENSION_METHOD_PREFIX, "ext.");
    let core_methods = [
        service::INFO,
        service::SHUTDOWN,
        projects::ADD,
        projects::LIST,
        projects::SHOW,
        projects::REMOVE,
        projects::SET_BASE,
        workspaces::CREATE,
        workspaces::LIST,
        workspaces::SHOW,
        workspaces::REMOVE,
        events::SUBSCRIBE,
        config::GET,
        config::SET,
        config::LIST,
    ];
    for method in core_methods {
        assert!(
            !method.starts_with(RESERVED_EXTENSION_METHOD_PREFIX),
            "{method} is in the extension namespace"
        );
    }
}

#[test]
fn service_info_has_the_documented_shape() {
    let info = service::ServiceInfo {
        version: "0.1.0".to_owned(),
        protocol: 1,
        capabilities: vec!["service/1".to_owned()],
        profile: "default".to_owned(),
        pid: 4242,
        started_at_ms: 1_767_225_600_000_i64,
        git: None,
    };
    assert_wire(
        &info,
        &json!({
            "version": "0.1.0",
            "protocol": 1,
            "capabilities": ["service/1"],
            "profile": "default",
            "pid": 4242,
            "started_at_ms": 1_767_225_600_000_i64
        }),
    );
}

#[test]
fn git_status_has_one_shape_per_state() {
    let cases = [
        (
            service::GitStatus::Present {
                version: "2.49.0".to_owned(),
            },
            json!({ "state": "present", "version": "2.49.0" }),
        ),
        (service::GitStatus::Missing, json!({ "state": "missing" })),
        (
            service::GitStatus::TooOld {
                version: "2.24.0".to_owned(),
                required: "2.25.0".to_owned(),
            },
            json!({ "state": "too_old", "version": "2.24.0", "required": "2.25.0" }),
        ),
        (
            service::GitStatus::Unknown {
                reason: "unrecognised version line".to_owned(),
            },
            json!({ "state": "unknown", "reason": "unrecognised version line" }),
        ),
    ];
    for (status, expected) in &cases {
        assert_wire(status, expected);
    }
}

#[test]
fn service_info_from_an_older_service_has_no_git_status() {
    let info: service::ServiceInfo = serde_json::from_value(json!({
        "version": "0.1.0",
        "protocol": 1,
        "capabilities": [],
        "profile": "default",
        "pid": 1,
        "started_at_ms": 0
    }))
    .expect("an older service's reply decodes");
    assert_eq!(info.git, None);
}

#[test]
fn project_add_params_omit_unset_options() {
    assert_wire(
        &projects::ProjectAddParams {
            path: "/repos/app".to_owned(),
            name: None,
            kind: None,
            workspaces_root: None,
            icon: None,
        },
        &json!({ "path": "/repos/app" }),
    );
}

#[test]
fn project_add_params_carry_set_options() {
    assert_wire(
        &projects::ProjectAddParams {
            path: "/repos/app".to_owned(),
            name: Some("app".to_owned()),
            kind: Some(projects::ProjectKind::Folder),
            workspaces_root: Some("/work".to_owned()),
            icon: None,
        },
        &json!({
            "path": "/repos/app",
            "name": "app",
            "kind": "folder",
            "workspaces_root": "/work"
        }),
    );
}

#[test]
fn project_inspect_params_and_inspection_have_the_documented_shape() {
    assert_wire(
        &projects::ProjectInspectParams {
            path: "/repos/app/src".to_owned(),
        },
        &json!({ "path": "/repos/app/src" }),
    );
    assert_wire(
        &projects::ProjectInspection {
            path: "/notes".to_owned(),
            kind: projects::ProjectKind::Folder,
            registered: None,
        },
        &json!({ "path": "/notes", "kind": "folder" }),
    );
    assert_wire(
        &projects::ProjectInspection {
            path: "/repos/app".to_owned(),
            kind: projects::ProjectKind::Git,
            registered: Some("p1".to_owned()),
        },
        &json!({ "path": "/repos/app", "kind": "git", "registered": "p1" }),
    );
}

#[test]
fn project_set_base_params_send_null_to_clear_the_base() {
    assert_wire(
        &projects::ProjectSetBaseParams {
            project: "app".to_owned(),
            base: Some("origin/develop".to_owned()),
        },
        &json!({ "project": "app", "base": "origin/develop" }),
    );
    assert_wire(
        &projects::ProjectSetBaseParams {
            project: "app".to_owned(),
            base: None,
        },
        &json!({ "project": "app", "base": null }),
    );
}

#[test]
fn project_omits_base_ref_until_it_is_set() {
    let mut project = sample_project();
    assert_wire(&project, &sample_project_json());

    project.base_ref = Some("origin/develop".to_owned());
    let mut expected = sample_project_json();
    expected["base_ref"] = json!("origin/develop");
    assert_wire(&project, &expected);
}

#[test]
fn project_kind_uses_snake_case_names() {
    assert_wire(&projects::ProjectKind::Git, &json!("git"));
    assert_wire(&projects::ProjectKind::Folder, &json!("folder"));
}

#[test]
fn project_and_project_list_have_the_documented_shape() {
    assert_wire(&sample_project(), &sample_project_json());
    assert_wire(
        &projects::ProjectList {
            projects: vec![sample_project()],
        },
        &json!({ "projects": [sample_project_json()] }),
    );
}

#[test]
fn project_remove_params_keep_the_refusal_unless_forget_workspaces_is_sent() {
    assert_wire(
        &projects::ProjectRemoveParams {
            project: "app".to_owned(),
            forget_workspaces: false,
        },
        &json!({ "project": "app" }),
    );
    assert_wire(
        &projects::ProjectRemoveParams {
            project: "app".to_owned(),
            forget_workspaces: true,
        },
        &json!({ "project": "app", "forget_workspaces": true }),
    );
}

#[test]
fn project_ref_uses_the_project_key() {
    assert_wire(
        &projects::ProjectRef {
            project: "app".to_owned(),
        },
        &json!({ "project": "app" }),
    );
}

#[test]
fn workspace_create_params_omit_unset_options_but_always_write_existing_branch() {
    assert_wire(
        &workspaces::WorkspaceCreateParams {
            project: "app".to_owned(),
            name: "feat-x".to_owned(),
            branch: None,
            base: None,
            existing_branch: false,
            refresh_base: false,
            path: None,
            request_id: None,
            no_setup: false,
            unique_name: false,
        },
        &json!({ "project": "app", "name": "feat-x", "existing_branch": false }),
    );
}

#[test]
fn workspace_create_params_without_a_name_decode_as_a_blank_name() {
    let params: workspaces::WorkspaceCreateParams =
        serde_json::from_value(json!({ "project": "app" })).expect("decodes");
    assert_eq!(params.name, "");
}

#[test]
fn workspace_create_params_write_refresh_base_only_when_it_is_set() {
    assert_wire(
        &workspaces::WorkspaceCreateParams {
            project: "app".to_owned(),
            name: "feat-x".to_owned(),
            branch: None,
            base: None,
            existing_branch: false,
            refresh_base: true,
            path: None,
            request_id: None,
            no_setup: false,
            unique_name: false,
        },
        &json!({
            "project": "app",
            "name": "feat-x",
            "existing_branch": false,
            "refresh_base": true
        }),
    );
}

#[test]
fn workspace_create_params_default_existing_branch_to_false() {
    let params: workspaces::WorkspaceCreateParams =
        serde_json::from_value(json!({ "project": "app", "name": "feat-x" })).expect("decodes");
    assert!(!params.existing_branch);
    assert!(!params.refresh_base);
    assert!(params.branch.is_none());
    assert!(params.request_id.is_none());
}

#[test]
fn workspace_list_params_project_is_optional() {
    assert_wire(
        &workspaces::WorkspaceListParams { project: None },
        &json!({}),
    );
    assert_wire(
        &workspaces::WorkspaceListParams {
            project: Some("app".to_owned()),
        },
        &json!({ "project": "app" }),
    );
}

#[test]
fn workspace_remove_params_default_force_to_false() {
    let params: workspaces::WorkspaceRemoveParams =
        serde_json::from_value(json!({ "workspace": "feat-x" })).expect("decodes");
    assert_eq!(
        params,
        workspaces::WorkspaceRemoveParams {
            workspace: "feat-x".to_owned(),
            force: false,
            discard_ignored: false,
            forget_orphan: false,
        }
    );
    assert_wire(&params, &json!({ "workspace": "feat-x", "force": false }));
}

#[test]
fn workspace_remove_params_carry_the_ignored_and_orphan_waivers() {
    assert_wire(
        &workspaces::WorkspaceRemoveParams {
            workspace: "feat-x".to_owned(),
            force: true,
            discard_ignored: true,
            forget_orphan: true,
        },
        &json!({
            "workspace": "feat-x",
            "force": true,
            "discard_ignored": true,
            "forget_orphan": true
        }),
    );
}

#[test]
fn workspace_update_params_send_only_the_fields_they_change() {
    assert_wire(
        &workspaces::WorkspaceUpdateParams {
            workspace: "feat-x".to_owned(),
            name: None,
            comment: Some(String::new()),
        },
        &json!({ "workspace": "feat-x", "comment": "" }),
    );
}

#[test]
fn workspace_carries_its_comment_only_when_one_is_set() {
    let mut workspace = sample_workspace();
    assert!(
        serde_json::to_value(&workspace)
            .unwrap()
            .get("comment")
            .is_none()
    );
    workspace.comment = Some("waiting on review".to_owned());
    assert_eq!(
        serde_json::to_value(&workspace).unwrap()["comment"],
        "waiting on review"
    );
}

#[test]
fn workspace_check_omits_registration_for_a_folder_and_reason_when_all_pass() {
    assert_wire(
        &workspaces::WorkspaceCheck {
            id: "w-1".to_owned(),
            name: "notes".to_owned(),
            kind: workspaces::WorkspaceKind::Folder,
            path: "/notes".to_owned(),
            exists: true,
            readable: true,
            registered: None,
            ok: true,
            reason: None,
        },
        &json!({
            "id": "w-1",
            "name": "notes",
            "kind": "folder",
            "path": "/notes",
            "exists": true,
            "readable": true,
            "ok": true
        }),
    );
}

#[test]
fn workspace_check_carries_git_registration_and_the_first_failure() {
    assert_wire(
        &workspaces::WorkspaceCheck {
            id: "w-2".to_owned(),
            name: "feat-x".to_owned(),
            kind: workspaces::WorkspaceKind::Worktree,
            path: "/work/app/feat-x".to_owned(),
            exists: true,
            readable: true,
            registered: Some(false),
            ok: false,
            reason: Some("git no longer lists this worktree".to_owned()),
        },
        &json!({
            "id": "w-2",
            "name": "feat-x",
            "kind": "worktree",
            "path": "/work/app/feat-x",
            "exists": true,
            "readable": true,
            "registered": false,
            "ok": false,
            "reason": "git no longer lists this worktree"
        }),
    );
}

#[test]
fn workspace_ref_uses_the_workspace_key() {
    assert_wire(
        &workspaces::WorkspaceRef {
            workspace: "feat-x".to_owned(),
        },
        &json!({ "workspace": "feat-x" }),
    );
}

#[test]
fn workspace_kind_and_state_use_snake_case_names() {
    assert_wire(&workspaces::WorkspaceKind::Worktree, &json!("worktree"));
    assert_wire(&workspaces::WorkspaceKind::Folder, &json!("folder"));
    assert_wire(&workspaces::WorkspaceState::Ready, &json!("ready"));
    assert_wire(&workspaces::WorkspaceState::Removing, &json!("removing"));
}

#[test]
fn workspace_has_the_documented_shape() {
    assert_wire(&sample_workspace(), &sample_workspace_json());
}

#[test]
fn workspace_omits_unset_branch_and_base_ref() {
    let workspace = workspaces::Workspace {
        kind: workspaces::WorkspaceKind::Folder,
        branch: None,
        base_ref: None,
        ..sample_workspace()
    };
    assert_wire(
        &workspace,
        &json!({
            "id": "w-1",
            "project_id": "p-1",
            "kind": "folder",
            "name": "feat-x",
            "path": "/work/app/feat-x",
            "created_branch": true,
            "state": "ready",
            "missing": false,
            "created_at_ms": 1_767_225_600_500_i64
        }),
    );
}

#[test]
fn workspace_carries_git_lock_and_prunable_reasons_only_when_set() {
    let workspace = workspaces::Workspace {
        locked: Some("on a usb disk".to_owned()),
        prunable: Some(String::new()),
        ..sample_workspace()
    };
    let mut expected = sample_workspace_json();
    expected["locked"] = json!("on a usb disk");
    expected["prunable"] = json!("");
    assert_wire(&workspace, &expected);
}

#[test]
fn unregistered_worktree_carries_git_lock_and_prunable_reasons_only_when_set() {
    assert_wire(
        &workspaces::UnregisteredWorktree {
            project_id: "p1".into(),
            path: "/work/ws/lost".into(),
            branch: None,
            locked: Some(String::new()),
            prunable: Some("gitdir file points to non-existent location".into()),
        },
        &json!({
            "project_id": "p1",
            "path": "/work/ws/lost",
            "locked": "",
            "prunable": "gitdir file points to non-existent location"
        }),
    );
}

#[test]
fn workspace_list_has_the_documented_shape() {
    assert_wire(
        &workspaces::WorkspaceList {
            workspaces: vec![sample_workspace()],
            unregistered: Vec::new(),
        },
        &json!({ "workspaces": [sample_workspace_json()] }),
    );
}

#[test]
fn workspace_list_reports_unregistered_worktrees_and_an_older_list_still_decodes() {
    assert_wire(
        &workspaces::WorkspaceList {
            workspaces: Vec::new(),
            unregistered: vec![workspaces::UnregisteredWorktree {
                project_id: "p1".into(),
                path: "/work/ws/lost".into(),
                branch: None,
                locked: None,
                prunable: None,
            }],
        },
        &json!({
            "workspaces": [],
            "unregistered": [{ "project_id": "p1", "path": "/work/ws/lost" }]
        }),
    );
    let older: workspaces::WorkspaceList =
        serde_json::from_value(json!({ "workspaces": [] })).expect("older list decodes");
    assert!(older.unregistered.is_empty());
}

#[test]
fn workspace_removed_omits_reason_when_branch_was_deleted() {
    assert_wire(
        &workspaces::WorkspaceRemoved {
            workspace: sample_workspace(),
            branch_deleted: true,
            branch_kept_reason: None,
            directory_kept: false,
        },
        &json!({ "workspace": sample_workspace_json(), "branch_deleted": true }),
    );
}

#[test]
fn workspace_removed_says_when_a_directory_was_kept() {
    assert_wire(
        &workspaces::WorkspaceRemoved {
            workspace: sample_workspace(),
            branch_deleted: false,
            branch_kept_reason: None,
            directory_kept: true,
        },
        &json!({
            "workspace": sample_workspace_json(),
            "branch_deleted": false,
            "directory_kept": true
        }),
    );
}

#[test]
fn workspace_removed_carries_reason_when_branch_was_kept() {
    assert_wire(
        &workspaces::WorkspaceRemoved {
            workspace: sample_workspace(),
            branch_deleted: false,
            branch_kept_reason: Some("branch has unmerged commits".to_owned()),
            directory_kept: false,
        },
        &json!({
            "workspace": sample_workspace_json(),
            "branch_deleted": false,
            "branch_kept_reason": "branch has unmerged commits"
        }),
    );
}

#[test]
fn subscribe_result_carries_the_stream_id() {
    assert_wire(&events::Subscribed { stream: 5 }, &json!({ "stream": 5 }));
}

#[test]
fn change_event_has_topic_and_id() {
    assert_wire(
        &events::ChangeEvent {
            topic: events::TOPIC_WORKSPACE_REMOVING.to_owned(),
            id: "w-1".to_owned(),
        },
        &json!({ "topic": "workspace.removing", "id": "w-1" }),
    );
}

#[test]
fn topic_constants_match_the_contract() {
    assert_eq!(events::TOPIC_WORKSPACE_CREATED, "workspace.created");
    assert_eq!(events::TOPIC_WORKSPACE_REMOVING, "workspace.removing");
    assert_eq!(events::TOPIC_WORKSPACE_REMOVED, "workspace.removed");
    assert_eq!(events::TOPIC_PROJECT_ADDED, "project.added");
    assert_eq!(events::TOPIC_PROJECT_REMOVED, "project.removed");
    assert_eq!(events::TOPIC_PROJECT_CHANGED, "project.changed");
}

#[test]
fn config_key_omits_owner_when_unset() {
    assert_wire(
        &config::ConfigKey {
            owner: None,
            key: "ui.zoom".to_owned(),
        },
        &json!({ "key": "ui.zoom" }),
    );
}

#[test]
fn config_key_carries_owner_when_set() {
    assert_wire(
        &config::ConfigKey {
            owner: Some("tools".to_owned()),
            key: "ui.zoom".to_owned(),
        },
        &json!({ "owner": "tools", "key": "ui.zoom" }),
    );
}

#[test]
fn config_set_carries_any_json_value_including_null() {
    assert_wire(
        &config::ConfigSet {
            owner: None,
            key: "ui.zoom".to_owned(),
            value: json!(2),
        },
        &json!({ "key": "ui.zoom", "value": 2 }),
    );
    assert_wire(
        &config::ConfigSet {
            owner: Some("tools".to_owned()),
            key: "ui.zoom".to_owned(),
            value: Value::Null,
        },
        &json!({ "owner": "tools", "key": "ui.zoom", "value": null }),
    );
}

#[test]
fn config_value_has_the_documented_shape() {
    assert_wire(&sample_config_value(), &sample_config_value_json());
}

#[test]
fn config_list_has_the_documented_shape() {
    assert_wire(
        &config::ConfigList {
            values: vec![sample_config_value()],
        },
        &json!({ "values": [sample_config_value_json()] }),
    );
}

#[test]
fn config_default_owner_is_core() {
    assert_eq!(config::DEFAULT_OWNER, "core");
}

#[test]
fn unknown_project_kind_in_a_list_decodes_as_unknown() {
    let mut project = sample_project_json();
    project["kind"] = json!("future_kind");
    let list: projects::ProjectList =
        serde_json::from_value(json!({ "projects": [project, sample_project_json()] }))
            .expect("decodes");
    assert_eq!(list.projects[0].kind, projects::ProjectKind::Unknown);
    assert_eq!(list.projects[1], sample_project());
}

#[test]
fn unknown_workspace_kind_and_state_in_a_list_decode_as_unknown() {
    let mut workspace = sample_workspace_json();
    workspace["kind"] = json!("future_kind");
    workspace["state"] = json!("archiving");
    let list: workspaces::WorkspaceList =
        serde_json::from_value(json!({ "workspaces": [workspace, sample_workspace_json()] }))
            .expect("decodes");
    assert_eq!(list.workspaces[0].kind, workspaces::WorkspaceKind::Unknown);
    assert_eq!(
        list.workspaces[0].state,
        workspaces::WorkspaceState::Unknown
    );
    assert_eq!(list.workspaces[1], sample_workspace());
}

#[test]
fn workspace_create_params_write_no_setup_only_when_it_is_set() {
    let params: workspaces::WorkspaceCreateParams =
        serde_json::from_value(json!({ "project": "app", "name": "feat-x" })).expect("decodes");
    assert!(!params.no_setup);
    let skipping = workspaces::WorkspaceCreateParams {
        no_setup: true,
        ..params
    };
    assert_eq!(
        serde_json::to_value(&skipping).expect("encodes")["no_setup"],
        json!(true)
    );
}

#[test]
fn workspace_create_params_write_unique_name_only_when_it_is_set() {
    let params: workspaces::WorkspaceCreateParams =
        serde_json::from_value(json!({ "project": "app", "name": "feat-x" })).expect("decodes");
    assert!(!params.unique_name);
    assert!(
        serde_json::to_value(&params)
            .expect("encodes")
            .get("unique_name")
            .is_none()
    );
    let unique = workspaces::WorkspaceCreateParams {
        unique_name: true,
        ..params
    };
    assert_eq!(
        serde_json::to_value(&unique).expect("encodes")["unique_name"],
        json!(true)
    );
}

#[test]
fn a_workspace_setup_outcome_is_tagged_and_absent_when_unset() {
    assert!(
        serde_json::to_value(sample_workspace())
            .expect("encodes")
            .get("setup")
            .is_none()
    );
    let started = workspaces::Workspace {
        setup: Some(workspaces::WorkspaceSetup::Started {
            terminal: "t-1".to_owned(),
        }),
        ..sample_workspace()
    };
    assert_eq!(
        serde_json::to_value(&started).expect("encodes")["setup"],
        json!({ "outcome": "started", "terminal": "t-1" })
    );
    assert_eq!(
        serde_json::to_value(workspaces::WorkspaceSetup::Failed {
            reason: "no shell".to_owned()
        })
        .expect("encodes"),
        json!({ "outcome": "failed", "reason": "no shell" })
    );
    let newer: workspaces::WorkspaceSetup =
        serde_json::from_value(json!({ "outcome": "queued" })).expect("decodes");
    assert_eq!(newer, workspaces::WorkspaceSetup::Unknown);
}

#[test]
fn workspace_carries_name_generated_only_when_it_holds() {
    let mut workspace = sample_workspace();
    assert!(
        serde_json::to_value(&workspace)
            .unwrap()
            .get("name_generated")
            .is_none()
    );
    workspace.name_generated = true;
    assert_eq!(
        serde_json::to_value(&workspace).unwrap()["name_generated"],
        true
    );
}

#[test]
fn workspace_rename_params_have_the_documented_shape() {
    assert_wire(
        &workspaces::WorkspaceRenameParams {
            workspace: "otter".to_owned(),
            name: "fix-login".to_owned(),
            from_generated: true,
            expected_branch: Some("otter".to_owned()),
        },
        &json!({
            "workspace": "otter",
            "name": "fix-login",
            "from_generated": true,
            "expected_branch": "otter"
        }),
    );
    let decoded: workspaces::WorkspaceRenameParams =
        serde_json::from_value(json!({ "workspace": "otter", "name": "x" })).unwrap();
    assert!(!decoded.from_generated);
    assert_eq!(decoded.expected_branch, None);
}
