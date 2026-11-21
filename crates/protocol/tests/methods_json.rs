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
    assert_eq!(workspaces::CREATE, "workspace.create");
    assert_eq!(workspaces::LIST, "workspace.list");
    assert_eq!(workspaces::SHOW, "workspace.show");
    assert_eq!(workspaces::REMOVE, "workspace.remove");
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
fn project_add_params_omit_unset_options() {
    assert_wire(
        &projects::ProjectAddParams {
            path: "/repos/app".to_owned(),
            name: None,
            kind: None,
            workspaces_root: None,
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
            path: None,
            request_id: None,
        },
        &json!({ "project": "app", "name": "feat-x", "existing_branch": false }),
    );
}

#[test]
fn workspace_create_params_default_existing_branch_to_false() {
    let params: workspaces::WorkspaceCreateParams =
        serde_json::from_value(json!({ "project": "app", "name": "feat-x" })).expect("decodes");
    assert!(!params.existing_branch);
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
